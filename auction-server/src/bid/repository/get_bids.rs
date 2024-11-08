use {
    super::{
        models::{
            self,
        },
        BidTrait,
        InMemoryStore,
        Repository,
    },
    crate::{
        api::RestError,
        bid::entities,
        kernel::entities::{
            ChainId,
            PermissionKey,
        },
        models::ProfileId,
    },
    sqlx::QueryBuilder,
    time::OffsetDateTime,
};

impl<T: BidTrait> Repository<T> {
    async fn get_auctions_by_bids_model(
        &self,
        bids: &[models::Bid<T>],
    ) -> Result<Vec<models::Auction>, RestError> {
        let auction_ids: Vec<entities::AuctionId> =
            bids.iter().filter_map(|bid| bid.auction_id).collect();
        sqlx::query_as("SELECT * FROM auction WHERE id = ANY($1)")
            .bind(auction_ids)
            .fetch_all(&self.db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to fetch auctions: {}", e);
                RestError::TemporarilyUnavailable
            })
    }

    async fn get_bids_model(
        &self,
        chain_id: ChainId,
        profile_id: ProfileId,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<models::Bid<T>>, RestError> {
        let mut query =
            QueryBuilder::new("SELECT * from bid where profile_id = ? AND chain_id = ?");
        query.push_bind(profile_id).push_bind(chain_id);
        if let Some(from_time) = from_time {
            query.push(" AND initiation_time >= ");
            query.push_bind(from_time);
        }
        query.push(" ORDER BY initiation_time ASC LIMIT 20");
        query
            .build_query_as()
            .fetch_all(&self.db)
            .await
            .map_err(|e| {
                tracing::error!("DB: Failed to fetch bids: {}", e);
                RestError::TemporarilyUnavailable
            })
    }

    pub async fn get_bids(
        &self,
        profile_id: ProfileId,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<entities::Bid<T>>, RestError> {
        let bids = self
            .get_bids_model(self.chain_id.clone(), profile_id, from_time)
            .await?;
        let auctions = self.get_auctions_by_bids_model(&bids).await?;

        Ok(bids
            .into_iter()
            .filter_map(|b| {
                let auction = match b.auction_id {
                    Some(auction_id) => auctions.clone().into_iter().find(|a| a.id == auction_id),
                    None => None,
                };
                b.get_bid_entity(auction.clone())
                    .map_err(|e| {
                        tracing::error!(
                            error = e.to_string(),
                            auction = ?auction,
                            bid = ?b,
                            "Failed to convert bid to entity"
                        );
                    })
                    .ok()
            })
            .collect())
    }
}
