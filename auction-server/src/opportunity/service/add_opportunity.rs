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
type OpportunityCreateType<T> =
    <OpportunityType<T> as entities::Opportunity>::OpportunityCreateAssociatedType;

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
            return Err(RestError::BadParameters(
                "Same opportunity is submitted recently".to_string(),
            ));
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

        println!("action: {:?}", action);

        let opportunity = if let OpportunityAction::Refresh(opp) = action {
            self.repo.refresh_in_memory_opportunity(opp.clone()).await
        } else {
            self.repo
                .add_opportunity(&self.db, opportunity_create.clone())
                .await?
        };

        self.store
            .ws
            .broadcast_sender
            .send(NewOpportunity(opportunity.clone().into()))
            .map_err(|e| {
                println!(
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
mod test {
    use std::{collections::HashMap, sync::{atomic::AtomicUsize, Arc}};

    use ethers::types::Bytes;
    use time::OffsetDateTime;
    use tokio::sync::RwLock;
    use uuid::Uuid;

    use super::*;
    use crate::{api::ws::WsState, kernel::db::DB, opportunity::{entities::{MockOpportunity, OpportunityCoreFields}, repository::{self, test::MockInMemoryStore, InMemoryStoreCoreFields, MockOpportunityMetadata}, service::MockChainType}, server::setup_metrics_recorder, state::Store};

    #[tokio::test]
    async fn test_add_opportunity() {
        let (broadcast_sender, broadcast_receiver) = tokio::sync::broadcast::channel(100);

        let db = DB::connect_lazy("https://mock_url").unwrap();

        let store = Arc::new(Store {
            db:               db.clone(),
            chains_evm:       HashMap::new(),
            chains_svm:       HashMap::new(),
            ws:               WsState {
                subscriber_counter: AtomicUsize::new(0),
                broadcast_sender,
                broadcast_receiver,
            },
            secret_key:       "mock_secret_key".to_string(),
            access_tokens:    RwLock::new(HashMap::new()),
            metrics_recorder: setup_metrics_recorder().unwrap(),
        });
        let config = HashMap::new();

        let mut in_memory_store = MockInMemoryStore::default();
        in_memory_store.expect_deref().return_const(InMemoryStoreCoreFields::new());

        let opportunity_context = MockOpportunity::new_with_current_time_context();
        opportunity_context.expect().returning(|_| {
            let mut opportunity = MockOpportunity::default();
            opportunity.expect_get_models_metadata().return_const(MockOpportunityMetadata::default());
            opportunity.expect_deref().return_const(OpportunityCoreFields {
                id: Uuid::new_v4(),
                permission_key: Bytes::default(),
                chain_id: "".to_string(),
                sell_tokens: vec![],
                buy_tokens: vec![],
                creation_time: OffsetDateTime::now_utc(),
                refresh_time: OffsetDateTime::now_utc(),
            });
            opportunity
        });

        let service = Service::<MockChainType> {
            store,
            db,
            repo: Arc::new(repository::Repository { in_memory_store: in_memory_store }),
            config,
        };

        let input = AddOpportunityInput {
            opportunity: entities::test::MockOpportunityCreate {},
        };
        service.add_opportunity(input).await.unwrap();
    }
}
