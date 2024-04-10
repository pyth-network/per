use {
    crate::{
        api::{
            opportunity::ChainIdQueryParams,
            ErrorBodyResponse,
            RestError,
        },
        state::{
            AuctionId,
            AuctionParams,
            PermissionKey,
            Store,
        },
    },
    axum::{
        extract::{
            Path,
            Query,
            State,
        },
        Json,
    },
    ethers::types::H256,
    serde::{
        Deserialize,
        Serialize,
    },
    std::sync::Arc,
    utoipa::ToSchema,
};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct AuctionParamsWithId {
    #[schema(value_type = String)]
    pub id:     AuctionId,
    pub params: AuctionParams,
}

/// Query for auctions with the permission key and (optionally) chain ID specified.
#[utoipa::path(get, path = "/v1/auctions/{permission_key}",
    params(
        ("permission_key"=String, description = "Permission key to query for"),
        ChainIdQueryParams
    ),
    responses(
        (status = 200, description = "Array of auctions with the permission key", body = Vec<AuctionParams>),
        (status = 400, response = ErrorBodyResponse),
        (status = 404, description = "Permission key was not found", body = ErrorBodyResponse),
    )
)]
pub async fn get_auctions(
    State(store): State<Arc<Store>>,
    Path(permission_key): Path<PermissionKey>,
    query_params: Query<ChainIdQueryParams>,
) -> Result<Json<Vec<AuctionParams>>, RestError> {
    let auctions = match &query_params.chain_id {
        Some(chain_id) => {
            let auction_records = sqlx::query!(
                "SELECT * FROM auction WHERE permission_key = $1 AND chain_id = $2",
                permission_key.as_ref(),
                chain_id
            )
            .fetch_all(&store.db)
            .await
            .map_err(|_| RestError::AuctionNotFound)?;

            auction_records
                .into_iter()
                .map(|auction| AuctionParams {
                    chain_id:       auction.chain_id,
                    permission_key: auction.permission_key.into(),
                    tx_hash:        H256::from_slice(auction.tx_hash.as_ref()),
                })
                .collect()
        }
        None => {
            let auction_records = sqlx::query!(
                "SELECT * FROM auction WHERE permission_key = $1",
                permission_key.as_ref()
            )
            .fetch_all(&store.db)
            .await
            .map_err(|_| RestError::AuctionNotFound)?;

            auction_records
                .into_iter()
                .map(|auction| AuctionParams {
                    chain_id:       auction.chain_id,
                    permission_key: auction.permission_key.into(),
                    tx_hash:        H256::from_slice(auction.tx_hash.as_ref()),
                })
                .collect()
        }
    };

    Ok(Json(auctions))
}

// Get auction with the specified ID.
pub async fn get_auction_with_id(
    store: Arc<Store>,
    auction_id: AuctionId,
) -> Result<AuctionParamsWithId, RestError> {
    let auction = sqlx::query!("SELECT * FROM auction WHERE id = $1", auction_id)
        .fetch_one(&store.db)
        .await
        .map_err(|_| RestError::BidNotFound)?;

    Ok(AuctionParamsWithId {
        id:     auction.id,
        params: AuctionParams {
            chain_id:       auction.chain_id,
            permission_key: auction.permission_key.into(),
            tx_hash:        H256::from_slice(auction.tx_hash.as_ref()),
        },
    })
}
