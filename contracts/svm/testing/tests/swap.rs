use {
    anchor_lang::{
        error::ErrorCode as AnchorErrorCode,
        AccountDeserialize,
    },
    anchor_spl::{
        associated_token::{
            get_associated_token_address_with_program_id,
            spl_associated_token_account::instruction::create_associated_token_account_idempotent,
        },
        token::spl_token,
        token_2022::spl_token_2022::{
            self,
            instruction::{
                initialize_account,
                mint_to_checked,
            },
        },
    },
    express_relay::{
        error::ErrorCode,
        state::FEE_SPLIT_PRECISION,
        FeeToken,
        SwapArgs,
    },
    litesvm::LiteSVM,
    solana_sdk::{
        clock::Clock,
        instruction::InstructionError,
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
    pub mint:          Pubkey,
    pub decimals:      u8,
    mint_authority:    Keypair,
    pub token_program: Pubkey,
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

    pub fn token_balance_matches(svm: &mut LiteSVM, account: &Pubkey, amount: u64) -> bool {
        let token_account_option = &mut svm.get_account(account).map(|account| {
            anchor_spl::token_interface::TokenAccount::try_deserialize(&mut account.data.as_slice())
                .unwrap()
        });

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

pub struct SwapSetupParams {
    pub platform_fee_bps:        u64,
    pub token_program_searcher:  Pubkey,
    pub token_decimals_searcher: u8,
    pub token_program_user:      Pubkey,
    pub token_decimals_user:     u8,
}

pub struct SwapSetupResult {
    pub svm:                      LiteSVM,
    pub user:                     Keypair,
    pub searcher:                 Keypair,
    pub token_searcher:           Token,
    pub token_user:               Token,
    pub router_ta_mint_searcher:  Pubkey,
    pub router_ta_mint_user:      Pubkey,
    pub relayer_signer:           Keypair,
    pub secondary_relayer_signer: Keypair,
}

pub fn setup_swap(args: SwapSetupParams) -> SwapSetupResult {
    let SetupResult {
        mut svm,
        admin,
        searcher,
        relayer_signer,
        secondary_relayer_signer,
        ..
    } = setup(None).expect("setup failed");

    let user = Keypair::new();
    let token_searcher = Token::create_mint(
        &mut svm,
        args.token_program_searcher,
        args.token_decimals_searcher,
    );
    let token_user =
        Token::create_mint(&mut svm, args.token_program_user, args.token_decimals_user);

    let set_swap_platform_fee_ix = set_swap_platform_fee_instruction(&admin, args.platform_fee_bps);
    submit_transaction(&mut svm, &[set_swap_platform_fee_ix], &admin, &[&admin]).unwrap();

    token_searcher.airdrop(&mut svm, &searcher.pubkey(), 10.);
    token_user.airdrop(&mut svm, &user.pubkey(), 10.);

    let router = Keypair::new().pubkey();
    let router_ta_mint_searcher = token_searcher.create_token_account(&mut svm, &router);
    let router_ta_mint_user = token_user.create_token_account(&mut svm, &router);

    SwapSetupResult {
        svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_searcher,
        router_ta_mint_user,
        relayer_signer,
        secondary_relayer_signer,
    }
}

#[test]
fn test_swaps() {
    test_swap_fee_mint_searcher(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token_2022::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token_2022::ID,
        token_decimals_user:     6,
    });

    test_swap_fee_mint_searcher(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     8,
    });

    test_swap_fee_mint_user(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token_2022::ID,
        token_decimals_searcher: 5,
        token_program_user:      spl_token::ID,
        token_decimals_user:     7,
    });

    test_swap_fee_mint_user(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 3,
        token_program_user:      spl_token_2022::ID,
        token_decimals_user:     4,
    });
}


