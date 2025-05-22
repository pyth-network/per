use {
    super::Service,
    crate::{
        api::RestError,
        auction::entities,
        opportunity::service::get_express_relay_metadata::GetExpressRelayMetadataInput,
    },
    anchor_lang::{
        AnchorDeserialize,
        Discriminator,
    },
    express_relay as express_relay_svm,
    solana_sdk::{
        instruction::CompiledInstruction,
        transaction::VersionedTransaction,
    },
};

pub struct GetBidTransactionDataSwapInput {
    pub transaction: VersionedTransaction,
}

pub struct GetBidTransactionDataSubmitBidInput {
    pub transaction: VersionedTransaction,
    pub bid:         Option<entities::Bid>,
}

pub struct GetBidTransactionDataInput {
    pub bid: entities::Bid,
}

impl Service {
    fn extract_submit_bid_data(
        instruction: &CompiledInstruction,
    ) -> Result<express_relay_svm::SubmitBidArgs, RestError> {
        let discriminator = express_relay_svm::instruction::SubmitBid::DISCRIMINATOR;
        express_relay_svm::SubmitBidArgs::try_from_slice(
            &instruction.data.as_slice()[discriminator.len()..],
        )
        .map_err(|e| {
            RestError::BadParameters(format!("Invalid submit_bid instruction data: {}", e))
        })
    }

    fn extract_express_relay_instruction(
        &self,
        transaction: VersionedTransaction,
        instruction_type: entities::BidPaymentInstructionType,
    ) -> Result<(usize, CompiledInstruction), RestError> {
        let valid_discriminators = match instruction_type {
            entities::BidPaymentInstructionType::SubmitBid => {
                vec![express_relay_svm::instruction::SubmitBid::DISCRIMINATOR]
            }
            entities::BidPaymentInstructionType::Swap => vec![
                express_relay_svm::instruction::Swap::DISCRIMINATOR,
                express_relay_svm::instruction::SwapV2::DISCRIMINATOR,
            ],
        };
        let instructions = Self::extract_program_instructions(
            &transaction,
            &self.config.chain_config.express_relay.program_id,
        )
        .into_iter()
        .map(|(index, instruction)| (index, instruction.clone()))
        .collect::<Vec<(usize, CompiledInstruction)>>();

        let (instruction_index, instruction) = match instructions.len() {
            1 => Ok(instructions
                .into_iter()
                .next()
                .expect("This can't happen because we just only go here if the length is 1")),
            _ => Err(RestError::InvalidExpressRelayInstructionCount(
                instructions.len(),
            )),
        }?;
        if valid_discriminators
            .iter()
            .all(|discriminator| !instruction.data.starts_with(discriminator))
        {
            return Err(RestError::BadParameters(
                "Wrong instruction type for Express Relay Program".to_string(),
            ));
        }
        Ok((instruction_index, instruction))
    }

    pub async fn get_bid_transaction_data_submit_bid(
        &self,
        input: GetBidTransactionDataSubmitBidInput,
    ) -> Result<entities::BidTransactionDataSubmitBid, RestError> {
        let (index, instruction) = self.extract_express_relay_instruction(
            input.transaction.clone(),
            entities::BidPaymentInstructionType::SubmitBid,
        )?;
        let data = Self::extract_submit_bid_data(&instruction)?;
        let accounts = match input.bid {
            Some(bid) => entities::OnChainAccounts {
                router:             bid.chain_data.router,
                permission_account: bid.chain_data.permission_account,
            },
            None => {
                let express_relay_config = &self.config.chain_config.express_relay;
                let permission_account = self
                    .extract_account(
                        &input.transaction,
                        &instruction,
                        express_relay_config
                            .submit_bid_instruction_account_positions
                            .permission_account,
                    )
                    .await?;
                let router = self
                    .extract_account(
                        &input.transaction,
                        &instruction,
                        express_relay_config
                            .submit_bid_instruction_account_positions
                            .router_account,
                    )
                    .await?;
                entities::OnChainAccounts {
                    router,
                    permission_account,
                }
            }
        };
        Ok(entities::BidTransactionDataSubmitBid {
            data,
            accounts,
            express_relay_instruction_index: index,
        })
    }

