use {
    crate::helpers::{
        generate_and_fund_key,
        submit_transaction,
    },
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{
            program_pack::Pack,
            system_instruction,
        },
        AccountDeserialize,
    },
    anchor_spl::{
        associated_token::{
            get_associated_token_address_with_program_id,
            spl_associated_token_account::instruction::create_associated_token_account_idempotent,
        },
        token_2022::{
            spl_token_2022,
            spl_token_2022::instruction::{
                initialize_account,
                mint_to_checked,
            },
        },
    },
    litesvm::LiteSVM,
    solana_sdk::signature::{
        Keypair,
        Signer,
    },
};

#[macro_export]
macro_rules! assert_token_balance_matches {
    ($svm:expr, $address:expr, $account:expr, $amount:expr $(,)?) => {
        let token_account_option = Token::get_token_account_opt($svm, $account);

        if token_account_option.is_none() {
            assert_eq!(
                0,
                $amount,
                "token account balance didn't exist but the expected amount wasn't zero for `{}`",
                stringify!($address)
            );
        } else {
            assert_eq!(
                token_account_option.unwrap().amount,
                $amount,
                "token account balance doesn't match expected value for `{}`",
                stringify!($address)
            );
        }
    };
}

#[macro_export]
macro_rules! assert_all_token_balances {
    ($svm:expr, $token_user:expr, {
        associated: {
            $($address:expr => $am_asc:expr),* $(,)?
        },
        raw: {
            $($raw_address:expr => $am_raw:expr),* $(,)?
        }
    }) => {
        $(
            $crate::assert_token_balance_matches!(
                $svm,
                $address,
                &$token_user.get_associated_token_address(&$address),
                $token_user.get_amount_with_decimals($am_asc),
            );
        )*
        $(
            $crate::assert_token_balance_matches!(
                $svm,
                $raw_address,
                &$raw_address,
                $token_user.get_amount_with_decimals($am_raw),
            );
        )*
    };
}

pub struct Token {
    pub mint:          Pubkey,
    pub decimals:      u8,
    mint_authority:    Keypair,
    pub token_program: Pubkey,
}

impl Clone for Token {
    fn clone(&self) -> Self {
        Self {
            mint:           self.mint,
            decimals:       self.decimals,
            mint_authority: self.mint_authority.insecure_clone(),
            token_program:  self.token_program,
        }
    }
}

impl Token {
    pub fn airdrop(&self, svm: &mut LiteSVM, destination: &Pubkey, amount: f64) {
        let instructions = vec![
            create_associated_token_account_idempotent(
                &self.mint_authority.pubkey(),
                destination,
                &self.mint,
                &self.token_program,
            ),
            mint_to_checked(
                &self.token_program,
                &self.mint,
                &get_associated_token_address_with_program_id(
                    destination,
                    &self.mint,
                    &self.token_program,
                ),
                &self.mint_authority.pubkey(),
                &[&self.mint_authority.pubkey()],
                self.get_amount_with_decimals(amount),
                self.decimals,
            )
            .unwrap(),
        ];
        submit_transaction(
            svm,
            &instructions,
            &self.mint_authority,
            &[&self.mint_authority],
        )
        .unwrap();
    }

    pub fn get_token_account_opt(
        svm: &mut LiteSVM,
        account: &Pubkey,
    ) -> Option<anchor_spl::token_interface::TokenAccount> {
        svm.get_account(account).map(|account| {
            anchor_spl::token_interface::TokenAccount::try_deserialize(&mut account.data.as_slice())
                .unwrap()
        })
    }

    pub fn token_balance_matches(svm: &mut LiteSVM, account: &Pubkey, amount: u64) -> bool {
        let token_account_option = Self::get_token_account_opt(svm, account);
        if token_account_option.is_none() {
            return amount == 0;
        }

        token_account_option.unwrap().amount == amount
    }

    pub fn create_token_account(&self, svm: &mut LiteSVM, owner: &Pubkey) -> Pubkey {
        let token_account = Keypair::new();
        let token_account_rent =
            svm.minimum_balance_for_rent_exemption(spl_token_2022::state::Account::LEN);
        let instructions = vec![
            system_instruction::create_account(
                &self.mint_authority.pubkey(),
                &token_account.pubkey(),
                token_account_rent,
                spl_token_2022::state::Account::LEN.try_into().unwrap(),
                &self.token_program,
            ),
            initialize_account(
                &self.token_program,
                &token_account.pubkey(),
                &self.mint,
                owner,
            )
            .unwrap(),
        ];
        submit_transaction(
            svm,
            &instructions,
            &self.mint_authority,
            &[&token_account, &self.mint_authority],
        )
        .unwrap();
        token_account.pubkey()
    }

    pub fn create_mint(svm: &mut LiteSVM, token_program: Pubkey, decimals: u8) -> Self {
        let mint = Keypair::new();
        let mint_authority = generate_and_fund_key(svm);
        let mint_rent = svm.minimum_balance_for_rent_exemption(spl_token_2022::state::Mint::LEN);
        let instructions = vec![
            system_instruction::create_account(
                &mint_authority.pubkey(),
                &mint.pubkey(),
                mint_rent,
                spl_token_2022::state::Mint::LEN.try_into().unwrap(),
                &token_program,
            ),
            spl_token_2022::instruction::initialize_mint(
                &token_program,
                &mint.pubkey(),
                &mint_authority.pubkey(),
                None,
                decimals,
            )
            .unwrap(),
        ];
        submit_transaction(
            svm,
            &instructions,
            &mint_authority,
            &[&mint, &mint_authority],
        )
        .unwrap();
        Self {
            mint: mint.pubkey(),
            decimals,
            mint_authority,
            token_program,
        }
    }

    pub fn get_amount_with_decimals(&self, amount: f64) -> u64 {
        (amount * 10f64.powi(self.decimals as i32)).floor() as u64
    }

    pub fn get_associated_token_address(&self, owner: &Pubkey) -> Pubkey {
        get_associated_token_address_with_program_id(owner, &self.mint, &self.token_program)
    }
}