fn test_swap_fee_mint_searcher(args: SwapSetupParams) {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_searcher,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(args);

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // searcher token balances
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&searcher.pubkey()),
        token_searcher.get_amount_with_decimals(10.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&user.pubkey()),
        token_user.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&get_express_relay_metadata_key()),
        token_searcher.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        token_searcher.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &router_ta_mint_searcher,
        token_searcher.get_amount_with_decimals(0.),
    ));

    // user token balances
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&searcher.pubkey()),
        token_user.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&user.pubkey()),
        token_user.get_amount_with_decimals(10.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &router_ta_mint_user,
        token_user.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&get_express_relay_metadata_key()),
        token_user.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        token_user.get_amount_with_decimals(0.),
    ));

    // searcher token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 3000,
        fee_token:        FeeToken::Searcher,
    };
    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_searcher,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap();

    // searcher token balances
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&searcher.pubkey()),
        token_searcher.get_amount_with_decimals(9.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&user.pubkey()),
        token_searcher.get_amount_with_decimals(0.6),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&get_express_relay_metadata_key()),
        token_searcher.get_amount_with_decimals(0.08),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        token_searcher.get_amount_with_decimals(0.02),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &router_ta_mint_searcher,
        token_searcher.get_amount_with_decimals(0.3),
    ));

    // user token balances
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&searcher.pubkey()),
        token_user.get_amount_with_decimals(1.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&user.pubkey()),
        token_user.get_amount_with_decimals(9.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&get_express_relay_metadata_key()),
        token_user.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        token_user.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &router_ta_mint_user,
        token_user.get_amount_with_decimals(0.),
    ));
}

fn test_swap_fee_mint_user(args: SwapSetupParams) {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_searcher,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(args);

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // searcher token balances
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&searcher.pubkey()),
        token_searcher.get_amount_with_decimals(10.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&user.pubkey()),
        token_user.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&get_express_relay_metadata_key()),
        token_searcher.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        token_searcher.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &router_ta_mint_searcher,
        token_searcher.get_amount_with_decimals(0.),
    ));

    // user token balances
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&searcher.pubkey()),
        token_user.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&user.pubkey()),
        token_user.get_amount_with_decimals(10.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &router_ta_mint_user,
        token_user.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&get_express_relay_metadata_key()),
        token_user.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        token_user.get_amount_with_decimals(0.),
    ));

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap();

    // searcher token balances
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&searcher.pubkey()),
        token_searcher.get_amount_with_decimals(9.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&user.pubkey()),
        token_searcher.get_amount_with_decimals(1.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&get_express_relay_metadata_key()),
        token_searcher.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_searcher.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        token_searcher.get_amount_with_decimals(0.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &router_ta_mint_searcher,
        token_searcher.get_amount_with_decimals(0.),
    ));

    // user token balances
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&searcher.pubkey()),
        token_user.get_amount_with_decimals(0.75),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&user.pubkey()),
        token_user.get_amount_with_decimals(9.),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&get_express_relay_metadata_key()),
        token_user.get_amount_with_decimals(0.08),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &token_user.get_associated_token_address(&express_relay_metadata.fee_receiver_relayer),
        token_user.get_amount_with_decimals(0.02),
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &router_ta_mint_user,
        token_user.get_amount_with_decimals(0.15),
    ));
}

#[test]
fn test_swap_expired_deadline() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp - 1, // <--- deadline is in the past
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(ErrorCode::DeadlinePassed.into()),
    );
}

#[test]
fn test_swap_invalid_referral_fee_bps() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: (FEE_SPLIT_PRECISION + 1) as u16, // <--- referral fee bps is too high
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(ErrorCode::InvalidReferralFee.into()),
    );
}

#[test]
fn test_swap_fee_calculation_overflow() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        5000, // <--- high platform fee bps
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 5001, // <--- referral fee bps + platform fee bps is more than 100%
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(result.err, 4, InstructionError::ArithmeticOverflow);
}

#[test]
fn test_swap_router_ta_has_wrong_mint() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::Searcher,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user, // <--- router should receive the searcher token
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(AnchorErrorCode::ConstraintTokenMint.into()),
    );
}

