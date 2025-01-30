use {
    super::{
        get_auction_by_id::GetAuctionByIdInput,
        update_bid_status::UpdateBidStatusInput,
        verification::SwapAccounts,
        Service,
    },
    crate::{
        api::RestError,
        auction::entities::{
            self,
            BidStatus,
        },
        kernel::entities::Svm,
    },
    solana_sdk::{
        signature::Signature,
        transaction::VersionedTransaction,
    },
    std::time::Duration,
    time::OffsetDateTime,
};

pub struct SubmitQuoteInput {
    pub auction_id:     entities::AuctionId,
    pub user_signature: Signature,
}

const DEADLINE_BUFFER: Duration = Duration::from_secs(2);

impl Service<Svm> {
    pub async fn submit_quote(
        &self,
        input: SubmitQuoteInput,
    ) -> Result<VersionedTransaction, RestError> {
        let auction: Option<entities::Auction<Svm>> = self
            .get_auction_by_id(GetAuctionByIdInput {
                auction_id: input.auction_id,
            })
            .await;

        match auction {
            Some(auction) => {
                let winner_bid = auction
                    .bids
                    .iter()
                    .find(|bid| bid.status.is_awaiting_signature() || bid.status.is_submitted())
                    .cloned()
                    .ok_or(RestError::BadParameters("Invalid quote".to_string()))?;

                if winner_bid.status.is_submitted() {
                    return Err(RestError::BadParameters(
                        "Quote is already submitted".to_string(),
                    ));
                }

                let mut bid = winner_bid.clone();
                let swap_instruction = self
                    .extract_express_relay_instruction(
                        bid.chain_data.transaction.clone(),
                        entities::BidPaymentInstructionType::Swap,
                    )
                    .map_err(|_| RestError::BadParameters("Invalid quote".to_string()))?;
                let SwapAccounts { user_wallet, .. } = self
                    .extract_swap_accounts(&bid.chain_data.transaction, &swap_instruction)
                    .await
                    .map_err(|_| RestError::BadParameters("Invalid quote".to_string()))?;
                let swap_args = Self::extract_swap_data(&swap_instruction)
                    .map_err(|_| RestError::BadParameters("Invalid quote".to_string()))?;

                if swap_args.deadline
                    < (OffsetDateTime::now_utc() - DEADLINE_BUFFER).unix_timestamp()
                {
                    return Err(RestError::BadParameters("Quote is expired".to_string()));
                }

                if !input.user_signature.verify(
                    &user_wallet.to_bytes(),
                    &bid.chain_data.transaction.message.serialize(),
                ) {
                    return Err(RestError::BadParameters("Invalid signature".to_string()));
                }

                let user_signature_pos = bid
                    .chain_data
                    .transaction
                    .message
                    .static_account_keys()
                    .iter()
                    .position(|p| p.eq(&user_wallet))
                    .expect("User wallet not found in transaction");
                bid.chain_data.transaction.signatures[user_signature_pos] = input.user_signature;

                // TODO add relayer signature after program update
                // self.add_relayer_signature(&mut bid);

                if bid.chain_data.bid_payment_instruction_type
                    == entities::BidPaymentInstructionType::Swap
                {
                    let tx_hash = self.send_transaction(&bid).await.map_err(|e| {
                        tracing::error!(error = ?e, "Error sending quote transaction to network");
                        RestError::TemporarilyUnavailable
                    })?;
                    let auction =
                        self.repo
                            .submit_auction(auction, tx_hash)
                            .await
                            .map_err(|e| {
                                tracing::error!(error = ?e, "Error repo submitting auction");
                                RestError::TemporarilyUnavailable
                            })?;
                    self.update_bid_status(UpdateBidStatusInput {
                        bid:        winner_bid.clone(),
                        new_status: entities::BidStatusSvm::Submitted {
                            auction: entities::BidStatusAuction {
                                id: auction.id,
                                tx_hash,
                            },
                        },
                    })
                    .await?;

                    Ok(bid.chain_data.transaction)
                } else {
                    Err(RestError::BadParameters("Invalid quote".to_string()))
                }
            }
            None => Err(RestError::BadParameters("Invalid quote".to_string())),
        }
    }
}
