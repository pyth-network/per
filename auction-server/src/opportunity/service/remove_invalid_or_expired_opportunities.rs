use {
    super::Service,
    crate::{
        api::ws::UpdateEvent,
        opportunity::entities::{
            self,
        },
    },
    time::{
        Duration,
        OffsetDateTime,
    },
};

const MAX_STALE_OPPORTUNITY_DURATION: Duration = Duration::minutes(2);

impl Service {
    pub async fn remove_invalid_or_expired_opportunities(&self) {
        let all_opportunities = self.repo.get_in_memory_opportunities().await;
        for (_, opportunities) in all_opportunities.iter() {
            // check each of the opportunities for this permission key for validity
            for opportunity in opportunities.iter() {
                if OffsetDateTime::now_utc() - opportunity.refresh_time
                    <= MAX_STALE_OPPORTUNITY_DURATION
                {
                    continue;
                }

                let reason = entities::OpportunityRemovalReason::Expired;
                tracing::info!(
                    opportunity = ?opportunity,
                    reason = ?reason,
                    "Removing Opportunity",
                );

                match self.repo.remove_opportunity(opportunity, reason).await {
                    Ok(()) => {
                        // If there are no more opportunities with this key, it means all of the
                        // opportunities have been removed for this key, so we can broadcast remove opportunities event.
                        if self
                            .repo
                            .get_in_memory_opportunities_by_key(&opportunity.get_key())
                            .await
                            .is_empty()
                        {
                            if let Err(e) = self.store.ws.broadcast_sender.send(
                                UpdateEvent::RemoveOpportunities(
                                    opportunity.get_opportunity_delete(),
                                ),
                            ) {
                                tracing::error!(
                                    error = e.to_string(),
                                    "Failed to broadcast remove opportunity"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = ?e, "Failed to remove opportunity");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use {
        crate::{
            api::ws::UpdateEvent,
            kernel::{
                entities::PermissionKeySvm,
                rpc_client_svm_tester::RpcClientSvmTester,
                test_utils::DEFAULT_CHAIN_ID,
            },
            opportunity::{
                entities::{
                    OpportunitySvm,
                    OpportunitySvmProgram,
                    OpportunitySvmProgramSwap,
                    TokenAmountSvm,
                },
                repository::MockDatabase,
                service::Service,
            },
        },
        express_relay_api_types::opportunity::{
            OpportunityDelete,
            OpportunityDeleteSvm,
            OpportunityDeleteV1Svm,
            ProgramSvm,
        },
        solana_sdk::pubkey::Pubkey,
        time::{
            Duration,
            OffsetDateTime,
        },
        uuid::Uuid,
    };

    fn make_test_opportunity(
        id: u128,
        creation_time: OffsetDateTime,
        refresh_time: OffsetDateTime,
    ) -> OpportunitySvm {
        OpportunitySvm {
            id: Uuid::from_u128(id),
            permission_key: PermissionKeySvm::try_from(&[2; 65][..]).expect("permission key"),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            sell_tokens: vec![TokenAmountSvm {
                token:  Pubkey::new_unique(),
                amount: 2,
            }],
            buy_tokens: vec![TokenAmountSvm {
                token:  Pubkey::new_unique(),
                amount: 1,
            }],
            creation_time,
            refresh_time,
            router: Pubkey::new_unique(),
            permission_account: Pubkey::new_unique(),
            program: OpportunitySvmProgram::Swap(
                OpportunitySvmProgramSwap::default_test_with_user_wallet_address(
                    Pubkey::new_unique(),
                ),
            ),
            profile_id: None,
        }
    }

    async fn push_test_opportunity(service: &Service, test_opportunity: OpportunitySvm) {
        service
            .repo
            .in_memory_store
            .opportunities
            .write()
            .await
            .entry(test_opportunity.get_key())
            .or_insert_with(Vec::new)
            .push(test_opportunity);
    }

    #[tokio::test]
    async fn test_dont_remove_valid_opportunities() {
        let rpc_client = RpcClientSvmTester::new();
        let mut mock_db = MockDatabase::default();

        mock_db.expect_add_opportunity().returning(|_| Ok(()));

        let (service, ws_receiver) =
            Service::new_with_mocks_svm(DEFAULT_CHAIN_ID.to_string(), mock_db, &rpc_client);

        // recent opportunity
        let test_time = OffsetDateTime::now_utc() - Duration::minutes(1);
        let test_opportunity = make_test_opportunity(1, test_time, test_time);

        push_test_opportunity(&service, test_opportunity.clone()).await;

        service.remove_invalid_or_expired_opportunities().await;

        // check opportunities
        let all_opportunities_by_test_key = service
            .repo
            .get_in_memory_opportunities()
            .await
            .remove(&test_opportunity.get_key())
            .expect("opportunity should exist");

        assert_eq!(all_opportunities_by_test_key, vec![test_opportunity]);
        assert!(ws_receiver.is_empty());
    }

    #[tokio::test]
    async fn test_remove_one_expired_opportunity() {
        let rpc_client = RpcClientSvmTester::new();
        let mut mock_db = MockDatabase::default();

        mock_db.expect_add_opportunity().returning(|_| Ok(()));
        mock_db.expect_remove_opportunity().returning(|_, _| Ok(()));

        let (service, ws_receiver) =
            Service::new_with_mocks_svm(DEFAULT_CHAIN_ID.to_string(), mock_db, &rpc_client);

        // recent opportunity
        let time_alive = OffsetDateTime::now_utc() - Duration::minutes(1);
        let time_expired = OffsetDateTime::now_utc() - Duration::minutes(5);
        let live_opportunity = make_test_opportunity(1, time_alive, time_alive);
        let expired_opportunity = make_test_opportunity(2, time_expired, time_expired);

        push_test_opportunity(&service, live_opportunity.clone()).await;
        push_test_opportunity(&service, expired_opportunity.clone()).await;

        service.remove_invalid_or_expired_opportunities().await;

        // check opportunities
        let all_opportunities_by_test_key = service
            .repo
            .get_in_memory_opportunities()
            .await
            .remove(&live_opportunity.get_key())
            .expect("opportunity should exist");

        assert_eq!(all_opportunities_by_test_key, vec![live_opportunity]);
        // not all opportunities under the permission key were removed, we dont broadcast WS message
        assert!(ws_receiver.is_empty());
    }

    #[tokio::test]
    async fn test_remove_all_expired_opportunities_from_key() {
        let rpc_client = RpcClientSvmTester::new();
        let mut mock_db = MockDatabase::default();

        mock_db.expect_add_opportunity().returning(|_| Ok(()));
        mock_db.expect_remove_opportunity().returning(|_, _| Ok(()));

        let (service, mut ws_receiver) =
            Service::new_with_mocks_svm(DEFAULT_CHAIN_ID.to_string(), mock_db, &rpc_client);

        // recent opportunity
        let time_expired = OffsetDateTime::now_utc() - Duration::minutes(5);
        let live_opportunity = make_test_opportunity(1, time_expired, time_expired);
        let expired_opportunity = make_test_opportunity(2, time_expired, time_expired);

        let removed_permission_acc = expired_opportunity.permission_account;
        let removed_router = expired_opportunity.router;

        push_test_opportunity(&service, live_opportunity.clone()).await;
        push_test_opportunity(&service, expired_opportunity.clone()).await;

        service.remove_invalid_or_expired_opportunities().await;

        // check opportunities
        let all_opportunities_by_test_key = service.repo.get_in_memory_opportunities().await;
        assert!(all_opportunities_by_test_key.is_empty());

        let update = ws_receiver
            .try_recv()
            .expect("opportunity removal should be sent as ws update");
        assert_eq!(
            update,
            UpdateEvent::RemoveOpportunities(OpportunityDelete::Svm(OpportunityDeleteSvm::V1(
                OpportunityDeleteV1Svm {
                    permission_account: removed_permission_acc,
                    router:             removed_router,
                    chain_id:           DEFAULT_CHAIN_ID.to_string(),
                    program:            ProgramSvm::Swap,
                }
            )))
        );
    }
}
