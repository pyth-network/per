use {
    super::{
        ChainTrait,
        Repository,
    },
    crate::{
        api::RestError,
        auction::entities,
        models::ProfileId,
    },
    time::OffsetDateTime,
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_bids(
        &self,
        profile_id: ProfileId,
        from_time: Option<OffsetDateTime>,
    ) -> Result<Vec<entities::Bid<T>>, RestError> {
        let bids = self
            .db
            .get_bids(self.chain_id.clone(), profile_id, from_time)
            .await?;
        let auctions = self.db.get_auctions_by_bids(&bids).await?;

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
