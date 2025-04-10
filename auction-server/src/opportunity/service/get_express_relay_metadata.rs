use {
    super::Service,
    crate::{
        api::RestError,
        kernel::entities::ChainId,
        opportunity::service::ConfigSvm,
    },
    ::express_relay as express_relay_svm,
    anchor_lang::AccountDeserialize,
    express_relay::state::ExpressRelayMetadata,
    solana_sdk::{
        account::Account,
        pubkey::Pubkey,
    },
};

pub struct GetExpressRelayMetadata {
    pub chain_id: ChainId,
}

// TODO: Move this to kernel
impl Service {
    /// Fetches the express relay program metadata for fee calculations
    /// Uses one hour cache to avoid repeated RPC calls
    pub async fn get_express_relay_metadata(
        &self,
        input: GetExpressRelayMetadata,
    ) -> Result<ExpressRelayMetadata, RestError> {
        let token_program_query = self.repo.query_express_relay_metadata().await;
        let metadata = match token_program_query {
            Some(metadata) => metadata,
            None => {
                let metadata_account = self
                    .fetch_express_relay_metadata_account(&input.chain_id)
                    .await?;
                let metadata = ExpressRelayMetadata::try_deserialize(&mut metadata_account.data.as_slice())
                    .map_err(|err| {
                        tracing::error!(error = ?err,"Failed to deserialize express relay metadata account");
                        RestError::TemporarilyUnavailable
                    })?;
                self.repo
                    .cache_express_relay_metadata(metadata.clone())
                    .await;
                metadata
            }
        };

        Ok(metadata)
    }

    async fn fetch_express_relay_metadata_account(
        &self,
        chain_id: &ChainId,
    ) -> Result<Account, RestError> {
        let config = self.get_config(chain_id)?;
        let metadata_address = Self::calculate_metadata_address(config);
        let metadata_account = config
            .rpc_client
            .get_account(&metadata_address)
            .await
            .map_err(|err| {
                tracing::error!(error=?err,
                    "Failed to retrieve express relay metadata account from rpc"
                );
                RestError::TemporarilyUnavailable
            })?;
        Ok(metadata_account)
    }

    pub fn calculate_metadata_address(config: &ConfigSvm) -> Pubkey {
        let program_id = config
            .auction_service_container
            .get_service()
            .get_express_relay_program_id();
        let seed = express_relay_svm::state::SEED_METADATA;
        Pubkey::find_program_address(&[seed], &program_id).0
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            auction::service::StatefulMockAuctionService,
            kernel::{
                rpc_client_svm_tester::RpcClientSvmTester,
                test_utils::DEFAULT_CHAIN_ID,
            },
            opportunity::repository::MockDatabase,
        },
        solana_sdk::pubkey::Pubkey,
    };

    #[tokio::test]
    async fn test_hit_cached_metadata() {
        let rpc_client = RpcClientSvmTester::new();
        let mock_db = MockDatabase::default();

        let (service, _) =
            Service::new_with_mocks_svm(DEFAULT_CHAIN_ID.to_string(), mock_db, &rpc_client);

        let params = ExpressRelayMetadata {
            admin:                    Pubkey::new_unique(),
            relayer_signer:           Pubkey::new_unique(),
            fee_receiver_relayer:     Pubkey::new_unique(),
            split_router_default:     0,
            split_relayer:            0,
            swap_platform_fee_bps:    0,
            secondary_relayer_signer: Pubkey::new_unique(),
        };
        service
            .repo
            .cache_express_relay_metadata(params.clone())
            .await;

        let params_received = service
            .get_express_relay_metadata(GetExpressRelayMetadata {
                chain_id: "doesntactuallymatter".to_string(),
            })
            .await
            .expect("Failed to get express relay metadata");

        assert_eq!(params_received.admin, params.admin);
        assert_eq!(params_received.relayer_signer, params.relayer_signer);
        assert_eq!(
            params_received.fee_receiver_relayer,
            params.fee_receiver_relayer
        );
    }

    #[tokio::test]
    async fn test_metadata_rpc_fetch() {
        let rpc_client = RpcClientSvmTester::new();
        let mock_db = MockDatabase::default();

        let (service, _) =
            Service::new_with_mocks_svm(DEFAULT_CHAIN_ID.to_string(), mock_db, &rpc_client);

        let admin_pubkey = Pubkey::new_unique();
        rpc_client
            .can_next_account_as_metadata(ExpressRelayMetadata {
                admin:                    admin_pubkey,
                relayer_signer:           Pubkey::new_unique(),
                fee_receiver_relayer:     Pubkey::new_unique(),
                split_router_default:     0,
                split_relayer:            0,
                swap_platform_fee_bps:    0,
                secondary_relayer_signer: Pubkey::new_unique(),
            })
            .await;

        let mut auction_service_in_call = StatefulMockAuctionService::default();
        auction_service_in_call
            .expect_get_express_relay_program_id()
            .returning(|| {
                // relay program id
                express_relay::id()
            });

        let auction_service = crate::auction::service::MockService::new(auction_service_in_call);
        let config = service
            .get_config(&(DEFAULT_CHAIN_ID.to_string()))
            .expect("Failed to get opportunity service evm config");
        config
            .auction_service_container
            .inject_mock_service(auction_service);

        let params_received = service
            .get_express_relay_metadata(GetExpressRelayMetadata {
                chain_id: DEFAULT_CHAIN_ID.to_string(),
            })
            .await
            .expect("Failed to get express relay metadata in test");

        assert_eq!(params_received.admin, admin_pubkey);
        rpc_client.check_all_uncanned().await;
    }
}
