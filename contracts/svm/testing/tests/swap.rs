use {
    anchor_spl::{
        associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent,
        token::{
            initialize_mint,
            spl_token::{
                self,
                instruction::mint_to,
            },
        },
    },
    litesvm::LiteSVM,
    solana_sdk::{
        program_pack::Pack,
        pubkey::Pubkey,
        signature::{
            Keypair,
            Signer,
        },
        system_instruction,
    },
    testing::{
        helpers::{
            generate_and_fund_key,
            submit_transaction,
        },
        setup::{
            setup,
            SetupParams,
            SetupResult,
        },
    },
};

pub struct Token {
    pub mint:       Pubkey,
    pub decimals:   u8,
    mint_authority: Keypair,
}

impl Token {
    pub fn airdrop(&self, svm: &mut LiteSVM, destination: &Pubkey) {
        let mut instructions = vec![];
        instructions.push(create_associated_token_account_idempotent(
            &self.mint_authority.pubkey(),
            destination,
            &self.mint,
            &spl_token::ID,
        ));
        instructions.push(
            mint_to(
                &spl_token::ID,
                &self.mint,
                destination,
                &self.mint_authority.pubkey(),
                &[&self.mint_authority.pubkey()],
                10 * 10_u64.pow(self.decimals as u32),
            )
            .unwrap(),
        );
        submit_transaction(
            svm,
            &instructions,
            &self.mint_authority,
            &[&self.mint_authority],
        )
        .unwrap();
    }

    pub fn new(svm: &mut LiteSVM, decimals: u8) -> Self {
        let mint = Keypair::new();
        let mint_authority = generate_and_fund_key(svm);
        let mint_rent = svm.minimum_balance_for_rent_exemption(spl_token::state::Mint::LEN);
        let instructions = vec![
            system_instruction::create_account(
                &mint_authority.pubkey(),
                &mint.pubkey(),
                mint_rent,
                spl_token::state::Mint::LEN.try_into().unwrap(),
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_mint(
                &spl_token::id(),
                &mint.pubkey(),
                &mint_authority.pubkey(),
                None,
                0,
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
        }
    }
}

pub struct SwapInfo {
    pub svm:          LiteSVM,
    pub trader:       Keypair,
    pub searcher:     Keypair,
    pub input_token:  Token,
    pub output_token: Token,
}

pub fn setup_swap() -> SwapInfo {
    let mut setup_result = setup(SetupParams {
        split_router_default: 4000,
        split_relayer:        2000,
    })
    .expect("setup failed");

    let trader = Keypair::new();
    let input_token = Token::new(&mut setup_result.svm, 6);
    let output_token = Token::new(&mut setup_result.svm, 6);

    input_token.airdrop(&mut setup_result.svm, &setup_result.searcher.pubkey());
    output_token.airdrop(&mut setup_result.svm, &trader.pubkey());

    SwapInfo {
        svm: setup_result.svm,
        trader,
        searcher: setup_result.searcher,
        input_token,
        output_token,
    }
}

#[test]
fn test_swap() {
    // let setup_result = setup(SetupParams {
    //     split_router_default: SPLIT_ROUTER_DEFAULT,
    //     split_relayer:        SPLIT_RELAYER,
    // })
    // .expect("setup failed");
}
