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
    async fn get_bid_to_submit(
        &self,
        auction_id: entities::AuctionId,
    ) -> Result<(entities::Auction<Svm>, entities::Bid<Svm>), RestError> {
        let auction: entities::Auction<Svm> = self
            .get_auction_by_id(GetAuctionByIdInput { auction_id })
            .await
            .ok_or(RestError::BadParameters("Quote not found. The provided reference ID may be invalid, already finalized on-chain, or canceled.".to_string()))?;

        let winner_bid = auction
            .bids
            .iter()
            .find(|bid| bid.status.is_awaiting_signature() || bid.status.is_sent_to_user_for_submission() || bid.status.is_submitted() || bid.status.is_cancelled())
            .cloned()
            .ok_or(RestError::BadParameters("This quote has already been submitted and finalized on-chain. No further changes are allowed.".to_string()))?;

        if winner_bid.status.is_cancelled() {
            Err(RestError::BadParameters(
                "This quote has already been cancelled.".to_string(),
            ))
        } else if winner_bid.status.is_submitted() {
            Err(RestError::BadParameters(
                "Quote is already submitted on-chain.".to_string(),
            ))
        } else {
            Ok((auction, winner_bid))
        }
    }

    pub async fn sign_bid_and_submit_auction(
        &self,
        bid: entities::Bid<Svm>,
        auction: entities::Auction<Svm>,
    ) -> Result<VersionedTransaction, RestError> {
        let mut bid = bid;
        self.add_relayer_signature(&mut bid);
        let auction = self.get_auction_by_id(GetAuctionByIdInput {auction_id: auction.id,
        }).await.ok_or_else(|| {
            tracing::error!(auction_id = %auction.id, "Auction not found when getting most recent version");
            RestError::TemporarilyUnavailable
        })?;
        self.repo
            .submit_auction(auction, bid.chain_data.transaction.signatures[0])
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Error repo submitting auction");
                RestError::TemporarilyUnavailable
            })?;
        Ok(bid.chain_data.transaction)
    }

    #[tracing::instrument(skip_all, err(level = tracing::Level::TRACE))]
    async fn submit_auction_bid_for_lock(
        &self,
        bid: entities::Bid<Svm>,
        auction: entities::Auction<Svm>,
        lock: entities::BidLock,
    ) -> Result<(), RestError> {
        let _lock = lock.lock().await;

        // Make sure the bid is still awaiting signature, we also get the latest saved version of the auction
        let (auction, _) = self.get_bid_to_submit(auction.id).await?;

        let tx_hash = bid.chain_data.transaction.signatures[0];
        let auction = self
            .repo
            .submit_auction(auction, tx_hash)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Error repo submitting auction");
                RestError::TemporarilyUnavailable
            })?;
        self.update_bid_status(UpdateBidStatusInput {
            bid:        bid.clone(),
            new_status: entities::BidStatusSvm::Submitted {
                auction: entities::BidStatusAuction {
                    id: auction.id,
                    tx_hash,
                },
            },
        })
        .await?;

        // Send transaction after updating bid status to make sure the bid is not cancellable anymore
        // If we submit the transaction before updating the bid status, the DB update can be failed and the bid can be cancelled later.
        // This will cause the transaction to be submitted but the bid to be cancelled.
        self.send_transaction(&bid).await;
        Ok(())
    }

    #[tracing::instrument(skip_all, err(level = tracing::Level::TRACE), fields(bid_id, auction_id = %input.auction_id))]
    pub async fn submit_quote(
        &self,
        input: SubmitQuoteInput,
    ) -> Result<VersionedTransaction, RestError> {
        let (auction, winner_bid) = self.get_bid_to_submit(input.auction_id).await?;

        let mut bid = winner_bid.clone();
        tracing::Span::current().record("bid_id", bid.id.to_string());
        let (_, swap_instruction) = self
            .extract_express_relay_instruction(
                bid.chain_data.transaction.clone(),
                entities::BidPaymentInstructionType::Swap,
            )
            .map_err(|_| RestError::BadParameters("Invalid quote.".to_string()))?;
        let SwapAccounts { user_wallet, .. } = self
            .extract_swap_accounts(&bid.chain_data.transaction, &swap_instruction)
            .await
            .map_err(|_| RestError::BadParameters("Invalid quote.".to_string()))?;
        let swap_args = Self::extract_swap_data(&swap_instruction)
            .map_err(|_| RestError::BadParameters("Invalid quote.".to_string()))?;

        if swap_args.deadline < (OffsetDateTime::now_utc() - DEADLINE_BUFFER).unix_timestamp() {
            return Err(RestError::BadParameters("Quote is expired.".to_string()));
        }

        if !input.user_signature.verify(
            &user_wallet.to_bytes(),
            &bid.chain_data.transaction.message.serialize(),
        ) {
            return Err(RestError::BadParameters(
                "Invalid user signature.".to_string(),
            ));
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
        self.add_relayer_signature(&mut bid);

        if bid.chain_data.bid_payment_instruction_type != entities::BidPaymentInstructionType::Swap
        {
            return Err(RestError::BadParameters("Invalid quote.".to_string()));
        }

        let bid_lock = self
            .repo
            .get_or_create_in_memory_bid_lock(winner_bid.id)
            .await;
        self.submit_auction_bid_for_lock(bid.clone(), auction, bid_lock)
            .await?;
        self.repo.remove_in_memory_bid_lock(&winner_bid.id).await;
        Ok(bid.chain_data.transaction)
    }
}
