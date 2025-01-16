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
                BidPaymentInstructionType,
                BidStatus,
                BidStatusAuction,
            },
            service::{
                add_auction::AddAuctionInput,
                auction_manager::AuctionManager,
                get_live_bids::GetLiveBidsInput,
                update_bid_status::UpdateBidStatusInput,
                update_submitted_auction::UpdateSubmittedAuctionInput,
                Service as AuctionService,
            },
        },
        kernel::entities::PermissionKeySvm,
        opportunity::{
            entities,
            service::add_opportunity::AddOpportunityInput,
        },
    },
    axum_prometheus::metrics,
    // TODO: is it okay to import api types into the service layer?
    express_relay_api_types::opportunity::ProgramSvm,
    futures::future::join_all,
    solana_sdk::{
        clock::Slot,
        pubkey::Pubkey,
    },
    std::time::Duration,
    time::OffsetDateTime,
    tokio::time::sleep,
};

/// Time to wait for searchers to submit bids.
const BID_COLLECTION_TIME: Duration = Duration::from_millis(500);

pub struct GetQuoteInput {
    pub quote_create: entities::QuoteCreate,
    pub program:      ProgramSvm,
}

pub fn get_quote_permission_key(
    tokens: &entities::QuoteTokens,
    user_wallet_address: &Pubkey,
    referral_fee_bps: u16,
) -> Pubkey {
    // get pda seeded by user_wallet_address, referral_fee_bps, mints, and token amount
    let input_token_amount: [u8; 8];
    let output_token_amount: [u8; 8];
    let referral_fee_bps = referral_fee_bps.to_le_bytes();
    let seeds = match tokens {
        entities::QuoteTokens::InputTokenSpecified {
            input_token,
            output_token,
        } => {
            let input_token_mint = input_token.token.as_ref();
            let output_token_mint = output_token.as_ref();
            input_token_amount = input_token.amount.to_le_bytes();
            [
                user_wallet_address.as_ref(),
                input_token_mint,
                input_token_amount.as_ref(),
                output_token_mint,
                referral_fee_bps.as_ref(),
            ]
        }
        entities::QuoteTokens::OutputTokenSpecified {
            input_token,
            output_token,
        } => {
            let input_token_mint = input_token.as_ref();
            let output_token_mint = output_token.token.as_ref();
            output_token_amount = output_token.amount.to_le_bytes();
            [
                user_wallet_address.as_ref(),
                input_token_mint,
                output_token_mint,
                output_token_amount.as_ref(),
                referral_fee_bps.as_ref(),
            ]
        }
    };
    // since this permission key will not be used on-chain, we don't need to use the express relay program_id.
    // we can use a distinctive bytes object for the program_id
    Pubkey::find_program_address(&seeds, &Pubkey::default()).0
}

