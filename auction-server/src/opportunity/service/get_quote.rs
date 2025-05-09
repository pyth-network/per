use {
    super::{
        get_quote_request_account_balances::QuoteRequestAccountBalancesInput,
        get_token_program::GetTokenProgramInput,
        Service,
    },
    crate::{
        api::RestError,
        auction::{
            self,
            entities::{
                Auction,
                BidPaymentInstructionType,
                BidStatusAuction,
                BidStatusSvm,
            },
            service::{
                add_auction::AddAuctionInput,
                auction_manager::AuctionManager,
                get_pending_bids::GetLiveBidsInput,
                update_bid_status::UpdateBidStatusInput,
                verification::{
                    get_current_time_rounded_with_offset,
                    BID_MAXIMUM_LIFE_TIME_SVM,
                    BID_MINIMUM_LIFE_TIME_SVM_OTHER,
                },
            },
        },
        kernel::entities::PermissionKeySvm,
        opportunity::{
            entities::{
                self,
                OpportunitySvmProgram,
                OpportunitySvmProgramSwap,
                TokenAccountInitializationConfig,
                TokenAccountInitializationConfigs,
                TokenAmountSvm,
            },
            service::{
                add_opportunity::AddOpportunityInput,
                get_express_relay_metadata::GetExpressRelayMetadataInput,
            },
        },
    },
    ::express_relay::FeeToken,
    axum_prometheus::metrics,
    express_relay::state::{
        FEE_BPS_TO_PPM,
        FEE_SPLIT_PRECISION_PPM,
    },
    express_relay_api_types::opportunity::ProgramSvm,
    rand::Rng,
    solana_sdk::pubkey::Pubkey,
    spl_associated_token_account::get_associated_token_address_with_program_id,
    spl_token::native_mint,
    std::time::Duration,
    time::OffsetDateTime,
    tokio::time::sleep,
};
// FeeToken and TokenSpecified combinations possible and how they are handled:
// --------------------------------------------------------------------------------------------
// FeeToken=SearcherToken, TokenSpecified=SearcherTokenSpecified
// User wants the amount specified in the api AFTER the fees so we increase it to factor
// in fees before creating and broadcasting the swap opportunity
// --------------------------------------------------------------------------------------------
// FeeToken=SearcherToken, TokenSpecified=UserTokenSpecified
// get_quote function will return the searcher amount after fees
// --------------------------------------------------------------------------------------------
// FeeToken=UserToken, TokenSpecified=SearcherTokenSpecified
// Searcher bid amount (minimum they are willing to receive after the fees)
// is scaled up in the sdk to factor in the fees
// --------------------------------------------------------------------------------------------
// FeeToken=UserToken, TokenSpecified=UserTokenSpecified
// Sdk shows a smaller amount (after fees) to the searcher for their pricing engine
// while keeping the original amount (before fees) in the bid
// --------------------------------------------------------------------------------------------

/// Prefix for indicative price taker keys
/// We use the first 24 bytes of "Price11111111111111111111111111111111111112"
pub const INDICATIVE_PRICE_TAKER_BASE: Pubkey =
    Pubkey::from_str_const("Price11111111111111111111111111111111111112");

/// For users that don't provide a wallet we assign them a random public key prefixed by INDICATIVE_PRICE_TAKER_BASE
pub fn generate_indicative_price_taker() -> Pubkey {
    let mut key_bytes = [0u8; 32];

    key_bytes[0..24].copy_from_slice(&INDICATIVE_PRICE_TAKER_BASE.as_array()[0..24]);

    let mut rng = rand::thread_rng();
    let rand_bytes: [u8; 8] = rng.gen();
    key_bytes[24..32].copy_from_slice(&rand_bytes);

    Pubkey::new_from_array(key_bytes)
}

/// Checks if a wallet address has the indicative price taker prefix (first 28 bytes).
pub fn is_indicative_price_taker(wallet_address: &Pubkey) -> bool {
    let wallet_bytes = wallet_address.as_ref();
    wallet_bytes[0..24] == INDICATIVE_PRICE_TAKER_BASE.as_array()[0..24]
}

pub struct GetQuoteInput {
    pub quote_create: entities::QuoteCreate,
}

/// Get a pubkey based on router_token_account, user_wallet_address, referral_fee_ppm, mints, and token amounts
/// This pubkey is never mentioned on-chain and is only used internally
/// to distinguish between different swap bids
pub fn get_quote_virtual_permission_account(
    tokens: &entities::QuoteTokens,
    user_wallet_address: &Pubkey,
    router_token_account: &Pubkey,
    referral_fee_ppm: u64,
) -> Pubkey {
    let user_token_amount: [u8; 8];
    let searcher_token_amount: [u8; 8];
    let referral_fee_ppm = referral_fee_ppm.to_le_bytes();
    let seeds = match tokens {
        entities::QuoteTokens::UserTokenSpecified {
            user_token,
            searcher_token,
        } => {
            let user_token_mint = user_token.token.as_ref();
            let searcher_token_mint = searcher_token.as_ref();
            user_token_amount = user_token.amount.to_le_bytes();
            [
                router_token_account.as_ref(),
                user_wallet_address.as_ref(),
                searcher_token_mint,
                user_token_mint,
                user_token_amount.as_ref(),
                referral_fee_ppm.as_ref(),
            ]
        }
        entities::QuoteTokens::SearcherTokenSpecified {
            user_token,
            searcher_token,
        } => {
            let user_token_mint = user_token.as_ref();
            let searcher_token_mint = searcher_token.token.as_ref();
            searcher_token_amount = searcher_token.amount.to_le_bytes();
            [
                router_token_account.as_ref(),
                user_wallet_address.as_ref(),
                searcher_token_mint,
                searcher_token_amount.as_ref(),
                user_token_mint,
                referral_fee_ppm.as_ref(),
            ]
        }
    };
    // since this permission key will not be used on-chain, we don't need to use the express relay program_id.
    // we can use a distinctive bytes object for the program_id
    Pubkey::find_program_address(&seeds, &Pubkey::default()).0
}

/// Determines if the fee token should be the user token or the searcher token.
/// If the user token is explicitly tiered higher than the searcher token, the fee token will be the user token, and vice versa.
/// If neither token tier has been specified, the fee token is set to the user token to simplify the logic for searchers.
fn get_fee_token(
    user_mint: Pubkey,
    searcher_mint: Pubkey,
    ordered_fee_tokens: &[Pubkey],
) -> entities::FeeToken {
    let user_token_tier = ordered_fee_tokens
        .iter()
        .position(|&x| x == user_mint)
        .unwrap_or(usize::MAX);
    let searcher_token_tier = ordered_fee_tokens
        .iter()
        .position(|&x| x == searcher_mint)
        .unwrap_or(usize::MAX);
    if user_token_tier <= searcher_token_tier {
        entities::FeeToken::UserToken
    } else {
        entities::FeeToken::SearcherToken
    }
}

