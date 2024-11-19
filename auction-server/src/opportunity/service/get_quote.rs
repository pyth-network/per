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
            service::{
                add_opportunity::AddOpportunityInput,
                estimate_price::EstimatePriceInput,
            },
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
        output_amount: u64,
    ) -> Result<entities::OpportunityCreateSvm, RestError> {
        let chain_config = self.get_config(&quote_create.chain_id)?;
        let router = chain_config.wallet_program_router_account;
        let permission_account = Pubkey::new_from_array(rand::thread_rng().gen());

        let core_fields = entities::OpportunityCoreFieldsCreate {
            permission_key: entities::OpportunitySvm::get_permission_key(
                router,
                permission_account,
            ),
            chain_id:       quote_create.chain_id,
            sell_tokens:    vec![quote_create.input_token],
            buy_tokens:     vec![entities::TokenAmountSvm {
                token:  quote_create.output_mint_token,
                amount: output_amount,
            }],
        };

        Ok(entities::OpportunityCreateSvm {
            core_fields,
            router,
            permission_account,
            program: entities::OpportunitySvmProgram::Phantom(
                entities::OpportunitySvmProgramWallet {
                    user_wallet_address:         quote_create.user_wallet_address,
                    maximum_slippage_percentage: quote_create.maximum_slippage_percentage,
                },
            ),
            // TODO extract latest slot
            slot: Slot::default(),
        })
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_quote(&self, input: GetQuoteInput) -> Result<entities::Quote, RestError> {
        let config = self.get_config(&input.quote_create.chain_id)?;
        let auction_service = config
            .auction_service
            .as_ref()
            .expect("Failed to get auction service");

        // TODO Check for the input amount
        tracing::info!(quote_create = ?input.quote_create, "Received request to get quote");
        let output_amount = self
            .estimate_price(EstimatePriceInput {
                quote_create: input.quote_create.clone(),
            })
            .await?;

        let opportunity_create = self
            .get_opportunity_create_for_quote(input.quote_create.clone(), output_amount)
            .await?;
        let opportunity = self
            .add_opportunity(AddOpportunityInput {
                opportunity: opportunity_create,
            })
            .await?;

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
            ("wallet", "phantom".to_string()),
            ("total_bids", total_bids),
        ];
        metrics::counter!("get_quote_total_bids", &labels).increment(1);

        if bids.is_empty() {
            tracing::warn!(opportunity = ?opportunity, "No bids found for quote opportunity");

            return Err(RestError::QuoteNotFound);
        }

        // Find winner bid: the bid with the highest bid amount
        bids.sort_by(|a, b| b.amount.cmp(&a.amount));
        let winner_bid = bids.first().expect("failed to get first bid");

        // Find the submit bid instruction from bid transaction to extract the deadline
        let submit_bid_instruction = auction_service
            .verify_submit_bid_instruction(winner_bid.chain_data.transaction.clone())
            .map_err(|e| {
                tracing::error!("Failed to verify submit bid instruction: {:?}", e);
                RestError::TemporarilyUnavailable
            })?;
        let submit_bid_data = AuctionService::<Svm>::extract_submit_bid_data(
            &submit_bid_instruction,
        )
        .map_err(|e| {
            tracing::error!("Failed to extract submit bid data: {:?}", e);
            RestError::TemporarilyUnavailable
        })?;

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
            transaction:                 bid.chain_data.transaction.clone(),
            expiration_time:             submit_bid_data.deadline,
            input_token:                 input.quote_create.input_token,
            output_token:                entities::TokenAmountSvm {
                token:  input.quote_create.output_mint_token,
                amount: output_amount,
            },
            maximum_slippage_percentage: input.quote_create.maximum_slippage_percentage,
            chain_id:                    input.quote_create.chain_id,
        })
    }
}