impl Service<ChainTypeSvm> {
    async fn get_opportunity_create_for_quote(
        &self,
        quote_create: entities::QuoteCreate,
        program: &ProgramSvm,
    ) -> Result<entities::OpportunityCreateSvm, RestError> {
        let router = quote_create.router;
        let permission_account = get_quote_permission_key(
            &quote_create.tokens,
            &quote_create.user_wallet_address,
            quote_create.referral_fee_bps,
        );

        // TODO*: we should fix the Opportunity struct (or create a new format) to more clearly distinguish Swap opps from traditional opps
        // currently, we are using the same struct and just setting the unspecified token amount to 0
        let (input_mint, input_amount, output_mint, output_amount) = match quote_create.tokens {
            entities::QuoteTokens::InputTokenSpecified {
                input_token,
                output_token,
            } => (input_token.token, input_token.amount, output_token, 0),
            entities::QuoteTokens::OutputTokenSpecified {
                input_token,
                output_token,
            } => (input_token, 0, output_token.token, output_token.amount),
        };

        let core_fields = entities::OpportunityCoreFieldsCreate {
            permission_key: entities::OpportunitySvm::get_permission_key(
                BidPaymentInstructionType::Swap,
                router,
                permission_account,
            ),
            chain_id:       quote_create.chain_id.clone(),
            sell_tokens:    vec![entities::TokenAmountSvm {
                token:  output_mint,
                amount: output_amount,
            }],
            buy_tokens:     vec![entities::TokenAmountSvm {
                token:  input_mint,
                amount: input_amount,
            }],
        };

        let input_token_program = self
            .get_token_program(GetTokenProgramInput {
                chain_id: quote_create.chain_id.clone(),
                mint:     input_mint,
            })
            .await
            .map_err(|err| {
                tracing::error!("Failed to get input token program: {:?}", err);
                RestError::BadParameters("Input token program not found".to_string())
            })?;
        let output_token_program = self
            .get_token_program(GetTokenProgramInput {
                chain_id: quote_create.chain_id.clone(),
                mint:     output_mint,
            })
            .await
            .map_err(|err| {
                tracing::error!("Failed to get output token program: {:?}", err);
                RestError::BadParameters("Output token program not found".to_string())
            })?;

        let program_opportunity = match program {
            ProgramSvm::SwapKamino => {
                entities::OpportunitySvmProgram::SwapKamino(entities::OpportunitySvmProgramSwap {
                    user_wallet_address: quote_create.user_wallet_address,
                    // TODO*: we should determine this more intelligently
                    fee_token: entities::FeeToken::InputToken,
                    referral_fee_bps: quote_create.referral_fee_bps,
                    input_token_program,
                    output_token_program,
                })
            }
            _ => {
                return Err(RestError::Forbidden);
            }
        };

        Ok(entities::OpportunityCreateSvm {
            core_fields,
            router,
            permission_account,
            program: program_opportunity,
            // TODO* extract latest slot
            slot: Slot::default(),
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
            .remove_opportunity(&self.db, &opportunity, removal_reason)
            .await
        {
            tracing::error!("Failed to remove opportunity: {:?}", e);
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_quote(&self, input: GetQuoteInput) -> Result<entities::Quote, RestError> {
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
        let input_token = opportunity.buy_tokens[0].clone();
        let output_token = opportunity.sell_tokens[0].clone();
        if input_token.amount == 0 && output_token.amount == 0 {
            tracing::error!(opportunity = ?opportunity, "Both token amounts are zero for swap opportunity");
            return Err(RestError::BadParameters(
                "Both token amounts are zero for swap opportunity".to_string(),
            ));
        }

        // NOTE: This part will be removed after refactoring the permission key type
        let slice: [u8; 65] = opportunity
            .permission_key
            .to_vec()
            .try_into()
            .expect("Failed to convert permission key to slice");
        let permission_key_svm = PermissionKeySvm(slice);
        // Wait to make sure searchers had enough time to submit bids
        sleep(BID_COLLECTION_TIME).await;

        let bid_collection_time = OffsetDateTime::now_utc();
        let mut bids = auction_service
            .get_live_bids(GetLiveBidsInput {
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

        if bids.is_empty() {
            tracing::warn!(opportunity = ?opportunity, "No bids found for quote opportunity");

            // Remove opportunity to prevent further bids
            // The handle auction loop will take care of the bids that were submitted late
            self.remove_quote_opportunity(opportunity.clone()).await;
            return Err(RestError::QuoteNotFound);
        }

        // Find winner bid:
        match input.quote_create.tokens {
            entities::QuoteTokens::InputTokenSpecified { .. } => {
                // highest bid = best (most output token returned)
                bids.sort_by(|a, b| b.amount.cmp(&a.amount));
            }
            entities::QuoteTokens::OutputTokenSpecified { .. } => {
                // lowest bid = best (least input token consumed)
                bids.sort_by(|a, b| a.amount.cmp(&b.amount));
            }
        }
        let winner_bid = bids.first().expect("failed to get first bid");

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

        // Bids is not empty
        let auction = Auction::try_new(bids.clone(), bid_collection_time)
            .expect("Failed to create auction for bids");

        let mut auction = auction_service
            .add_auction(AddAuctionInput { auction })
            .await?;

        let bid = winner_bid.clone();
        let signature = bid.chain_data.transaction.signatures[0];
        auction = auction_service
            .update_submitted_auction(UpdateSubmittedAuctionInput {
                auction,
                transaction_hash: signature,
            })
            .await?;

        join_all(auction.bids.iter().map(|bid| {
            auction_service.update_bid_status(UpdateBidStatusInput {
                new_status: AuctionService::get_new_status(
                    bid,
                    &vec![winner_bid.clone()],
                    BidStatusAuction {
                        tx_hash: signature,
                        id:      auction.id,
                    },
                ),
                bid:        bid.clone(),
            })
        }))
        .await;

        // Remove opportunity to prevent further bids
        // The handle auction loop will take care of the bids that were submitted late
        self.remove_quote_opportunity(opportunity.clone()).await;

        // we check the winner bid status here to make sure the winner bid was successfully entered into the db as submitted
        // this is because: if the winner bid was not successfully entered as submitted, that could indicate the presence of
        // duplicate auctions for the same quote. in such a scenario, one auction will conclude first and update the bid status
        // of its winner bid, and we want to ensure that a winner bid whose status is already updated is not further updated
        // to prevent a new status update from being broadcast
        let live_bids = auction_service
            .get_live_bids(GetLiveBidsInput {
                permission_key: permission_key_svm,
            })
            .await;
        if !live_bids
            .iter()
            .any(|bid| bid.id == winner_bid.id && bid.status.is_submitted())
        {
            tracing::error!(winner_bid = ?winner_bid, opportunity = ?opportunity, "Failed to update winner bid status");
            return Err(RestError::TemporarilyUnavailable);
        }

        Ok(entities::Quote {
            transaction: bid.chain_data.transaction.clone(),
            expiration_time: deadline,
            input_token,
            output_token, // TODO*: incorporate fees (when fees are in the output token)
            chain_id: input.quote_create.chain_id,
        })
    }
}
