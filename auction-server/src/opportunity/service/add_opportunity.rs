use {
    super::{
        verification::Verification,
        ChainType,
        Service,
    },
    crate::{
        api::{
            ws::UpdateEvent::NewOpportunity,
            RestError,
        },
        opportunity::{
            entities::{
                self,
                Opportunity,
                OpportunityCreate,
            },
            repository::InMemoryStore,
            service::verification::VerifyOpportunityInput,
        },
    },
};

pub struct AddOpportunityInput<T: entities::OpportunityCreate> {
    pub opportunity: T,
}

type OpportunityType<T> = <<T as ChainType>::InMemoryStore as InMemoryStore>::Opportunity;
type OpportunityCreateType<T> = <OpportunityType<T> as entities::Opportunity>::OpportunityCreate;

#[derive(Debug, Clone)]
enum OpportunityAction<T: entities::Opportunity> {
    Add,
    Refresh(T),
    Ignore,
}

impl<T: ChainType> Service<T>
where
    Service<T>: Verification<T>,
{
    async fn assess_action(
        &self,
        opportunity: &OpportunityCreateType<T>,
    ) -> OpportunityAction<OpportunityType<T>> {
        let opportunities = self
            .repo
            .get_in_memory_opportunities_by_key(&opportunity.get_key())
            .await;
        for opp in opportunities.into_iter() {
            let comparison = opp.compare(opportunity);
            if let entities::OpportunityComparison::Duplicate = comparison {
                return OpportunityAction::Ignore;
            }
            if let entities::OpportunityComparison::NeedsRefresh = comparison {
                return OpportunityAction::Refresh(opp);
            }
        }
        OpportunityAction::Add
    }
    pub async fn add_opportunity(
        &self,
        input: AddOpportunityInput<OpportunityCreateType<T>>,
    ) -> Result<<T::InMemoryStore as InMemoryStore>::Opportunity, RestError> {
        let opportunity_create = input.opportunity;
        let action = self.assess_action(&opportunity_create).await;
        if let OpportunityAction::Ignore = action {
            tracing::info!("Submitted opportunity ignored: {:?}", opportunity_create);
            return Err(RestError::DuplicateOpportunity);
        }

        self.verify_opportunity(VerifyOpportunityInput {
            opportunity: opportunity_create.clone(),
        })
        .await
        .map_err(|e| {
            tracing::warn!(
                "Failed to verify opportunity: {:?} - opportunity: {:?}",
                e,
                opportunity_create,
            );
            e
        })?;

        let opportunity = if let OpportunityAction::Refresh(opp) = action {
            self.repo.refresh_in_memory_opportunity(opp.clone()).await
        } else {
            self.repo
                .add_opportunity(opportunity_create.clone())
                .await?
        };

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

        let opportunities_map = &self.repo.get_in_memory_opportunities().await;
        tracing::debug!("number of permission keys: {}", opportunities_map.len());
        tracing::debug!(
            "number of opportunities for key: {}",
            opportunities_map
                .get(&opportunity.get_key())
                .map_or(0, |opps| opps.len())
        );

        Ok(opportunity)
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::{
            api::ws,
            kernel::traced_sender_svm::tests::MockRpcClient,
            opportunity::{
                entities::{
                    OpportunityCoreFieldsCreate,
                    OpportunityCreateSvm,
                    OpportunityKey,
                    OpportunitySvmProgram,
                    OpportunitySvmProgramLimo,
                    TokenAmountSvm,
                },
                repository::MockDatabase,
                service::{
                    add_opportunity::AddOpportunityInput,
                    ChainTypeSvm,
                    Service,
                },
            },
        },
        ethers::{
            types::Bytes,
            utils::hex::FromHex,
        },
        solana_sdk::pubkey::Pubkey,
    };

    #[tokio::test]
    async fn test_add_opportunity() {
        let chain_id = "solana".to_string();
        let rpc_client = MockRpcClient::default();
        let mut mock_db = MockDatabase::default();

        mock_db.expect_add_opportunity().returning(|_| Ok(()));

        let (service, mut ws_receiver) =
            Service::<ChainTypeSvm>::new_with_mocks_svm(chain_id.clone(), mock_db, rpc_client);

        let permission_account = Pubkey::new_unique();
        let router = Pubkey::new_unique();

        let sell_token = Pubkey::new_unique();
        let sell_amount = 2;
        let buy_token = Pubkey::new_unique();
        let buy_amount = 1;

        let permission_key = Bytes::from_hex("0xdeadbeef").unwrap();
        let slot = 3;

        let order_address = Pubkey::new_unique();
        let order = vec![1, 2, 3, 4];

        let opportunity_create = OpportunityCreateSvm {
            core_fields: OpportunityCoreFieldsCreate::<TokenAmountSvm> {
                permission_key: permission_key.clone(),
                chain_id:       chain_id.clone(),
                sell_tokens:    vec![TokenAmountSvm {
                    token:  sell_token,
                    amount: sell_amount,
                }],
                buy_tokens:     vec![TokenAmountSvm {
                    token:  buy_token,
                    amount: buy_amount,
                }],
            },
            router,
            permission_account,
            program: OpportunitySvmProgram::Limo(OpportunitySvmProgramLimo {
                order: order.clone(),
                order_address,
                slot,
            }),
        };

        let opportunity = service
            .add_opportunity(AddOpportunityInput {
                opportunity: opportunity_create.clone(),
            })
            .await
            .unwrap();
        assert!(opportunity.core_fields.creation_time <= opportunity.core_fields.refresh_time);
        assert_eq!(
            opportunity.core_fields.permission_key,
            opportunity_create.core_fields.permission_key
        );
        assert_eq!(
            opportunity.core_fields.chain_id,
            opportunity_create.core_fields.chain_id
        );
        assert_eq!(
            opportunity.core_fields.sell_tokens,
            opportunity_create.core_fields.sell_tokens
        );
        assert_eq!(
            opportunity.core_fields.buy_tokens,
            opportunity_create.core_fields.buy_tokens
        );
        assert_eq!(opportunity.router, router);
        assert_eq!(opportunity.permission_account, permission_account);
        assert_eq!(
            opportunity.program,
            OpportunitySvmProgram::Limo(OpportunitySvmProgramLimo {
                order: order.clone(),
                order_address,
                slot,
            })
        );

        let opportunities = service.repo.get_in_memory_opportunities().await;
        let opportunities_by_key = service
            .repo
            .get_in_memory_opportunities_by_key(&OpportunityKey(
                chain_id.clone(),
                permission_key.clone(),
            ))
            .await;

        assert_eq!(opportunities.len(), 1);
        assert_eq!(
            opportunities
                .get(&OpportunityKey(chain_id.clone(), permission_key.clone()))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(opportunities_by_key.len(), 1);
        assert_eq!(
            opportunities_by_key[0],
            opportunities
                .get(&OpportunityKey(chain_id, permission_key))
                .unwrap()[0]
        );
        assert_eq!(opportunities_by_key[0], opportunity);

        let event = ws_receiver.try_recv().unwrap();
        assert_eq!(
            event,
            ws::UpdateEvent::NewOpportunity(opportunity.clone().into())
        );
        assert!(ws_receiver.is_empty());
    }
}