    pub async fn extract_swap_accounts(
        &self,
        tx: &VersionedTransaction,
        swap_instruction: &CompiledInstruction,
    ) -> Result<entities::SwapAccounts, RestError> {
        let positions = &self
            .config
            .chain_config
            .express_relay
            .swap_instruction_account_positions;

        let searcher = self
            .extract_account(tx, swap_instruction, positions.searcher_account)
            .await?;
        let user_wallet = self
            .extract_account(tx, swap_instruction, positions.user_wallet_account)
            .await?;
        let mint_searcher = self
            .extract_account(tx, swap_instruction, positions.mint_searcher_account)
            .await?;
        let mint_user = self
            .extract_account(tx, swap_instruction, positions.mint_user_account)
            .await?;
        let router_token_account = self
            .extract_account(tx, swap_instruction, positions.router_token_account)
            .await?;
        let token_program_searcher = self
            .extract_account(tx, swap_instruction, positions.token_program_searcher)
            .await?;
        let token_program_user = self
            .extract_account(tx, swap_instruction, positions.token_program_user)
            .await?;

        Ok(entities::SwapAccounts {
            searcher,
            user_wallet,
            mint_searcher,
            mint_user,
            router_token_account,
            token_program_searcher,
            token_program_user,
        })
    }

    pub async fn extract_swap_data(
        &self,
        instruction: &CompiledInstruction,
    ) -> Result<express_relay_svm::SwapV2Args, RestError> {
        if instruction
            .data
            .starts_with(express_relay_svm::instruction::Swap::DISCRIMINATOR)
        {
            let discriminator = express_relay_svm::instruction::Swap::DISCRIMINATOR;
            let express_relay_metadata = self
                .opportunity_service
                .get_express_relay_metadata(GetExpressRelayMetadataInput {
                    chain_id: self.config.chain_id.clone(),
                })
                .await?;
            let swap_args = express_relay_svm::SwapArgs::try_from_slice(
                &instruction.data.as_slice()[discriminator.len()..],
            )
            .map_err(|e| {
                RestError::BadParameters(format!("Invalid swap instruction data: {}", e))
            })?;
            Ok(swap_args.convert_to_v2(express_relay_metadata.swap_platform_fee_bps))
        } else {
            let discriminator = express_relay_svm::instruction::SwapV2::DISCRIMINATOR;
            express_relay_svm::SwapV2Args::try_from_slice(
                &instruction.data.as_slice()[discriminator.len()..],
            )
            .map_err(|e| RestError::BadParameters(format!("Invalid swap instruction data: {}", e)))
        }
    }

    pub async fn get_bid_transaction_data_swap(
        &self,
        input: GetBidTransactionDataSwapInput,
    ) -> Result<entities::BidTransactionDataSwap, RestError> {
        let (index, instruction) = self.extract_express_relay_instruction(
            input.transaction.clone(),
            entities::BidPaymentInstructionType::Swap,
        )?;
        Ok(entities::BidTransactionDataSwap {
            data:                            self.extract_swap_data(&instruction).await?,
            accounts:                        self
                .extract_swap_accounts(&input.transaction, &instruction)
                .await?,
            express_relay_instruction_index: index,
        })
    }

    pub async fn get_bid_transaction_data(
        &self,
        input: GetBidTransactionDataInput,
    ) -> Result<entities::BidTransactionData, RestError> {
        match input.bid.chain_data.bid_payment_instruction_type {
            entities::BidPaymentInstructionType::SubmitBid => {
                let transaction_data = self
                    .get_bid_transaction_data_submit_bid(GetBidTransactionDataSubmitBidInput {
                        transaction: input.bid.chain_data.transaction.clone(),
                        bid:         Some(input.bid.clone()),
                    })
                    .await?;
                Ok(entities::BidTransactionData::SubmitBid(transaction_data))
            }
            entities::BidPaymentInstructionType::Swap => {
                let transaction_data = self
                    .get_bid_transaction_data_swap(GetBidTransactionDataSwapInput {
                        transaction: input.bid.chain_data.transaction.clone(),
                    })
                    .await?;
                Ok(entities::BidTransactionData::Swap(transaction_data))
            }
        }
    }
}
