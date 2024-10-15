use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::{
            ws::UpdateEvent::NewOpportunity,
            RestError,
        },
        auction::{
            extract_submit_bid_data,
            verify_submit_bid_instruction_svm,
            ChainStore,
        },
        opportunity::{
            entities,
            service::estimate_price::EstimatePriceInput,
        },
        state::{
            ChainStoreSvm,
            UnixTimestampMicros,
        },
    },
    axum_prometheus::metrics,
    solana_sdk::{
        clock::Slot,
        hash::Hash,
        pubkey::Pubkey,
    },
    time::OffsetDateTime,
    tokio::time::sleep,
    tracing::instrument,
    uuid::Uuid,
};

pub struct GetQuoteInput {
    pub quote_create: entities::QuoteCreate,
}

impl Service<ChainTypeSvm> {
    fn get_opportunity_for_quote(
        &self,
        quote_create: entities::QuoteCreate,
        output_amount: u64,
    ) -> Result<entities::OpportunitySvm, RestError> {
        let chain_config = self.get_config(&quote_create.chain_id)?;
        let router = chain_config.phantom_router_account;
        let permission_account = Pubkey::new_unique();
        let odt = OffsetDateTime::now_utc();

        let core_fields = entities::OpportunityCoreFields {
            id:             Uuid::new_v4(),
            permission_key: [router.to_bytes(), permission_account.to_bytes()]
                .concat()
                .into(),
            chain_id:       quote_create.chain_id,
            sell_tokens:    vec![quote_create.input_token.into()],
            buy_tokens:     vec![entities::TokenAmountSvm {
                token:  quote_create.output_mint_token,
                amount: output_amount,
            }],
            creation_time:  odt.unix_timestamp_nanos() / 1000 as UnixTimestampMicros,
        };

        Ok(entities::OpportunitySvm {
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

    #[instrument(
        target = "metrics",
        fields(category = "get_quote", name = "phantom"),
        skip_all
    )]
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

        let opportunity =
            self.get_opportunity_for_quote(input.quote_create.clone(), output_amount)?;
        self.store
            .ws
            .broadcast_sender
            .send(NewOpportunity(opportunity.clone().into()))
            .map_err(|e| {
                tracing::error!(
                    "Failed to send update: {} - opportunity: {:?}",
                    e,
                    opportunity
                );
                RestError::TemporarilyUnavailable
            })?;

        // Wait to make sure searchers had enough time to submit bids
        sleep(ChainStoreSvm::AUCTION_MINIMUM_LIFETIME).await;

        let mut bids = chain_store.get_bids(&opportunity.permission_key).await;

        // Add logs and metrics
        tracing::info!(
            opportunity = ?opportunity,
            bids = ?bids,
            "Bids received for quote opportunity",
        );
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