impl Service {
    #[tracing::instrument(skip_all, err(level = tracing::Level::TRACE))]
    async fn get_opportunity_create_for_quote(
        &self,
        quote_create: entities::QuoteCreate,
    ) -> Result<entities::OpportunityCreateSvm, RestError> {
        let (mint_user, mint_searcher) = match quote_create.tokens.clone() {
            entities::QuoteTokens::UserTokenSpecified {
                user_token,
                searcher_token,
            } => (user_token.token, searcher_token),
            entities::QuoteTokens::SearcherTokenSpecified {
                user_token,
                searcher_token,
            } => (user_token, searcher_token.token),
        };
        let config = self.get_config(&quote_create.chain_id)?;

        // validate token mints being whitelisted at the earliest point to fail fast
        if !config.token_whitelist.is_token_mint_allowed(&mint_user) {
            return Err(RestError::TokenMintNotAllowed(
                "Input".to_string(),
                mint_user.to_string(),
            ));
        }
        if !config.token_whitelist.is_token_mint_allowed(&mint_searcher) {
            return Err(RestError::TokenMintNotAllowed(
                "Output".to_string(),
                mint_searcher.to_string(),
            ));
        }

        let referral_fee_info =
            self.unwrap_referral_fee_info(quote_create.referral_fee_info, &quote_create.chain_id)?;

        // TODO*: we should fix the Opportunity struct (or create a new format) to more clearly distinguish Swap opps from traditional opps
        // currently, we are using the same struct and just setting the unspecified token amount to 0
        let metadata = self
            .get_express_relay_metadata(GetExpressRelayMetadataInput {
                chain_id: quote_create.chain_id.clone(),
            })
            .await?;

        let fee_token = get_fee_token(mint_user, mint_searcher, &config.ordered_fee_tokens);
        let (searcher_amount, user_amount) = match (quote_create.tokens.clone(), fee_token.clone())
        {
            (
                entities::QuoteTokens::SearcherTokenSpecified { searcher_token, .. },
                entities::FeeToken::SearcherToken,
            ) => {
                // This is not exactly accurate and may overestimate the amount needed
                // because of floor / ceil rounding errors.
                let denominator: u64 = FEE_SPLIT_PRECISION_PPM
                    - referral_fee_info.referral_fee_ppm
                    - (metadata.swap_platform_fee_bps * FEE_BPS_TO_PPM);
                let numerator = searcher_token.amount * FEE_SPLIT_PRECISION_PPM;
                let amount_including_fees = numerator.div_ceil(denominator);
                (amount_including_fees, 0u64)
            }
            (
                entities::QuoteTokens::SearcherTokenSpecified { searcher_token, .. },
                entities::FeeToken::UserToken,
            ) => (searcher_token.amount, 0u64),
            (entities::QuoteTokens::UserTokenSpecified { user_token, .. }, _) => {
                (0, user_token.amount)
            }
        };
        let token_program_searcher = self
            .get_token_program(GetTokenProgramInput {
                chain_id: quote_create.chain_id.clone(),
                mint:     mint_searcher,
            })
            .await
            .map_err(|err| {
                tracing::error!("Failed to get searcher token program: {:?}", err);
                RestError::BadParameters("Searcher token program not found".to_string())
            })?;
        let token_program_user = self
            .get_token_program(GetTokenProgramInput {
                chain_id: quote_create.chain_id.clone(),
                mint:     mint_user,
            })
            .await
            .map_err(|err| {
                tracing::error!("Failed to get user token program: {:?}", err);
                RestError::BadParameters("User token program not found".to_string())
            })?;

        let router_token_account = match fee_token {
            entities::FeeToken::SearcherToken => get_associated_token_address_with_program_id(
                &referral_fee_info.router,
                &mint_searcher,
                &token_program_searcher,
            ),
            entities::FeeToken::UserToken => get_associated_token_address_with_program_id(
                &referral_fee_info.router,
                &mint_user,
                &token_program_user,
            ),
        };
        // this uses the fee-adjusted token amounts to correctly calculate the permission account
        let tokens_for_permission = match quote_create.tokens {
            entities::QuoteTokens::UserTokenSpecified {
                user_token,
                searcher_token,
            } => entities::QuoteTokens::UserTokenSpecified {
                user_token: TokenAmountSvm {
                    token:  user_token.token,
                    amount: user_amount,
                },
                searcher_token,
            },
            entities::QuoteTokens::SearcherTokenSpecified {
                user_token,
                searcher_token,
            } => entities::QuoteTokens::SearcherTokenSpecified {
                user_token,
                searcher_token: TokenAmountSvm {
                    token:  searcher_token.token,
                    amount: searcher_amount,
                },
            },
        };
        let (user_wallet_address, user_mint_user_balance, mut token_account_initialization_configs) =
            match quote_create.user_wallet_address {
                Some(address) => {
                    let balances = self
                        .get_quote_request_account_balances(QuoteRequestAccountBalancesInput {
                            user_wallet_address: address,
                            mint_searcher,
                            mint_user,
                            router: referral_fee_info.router,
                            fee_token: fee_token.clone(),
                            token_program_searcher,
                            token_program_user,
                            chain_id: quote_create.chain_id.clone(),
                        })
                        .await?;

                    let mint_user_is_wrapped_sol = mint_user == native_mint::id();
                    (
                        address,
                        balances.get_user_ata_mint_user_balance(mint_user_is_wrapped_sol),
                        balances.get_token_account_initialization_configs(),
                    )
                }
                None => {
                    // For indicative quotes, we don't need to initialize any token accounts as the transaction will never be simulated nor broadcasted
                    (
                        generate_indicative_price_taker(),
                        0,
                        TokenAccountInitializationConfigs::none_needed(),
                    )
                }
            };

        if referral_fee_info.referral_fee_ppm == 0 {
            // If the referral fee is 0, we can skip the initialization of the router token account
            // TODO: enable this at some point
            // token_account_initialization_configs.router_fee_receiver_ta =
            //     TokenAccountInitializationConfig::Unneeded;
        }
        if metadata.swap_platform_fee_bps == 0 {
            // If the platform fee is 0, we can skip the initialization of the express relay and relayer token accounts
            token_account_initialization_configs.express_relay_fee_receiver_ata =
                TokenAccountInitializationConfig::Unneeded;
            token_account_initialization_configs.relayer_fee_receiver_ata =
                TokenAccountInitializationConfig::Unneeded;
        }

        let permission_account = get_quote_virtual_permission_account(
            &tokens_for_permission,
            &user_wallet_address,
            &router_token_account,
            referral_fee_info.referral_fee_ppm,
        );

        let referral_fee_bps =
            u16::try_from(referral_fee_info.referral_fee_ppm / FEE_BPS_TO_PPM)
                .map_err(|_| RestError::BadParameters("Referral fee too high".to_string()))?;

        let program_opportunity =
            entities::OpportunitySvmProgram::Swap(entities::OpportunitySvmProgramSwap {
                user_wallet_address,
                fee_token,
                referral_fee_bps,
                referral_fee_ppm: referral_fee_info.referral_fee_ppm,
                platform_fee_bps: metadata.swap_platform_fee_bps,
                platform_fee_ppm: metadata.swap_platform_fee_bps * FEE_BPS_TO_PPM,
                token_program_user,
                user_mint_user_balance,
                token_account_initialization_configs,
                token_program_searcher,
                memo: quote_create.memo,
                cancellable: quote_create.cancellable,
                minimum_lifetime: quote_create.minimum_lifetime,
                minimum_deadline: get_current_time_rounded_with_offset(
                    quote_create
                        .minimum_lifetime
                        .map(|lifetime| Duration::from_secs(lifetime as u64))
                        .unwrap_or(BID_MINIMUM_LIFE_TIME_SVM_OTHER),
                ),
            });

        Ok(entities::OpportunityCreateSvm {
            permission_key: entities::OpportunitySvm::get_permission_key(
                BidPaymentInstructionType::Swap,
                referral_fee_info.router,
                permission_account,
            ),
            chain_id: quote_create.chain_id.clone(),
            sell_tokens: vec![entities::TokenAmountSvm {
                token:  mint_searcher,
                amount: searcher_amount,
            }],
            buy_tokens: vec![entities::TokenAmountSvm {
                token:  mint_user,
                amount: user_amount,
            }],
            router: referral_fee_info.router,
            permission_account,
            program: program_opportunity,
            profile_id: quote_create.profile_id,
        })
    }

