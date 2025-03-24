use {
    super::{
        get_quote_request_account_balances::QuoteRequestAccountBalancesInput,
        get_token_program::GetTokenProgramInput,
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        auction::{
            entities::{
                Auction,
                BidPaymentInstructionType,
                BidStatusAuction,
            },
            service::{
                add_auction::AddAuctionInput,
                auction_manager::AuctionManager,
                get_pending_bids::GetLiveBidsInput,
                update_bid_status::UpdateBidStatusInput,
                Service as AuctionService,
            },
        },
        kernel::entities::PermissionKeySvm,
        opportunity::{
            entities::{
                self,
                OpportunitySvmProgram,
                OpportunitySvmProgramSwap,
                TokenAccountInitializationConfigs,
                TokenAmountSvm,
            },
            service::{
                add_opportunity::AddOpportunityInput,
                get_express_relay_metadata::GetExpressRelayMetadata,
            },
        },
    },
    ::express_relay::{
        state::FEE_SPLIT_PRECISION,
        FeeToken,
    },
    axum_prometheus::metrics,
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

/// Time to wait for searchers to submit bids.
const BID_COLLECTION_TIME: Duration = Duration::from_millis(500);

pub struct GetQuoteInput {
    pub quote_create: entities::QuoteCreate,
}

/// Get a pubkey based on router_token_account, user_wallet_address, referral_fee_bps, mints, and token amounts
/// This pubkey is never mentioned on-chain and is only used internally
/// to distinguish between different swap bids
pub fn get_quote_virtual_permission_account(
    tokens: &entities::QuoteTokens,
    user_wallet_address: &Pubkey,
    router_token_account: &Pubkey,
    referral_fee_bps: u16,
) -> Pubkey {
    let user_token_amount: [u8; 8];
    let searcher_token_amount: [u8; 8];
    let referral_fee_bps = referral_fee_bps.to_le_bytes();
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
                referral_fee_bps.as_ref(),
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
                referral_fee_bps.as_ref(),
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

impl Service<ChainTypeSvm> {
    async fn get_opportunity_create_for_quote(
        &self,
        quote_create: entities::QuoteCreate,
    ) -> Result<entities::OpportunityCreateSvm, RestError> {
        let referral_fee_info = self
            .unwrap_referral_fee_info(quote_create.referral_fee_info, &quote_create.chain_id)
            .await?;

        // TODO*: we should fix the Opportunity struct (or create a new format) to more clearly distinguish Swap opps from traditional opps
        // currently, we are using the same struct and just setting the unspecified token amount to 0
        let metadata = self
            .get_express_relay_metadata(GetExpressRelayMetadata {
                chain_id: quote_create.chain_id.clone(),
            })
            .await?;
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
        let fee_token = get_fee_token(mint_user, mint_searcher, &config.ordered_fee_tokens);
        let (searcher_amount, user_amount) = match (quote_create.tokens.clone(), fee_token.clone())
        {
            (
                entities::QuoteTokens::SearcherTokenSpecified { searcher_token, .. },
                entities::FeeToken::SearcherToken,
            ) => {
                // This is not exactly accurate and may overestimate the amount needed
                // because of floor / ceil rounding errors.
                let denominator: u64 = FEE_SPLIT_PRECISION
                    - <u16 as Into<u64>>::into(referral_fee_info.referral_fee_bps)
                    - metadata.swap_platform_fee_bps;
                let numerator = searcher_token.amount * FEE_SPLIT_PRECISION;
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
        let (user_wallet_address, user_mint_user_balance, token_account_initialization_config) =
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

        let permission_account = get_quote_virtual_permission_account(
            &tokens_for_permission,
            &user_wallet_address,
            &router_token_account,
            referral_fee_info.referral_fee_bps,
        );

        let core_fields = entities::OpportunityCoreFieldsCreate {
            permission_key: entities::OpportunitySvm::get_permission_key(
                BidPaymentInstructionType::Swap,
                referral_fee_info.router,
                permission_account,
            ),
            chain_id:       quote_create.chain_id.clone(),
            sell_tokens:    vec![entities::TokenAmountSvm {
                token:  mint_searcher,
                amount: searcher_amount,
            }],
            buy_tokens:     vec![entities::TokenAmountSvm {
                token:  mint_user,
                amount: user_amount,
            }],
        };

        let program_opportunity =
            entities::OpportunitySvmProgram::Swap(entities::OpportunitySvmProgramSwap {
                user_wallet_address,
                fee_token,
                referral_fee_bps: referral_fee_info.referral_fee_bps,
                platform_fee_bps: metadata.swap_platform_fee_bps,
                token_program_user,
                user_mint_user_balance,
                token_account_initialization_config,
                token_program_searcher,
            });

        Ok(entities::OpportunityCreateSvm {
            core_fields,
            router: referral_fee_info.router,
            permission_account,
            program: program_opportunity,
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

    #[tracing::instrument(skip_all)]
    pub async fn get_quote(&self, input: GetQuoteInput) -> Result<entities::Quote, RestError> {
        let referral_fee_info = self
            .unwrap_referral_fee_info(
                input.quote_create.referral_fee_info.clone(),
                &input.quote_create.chain_id,
            )
            .await?;

        // TODO use compute_swap_fees to make sure instead when the metadata is fetched from on-chain
        if FEE_SPLIT_PRECISION < referral_fee_info.referral_fee_bps.into() {
            return Err(RestError::BadParameters(format!(
                "Referral fee bps higher than {}",
                FEE_SPLIT_PRECISION
            )));
        }

        let config = self.get_config(&input.quote_create.chain_id)?;
        let auction_service = config.get_auction_service().await;

        tracing::info!(quote_create = ?input.quote_create, "Received request to get quote");

        let opportunity_create = self
            .get_opportunity_create_for_quote(input.quote_create.clone())
            .await?;
        let opportunity = self
            .add_opportunity(AddOpportunityInput {
                opportunity: opportunity_create,
            })
            .await?;
        let searcher_token = opportunity.sell_tokens[0].clone();
        let user_token = opportunity.buy_tokens[0].clone();
        if searcher_token.amount == 0 && user_token.amount == 0 {
            return Err(RestError::BadParameters(
                "Token amount cannot be zero".to_string(),
            ));
        }

        // Wait to make sure searchers had enough time to submit bids
        sleep(BID_COLLECTION_TIME).await;

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

        let (_, swap_instruction) = auction_service
            .extract_express_relay_instruction(
                winner_bid.chain_data.transaction.clone(),
                BidPaymentInstructionType::Swap,
            )
            .map_err(|e| {
                tracing::error!("Failed to verify swap instruction: {:?}", e);
                RestError::TemporarilyUnavailable
            })?;
        let swap_data = AuctionService::extract_swap_data(&swap_instruction).map_err(|e| {
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
                                new_status: AuctionService::get_new_status(
                                    &bid,
                                    &vec![winner_bid],
                                    BidStatusAuction {
                                        tx_hash: signature,
                                        id:      auction.id,
                                    },
                                    false,
                                ),
                                bid,
                            })
                            .await
                    }
                });
            }
        });

        // We check if the winner bid status is successfully updated.
        // This is important for the submit_quote function to work correctly.
        if !auction_service
            .update_bid_status(UpdateBidStatusInput {
                new_status: AuctionService::get_new_status(
                    winner_bid,
                    &vec![winner_bid.clone()],
                    BidStatusAuction {
                        tx_hash: signature,
                        id:      auction.id,
                    },
                    false,
                ),
                bid:        winner_bid.clone(),
            })
            .await?
        {
            // This can only happen if the bid is already updated by another auction for another get_quote request
            // TODO We should handle this case more gracefully
            return Err(RestError::DuplicateOpportunity);
        }

        let metadata = self
            .get_express_relay_metadata(GetExpressRelayMetadata {
                chain_id: input.quote_create.chain_id.clone(),
            })
            .await?;

        let fee_token = match swap_data.fee_token {
            FeeToken::Searcher => searcher_token.token,
            FeeToken::User => user_token.token,
        };
        let compute_fees = |amount: u64| {
            metadata
                .compute_swap_fees(referral_fee_info.referral_fee_bps, amount)
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
                amount: fees.relayer_fee,
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
    use super::*;


    #[test]
    fn test_indicative_price_taker() {
        let x = generate_indicative_price_taker();
        let formatted_key = x.to_string();
        assert_eq!(&formatted_key[0..4], "Pric");
        assert!(is_indicative_price_taker(&x));
    }
}
