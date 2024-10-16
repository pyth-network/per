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
    solana_sdk::{
        clock::Slot,
        hash::Hash,
        pubkey::Pubkey,
    },
    std::time::Duration,
    time::OffsetDateTime,
    tokio::time::sleep,
};

// Time to wait for searchers to submit bids
const BID_COLLECTION_TIME: Duration = Duration::from_millis(500);

pub struct GetQuoteInput {
    pub quote_create: entities::QuoteCreate,
}

impl Service<ChainTypeSvm> {
    fn get_opportunity_create_for_quote(
        &self,
        quote_create: entities::QuoteCreate,
        output_amount: u64,
    ) -> Result<entities::OpportunityCreateSvm, RestError> {
        let chain_config = self.get_config(&quote_create.chain_id)?;
        let router = chain_config.phantom_router_account;
        let permission_account = Pubkey::new_unique();

        let core_fields = entities::OpportunityCoreFieldsCreate {
            permission_key: [router.to_bytes(), permission_account.to_bytes()]
                .concat()
                .into(),
            chain_id:       quote_create.chain_id,
            sell_tokens:    vec![quote_create.input_token.into()],
            buy_tokens:     vec![entities::TokenAmountSvm {
                token:  quote_create.output_mint_token,
                amount: output_amount,
            }],
        };

        Ok(entities::OpportunityCreateSvm {
            core_fields,
            router,
            permission_account,
            // TODO extract latest block hash
            block_hash: Hash::default(),
            program: entities::OpportunitySvmProgram::Phantom(
                entities::OpportunitySvmProgramWallet {
                    user_wallet_address:         quote_create.user_wallet_address,
                    maximum_slippage_percentage: quote_create.maximum_slippage_percentage,
                },
            ),
            // TODO extract latest slog
            slot: Slot::default(),
        })
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_quote(&self, input: GetQuoteInput) -> Result<entities::Quote, RestError> {
        let chain_store = self
            .store
            .chains_svm
            .get(&input.quote_create.chain_id)
            .ok_or(RestError::BadParameters("Chain not found".to_string()))?;
        // TODO Check for the input amount
        tracing::info!(quote_create = ?input.quote_create, "Received request to get quote");
        let output_amount = self
            .estimate_price(EstimatePriceInput {
                quote_create: input.quote_create.clone(),
            })
            .await?;

        let opportunity_create =
            self.get_opportunity_create_for_quote(input.quote_create.clone(), output_amount)?;
        let opportunity = self
            .add_opportunity(AddOpportunityInput {
                opportunity: opportunity_create.clone(),
            })
            .await?;

        // Wait to make sure searchers had enough time to submit bids
        sleep(BID_COLLECTION_TIME).await;

        let bid_collection_time = OffsetDateTime::now_utc();
        let mut bids = chain_store.get_bids(&opportunity.permission_key).await;

        // Add metrics
        let labels = [
            ("chain_id", input.quote_create.chain_id.to_string()),
            ("wallet", "phantom".to_string()),
            ("total_bids", bids.len().to_string()),
        ];
        metrics::counter!("get_quote_total_bids", &labels).increment(1);

        if bids.len() == 0 {
            tracing::warn!(opportunity = ?opportunity, "No bids found for quote opportunity");

            return Err(RestError::QuoteNotFound);
        }

        // Find winner bid: the bid with the highest bid amount
        bids.sort_by(|a, b| b.core_fields.bid_amount.cmp(&a.core_fields.bid_amount));
        let winner_bid = bids.first().expect("failed to get first bid");

        // Find the submit bid instruction from bid transaction to extract the deadline
        let submit_bid_instruction =
            verify_submit_bid_instruction_svm(chain_store, winner_bid.transaction.clone())
                .map_err(|e| {
                    tracing::error!("Failed to verify submit bid instruction: {:?}", e);
                    RestError::TemporarilyUnavailable
                })?;
        let submit_bid_data = extract_submit_bid_data(submit_bid_instruction).map_err(|e| {
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

        let signature = bid.transaction.signatures[0].clone();
        let converted_tx_hash = BidStatusSvm::convert_tx_hash(&signature);
        auction = self
            .store
            .submit_auction(chain_store, auction, converted_tx_hash)
            .map_err(|e| {
                tracing::error!("Failed to submit auction: {:?}", e);
                RestError::TemporarilyUnavailable
            })
            .await?;

        self.store.task_tracker.spawn({
            let (store, chain_id, winner_bid, bids) = (
                self.store.clone(),
                input.quote_create.chain_id.clone(),
                winner_bid.clone(),
                bids.clone(),
            );
            async move {
                if let Some(chain_store) = store.chains_svm.get(&chain_id) {
                    tokio::join!(
                        broadcast_submitted_bids(
                            store.clone(),
                            chain_store,
                            vec![winner_bid.clone()],
                            signature,
                            auction.clone()
                        ),
                        broadcast_lost_bids(
                            store.clone(),
                            chain_store,
                            bids,
                            vec![winner_bid],
                            Some(signature),
                            Some(&auction)
                        ),
                    );
                }
            }
        });

        Ok(entities::Quote {
            transaction:                 winner_bid.transaction.clone(),
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
