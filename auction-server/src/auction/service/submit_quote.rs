use {
    super::{
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
};

pub struct SubmitQuoteInput {
    pub bid_id:         entities::BidId,
    pub user_signature: Signature,
}

impl Service<Svm> {
    pub async fn submit_quote(
        &self,
        input: SubmitQuoteInput,
    ) -> Result<VersionedTransaction, RestError> {
        let bid: Option<entities::Bid<Svm>> = self.repo.get_in_memory_bid_by_id(input.bid_id).await;
        match bid {
            Some(mut bid) => {
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

                // TODO change it to a better state (Wait for user signature)
                if bid.status.is_submitted() {
                    if bid.chain_data.bid_payment_instruction_type
                        == entities::BidPaymentInstructionType::Swap
                    {
                        self.send_transaction(&bid).await.map_err(|e| {
                            tracing::error!(error = ?e, "Error sending quote transaction to network");
                            RestError::TemporarilyUnavailable
                        })?;
                        Ok(bid.chain_data.transaction)
                    } else {
                        Err(RestError::BadParameters("Invalid quote".to_string()))
                    }
                } else {
                    Err(RestError::BadParameters(
                        "Quote is not valid anymore".to_string(),
                    ))
                }
            }
            None => Err(RestError::BadParameters(
                "Quote is not valid anymore".to_string(),
            )),
        }
    }
}