    async fn remove_quote_opportunity(&self, opportunity: entities::OpportunitySvm) {
        // TODO
        // Maybe we should add state for opportunity.
        // Right now logic for removing halted/expired bids, checks if opportunity exists.
        // We should remove opportunity only after the auction bid result is broadcasted.
        // This is to make sure we are not gonna remove the bids that are currently in the auction in the handle_auction loop.
        let removal_reason = entities::OpportunityRemovalReason::Invalid(
            RestError::InvalidOpportunity("Auction finished for the opportunity".to_string()),
        );
        if let Err(e) = self
            .repo
            .remove_opportunity(&opportunity, removal_reason)
            .await
        {
            tracing::error!("Failed to remove opportunity: {:?}", e);
        }
    }

    #[tracing::instrument(
        skip_all,
        err(level = tracing::Level::TRACE),
        fields(
            opportunity_id,
            auction_id,
            searcher_token,
            user_token,
            bid_ids,
            winner_bid
        )
    )]
    pub async fn get_quote(&self, input: GetQuoteInput) -> Result<entities::Quote, RestError> {
        let referral_fee_info = self.unwrap_referral_fee_info(
            input.quote_create.referral_fee_info.clone(),
            &input.quote_create.chain_id,
        )?;

        if Duration::from_secs(input.quote_create.minimum_lifetime.unwrap_or(0) as u64)
            > BID_MAXIMUM_LIFE_TIME_SVM
        {
            return Err(RestError::BadParameters(format!(
                "Minimum lifetime cannot be greater than {} seconds",
                BID_MAXIMUM_LIFE_TIME_SVM.as_secs()
            )));
        }

        // TODO use compute_swap_fees to make sure instead when the metadata is fetched from on-chain
        if FEE_SPLIT_PRECISION_PPM < referral_fee_info.referral_fee_ppm {
            return Err(RestError::BadParameters(format!(
                "Referral fee ppm higher than {}",
                FEE_SPLIT_PRECISION_PPM
            )));
        }

        let config = self.get_config(&input.quote_create.chain_id)?;
        let auction_service = config.auction_service_container.get_service();

        tracing::info!(quote_create = ?input.quote_create, "Received request to get quote");

        let opportunity_create = self
            .get_opportunity_create_for_quote(input.quote_create.clone())
            .await?;
        let opportunity = self
            .add_opportunity(AddOpportunityInput {
                opportunity: opportunity_create,
            })
            .await?;
        tracing::Span::current().record("opportunity_id", opportunity.id.to_string());
        let searcher_token = opportunity.sell_tokens[0].clone();
        let user_token = opportunity.buy_tokens[0].clone();
        tracing::Span::current().record("searcher_token", format!("{:?}", searcher_token));
        tracing::Span::current().record("user_token", format!("{:?}", user_token));
        if searcher_token.amount == 0 && user_token.amount == 0 {
            return Err(RestError::BadParameters(
                "Token amount cannot be zero".to_string(),
            ));
        }

        // Wait to make sure searchers had enough time to submit bids
        sleep(config.auction_time).await;

        // NOTE: This part will be removed after refactoring the permission key type
        let slice: [u8; 65] = opportunity
            .permission_key
            .to_vec()
            .try_into()
            .expect("Failed to convert permission key to slice");
        let permission_key_svm = PermissionKeySvm(slice);

        let bid_collection_time = OffsetDateTime::now_utc();
        let mut bids = auction_service
            .get_pending_bids(GetLiveBidsInput {
                permission_key: permission_key_svm.clone(),
            })
            .await;
        tracing::Span::current().record(
            "bid_ids",
            tracing::field::display(crate::auction::entities::BidContainerTracing(&bids)),
        );
        let total_bids = if bids.len() < 10 {
            bids.len().to_string()
        } else {
            "+9".to_string()
        };
        // Add metrics
        let labels = [
            ("chain_id", input.quote_create.chain_id.to_string()),
            ("program", ProgramSvm::Swap.to_string()),
            ("total_bids", total_bids),
        ];
        metrics::counter!("get_quote_total_bids", &labels).increment(1);

        if bids.is_empty() {
            tracing::warn!(opportunity = ?opportunity, "No bids found for quote opportunity");

            // Remove opportunity to prevent further bids
            // The handle auction loop will take care of the bids that were submitted late
            self.remove_quote_opportunity(opportunity.clone()).await;
            return Err(RestError::QuoteNotFound);
        }

        // Find winner bid:
        match input.quote_create.tokens {
            entities::QuoteTokens::UserTokenSpecified { .. } => {
                // highest bid = best (most searcher token returned)
                bids.sort_by(|a, b| b.amount.cmp(&a.amount));
            }
            entities::QuoteTokens::SearcherTokenSpecified { .. } => {
                // lowest bid = best (least user token consumed)
                bids.sort_by(|a, b| a.amount.cmp(&b.amount));
            }
        }
        let winner_bid = bids.first().expect("failed to get first bid");
        tracing::Span::current().record("winner_bid_id", winner_bid.id.to_string());

        let (_, swap_instruction) = auction_service
            .extract_express_relay_instruction(
                winner_bid.chain_data.transaction.clone(),
                BidPaymentInstructionType::Swap,
            )
            .map_err(|e| {
                tracing::error!("Failed to verify swap instruction: {:?}", e);
                RestError::TemporarilyUnavailable
            })?;
        let swap_data = auction_service
            .extract_swap_data(&swap_instruction)
            .await
            .map_err(|e| {
                tracing::error!("Failed to extract swap data: {:?}", e);
                RestError::TemporarilyUnavailable
            })?;
        let deadline = swap_data.deadline;

        // Bids are not empty
        let mut auction = Auction::try_new(bids.clone(), bid_collection_time)
            .expect("Failed to create auction for bids");
        // Add tx_hash to auction to make sure conclude_auction works correctly
        // TODO These are auctions that are not submitted but has tx_hash of the winner bid inside
        // TODO Maybe we should think a bit more about how to handle these auctions and the overall auction model
        auction.tx_hash = Some(winner_bid.chain_data.transaction.signatures[0]);
        // NOTE: These auctions need user signature to be submitted later.
        // Later if we have a quote without last look, we can assume these auctions are submitted.
        let auction = auction_service
            .add_auction(AddAuctionInput { auction })
            .await?;
        tracing::Span::current().record("auction_id", auction.id.to_string());

        // Remove opportunity to prevent further bids
        // The handle auction loop will take care of the bids that were submitted late
        self.remove_quote_opportunity(opportunity.clone()).await;

        let signature = winner_bid.chain_data.transaction.signatures[0];
        // Update the status of all bids in the auction except the winner bid
        let auction_bids = auction.bids.clone();
        auction_bids.into_iter().for_each(|bid| {
            if bid.id != winner_bid.id {
                self.task_tracker.spawn({
                    let (auction_service, winner_bid) =
                        (auction_service.clone(), winner_bid.clone());
                    async move {
                        auction_service
                            .update_bid_status(UpdateBidStatusInput {
                                new_status: auction::service::Service::get_new_status(
                                    &bid,
                                    &[winner_bid],
                                    BidStatusAuction {
                                        tx_hash: signature,
                                        id:      auction.id,
                                    },
                                ),
                                bid,
                            })
                            .await
                    }
                });
            }
        });

        let new_status = if input.quote_create.cancellable {
            BidStatusSvm::AwaitingSignature {
                auction: BidStatusAuction {
                    tx_hash: signature,
                    id:      auction.id,
                },
            }
        } else {
            BidStatusSvm::SentToUserForSubmission {
                auction: BidStatusAuction {
                    tx_hash: signature,
                    id:      auction.id,
                },
            }
        };
        // We check if the winner bid status is successfully updated.
        // This is important for the submit_quote function to work correctly.
        if !auction_service
            .update_bid_status(UpdateBidStatusInput {
                new_status,
                bid: winner_bid.clone(),
            })
            .await?
        {
            // This can only happen if the bid is already updated by another auction for another get_quote request
            // TODO We should handle this case more gracefully
            return Err(RestError::DuplicateOpportunity);
        }

        let metadata = self
            .get_express_relay_metadata(GetExpressRelayMetadataInput {
                chain_id: input.quote_create.chain_id.clone(),
            })
            .await?;

        let fee_token = match swap_data.fee_token {
            FeeToken::Searcher => searcher_token.token,
            FeeToken::User => user_token.token,
        };
        let compute_fees = |amount: u64| {
            metadata
                .compute_swap_fees_with_default_platform_fee(
                    referral_fee_info.referral_fee_ppm,
                    amount,
                )
                .map_err(|e| {
                    tracing::error!("Failed to compute swap fees: {:?}", e);
                    RestError::TemporarilyUnavailable
                })
        };
        let (searcher_amount, user_amount, fees) = match swap_data.fee_token {
            FeeToken::Searcher => {
                let swap_fees = compute_fees(swap_data.amount_searcher)?;
                (
                    swap_fees.remaining_amount,
                    swap_data.amount_user,
                    swap_fees.fees,
                )
            }
            FeeToken::User => (
                swap_data.amount_searcher,
                swap_data.amount_user,
                compute_fees(swap_data.amount_user)?.fees,
            ),
        };

        let OpportunitySvmProgramSwap {
            user_mint_user_balance,
            ..
        } = match &opportunity.program {
            OpportunitySvmProgram::Swap(swap) => swap,
            _ => return Err(RestError::TemporarilyUnavailable), // This should be unreachable
        };

        let (transaction, expiration_time) = if *user_mint_user_balance >= swap_data.amount_user {
            (
                Some(winner_bid.chain_data.transaction.clone()),
                Some(deadline),
            )
        } else {
            (None, None)
        };

        let transaction = match (&transaction, input.quote_create.cancellable) {
            (Some(_), false) => Some(
                auction_service
                    .sign_bid_and_submit_auction(winner_bid.clone(), auction.clone())
                    .await?,
            ),
            _ => transaction,
        };


        Ok(entities::Quote {
            transaction,
            expiration_time,

            searcher_token: TokenAmountSvm {
                token:  searcher_token.token,
                amount: searcher_amount,
            },
            user_token: TokenAmountSvm {
                token:  user_token.token,
                amount: user_amount,
            },
            referrer_fee: TokenAmountSvm {
                token:  fee_token,
                amount: fees.router_fee,
            },
            platform_fee: TokenAmountSvm {
                token:  fee_token,
                amount: fees.express_relay_fee + fees.relayer_fee,
            },
            chain_id: input.quote_create.chain_id,
            reference_id: auction.id,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::needless_update)]

    use {
        super::*,
        crate::{
            auction::{
                self,
                entities::{
                    BidChainDataSvm,
                    BidStatusSvm,
                },
                service::{
                    MockService as MockAuctionService,
                    StatefulMockAuctionService,
                },
            },
            config::TokenWhitelistConfig,
            kernel::{
                rpc_client_svm_tester::{
                    RpcClientSvmTester,
                    TokenAccountWithLamports,
                },
                test_utils::DEFAULT_CHAIN_ID,
            },
            opportunity::{
                entities::{
                    QuoteCreate,
                    QuoteTokens,
                    ReferralFeeInfo,
                },
                repository::MockDatabase,
                service::TokenWhitelist,
            },
        },
        anchor_lang::{
            AnchorSerialize,
            Discriminator,
        },
        express_relay::{
            state::ExpressRelayMetadata,
            SwapV2Args,
        },
        solana_sdk::{
            instruction::CompiledInstruction,
            signature::Signature,
            transaction::VersionedTransaction,
        },
        spl_token_2022::state::{
            Account as TokenAccount,
            AccountState,
        },
        std::time::Instant,
        uuid::Uuid,
    };

    // The default test auction id
    const DEFAULT_AUCTION_ID: Uuid = Uuid::from_u128(4242);

    #[derive(Clone, Default)]
    struct BidParams {
        id:        Option<Uuid>,
        amount:    Option<u64>,
        signature: Option<Vec<Signature>>,
    }

    fn make_test_bid(params: BidParams) -> auction::entities::Bid {
        let BidParams {
            id,
            signature,
            amount,
        } = params;

        auction::entities::Bid {
            id:              id.unwrap_or(Uuid::from_u128(1)),
            chain_id:        DEFAULT_CHAIN_ID.to_string(),
            initiation_time: OffsetDateTime::from_unix_timestamp(1200).unwrap(),
            profile_id:      None,
            amount:          amount.unwrap_or(100),
            status:          BidStatusSvm::Pending,
            chain_data:      BidChainDataSvm {
                transaction:                  VersionedTransaction {
                    signatures: signature.unwrap_or(vec![Signature::new_unique()]),
                    message:    Default::default(),
                },
                bid_payment_instruction_type: BidPaymentInstructionType::SubmitBid,
                router:                       Default::default(),
                permission_account:           Default::default(),
            },
        }
    }

    #[derive(Clone, Default)]
    struct AuctionServiceSequenceParams {
        bids:            Option<Vec<BidParams>>,
        swap_args:       Option<SwapV2Args>,
        skip_bid_update: bool,
    }

    fn setup_mock_auction_service(
        params: AuctionServiceSequenceParams,
    ) -> StatefulMockAuctionService {
        let mut auction_service = StatefulMockAuctionService::default();
        let AuctionServiceSequenceParams {
            bids,
            swap_args,
            skip_bid_update,
        } = params;

        auction_service
            .expect_get_express_relay_program_id()
            .returning(|| {
                // relay program id
                express_relay::id()
            });

        let bids_pending = bids.clone();
        auction_service
            .expect_get_pending_bids()
            .return_once(move |_| {
                bids_pending
                    .unwrap_or(vec![BidParams::default()])
                    .into_iter()
                    .map(make_test_bid)
                    .collect()
            });

        let swap_args = swap_args.unwrap_or(SwapV2Args {
            deadline:              1,
            amount_searcher:       100,
            amount_user:           1,
            referral_fee_ppm:      0,
            fee_token:             FeeToken::User,
            swap_platform_fee_ppm: 0,
        });
        auction_service
            .expect_extract_express_relay_instruction()
            .return_once(move |_, _| {
                let mut data = express_relay::instruction::SwapV2::DISCRIMINATOR.to_vec();
                data.append(&mut swap_args.try_to_vec().unwrap());
                Ok((
                    1,
                    CompiledInstruction {
                        program_id_index: 0,
                        accounts: vec![],
                        data,
                    },
                ))
            });

        auction_service
            .expect_extract_swap_data()
            .return_once(move |_| Ok(swap_args));

        auction_service.expect_add_auction().returning(move |_| {
            Ok(Auction {
                id:                  DEFAULT_AUCTION_ID,
                chain_id:            DEFAULT_CHAIN_ID.to_string(),
                permission_key:      PermissionKeySvm([0; 65]),
                creation_time:       OffsetDateTime::from_unix_timestamp(1300).unwrap(),
                conclusion_time:     None,
                bid_collection_time: OffsetDateTime::from_unix_timestamp(1200).unwrap(),
                submission_time:     None,
                tx_hash:             None,
                bids:                bids
                    .clone()
                    .unwrap_or(vec![BidParams::default()])
                    .into_iter()
                    .map(make_test_bid)
                    .collect(),
            })
        });

        if !skip_bid_update {
            auction_service
                .expect_update_bid_status()
                .returning(|_| Ok(true));
        }

        auction_service
            .expect_sign_bid_and_submit_auction()
            .returning(|_, _| {
                Ok(VersionedTransaction {
                    signatures: vec![Signature::new_unique()],
                    message:    Default::default(),
                })
            });

        auction_service
    }

    #[derive(Clone, Default)]
    struct QuoteSequenceParams {
        auction_service_sequence: AuctionServiceSequenceParams,
        metadata:                 Option<ExpressRelayMetadata>,
        enable_token_whitelist:   bool,
    }

    struct QuoteSequence {
        service:         Service,
        auction_service: StatefulMockAuctionService,
        rpc_client:      RpcClientSvmTester,

        token_program_user:     Pubkey,
        token_program_searcher: Pubkey,

        allowed_token_mint_1: Pubkey,
        allowed_token_mint_2: Pubkey,
    }

    async fn setup_basic_sequence(params: QuoteSequenceParams) -> QuoteSequence {
        let QuoteSequenceParams {
            auction_service_sequence: auction_service_params,
            metadata,
            enable_token_whitelist,
        } = params;

        let chain_id = DEFAULT_CHAIN_ID.to_string();
        let rpc_client = RpcClientSvmTester::new();
        let mut mock_db = MockDatabase::default();
        mock_db.expect_add_opportunity().returning(|_| Ok(()));
        mock_db.expect_remove_opportunity().returning(|_, _| Ok(()));

        let test_token_program_user = Pubkey::new_unique();
        let test_token_program_searcher = Pubkey::new_unique();
        let (mut service, _) = Service::new_with_mocks_svm(chain_id.clone(), mock_db, &rpc_client);
        let config = service.config.get_mut(&chain_id).unwrap();

        let allowed_token_mint_1 = Pubkey::new_unique();
        let allowed_token_mint_2 = Pubkey::new_unique();
        config
            .accepted_token_programs
            .extend([test_token_program_user, test_token_program_searcher]);
        config.token_whitelist = TokenWhitelist::from(TokenWhitelistConfig {
            enabled:         enable_token_whitelist,
            whitelist_mints: vec![allowed_token_mint_1, allowed_token_mint_2],
        });

        service
            .repo
            .cache_express_relay_metadata(metadata.unwrap_or_default())
            .await;

        let auction_service = setup_mock_auction_service(auction_service_params);

        QuoteSequence {
            service,
            auction_service,
            rpc_client,
            token_program_user: test_token_program_user,
            token_program_searcher: test_token_program_searcher,
            allowed_token_mint_1,
            allowed_token_mint_2,
        }
    }

    fn inject_auction_service(
        service: &Service,
        auction_service_in_call: StatefulMockAuctionService,
    ) {
        let auction_service = MockAuctionService::new(auction_service_in_call);

        let config = service
            .get_config(&(DEFAULT_CHAIN_ID.to_string()))
            .expect("Failed to get opportunity service evm config");
        config
            .auction_service_container
            .inject_mock_service(auction_service);
    }

    #[test]
    fn test_indicative_price_taker() {
        let x = generate_indicative_price_taker();
        let formatted_key = x.to_string();
        assert_eq!(&formatted_key[0..4], "Pric");
        assert!(is_indicative_price_taker(&x));
    }

    #[tokio::test]
    async fn test_get_quote_indicative_happy_path() {
        let win_sig = Signature::new_unique();

        let QuoteSequence {
            service,
            mut auction_service,
            token_program_user,
            token_program_searcher,
            allowed_token_mint_1,
            allowed_token_mint_2,
            ..
        } = setup_basic_sequence(QuoteSequenceParams {
            auction_service_sequence: AuctionServiceSequenceParams {
                bids: Some(vec![BidParams {
                    id: Some(Uuid::from_u128(1)),
                    signature: Some(vec![win_sig]),
                    ..Default::default()
                }]),
                swap_args: Some(SwapV2Args {
                    deadline:              10,
                    amount_searcher:       101,
                    amount_user:           1,
                    referral_fee_ppm:      0,
                    fee_token:             FeeToken::User,
                    swap_platform_fee_ppm: 0,
                }),
                skip_bid_update: true,
                ..Default::default()
            },
            enable_token_whitelist: true,
            ..Default::default()
        })
        .await;

        auction_service
            .expect_update_bid_status()
            .times(1)
            .returning(move |update| {
                assert_eq!(update.bid.id, Uuid::from_u128(1));
                assert_eq!(
                    update.new_status,
                    BidStatusSvm::AwaitingSignature {
                        auction: BidStatusAuction {
                            id:      DEFAULT_AUCTION_ID,
                            tx_hash: win_sig,
                        },
                    }
                );

                Ok(true)
            });

        inject_auction_service(&service, auction_service);

        let user_token = allowed_token_mint_1;
        let searcher_token = allowed_token_mint_2;
        service
            .repo
            .cache_token_program(searcher_token, token_program_user)
            .await;
        service
            .repo
            .cache_token_program(user_token, token_program_searcher)
            .await;

        let quote = service
            .get_quote(GetQuoteInput {
                quote_create: QuoteCreate {
                    user_wallet_address: None,
                    tokens:              QuoteTokens::UserTokenSpecified {
                        user_token: TokenAmountSvm {
                            token:  user_token,
                            amount: 2,
                        },
                        searcher_token,
                    },
                    referral_fee_info:   None,
                    chain_id:            DEFAULT_CHAIN_ID.to_string(),
                    memo:                None,
                    cancellable:         true,
                    minimum_lifetime:    None,
                    profile_id:          None,
                },
            })
            .await
            .expect("Failed to submit quote");

        assert_eq!(quote.reference_id, DEFAULT_AUCTION_ID);
        assert_eq!(
            quote.searcher_token,
            TokenAmountSvm {
                token:  searcher_token,
                amount: 101,
            }
        );
        assert_eq!(
            quote.user_token,
            TokenAmountSvm {
                token:  user_token,
                amount: 1,
            }
        );
        assert_eq!(
            quote.referrer_fee,
            TokenAmountSvm {
                token:  user_token,
                amount: 0,
            }
        );
        assert_eq!(
            quote.platform_fee,
            TokenAmountSvm {
                token:  user_token,
                amount: 0,
            }
        );
        assert_eq!(quote.transaction, None);
    }

    #[tokio::test]
    async fn test_get_quote_indicative_invalid_referral_fee() {
        let QuoteSequence {
            service,
            auction_service,
            ..
        } = setup_basic_sequence(QuoteSequenceParams::default()).await;
        inject_auction_service(&service, auction_service);

        let result = service
            .get_quote(GetQuoteInput {
                quote_create: QuoteCreate {
                    user_wallet_address: None,
                    tokens:              QuoteTokens::UserTokenSpecified {
                        user_token:     TokenAmountSvm {
                            token:  Pubkey::new_unique(),
                            amount: 2,
                        },
                        searcher_token: Pubkey::new_unique(),
                    },
                    referral_fee_info:   Some(ReferralFeeInfo {
                        router:           Pubkey::new_unique(),
                        referral_fee_ppm: 1_000_001,
                    }),
                    chain_id:            DEFAULT_CHAIN_ID.to_string(),
                    memo:                None,
                    cancellable:         true,
                    minimum_lifetime:    None,
                    profile_id:          None,
                },
            })
            .await;

        assert_eq!(
            result,
            Err(RestError::BadParameters(
                "Referral fee ppm higher than 1000000".to_string()
            ))
        );
    }

    #[tokio::test]
    async fn test_get_quote_indicative_no_bids() {
        let QuoteSequence {
            service,
            auction_service,
            token_program_user,
            token_program_searcher,
            ..
        } = setup_basic_sequence(QuoteSequenceParams {
            auction_service_sequence: AuctionServiceSequenceParams {
                bids: Some(Vec::new()),
                ..Default::default()
            },
            ..Default::default()
        })
        .await;
        inject_auction_service(&service, auction_service);

        let user_token = Pubkey::new_unique();
        let searcher_token = Pubkey::new_unique();
        service
            .repo
            .cache_token_program(searcher_token, token_program_user)
            .await;
        service
            .repo
            .cache_token_program(user_token, token_program_searcher)
            .await;

        let result = service
            .get_quote(GetQuoteInput {
                quote_create: QuoteCreate {
                    user_wallet_address: None,
                    tokens:              QuoteTokens::UserTokenSpecified {
                        user_token: TokenAmountSvm {
                            token:  user_token,
                            amount: 2,
                        },
                        searcher_token,
                    },
                    referral_fee_info:   None,
                    chain_id:            DEFAULT_CHAIN_ID.to_string(),
                    memo:                None,
                    cancellable:         true,
                    minimum_lifetime:    None,
                    profile_id:          None,
                },
            })
            .await;

        // no bids were submitted
        assert_eq!(result, Err(RestError::QuoteNotFound));
    }

    #[tokio::test]
    async fn test_get_quote_indicative_searcher_lowest_wins() {
        let win_sig = Signature::new_unique();
        let lose_sig = Signature::new_unique();

        let QuoteSequence {
            service,
            mut auction_service,
            token_program_user,
            token_program_searcher,
            ..
        } = setup_basic_sequence(QuoteSequenceParams {
            auction_service_sequence: AuctionServiceSequenceParams {
                bids: Some(vec![
                    BidParams {
                        id: Some(Uuid::from_u128(1)),
                        amount: Some(100),
                        signature: Some(vec![lose_sig]),
                        ..Default::default()
                    },
                    BidParams {
                        id: Some(Uuid::from_u128(2)),
                        amount: Some(10),
                        signature: Some(vec![win_sig]),
                        ..Default::default()
                    },
                ]),
                skip_bid_update: true,
                ..Default::default()
            },
            ..Default::default()
        })
        .await;

        auction_service
            .expect_update_bid_status()
            .times(2)
            .returning(move |update| {
                if update.bid.id == Uuid::from_u128(2) {
                    assert_eq!(
                        update.new_status,
                        BidStatusSvm::AwaitingSignature {
                            auction: BidStatusAuction {
                                id:      DEFAULT_AUCTION_ID,
                                tx_hash: win_sig,
                            },
                        }
                    );
                } else {
                    assert_eq!(
                        update.new_status,
                        BidStatusSvm::Lost {
                            auction: Some(BidStatusAuction {
                                id:      DEFAULT_AUCTION_ID,
                                // signature is just in the parameters, it doesnt actually get updated in the DB
                                tx_hash: win_sig,
                            }),
                        }
                    );
                }

                Ok(true)
            });

        inject_auction_service(&service, auction_service);

        let user_token = Pubkey::new_unique();
        let searcher_token = Pubkey::new_unique();
        service
            .repo
            .cache_token_program(searcher_token, token_program_user)
            .await;
        service
            .repo
            .cache_token_program(user_token, token_program_searcher)
            .await;

        service
            .get_quote(GetQuoteInput {
                quote_create: QuoteCreate {
                    user_wallet_address: None,
                    tokens:              QuoteTokens::SearcherTokenSpecified {
                        searcher_token: TokenAmountSvm {
                            token:  searcher_token,
                            amount: 2,
                        },
                        user_token,
                    },
                    referral_fee_info:   None,
                    chain_id:            DEFAULT_CHAIN_ID.to_string(),
                    memo:                None,
                    cancellable:         true,
                    minimum_lifetime:    None,
                    profile_id:          None,
                },
            })
            .await
            .expect("Failed to submit quote");

        // flush rest of the updates
        service.task_tracker.close();
        service.task_tracker.wait().await;
    }

    #[tokio::test]
    async fn test_get_quote_indicative_fee_estimation() {
        let QuoteSequence {
            service,
            auction_service,
            token_program_user,
            token_program_searcher,
            ..
        } = setup_basic_sequence(QuoteSequenceParams {
            auction_service_sequence: AuctionServiceSequenceParams {
                swap_args: Some(SwapV2Args {
                    deadline:              10,
                    amount_searcher:       20000,
                    amount_user:           2000,
                    referral_fee_ppm:      1500,
                    fee_token:             FeeToken::User,
                    swap_platform_fee_ppm: 0,
                }),
                ..Default::default()
            },
            metadata: Some(ExpressRelayMetadata {
                swap_platform_fee_bps: 10,
                split_relayer: 5000,
                ..Default::default()
            }),
            ..Default::default()
        })
        .await;
        inject_auction_service(&service, auction_service);

        let user_token = Pubkey::new_unique();
        let searcher_token = Pubkey::new_unique();
        service
            .repo
            .cache_token_program(searcher_token, token_program_user)
            .await;
        service
            .repo
            .cache_token_program(user_token, token_program_searcher)
            .await;

        let quote = service
            .get_quote(GetQuoteInput {
                quote_create: QuoteCreate {
                    user_wallet_address: None,
                    tokens:              QuoteTokens::UserTokenSpecified {
                        user_token: TokenAmountSvm {
                            token:  user_token,
                            amount: 2000,
                        },
                        searcher_token,
                    },
                    referral_fee_info:   None,
                    chain_id:            DEFAULT_CHAIN_ID.to_string(),
                    memo:                None,
                    cancellable:         true,
                    minimum_lifetime:    None,
                    profile_id:          None,
                },
            })
            .await
            .expect("Failed to submit quote");

        assert_eq!(quote.referrer_fee.amount, 0);
        assert_eq!(quote.platform_fee.amount, 2);
    }

    #[tokio::test]
    async fn test_get_quote_connected_wallet_happy_path() {
        let winner_sig = Signature::new_unique();

        let QuoteSequence {
            service,
            auction_service,
            rpc_client,
            token_program_user,
            token_program_searcher,
            ..
        } = setup_basic_sequence(QuoteSequenceParams {
            auction_service_sequence: AuctionServiceSequenceParams {
                swap_args: Some(SwapV2Args {
                    deadline:              10,
                    amount_searcher:       2000,
                    amount_user:           2000,
                    referral_fee_ppm:      1500,
                    fee_token:             FeeToken::Searcher,
                    swap_platform_fee_ppm: 0,
                }),
                bids: Some(vec![BidParams {
                    signature: Some(vec![winner_sig]),
                    amount: Some(100),
                    ..Default::default()
                }]),
                ..Default::default()
            },
            metadata: Some(ExpressRelayMetadata {
                swap_platform_fee_bps: 10,
                split_relayer: 5000,
                ..Default::default()
            }),
            ..Default::default()
        })
        .await;
        inject_auction_service(&service, auction_service);

        let user_token = Pubkey::new_unique();
        let searcher_token = Pubkey::new_unique();
        service
            .repo
            .cache_token_program(searcher_token, token_program_user)
            .await;
        service
            .repo
            .cache_token_program(user_token, token_program_searcher)
            .await;

        rpc_client
            .can_next_multi_call_token_accounts(
                std::iter::repeat(TokenAccountWithLamports {
                    lamports:      0,
                    token_account: TokenAccount {
                        amount: 5000,
                        state: AccountState::Initialized,
                        ..Default::default()
                    },
                })
                .take(6)
                .collect(),
            )
            .await;

        let user_wallet = Pubkey::new_unique();
        let quote = service
            .get_quote(GetQuoteInput {
                quote_create: QuoteCreate {
                    user_wallet_address: Some(user_wallet),
                    tokens:              QuoteTokens::SearcherTokenSpecified {
                        searcher_token: TokenAmountSvm {
                            token:  searcher_token,
                            amount: 2000,
                        },
                        user_token,
                    },
                    referral_fee_info:   None,
                    chain_id:            DEFAULT_CHAIN_ID.to_string(),
                    memo:                None,
                    cancellable:         true,
                    minimum_lifetime:    None,
                    profile_id:          None,
                },
            })
            .await
            .expect("Failed to submit quote");

        // winning bid signature is present, user can sign it
        assert_eq!(quote.transaction.unwrap().signatures, vec![winner_sig]);
        rpc_client.check_all_uncanned().await;
    }

    #[tokio::test]
    async fn test_get_quote_connected_not_enough_funds() {
        let QuoteSequence {
            service,
            auction_service,
            rpc_client,
            token_program_user,
            token_program_searcher,
            ..
        } = setup_basic_sequence(QuoteSequenceParams {
            auction_service_sequence: AuctionServiceSequenceParams {
                swap_args: Some(SwapV2Args {
                    deadline:              10,
                    amount_searcher:       2000,
                    amount_user:           2000,
                    referral_fee_ppm:      1500,
                    fee_token:             FeeToken::Searcher,
                    swap_platform_fee_ppm: 0,
                }),
                ..Default::default()
            },
            metadata: Some(ExpressRelayMetadata {
                swap_platform_fee_bps: 10,
                split_relayer: 5000,
                ..Default::default()
            }),
            ..Default::default()
        })
        .await;
        inject_auction_service(&service, auction_service);

        let user_token = Pubkey::new_unique();
        let searcher_token = Pubkey::new_unique();
        service
            .repo
            .cache_token_program(searcher_token, token_program_user)
            .await;
        service
            .repo
            .cache_token_program(user_token, token_program_searcher)
            .await;

        rpc_client
            .can_next_multi_call_token_accounts(
                std::iter::repeat(TokenAccountWithLamports {
                    lamports:      0,
                    token_account: TokenAccount {
                        amount: 10, // poor
                        state: AccountState::Initialized,
                        ..Default::default()
                    },
                })
                .take(6)
                .collect(),
            )
            .await;

        let user_wallet = Pubkey::new_unique();
        let quote = service
            .get_quote(GetQuoteInput {
                quote_create: QuoteCreate {
                    user_wallet_address: Some(user_wallet),
                    tokens:              QuoteTokens::SearcherTokenSpecified {
                        searcher_token: TokenAmountSvm {
                            token:  searcher_token,
                            amount: 2000,
                        },
                        user_token,
                    },
                    referral_fee_info:   None,
                    chain_id:            DEFAULT_CHAIN_ID.to_string(),
                    memo:                None,
                    cancellable:         true,
                    minimum_lifetime:    None,
                    profile_id:          None,
                },
            })
            .await
            .expect("Failed to submit quote");

        // quote became indicative
        assert_eq!(quote.transaction, None);
        rpc_client.check_all_uncanned().await;
    }

    #[tokio::test]
    async fn test_get_quote_token_not_whitelisted() {
        let QuoteSequence {
            service,
            auction_service,
            allowed_token_mint_1,
            ..
        } = setup_basic_sequence(QuoteSequenceParams {
            enable_token_whitelist: true,
            ..Default::default()
        })
        .await;
        inject_auction_service(&service, auction_service);
        let invalid_mint = Pubkey::new_unique();

        let result = service
            .get_quote(GetQuoteInput {
                quote_create: QuoteCreate {
                    user_wallet_address: None,
                    tokens:              QuoteTokens::UserTokenSpecified {
                        user_token:     TokenAmountSvm {
                            token:  invalid_mint,
                            amount: 2,
                        },
                        searcher_token: allowed_token_mint_1,
                    },
                    referral_fee_info:   Some(ReferralFeeInfo {
                        router:           Pubkey::new_unique(),
                        referral_fee_ppm: 20_000,
                    }),
                    chain_id:            DEFAULT_CHAIN_ID.to_string(),
                    memo:                None,
                    cancellable:         true,
                    minimum_lifetime:    None,
                    profile_id:          None,
                },
            })
            .await;

        assert_eq!(
            result,
            Err(RestError::TokenMintNotAllowed(
                "Input".to_string(),
                invalid_mint.to_string()
            ))
        );

        let result = service
            .get_quote(GetQuoteInput {
                quote_create: QuoteCreate {
                    user_wallet_address: None,
                    tokens:              QuoteTokens::UserTokenSpecified {
                        user_token:     TokenAmountSvm {
                            token:  allowed_token_mint_1, // invalid token
                            amount: 2,
                        },
                        searcher_token: invalid_mint,
                    },
                    referral_fee_info:   Some(ReferralFeeInfo {
                        router:           Pubkey::new_unique(),
                        referral_fee_ppm: 20_000,
                    }),
                    chain_id:            DEFAULT_CHAIN_ID.to_string(),
                    memo:                None,
                    cancellable:         true,
                    minimum_lifetime:    None,
                    profile_id:          None,
                },
            })
            .await;

        assert_eq!(
            result,
            Err(RestError::TokenMintNotAllowed(
                "Output".to_string(),
                invalid_mint.to_string()
            ))
        );
    }

    #[tokio::test]
    async fn test_get_quote_indicative_auction_time() {
        let QuoteSequence {
            mut service,
            auction_service,
            token_program_user,
            token_program_searcher,
            ..
        } = setup_basic_sequence(QuoteSequenceParams {
            auction_service_sequence: AuctionServiceSequenceParams {
                ..Default::default()
            },
            ..Default::default()
        })
        .await;

        inject_auction_service(&service, auction_service);

        let user_token = Pubkey::new_unique();
        let searcher_token = Pubkey::new_unique();
        service
            .repo
            .cache_token_program(searcher_token, token_program_user)
            .await;
        service
            .repo
            .cache_token_program(user_token, token_program_searcher)
            .await;

        service
            .config
            .get_mut(DEFAULT_CHAIN_ID)
            .expect("chain")
            .auction_time = Duration::from_millis(460);

        let start = Instant::now();
        let _ = service
            .get_quote(GetQuoteInput {
                quote_create: QuoteCreate {
                    user_wallet_address: None,
                    tokens:              QuoteTokens::UserTokenSpecified {
                        user_token: TokenAmountSvm {
                            token:  user_token,
                            amount: 2,
                        },
                        searcher_token,
                    },
                    referral_fee_info:   None,
                    chain_id:            DEFAULT_CHAIN_ID.to_string(),
                    memo:                None,
                    cancellable:         true,
                    minimum_lifetime:    None,
                    profile_id:          None,
                },
            })
            .await
            .expect("Failed to submit quote");
        let elapsed = start.elapsed();
        assert!(elapsed > Duration::from_millis(460));
    }
}
