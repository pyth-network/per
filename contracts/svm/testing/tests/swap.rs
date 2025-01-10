use {
    anchor_lang::AccountDeserialize,
    anchor_spl::{
        associated_token::{
            get_associated_token_address, get_associated_token_address_with_program_id, spl_associated_token_account::instruction::create_associated_token_account_idempotent
        }, token::{self, spl_token}, token_2022::spl_token_2022::{self, instruction::{initialize_account2, mint_to, mint_to_checked}}
    },
    express_relay::{
        error::ErrorCode,
        FeeToken,
        SwapArgs,
    },
    litesvm::LiteSVM,
    solana_sdk::{
        clock::Clock,
        program_pack::Pack,
        pubkey::Pubkey,
        signature::{
            Keypair,
            Signer,
        },
        system_instruction,
    },
    testing::{
        express_relay::{
            helpers::{
                get_express_relay_metadata,
                get_express_relay_metadata_key,
            },
            set_swap_platform_fee::set_swap_platform_fee_instruction,
            swap::build_swap_instructions,
        },
        helpers::{
            assert_custom_error,
            generate_and_fund_key,
            submit_transaction,
        },
        setup::{
            setup,
            SetupResult,
        },
    },
};

pub struct Token {
    pub mint:       Pubkey,
    pub decimals:   u8,
    mint_authority: Keypair,
    pub token_program: Pubkey 
}

impl Token {
    pub fn airdrop(&self, svm: &mut LiteSVM, destination: &Pubkey) {
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
                &get_associated_token_address_with_program_id(destination, &self.mint, &self.token_program),
                &self.mint_authority.pubkey(),
                &[&self.mint_authority.pubkey()],
                self.get_amount_with_decimals(10f64),
                self.decimals
            ).unwrap()
        ];
        submit_transaction(
            svm,
            &instructions,
            &self.mint_authority,
            &[&self.mint_authority],
        )
        .unwrap();
    }

    pub fn assert_token_balance(svm: &mut LiteSVM, account: &Pubkey, amount: u64) {
        let token_account_option = &mut svm.get_account(account).map(|account| {
            anchor_spl::token_interface::TokenAccount::try_deserialize(&mut account.data.as_slice()).unwrap()
        });

        assert!(
            token_account_option.is_none() && amount == 0
                || token_account_option.unwrap().amount == amount
        );
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
            initialize_account2(&self.token_program, &token_account.pubkey(), &self.mint, owner)
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

    pub fn new(svm: &mut LiteSVM, token_program: Pubkey, decimals: u8) -> Self {
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
        (amount * 10_f64.powi(self.decimals as i32)).floor() as u64
    }

    pub fn get_associated_token_address(&self, owner: &Pubkey) -> Pubkey {
        get_associated_token_address_with_program_id(owner, &self.mint, &self.token_program)
    }
}

pub struct SwapSetupParams{
    pub platform_fee_bps: u64,
    pub input_token_program: Pubkey,
    pub input_token_decimals: u8,
    pub output_token_program: Pubkey,
    pub output_token_decimals: u8,
}

pub struct SwapSetupResult {
    pub svm:              LiteSVM,
    pub trader:           Keypair,
    pub searcher:         Keypair,
    pub input_token:      Token,
    pub output_token:     Token,
    pub router_input_ta:  Pubkey,
    pub router_output_ta: Pubkey,
}

pub fn setup_swap(args: SwapSetupParams) -> SwapSetupResult {
    let SetupResult {
        mut svm,
        admin,
        searcher,
        ..
    } = setup(None).expect("setup failed");

    let trader = Keypair::new();
    let input_token = Token::new(&mut svm, args.input_token_program, args.input_token_decimals);
    let output_token = Token::new(&mut svm, args.output_token_program, args.output_token_decimals);

    let set_swap_platform_fee_ix = set_swap_platform_fee_instruction(&admin, args.platform_fee_bps);
    submit_transaction(&mut svm, &[set_swap_platform_fee_ix], &admin, &[&admin]).unwrap();

    input_token.airdrop(&mut svm, &searcher.pubkey());
    output_token.airdrop(&mut svm, &trader.pubkey());

    let router = Keypair::new().pubkey();
    let router_input_ta = input_token.create_token_account(&mut svm, &router);
    let router_output_ta = output_token.create_token_account(&mut svm, &router);

    SwapSetupResult {
        svm,
        trader,
        searcher,
        input_token,
        output_token,
        router_input_ta,
        router_output_ta,
    }
}

#[test]
fn test_swaps() {
    test_swap(SwapSetupParams {
        platform_fee_bps: 1000,
        input_token_program: spl_token_2022::ID,
        input_token_decimals: 6,
        output_token_program: spl_token_2022::ID,
        output_token_decimals: 6,
    });

    test_swap(SwapSetupParams {
        platform_fee_bps: 1000,
        input_token_program: spl_token::ID,
        input_token_decimals: 6,
        output_token_program: spl_token::ID,
        output_token_decimals: 8,
    });
}


