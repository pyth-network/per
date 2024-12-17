use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        auction::{
            entities::{
                Auction,
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
        kernel::entities::{
            PermissionKeySvm,
            Svm,
        },
        opportunity::{
            entities,
            service::add_opportunity::AddOpportunityInput,
        },
    },
    axum_prometheus::metrics,
    futures::future::join_all,
    rand::Rng,
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
}

impl Service<ChainTypeSvm> {
    async fn get_opportunity_create_for_quote(
        &self,
        quote_create: entities::QuoteCreate,
    ) -> Result<entities::OpportunityCreateSvm, RestError> {
        let chain_config = self.get_config(&quote_create.chain_id)?;
        let router = chain_config.wallet_program_router_account;
        let permission_account = Pubkey::new_from_array(rand::thread_rng().gen());

        // TODO: we should fix the Opportunity struct (or create a new format) to more clearly distinguish Swap opps from traditional opps
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
                router,
                permission_account,
            ),
            chain_id:       quote_create.chain_id,
            sell_tokens:    vec![entities::TokenAmountSvm {
                token:  input_mint,
                amount: input_amount,
            }],
            buy_tokens:     vec![entities::TokenAmountSvm {
                token:  output_mint,
                amount: output_amount,
            }],
        };

        Ok(entities::OpportunityCreateSvm {
            core_fields,
            router,
            permission_account,
            program: entities::OpportunitySvmProgram::Swap(entities::OpportunitySvmProgramWallet {
                user_wallet_address:         quote_create.user_wallet_address,
                maximum_slippage_percentage: quote_create.maximum_slippage_percentage,
            }),
            // TODO extract latest slot
            slot: Slot::default(),
        })
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_quote(&self, input: GetQuoteInput) -> Result<entities::Quote, RestError> {
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
        let input_specified = matches!(
            input.quote_create.tokens,
            entities::QuoteTokens::InputTokenSpecified { .. }
        );
        let input_token = opportunity.buy_tokens[0].clone();
        let output_token = opportunity.sell_tokens[0].clone();

        // NOTE: This part will be removed after refactoring the permission key type
        let slice: [u8; 64] = opportunity
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
                permission_key: permission_key_svm,
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
            ("wallet", "phantom".to_string()), // TODO: deduce the originator based on the router account
            ("total_bids", total_bids),
        ];
        metrics::counter!("get_quote_total_bids", &labels).increment(1);

        if bids.is_empty() {
            tracing::warn!(opportunity = ?opportunity, "No bids found for quote opportunity");

            return Err(RestError::QuoteNotFound);
        }

        // Find winner bid:
        if input_specified {
            // highest bid = best (most output token returned)
            bids.sort_by(|a, b| b.amount.cmp(&a.amount));
        } else {
            // lowest bid = best (least input token consumed)
            bids.sort_by(|a, b| a.amount.cmp(&b.amount));
        }
        let winner_bid = bids.first().expect("failed to get first bid");

        // // TODO: uncomment this once Swap instruction is implemented
        // // Find the swap instruction from bid transaction to extract the deadline
        // let swap_instruction = auction_service
        //     .verify_swap_instruction(winner_bid.chain_data.transaction.clone())
        //     .map_err(|e| {
        //         tracing::error!("Failed to verify swap instruction: {:?}", e);
        //         RestError::TemporarilyUnavailable
        //     })?;
        // let swap_data = AuctionService::<Svm>::extract_swap_data(
        //     &swap_instruction,
        // )
        // .map_err(|e| {
        //     tracing::error!("Failed to extract swap data: {:?}", e);
        //     RestError::TemporarilyUnavailable
        // })?;
        let deadline = i64::MAX;

        // Bids is not empty
        let auction = Auction::try_new(bids.clone(), bid_collection_time)
            .expect("Failed to create auction for bids");

        let mut auction = auction_service
            .add_auction(AddAuctionInput { auction })
            .await?;

        let mut bid = winner_bid.clone();
        auction_service.add_relayer_signature(&mut bid);

        let signature = bid.chain_data.transaction.signatures[0];
        auction = auction_service
            .update_submitted_auction(UpdateSubmittedAuctionInput {
                auction,
                transaction_hash: signature,
            })
            .await?;

        self.task_tracker.spawn({
            let (repo, db, winner_bid) = (self.repo.clone(), self.db.clone(), winner_bid.clone());
            let auction_service = auction_service.clone();
            async move {
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

                // TODO
                // Maybe we should add state for opportunity.
                // Right now logic for removing halted/expired bids, checks if opportunity exists.
                // We should remove opportunity only after the auction bid result is broadcasted.
                // This is to make sure we are not gonna remove the bids that are currently in the auction in the handle_auction loop.
                let removal_reason =
                    entities::OpportunityRemovalReason::Invalid(RestError::InvalidOpportunity(
                        "Auction finished for the opportunity".to_string(),
                    ));
                if let Err(e) = repo
                    .remove_opportunity(&db, &opportunity, removal_reason)
                    .await
                {
                    tracing::error!("Failed to remove opportunity: {:?}", e);
                }
            }
        });

        Ok(entities::Quote {
            transaction: bid.chain_data.transaction.clone(),
            expiration_time: deadline,
            input_token,
            output_token,
            maximum_slippage_percentage: input.quote_create.maximum_slippage_percentage,
            chain_id: input.quote_create.chain_id,
        })
    }
}
