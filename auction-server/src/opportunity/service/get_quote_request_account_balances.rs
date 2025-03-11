use {
    super::{
        get_express_relay_metadata::GetExpressRelayMetadata,
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        kernel::entities::ChainId,
        opportunity::entities::{
            FeeToken,
            TokenAccountInitializationConfig,
            TokenAccountInitializationConfigs,
        },
    },
    solana_sdk::{
        program_pack::Pack,
        pubkey::Pubkey,
        rent::Rent,
    },
    spl_associated_token_account::get_associated_token_address_with_program_id,
    spl_token_2022::{
        extension::StateWithExtensions as TokenAccountWithExtensions,
        state::Account as TokenAccount,
    },
};

pub struct QuoteRequestAccountBalancesInput {
    pub chain_id:               ChainId,
    pub fee_token:              FeeToken,
    pub user_wallet_address:    Pubkey,
    pub router:                 Pubkey,
    pub mint_searcher:          Pubkey,
    pub mint_user:              Pubkey,
    pub token_program_searcher: Pubkey,
    pub token_program_user:     Pubkey,
}

/// The balances of some of the accounts that will be used in the swap
/// - If a token account doesn't exist, the balance is `None`
/// - If a token account exists but has no balance, the balance is `Some(0)`
pub struct QuoteRequestAccountBalances {
    pub user_wallet:                    u64,
    pub user_ata_mint_searcher:         Option<u64>,
    pub user_ata_mint_user:             Option<u64>,
    pub router_fee_receiver_ta:         Option<u64>,
    pub relayer_fee_receiver_ata:       Option<u64>,
    pub express_relay_fee_receiver_ata: Option<u64>,
}

impl QuoteRequestAccountBalances {
    pub fn get_user_ata_mint_user_balance(&self, mint_user_is_wrapped_sol: bool) -> u64 {
        if mint_user_is_wrapped_sol {
            self.user_wallet
                .saturating_add(self.user_ata_mint_user.unwrap_or_default())
        } else {
            self.user_ata_mint_user.unwrap_or_default()
        }
    }

    pub fn get_token_account_initialization_configs(
        &self,
        mint_user_is_wrapped_sol: bool,
    ) -> TokenAccountInitializationConfigs {
        let rent = Rent::default();

        let user_payer = self.user_wallet.saturating_sub(rent.minimum_balance(0))
            >= rent.minimum_balance(2 * TokenAccount::LEN);

        TokenAccountInitializationConfigs {
            user_ata_mint_user:             if mint_user_is_wrapped_sol {
                Some(TokenAccountInitializationConfig::from_optional_balance(
                    self.user_ata_mint_user,
                    false,
                ))
            } else {
                None
            },
            user_ata_mint_searcher:         TokenAccountInitializationConfig::from_optional_balance(
                self.user_ata_mint_searcher,
                user_payer,
            ),
            router_fee_receiver_ta:         TokenAccountInitializationConfig::from_optional_balance(
                self.router_fee_receiver_ta,
                false,
            ),
            relayer_fee_receiver_ata:       TokenAccountInitializationConfig::from_optional_balance(
                self.relayer_fee_receiver_ata,
                false,
            ),
            express_relay_fee_receiver_ata: TokenAccountInitializationConfig::from_optional_balance(
                self.express_relay_fee_receiver_ata,
                false,
            ),
        }
    }
}


impl Service<ChainTypeSvm> {
    pub async fn get_quote_request_account_balances(
        &self,
        input: QuoteRequestAccountBalancesInput,
    ) -> Result<QuoteRequestAccountBalances, RestError> {
        let (mint_fee, token_program_fee) = if input.fee_token == FeeToken::SearcherToken {
            (input.mint_searcher, input.token_program_searcher)
        } else {
            (input.mint_user, input.token_program_user)
        };

        let metadata = self
            .get_express_relay_metadata(GetExpressRelayMetadata {
                chain_id: input.chain_id.clone(),
            })
            .await?;

        let config = self.get_config(&input.chain_id)?;
        let express_relay_metadata_address = Self::calculate_metadata_address(config).await;

        let user_ata_mint_user = get_associated_token_address_with_program_id(
            &input.user_wallet_address,
            &input.mint_user,
            &input.token_program_user,
        );

        let user_ata_mint_searcher = get_associated_token_address_with_program_id(
            &input.user_wallet_address,
            &input.mint_searcher,
            &input.token_program_searcher,
        );

        let router_fee_receiver_ta = get_associated_token_address_with_program_id(
            &input.router,
            &mint_fee,
            &token_program_fee,
        );

        let relayer_fee_receiver_ata = get_associated_token_address_with_program_id(
            &metadata.fee_receiver_relayer.to_bytes().into(),
            &mint_fee,
            &token_program_fee,
        );

        let express_relay_fee_receiver_ata = get_associated_token_address_with_program_id(
            &express_relay_metadata_address,
            &mint_fee,
            &token_program_fee,
        );

        let accounts = config.rpc_client.get_multiple_accounts(&[input.user_wallet_address, user_ata_mint_user, user_ata_mint_searcher, router_fee_receiver_ta, relayer_fee_receiver_ata, express_relay_fee_receiver_ata]).await.map_err(|err| {
            tracing::error!(error = ?err, "Failed to get quote request associated token accounts");
            RestError::TemporarilyUnavailable
        })?;

        let user_wallet = accounts[0]
            .as_ref()
            .map(|account| account.lamports)
            .unwrap_or_default();

        let token_balances: Vec<Option<u64>> = accounts[1..].iter()
            .map(|account| {
                account
                    .as_ref()
                    .map(|acc| {
                        TokenAccountWithExtensions::<TokenAccount>::unpack(acc.data.as_slice())
                            .map_err(|err| {
                                tracing::error!(error = ?err, "Failed to deserialize a token account");
                                RestError::TemporarilyUnavailable
                            })
                            .map(|token_account_with_extensions| token_account_with_extensions.base.amount)
                    })
                    .transpose()
            })
            .collect::<Result<Vec<Option<u64>>, RestError>>()?;

        Ok(QuoteRequestAccountBalances {
            user_wallet,
            user_ata_mint_user: token_balances[0],
            user_ata_mint_searcher: token_balances[1],
            router_fee_receiver_ta: token_balances[2],
            relayer_fee_receiver_ata: token_balances[3],
            express_relay_fee_receiver_ata: token_balances[4],
        })
    }
}