fn test_swap(args : SwapSetupParams) {
    let SwapSetupResult {
        mut svm,
        trader,
        searcher,
        input_token,
        output_token,
        router_input_ta,
        router_output_ta,
    } = setup_swap(args);

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // input token balances
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&searcher.pubkey()),
        input_token.get_amount_with_decimals(10f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&trader.pubkey()),
        output_token.get_amount_with_decimals(0f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&get_express_relay_metadata_key()),
        input_token.get_amount_with_decimals(0f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        input_token.get_amount_with_decimals(0f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &router_input_ta,
        input_token.get_amount_with_decimals(0f64),
    );

    // output token balances
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&searcher.pubkey()),
        output_token.get_amount_with_decimals(0f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&trader.pubkey()),
        output_token.get_amount_with_decimals(10f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&get_express_relay_metadata_key()),
        output_token.get_amount_with_decimals(0f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        output_token.get_amount_with_decimals(0f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &router_output_ta,
        output_token.get_amount_with_decimals(0f64),
    );

    // input token fee
    let swap_args = SwapArgs {
        deadline:         i64::MAX,
        amount_input:     input_token.get_amount_with_decimals(1f64),
        amount_output:    output_token.get_amount_with_decimals(1f64),
        referral_fee_bps: 3000,
        fee_token:        FeeToken::Input,
    };
    let instructions = build_swap_instructions(
        searcher.pubkey(),
        trader.pubkey(),
        None,
        None,
        router_input_ta,
        express_relay_metadata.fee_receiver_relayer,
        input_token.mint,
        output_token.mint,
        Some(input_token.token_program),
        Some(output_token.token_program),
        swap_args,
    );
    submit_transaction(&mut svm, &instructions, &searcher, &[&searcher, &trader]).unwrap();

    // input token balances
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&searcher.pubkey()),
        input_token.get_amount_with_decimals(9f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&trader.pubkey()),
        output_token.get_amount_with_decimals(0.6f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&get_express_relay_metadata_key()),
        input_token.get_amount_with_decimals(0.08f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        input_token.get_amount_with_decimals(0.02f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &router_input_ta,
        input_token.get_amount_with_decimals(0.3f64),
    );

    // output token balances
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&searcher.pubkey()),
        output_token.get_amount_with_decimals(1f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&trader.pubkey()),
        output_token.get_amount_with_decimals(9f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&get_express_relay_metadata_key()),
        output_token.get_amount_with_decimals(0f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        output_token.get_amount_with_decimals(0f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &router_output_ta,
        output_token.get_amount_with_decimals(0f64),
    );

    // output token fee
    let swap_args = SwapArgs {
        deadline:         i64::MAX,
        amount_input:     input_token.get_amount_with_decimals(1f64),
        amount_output:    output_token.get_amount_with_decimals(1f64),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::Output,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        trader.pubkey(),
        None,
        None,
        router_output_ta,
        express_relay_metadata.fee_receiver_relayer,
        input_token.mint,
        output_token.mint,
        Some(input_token.token_program),
        Some(output_token.token_program),
        swap_args,
    );
    submit_transaction(&mut svm, &instructions, &searcher, &[&searcher, &trader]).unwrap();

    // input token balances
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&searcher.pubkey()),
        input_token.get_amount_with_decimals(8f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&trader.pubkey()),
        output_token.get_amount_with_decimals(1.6f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&get_express_relay_metadata_key()),
        input_token.get_amount_with_decimals(0.08f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &input_token.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        input_token.get_amount_with_decimals(0.02f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &router_input_ta,
        input_token.get_amount_with_decimals(0.3f64),
    );

    // output token balances
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&searcher.pubkey()),
        output_token.get_amount_with_decimals(1.75f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&trader.pubkey()),
        output_token.get_amount_with_decimals(8f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&get_express_relay_metadata_key()),
        output_token.get_amount_with_decimals(0.08f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &output_token.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        output_token.get_amount_with_decimals(0.02f64),
    );
    Token::assert_token_balance(
        &mut svm,
        &router_output_ta,
        output_token.get_amount_with_decimals(0.15f64),
    );
}

#[test]
fn test_swap_expired_deadline() {
    let SwapSetupResult {
        mut svm,
        trader,
        searcher,
        input_token,
        output_token,
        router_output_ta,
        ..
    } = setup_swap(SwapSetupParams { platform_fee_bps: 1000, input_token_program: spl_token::ID, input_token_decimals: 6, output_token_program: spl_token::ID, output_token_decimals: 6 });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // output token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp - 1,
        amount_input:     input_token.get_amount_with_decimals(1f64),
        amount_output:    output_token.get_amount_with_decimals(1f64),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::Output,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        trader.pubkey(),
        None,
        None,
        router_output_ta,
        express_relay_metadata.fee_receiver_relayer,
        input_token.mint,
        output_token.mint,
        Some(input_token.token_program),
        Some(output_token.token_program),
        swap_args,
    );
    let result =
        submit_transaction(&mut svm, &instructions, &searcher, &[&searcher, &trader]).unwrap_err();
    assert_custom_error(result.err, 4, ErrorCode::DeadlinePassed.into());

    // now deadine is now
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_input:     input_token.get_amount_with_decimals(1f64),
        amount_output:    output_token.get_amount_with_decimals(1f64),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::Output,
    };
    let instructions = build_swap_instructions(
        searcher.pubkey(),
        trader.pubkey(),
        None,
        None,
        router_output_ta,
        express_relay_metadata.fee_receiver_relayer,
        input_token.mint,
        output_token.mint,
        Some(input_token.token_program),
        Some(output_token.token_program),
        swap_args,
    );
    submit_transaction(&mut svm, &instructions, &searcher, &[&searcher, &trader]).unwrap();
}
