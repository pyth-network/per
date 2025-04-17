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
        kernel::entities::ChainId,
        per_metrics::{
            SUBMIT_QUOTE_DEADLINE_BUFFER_METRIC,
            SUBMIT_QUOTE_DEADLINE_TOTAL,
        },
    },
    axum_prometheus::metrics,
    solana_sdk::{
        signature::Signature,
        transaction::VersionedTransaction,
    },
    time::OffsetDateTime,
};

pub struct SubmitQuoteInput {
    pub auction_id:     entities::AuctionId,
    pub user_signature: Signature,
}

const MIN_DEADLINE_BUFFER_SECS: i64 = 2;

impl Service {
    async fn get_winner_bid(
        &self,
        auction_id: entities::AuctionId,
    ) -> Result<(entities::Auction, entities::Bid), RestError> {
        let auction: entities::Auction = self
            .get_auction_by_id(GetAuctionByIdInput { auction_id })
            .await
            .ok_or(RestError::BadParameters("Quote not found. The provided reference ID may be invalid, already finalized on-chain, or canceled.".to_string()))?;

        let winner_bid = auction
            .bids
            .iter()
            .find(|bid| {
                bid.status.is_awaiting_signature()
                    || bid.status.is_sent_to_user_for_submission()
                    || bid.status.is_submitted()
                    || bid.status.is_cancelled()
            })
            .cloned()
            .ok_or(RestError::QuoteIsFinalized)?;

        Ok((auction, winner_bid))
    }

    pub async fn sign_bid_and_submit_auction(
        &self,
        bid: entities::Bid,
        auction: entities::Auction,
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
        signed_bid: entities::Bid,
        auction: entities::Auction,
        swap_args: express_relay::SwapArgs,
        lock: entities::BidLock,
    ) -> Result<(), RestError> {
        let _lock = lock.lock().await;

        // Make sure the bid is still not cancelled, we also get the latest saved version of the auction
        let (auction, bid_latest_version) = self.get_winner_bid(auction.id).await?;
        if bid_latest_version.status.is_submitted() {
            return Ok(());
        }

        if !self.is_within_deadline_buffer(bid_latest_version.chain_id.clone(), swap_args) {
            if bid_latest_version.status.is_sent_to_user_for_submission() {
                // TODO we are losing information here, need a better way for handling this situation
                // NOTE: These bids maybe submitted by the user, so we need to update the status to submission failed
                tracing::warn!(bid_id = ?bid_latest_version.id, auction_id = ?auction.id, "A non cancellable bid is submitted after the deadline buffer");
                return Err(RestError::QuoteIsExpired);
            }

            let tx_hash = bid_latest_version.chain_data.transaction.signatures[0];
            self.update_bid_status(UpdateBidStatusInput {
                bid:        bid_latest_version,
                new_status: entities::BidStatusSvm::SubmissionFailed {
                    auction: entities::BidStatusAuction {
                        id: auction.id,
                        tx_hash,
                    },
                    reason:  entities::BidSubmissionFailedReason::DeadlinePassed,
                },
            })
            .await?;
            return Err(RestError::QuoteIsExpired);
        }

        if bid_latest_version.status.is_cancelled() {
            self.update_bid_status(UpdateBidStatusInput {
                bid:        bid_latest_version.clone(),
                new_status: entities::BidStatusSvm::SubmissionFailed {
                    auction: entities::BidStatusAuction {
                        id:      auction.id,
                        tx_hash: bid_latest_version.chain_data.transaction.signatures[0],
                    },
                    reason:  entities::BidSubmissionFailedReason::Cancelled,
                },
            })
            .await?;
            return Err(RestError::QuoteIsCancelled);
        }

        let tx_hash = signed_bid.chain_data.transaction.signatures[0];
        let auction = self
            .repo
            .submit_auction(auction, tx_hash)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Error repo submitting auction");
                RestError::TemporarilyUnavailable
            })?;
        self.update_bid_status(UpdateBidStatusInput {
            bid:        signed_bid.clone(),
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
        self.send_transaction(&signed_bid).await;
        Ok(())
    }

    fn is_within_deadline_buffer(
        &self,
        chain_id: ChainId,
        swap_args: express_relay::SwapArgs,
    ) -> bool {
        let deadline_buffer_secs = swap_args.deadline - OffsetDateTime::now_utc().unix_timestamp();

        metrics::histogram!(
            SUBMIT_QUOTE_DEADLINE_BUFFER_METRIC,
            &[("chain_id", chain_id.clone()),]
        )
        .record(deadline_buffer_secs as f64);

        let result = if deadline_buffer_secs >= MIN_DEADLINE_BUFFER_SECS {
            "success"
        } else {
            "error"
        };

        metrics::counter!(
            SUBMIT_QUOTE_DEADLINE_TOTAL,
            &[
                ("chain_id", chain_id.clone()),
                ("result", result.to_string()),
            ]
        )
        .increment(1);

        deadline_buffer_secs >= MIN_DEADLINE_BUFFER_SECS
    }

    #[tracing::instrument(skip_all, err(level = tracing::Level::TRACE), fields(bid_id, auction_id = %input.auction_id))]
    pub async fn submit_quote(
        &self,
        input: SubmitQuoteInput,
    ) -> Result<VersionedTransaction, RestError> {
        let (auction, winner_bid) = self.get_winner_bid(input.auction_id).await?;

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

        let swap_args = Self::extract_swap_data(&swap_instruction)
            .map_err(|_| RestError::BadParameters("Invalid quote.".to_string()))?;

        let bid_lock = self
            .repo
            .get_or_create_in_memory_bid_lock(winner_bid.id)
            .await;
        // NOTE: Don't use ? here to make sure we are going to call the remove_in_memory_bid_lock function
        let result = self
            .submit_auction_bid_for_lock(bid.clone(), auction, swap_args, bid_lock)
            .await;
        self.repo.remove_in_memory_bid_lock(&winner_bid.id).await;
        match result {
            Ok(()) => Ok(bid.chain_data.transaction),
            Err(e) => Err(e),
        }
    }
}
