use {
    super::{
        models::{
            self,
        },
        Repository,
    },
    crate::{
        api::RestError,
        auction::{
            entities,
            service::ChainTrait,
        },
    },
    tracing::{
        info_span,
        Instrument,
    },
};

impl<T: ChainTrait> Repository<T> {
    pub async fn get_bid(&self, bid_id: entities::BidId) -> Result<entities::Bid<T>, RestError> {
        let bid: models::Bid<T> =
            sqlx::query_as("SELECT * FROM bid WHERE id = $1 AND chain_id = $2")
                .bind(bid_id)
                .bind(self.chain_id.clone())
                .fetch_one(&self.db)
                .instrument(info_span!("db_get_bid"))
                .await
                .map_err(|e| match e {
                    sqlx::Error::RowNotFound => RestError::BidNotFound,
                    _ => {
                        tracing::error!(
                            error = e.to_string(),
                            bid_id = bid_id.to_string(),
                            "Failed to get bid from db"
                        );
                        RestError::TemporarilyUnavailable
                    }
                })?;

        let auction: Option<models::Auction> = match bid.auction_id {
            Some(auction_id) => {
                let auction: models::Auction = sqlx::query_as("SELECT * FROM auction WHERE id = $1")
                    .bind(auction_id)
                    .fetch_one(&self.db)
                    .instrument(info_span!("db_get_bid_auction"))
                    .await
                    .map_err(|e| {
                        tracing::error!(error = e.to_string(), bid = ?bid, auction_id = auction_id.to_string(), "Failed to get auction for bid from db");
                        RestError::TemporarilyUnavailable
                    })?;
                Some(auction)
            }
            None => None,
        };

        bid.get_bid_entity(auction).map_err(|e| {
            tracing::error!(error = e.to_string(), bid = ?bid, "Failed to convert bid to entity");
            RestError::TemporarilyUnavailable
        })
    }
}
