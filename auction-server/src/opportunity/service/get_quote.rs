use {
    super::{
        get_token_program::GetTokenProgramInput,
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        auction::{
            entities::{
                Auction,
                Bid,
                BidPaymentInstructionType,
                BidStatus,
                BidStatusAuction,
            },
            service::{
                add_auction::AddAuctionInput,
                auction_manager::AuctionManager,
                get_auction_by_id::GetAuctionByIdInput,
                get_pending_bids::GetLiveBidsInput,
                update_bid_status::UpdateBidStatusInput,
                Service as AuctionService,
            },
        },
        kernel::entities::{
            PermissionKeySvm,
            Svm,
        },
        opportunity::{
            api::INDICATIVE_PRICE_TAKER,
            entities::{
                self,
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
    futures::future::join_all,
    solana_sdk::pubkey::Pubkey,
    spl_associated_token_account::get_associated_token_address_with_program_id,
    std::{
        str::FromStr,
        time::Duration,
    },
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


/// Time to wait for searchers to submit bids.
const BID_COLLECTION_TIME: Duration = Duration::from_millis(500);

pub struct GetQuoteInput {
    pub quote_create: entities::QuoteCreate,
    pub program:      ProgramSvm,
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

/// Determines if the fee token should be the user token or the searcher token
fn get_fee_token(user_mint: Pubkey, _searcher_mint: Pubkey) -> entities::FeeToken {
    // TODO*: we should determine this more intelligently
    // Prefer USDC and USDT as the fee token
    if user_mint == Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB").unwrap()
        || user_mint == Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap()
    {
        entities::FeeToken::UserToken
    } else {
        entities::FeeToken::SearcherToken
    }
}

impl Service<ChainTypeSvm> {
    async fn get_opportunity_create_for_quote(
        &self,
        quote_create: entities::QuoteCreate,
        program: &ProgramSvm,
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
        let (user_mint, searcher_mint) = match quote_create.tokens.clone() {
            entities::QuoteTokens::UserTokenSpecified {
                user_token,
                searcher_token,
            } => (user_token.token, searcher_token),
            entities::QuoteTokens::SearcherTokenSpecified {
                user_token,
                searcher_token,
            } => (user_token, searcher_token.token),
        };
        let fee_token = get_fee_token(user_mint, searcher_mint);
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
                mint:     searcher_mint,
            })
            .await
            .map_err(|err| {
                tracing::error!("Failed to get searcher token program: {:?}", err);
                RestError::BadParameters("Searcher token program not found".to_string())
            })?;
        let token_program_user = self
            .get_token_program(GetTokenProgramInput {
                chain_id: quote_create.chain_id.clone(),
                mint:     user_mint,
            })
            .await
            .map_err(|err| {
                tracing::error!("Failed to get user token program: {:?}", err);
                RestError::BadParameters("User token program not found".to_string())
            })?;

        let router_token_account = match fee_token {
            entities::FeeToken::SearcherToken => get_associated_token_address_with_program_id(
                &referral_fee_info.router.to_bytes().into(),
                &searcher_mint.to_bytes().into(),
                &token_program_searcher.to_bytes().into(),
            ),
            entities::FeeToken::UserToken => get_associated_token_address_with_program_id(
                &referral_fee_info.router.to_bytes().into(),
                &user_mint.to_bytes().into(),
                &token_program_user.to_bytes().into(),
            ),
        }
        .to_bytes()
        .into();
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
        let user_wallet_address = match quote_create.user_wallet_address {
            Some(address) => address,
            None => INDICATIVE_PRICE_TAKER,
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
                token:  searcher_mint,
                amount: searcher_amount,
            }],
            buy_tokens:     vec![entities::TokenAmountSvm {
                token:  user_mint,
                amount: user_amount,
            }],
        };

        let program_opportunity = match program {
            ProgramSvm::Swap => {
                entities::OpportunitySvmProgram::Swap(entities::OpportunitySvmProgramSwap {
                    user_wallet_address,
                    fee_token,
                    referral_fee_bps: referral_fee_info.referral_fee_bps,
                    platform_fee_bps: metadata.swap_platform_fee_bps,
                    token_program_user,
                    token_program_searcher,
                })
            }
            _ => {
                return Err(RestError::Forbidden);
            }
        };

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
            .get_opportunity_create_for_quote(input.quote_create.clone(), &input.program)
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
            ("program", input.program.to_string()),
            ("total_bids", total_bids),
        ];
        metrics::counter!("get_quote_total_bids", &labels).increment(1);

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

        let mut bids_filtered: Vec<Bid<Svm>> = Vec::new();
        // here optimize_bids is used to batch the bid simulation.
        // the first bid in the returned vector is the best bid that passes simulation.
        if !bids.is_empty() {
            if input.quote_create.user_wallet_address.is_none() {
                // TODO: we may want to filter out bids by simulation later, but for now we just take the best bid as an indicative price
                bids_filtered = bids.clone();
            } else {
                bids_filtered = auction_service
                    .optimize_bids(&bids)
                    .await
                    .map(|x| x.value)
                    .map_err(|e| {
                        tracing::error!("Failed to simulate swap bids: {:?}", e);
                        RestError::TemporarilyUnavailable
                    })?;
            }
        }

        if bids_filtered.is_empty() {
            tracing::warn!(opportunity = ?opportunity, "No bids found for quote opportunity");

            // Remove opportunity to prevent further bids
            // The handle auction loop will take care of the bids that were submitted late
            self.remove_quote_opportunity(opportunity.clone()).await;
            return Err(RestError::QuoteNotFound);
        }

        let winner_bid = bids_filtered.first().expect("failed to get first bid");

        let swap_instruction = auction_service
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
        join_all(auction.bids.iter().map(|bid| {
            auction_service.update_bid_status(UpdateBidStatusInput {
                new_status: AuctionService::get_new_status(
                    bid,
                    &vec![winner_bid.clone()],
                    BidStatusAuction {
                        tx_hash: signature,
                        id:      auction.id,
                    },
                    false,
                ),
                bid:        bid.clone(),
            })
        }))
        .await;

        // We check if the winner bid status is successfully updated.
        // This is important for the submit_quote function to work correctly.
        match auction_service
            .get_auction_by_id(GetAuctionByIdInput {
                auction_id: auction.id,
            })
            .await
        {
            Some(auction) => match auction.bids.iter().find(|bid| bid.id == winner_bid.id) {
                Some(bid) => {
                    if !bid.status.is_awaiting_signature() {
                        tracing::error!(winner_bid = ?winner_bid, opportunity = ?opportunity, "Failed to update winner bid status");
                        return Err(RestError::TemporarilyUnavailable);
                    }
                }
                None => {
                    tracing::error!(auction = ?auction, winner_bid = ?winner_bid, "Failed to winner bid from auction");
                    return Err(RestError::TemporarilyUnavailable);
                }
            },
            None => {
                tracing::error!(auction = ?auction, opportunity = ?opportunity, winner_bid = ?winner_bid, "Failed to get auction by id");
                return Err(RestError::TemporarilyUnavailable);
            }
        };

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

        let (transaction, expiration_time) = match input.quote_create.user_wallet_address {
            None => (None, None),
            Some(_) => (
                Some(winner_bid.chain_data.transaction.clone()),
                Some(deadline),
            ),
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
