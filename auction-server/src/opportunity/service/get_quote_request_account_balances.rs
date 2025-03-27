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
        extension::StateWithExtensions,
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

#[derive(Debug, PartialEq)]
pub enum TokenAccountBalance {
    Uninitialized,
    Initialized(u64),
}

impl TokenAccountBalance {
    pub fn get_balance(&self) -> u64 {
        match self {
            TokenAccountBalance::Uninitialized => 0,
            TokenAccountBalance::Initialized(balance) => *balance,
        }
    }

    pub fn get_initialization_config(&self, user_payer: bool) -> TokenAccountInitializationConfig {
        match self {
            TokenAccountBalance::Uninitialized => {
                if user_payer {
                    TokenAccountInitializationConfig::UserPayer
                } else {
                    TokenAccountInitializationConfig::SearcherPayer
                }
            }
            TokenAccountBalance::Initialized(_) => TokenAccountInitializationConfig::Unneeded,
        }
    }
}

impl From<Option<u64>> for TokenAccountBalance {
    fn from(balance: Option<u64>) -> Self {
        match balance {
            Some(balance) => TokenAccountBalance::Initialized(balance),
            None => TokenAccountBalance::Uninitialized,
        }
    }
}

/// The balances of some of the accounts that will be used in the swap
pub struct QuoteRequestAccountBalances {
    pub user_sol_balance:               u64,
    pub user_ata_mint_searcher:         TokenAccountBalance,
    pub user_ata_mint_user:             TokenAccountBalance,
    pub router_fee_receiver_ta:         TokenAccountBalance,
    pub relayer_fee_receiver_ata:       TokenAccountBalance,
    pub express_relay_fee_receiver_ata: TokenAccountBalance,
}

impl QuoteRequestAccountBalances {
    pub fn get_user_ata_mint_user_balance(&self, mint_user_is_wrapped_sol: bool) -> u64 {
        if mint_user_is_wrapped_sol {
            self.user_sol_balance // we assume the user doesn't have any balance in their wrapped sol account
        } else {
            self.user_ata_mint_user.get_balance()
        }
    }

    pub fn get_token_account_initialization_configs(&self) -> TokenAccountInitializationConfigs {
        let rent = Rent::default(); // TODO: this is not correct, we should use the rent of the chain, but probably fine for Solana mainnet

        // This is just a heuristic, we want users to pay for their token account if they have enough SOL, but still have some SOL left after the swap.
        // The user should have enough SOL for the rent of two token accounts, after the swap.
        let mut remaining_sol_balance = self.user_sol_balance;
        let user_payer_ata_mint_user = remaining_sol_balance
            >= 3 * rent.minimum_balance(TokenAccount::LEN)
            && self.user_ata_mint_user == TokenAccountBalance::Uninitialized;
        if user_payer_ata_mint_user {
            remaining_sol_balance =
                remaining_sol_balance.saturating_sub(rent.minimum_balance(TokenAccount::LEN));
        };

        let user_payer_ata_mint_searcher =
            remaining_sol_balance >= 3 * rent.minimum_balance(TokenAccount::LEN);

        TokenAccountInitializationConfigs {
            user_ata_mint_user:             self
                .user_ata_mint_user
                .get_initialization_config(user_payer_ata_mint_user), // This is useful for wrapped SOL, where the user balance is in their native wallet and their wrapped SOL account needs to be initialized before the swap.
            // Additionally, in (indicative) quotes for a user that has 0 funds in the user token account, we need searchers to initialize this account in their bids so the simulation fails with the `InsufficientUserFunds` error.
            user_ata_mint_searcher:         self
                .user_ata_mint_searcher
                .get_initialization_config(user_payer_ata_mint_searcher),
            router_fee_receiver_ta:         self
                .router_fee_receiver_ta
                .get_initialization_config(false),
            relayer_fee_receiver_ata:       self
                .relayer_fee_receiver_ata
                .get_initialization_config(false),
            express_relay_fee_receiver_ata: self
                .express_relay_fee_receiver_ata
                .get_initialization_config(false),
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
            &metadata.fee_receiver_relayer,
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

        let user_sol_balance = accounts[0]
            .as_ref()
            .map(|account| account.lamports)
            .unwrap_or_default();

        let token_balances: Vec<TokenAccountBalance> = accounts[1..].iter()
            .map(|account| {
                account
                    .as_ref()
                    .map(|acc| {
                        StateWithExtensions::<TokenAccount>::unpack(&acc.data[..TokenAccount::LEN])
                            .map_err(|err| {
                                tracing::error!(error = ?err, "Failed to deserialize a token account");
                                RestError::TemporarilyUnavailable
                            })
                            .map(|token_account| token_account.base.amount)
                    })
                    .transpose()
                    .map(|balance| balance.into())
            })
            .collect::<Result<Vec<TokenAccountBalance>, RestError>>()?;

        let [user_ata_mint_user, user_ata_mint_searcher, router_fee_receiver_ta, relayer_fee_receiver_ata, express_relay_fee_receiver_ata] =
            token_balances.try_into().unwrap(); // This won't panic because we know the length of the vector is 5

        Ok(QuoteRequestAccountBalances {
            user_sol_balance,
            user_ata_mint_user,
            user_ata_mint_searcher,
            router_fee_receiver_ta,
            relayer_fee_receiver_ata,
            express_relay_fee_receiver_ata,
        })
    }
}
