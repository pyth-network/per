use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        auction::{
            add_relayer_signature_svm,
            broadcast_lost_bids,
            broadcast_submitted_bids,
            extract_submit_bid_data,
            verify_submit_bid_instruction_svm,
            ChainStore,
        },
        opportunity::{
            entities,
            service::{
                add_opportunity::AddOpportunityInput,
                estimate_price::EstimatePriceInput,
            },
        },
        state::{
            BidStatusSvm,
            BidStatusTrait,
            ChainStoreSvm,
        },
    },
    axum_prometheus::metrics,
    futures::TryFutureExt,
    rand::Rng,
    solana_sdk::{
        clock::Slot,
        commitment_config::{
            CommitmentConfig,
            CommitmentLevel,
        },
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
        chain_store: &ChainStoreSvm,
    ) -> Result<entities::OpportunityCreateSvm, RestError> {
        let chain_config = self.get_config(&quote_create.chain_id)?;
        let router = chain_config.wallet_program_router_account;
        let permission_account = Pubkey::new_from_array(rand::thread_rng().gen());

        let core_fields = entities::OpportunityCoreFieldsCreate {
            permission_key: [router.to_bytes(), permission_account.to_bytes()]
                .concat()
                .into(),
            chain_id:       quote_create.chain_id,
            sell_tokens:    vec![quote_create.input_token],
            buy_tokens:     vec![entities::TokenAmountSvm {
                token:  quote_create.output_mint_token,
                amount: output_amount,
            }],
        };

        // TODO use some in memory caching for this part
        let (block_hash, _) = chain_store
            .client
            .get_latest_blockhash_with_commitment(CommitmentConfig {
                commitment: CommitmentLevel::Finalized,
            })
            .map_err(|e| {
                tracing::error!("Failed to get latest block hash: {:?}", e);
                RestError::TemporarilyUnavailable
            })
            .await?;

        Ok(entities::OpportunityCreateSvm {
            core_fields,
            router,
            permission_account,
            block_hash,
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
        let chain_store = self
            .store
            .chains_svm
            .get(&input.quote_create.chain_id)
            .ok_or(RestError::InvalidChainId)?;
        // TODO Check for the input amount
        tracing::info!(quote_create = ?input.quote_create, "Received request to get quote");
        let output_amount = self
            .estimate_price(EstimatePriceInput {
                quote_create: input.quote_create.clone(),
            })
            .await?;

        let opportunity_create = self
            .get_opportunity_create_for_quote(
                input.quote_create.clone(),
                output_amount,
                chain_store.as_ref(),
            )
            .await?;
        let opportunity = self
            .add_opportunity(AddOpportunityInput {
                opportunity: opportunity_create,
            })
            .await?;

        // Wait to make sure searchers had enough time to submit bids
        sleep(BID_COLLECTION_TIME).await;

        let bid_collection_time = OffsetDateTime::now_utc();
        let mut bids = chain_store.get_bids(&opportunity.permission_key).await;

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
        bids.sort_by(|a, b| b.bid_amount.cmp(&a.bid_amount));
        let winner_bid = bids.first().expect("failed to get first bid");

        // Find the submit bid instruction from bid transaction to extract the deadline
        let submit_bid_instruction = verify_submit_bid_instruction_svm(
            &chain_store.config.express_relay_program_id,
            winner_bid.transaction.clone(),
        )
        .map_err(|e| {
            tracing::error!("Failed to verify submit bid instruction: {:?}", e);
            RestError::TemporarilyUnavailable
        })?;
        let submit_bid_data = extract_submit_bid_data(&submit_bid_instruction).map_err(|e| {
            tracing::error!("Failed to extract submit bid data: {:?}", e);
            RestError::TemporarilyUnavailable
        })?;

        let mut auction = self
            .store
            .init_auction::<ChainStoreSvm>(
                opportunity.permission_key.clone(),
                opportunity.chain_id.clone(),
                bid_collection_time,
            )
            .map_err(|e| {
                tracing::error!("Failed to init auction: {:?}", e);
                RestError::TemporarilyUnavailable
            })
            .await?;

        let relayer = chain_store.express_relay_svm.relayer.clone();
        let mut bid = winner_bid.clone();
        add_relayer_signature_svm(relayer, &mut bid);

        let signature = bid.transaction.signatures[0];
        let converted_tx_hash = BidStatusSvm::convert_tx_hash(&signature);
        auction = self
            .store
            .submit_auction(chain_store.as_ref(), auction, converted_tx_hash)
            .map_err(|e| {
                tracing::error!("Failed to submit auction: {:?}", e);
                RestError::TemporarilyUnavailable
            })
            .await?;

        self.store.task_tracker.spawn({
            let (store, repo, db, winner_bid, bids) = (
                self.store.clone(),
                self.repo.clone(),
                self.db.clone(),
                winner_bid.clone(),
                bids.clone(),
            );
            let chain_store = chain_store.clone();
            async move {
                tokio::join!(
                    broadcast_submitted_bids(
                        store.clone(),
                        chain_store.as_ref(),
                        vec![winner_bid.clone()],
                        signature,
                        auction.clone()
                    ),
                    broadcast_lost_bids(
                        store.clone(),
                        chain_store.as_ref(),
                        bids,
                        vec![winner_bid],
                        Some(signature),
                        Some(&auction)
                    ),
                );
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
                    .remove_opportunity(&db, &opportunity, removal_reason.into())
                    .await
                {
                    tracing::error!("Failed to remove opportunity: {:?}", e);
                }
            }
        });

        Ok(entities::Quote {
            transaction:                 bid.transaction.clone(),
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