#[test]
fn test_swap_searcher_ta_has_wrong_mint() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let third_token = Token::create_mint(&mut svm, spl_token::ID, 6);
    third_token.airdrop(&mut svm, &searcher.pubkey(), 10.);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        Some(third_token.get_associated_token_address(&searcher.pubkey())), // <--- searcher ta (supposed to be of mint_searcher) has the wrong mint
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(AnchorErrorCode::ConstraintTokenMint.into()),
    );
}

#[test]
fn test_swap_searcher_ta_wrong_owner() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        Some(token_searcher.get_associated_token_address(&user.pubkey())), // <--- searcher ta (supposed to be of mint_searcher) has the wrong owner
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(AnchorErrorCode::ConstraintTokenOwner.into()),
    );
}

#[test]
fn test_swap_wrong_express_relay_fee_receiver() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        Keypair::new().pubkey(), // <--- wrong express relay fee receiver
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(AnchorErrorCode::ConstraintTokenOwner.into()),
    );
}

#[test]
fn test_swap_user_ata_mint_user_is_not_ata() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);
    let user_ata_mint_user = token_user.create_token_account(&mut svm, &user.pubkey());

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        Some(user_ata_mint_user), // <--- user ata (of mint_user) is not an ata
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(AnchorErrorCode::ConstraintAssociated.into()),
    );
}

#[test]
fn test_swap_wrong_mint_fee() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_searcher,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_searcher,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        Some(token_searcher.mint), // <--- wrong mint fee
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(AnchorErrorCode::ConstraintRaw.into()),
    );
}


#[test]
fn test_swap_fail_wrong_relayer_signer() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let wrong_relayer_signer = Keypair::new();
    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        wrong_relayer_signer.pubkey(), // <--- wrong relayer signer
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &wrong_relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(AnchorErrorCode::ConstraintHasOne.into()),
    );
}


#[test]
fn test_swap_insufficient_balance_user() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(11.), // <--- more than user has
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(ErrorCode::InsufficientUserFunds.into()),
    );
}

#[test]
fn test_swap_insufficient_balance_searcher() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_searcher,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // searcher token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(11.), // <--- more than searcher has
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::Searcher,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_searcher,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();

    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(ErrorCode::InsufficientSearcherFunds.into()),
    );
}

#[test]
fn test_swap_insufficient_balance_both_user_and_searcher() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(11.), // <--- more than searcher has
        amount_user:      token_user.get_amount_with_decimals(11.),     // <--- more than user has
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    let result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap_err();
    assert_custom_error(
        result.err,
        4,
        InstructionError::Custom(ErrorCode::InsufficientSearcherFunds.into()),
    );
}

#[test]
fn test_swap_exact_balance_user() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(10.),
        amount_user:      token_user.get_amount_with_decimals(10.), // exact balance of user
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        relayer_signer.pubkey(),
    );
    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap();
}


#[test]
fn test_swap_secondary_relayer() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        secondary_relayer_signer,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(10.),
        amount_user:      token_user.get_amount_with_decimals(10.), // exact balance of user
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        secondary_relayer_signer.pubkey(),
    );
    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &secondary_relayer_signer],
    )
    .unwrap();
}

#[test]
fn test_swap_wrong_relayer() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        router_ta_mint_user,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps:        1000,
        token_program_searcher:  spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user:      spl_token::ID,
        token_decimals_user:     6,
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(10.),
        amount_user:      token_user.get_amount_with_decimals(10.), // exact balance of user
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };
    let wrong_relayer_signer = Keypair::new();

    let instructions = build_swap_instructions(
        searcher.pubkey(),
        user.pubkey(),
        None,
        None,
        router_ta_mint_user,
        express_relay_metadata.fee_receiver_relayer,
        token_searcher.mint,
        token_user.mint,
        Some(token_searcher.token_program),
        Some(token_user.token_program),
        swap_args,
        None,
        None,
        wrong_relayer_signer.pubkey(),
    );
    let tx_result = submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &wrong_relayer_signer],
    )
    .expect_err("Transaction should fail");
    assert_custom_error(
        tx_result.err,
        4,
        InstructionError::Custom(AnchorErrorCode::ConstraintHasOne.into()),
    );
}
