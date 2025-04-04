use {
    anchor_lang::error::ErrorCode as AnchorErrorCode,
    anchor_spl::{
        token::spl_token,
        token_2022::spl_token_2022::{
            self,
        },
    },
    express_relay::{
        error::ErrorCode,
        state::FEE_SPLIT_PRECISION,
        FeeToken,
        SwapArgs,
        SwapV2Args,
    },
    litesvm::LiteSVM,
    solana_sdk::{
        clock::Clock,
        instruction::InstructionError,
        pubkey::Pubkey,
        signature::{
            Keypair,
            Signer,
        },
    },
    testing::{
        assert_all_token_balances,
        express_relay::{
            helpers::{
                get_express_relay_metadata,
                get_express_relay_metadata_key,
            },
            set_swap_platform_fee::set_swap_platform_fee_instruction,
            swap::{
                build_swap_instructions,
                SwapParamOverride,
                SwapParams,
            },
        },
        helpers::{
            assert_custom_error,
            submit_transaction,
        },
        setup::{
            setup,
            SetupParams,
            SetupResult,
        },
        token::Token,
    },
};

pub struct SwapSetupParams {
    pub platform_fee_bps:        u64,
    pub token_program_searcher:  Pubkey,
    pub token_decimals_searcher: u8,
    pub token_program_user:      Pubkey,
    pub token_decimals_user:     u8,
    pub program_setup_params:    SetupParams,
}
impl Default for SwapSetupParams {
    fn default() -> Self {
        Self {
            platform_fee_bps:        1000,
            token_program_searcher:  spl_token::ID,
            token_decimals_searcher: 6,
            token_program_user:      spl_token::ID,
            token_decimals_user:     6,
            program_setup_params:    Default::default(),
        }
    }
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
    } = setup(Some(args.program_setup_params)).expect("setup failed");

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
        token_program_searcher: spl_token_2022::ID,
        token_decimals_searcher: 6,
        token_program_user: spl_token_2022::ID,
        token_decimals_user: 6,
        ..Default::default()
    });

    test_swap_fee_mint_searcher(SwapSetupParams {
        token_program_searcher: spl_token::ID,
        token_decimals_searcher: 6,
        token_program_user: spl_token::ID,
        token_decimals_user: 8,
        ..Default::default()
    });

    test_swap_fee_mint_user(SwapSetupParams {
        token_program_searcher: spl_token_2022::ID,
        token_decimals_searcher: 5,
        token_program_user: spl_token::ID,
        token_decimals_user: 7,
        ..Default::default()
    });

    test_swap_fee_mint_user(SwapSetupParams {
        token_program_searcher: spl_token::ID,
        token_decimals_searcher: 3,
        token_program_user: spl_token_2022::ID,
        token_decimals_user: 4,
        ..Default::default()
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
    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_searcher,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });
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

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });
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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp - 1, // <--- deadline is in the past
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });

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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: (FEE_SPLIT_PRECISION + 1) as u16, // <--- referral fee bps is too high
        fee_token:        FeeToken::User,
    };
    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });

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
        platform_fee_bps: 5000, // <--- high platform fee bps
        ..Default::default()
    });

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 5001, // <--- referral fee bps + platform fee bps is more than 100%
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        overrides: Default::default(),
        swap_args,
        relayer_signer: relayer_signer.pubkey(),
    });
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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::Searcher,
    };
    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user, // <--- router should receive the searcher token
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });
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
    } = setup_swap(Default::default());

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


    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: SwapParamOverride {
            // searcher ta (supposed to be of mint_searcher) has the wrong mint,
            searcher_ta_mint_searcher: Some(
                third_token.get_associated_token_address(&searcher.pubkey()),
            ),
            ..Default::default()
        },
        relayer_signer: relayer_signer.pubkey(),
    });

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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: SwapParamOverride {
            // searcher ta (supposed to be of mint_searcher) has the wrong owner
            searcher_ta_mint_searcher: Some(
                token_searcher.get_associated_token_address(&user.pubkey()),
            ),
            ..Default::default()
        },
        relayer_signer: relayer_signer.pubkey(),
    });

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
    } = setup_swap(Default::default());

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: Keypair::new().pubkey(), // wrong express relay fee receiver
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });
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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);
    let user_ata_mint_user = token_user.create_token_account(&mut svm, &user.pubkey());

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: SwapParamOverride {
            // user ata (of mint_user) is not an ata
            user_ata_mint_user: Some(user_ata_mint_user),
            ..Default::default()
        },
        relayer_signer: relayer_signer.pubkey(),
    });

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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_searcher,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: SwapParamOverride {
            mint_fee: Some(token_searcher.mint), // wrong mint fee,
            ..Default::default()
        },
        relayer_signer: relayer_signer.pubkey(),
    });

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
    } = setup_swap(Default::default());

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

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: wrong_relayer_signer.pubkey(),
    });

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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(1.),
        amount_user:      token_user.get_amount_with_decimals(11.), // <--- more than user has
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });
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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // searcher token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(11.), // <--- more than searcher has
        amount_user:      token_user.get_amount_with_decimals(1.),
        referral_fee_bps: 1500,
        fee_token:        FeeToken::Searcher,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_searcher,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });

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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(11.), // <--- more than searcher has
        amount_user:      token_user.get_amount_with_decimals(11.),     // <--- more than user has
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });
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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(10.),
        amount_user:      token_user.get_amount_with_decimals(10.), // exact balance of user
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });
    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap();
}

#[test]
fn test_no_router_ata_check_when_fee_is_zero() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        relayer_signer,
        ..
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(10.),
        amount_user:      token_user.get_amount_with_decimals(10.), // exact balance of user
        referral_fee_bps: 0,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        // random mutable key instead of actual router token account
        router_fee_receiver_ta: Pubkey::new_unique(),
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });

    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap();
}

#[test]
fn test_no_express_relay_and_relayer_fee_receiver_ata_check_when_fee_is_zero() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        relayer_signer,
        router_ta_mint_user,
        ..
    } = setup_swap(SwapSetupParams {
        platform_fee_bps: 0, // no platform fee
        ..Default::default()
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

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: SwapParamOverride {
            express_relay_fee_receiver_ata: Some(Pubkey::new_unique()),
            relayer_fee_receiver_ata: Some(Pubkey::new_unique()),
            ..Default::default()
        },
        relayer_signer: relayer_signer.pubkey(),
    });
    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap();
}

#[test]
fn test_relayer_fee_receiver_non_ata() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        relayer_signer,
        router_ta_mint_user,
        ..
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(10.),
        amount_user:      token_user.get_amount_with_decimals(10.), // exact balance of user
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let relayer_fee_receiver_ta =
        token_user.create_token_account(&mut svm, &express_relay_metadata.fee_receiver_relayer);

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: SwapParamOverride {
            relayer_fee_receiver_ata: Some(relayer_fee_receiver_ta),
            ..Default::default()
        },
        relayer_signer: relayer_signer.pubkey(),
    });
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
        InstructionError::Custom(AnchorErrorCode::AccountNotAssociatedTokenAccount.into()),
    );
}

#[test]
fn test_express_relay_fee_receiver_non_ata() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        relayer_signer,
        router_ta_mint_user,
        ..
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(10.),
        amount_user:      token_user.get_amount_with_decimals(10.), // exact balance of user
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let express_relay_fee_receiver_ta =
        token_user.create_token_account(&mut svm, &get_express_relay_metadata_key());

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: SwapParamOverride {
            express_relay_fee_receiver_ata: Some(express_relay_fee_receiver_ta),
            ..Default::default()
        },
        relayer_signer: relayer_signer.pubkey(),
    });
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
        InstructionError::Custom(AnchorErrorCode::AccountNotAssociatedTokenAccount.into()),
    );
}

#[test]
fn test_no_relayer_fee_receiver_ata_check_when_fee_is_zero() {
    let SwapSetupResult {
        mut svm,
        user,
        searcher,
        token_searcher,
        token_user,
        relayer_signer,
        router_ta_mint_user,
        ..
    } = setup_swap(SwapSetupParams {
        program_setup_params: SetupParams {
            split_relayer: 0, // no relayer fee
            ..Default::default()
        },
        ..Default::default()
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

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: SwapParamOverride {
            relayer_fee_receiver_ata: Some(relayer_signer.pubkey()),
            ..Default::default()
        },
        relayer_signer: relayer_signer.pubkey(),
    });
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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // user token fee
    let swap_args = SwapArgs {
        deadline:         svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:  token_searcher.get_amount_with_decimals(10.),
        amount_user:      token_user.get_amount_with_decimals(10.), // exact balance of user
        referral_fee_bps: 1500,
        fee_token:        FeeToken::User,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: secondary_relayer_signer.pubkey(),
    });


    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &secondary_relayer_signer],
    )
    .unwrap();
}

#[test]
fn test_swap_v2_mint_searcher() {
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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // searcher token balances
    assert_all_token_balances!(
        &mut svm,
        token_searcher,
        {
            associated: {
                searcher.pubkey() => 10.0,
                user.pubkey() => 0.0,
                get_express_relay_metadata_key() => 0.0,
                express_relay_metadata.fee_receiver_relayer => 0.0,
            },
            raw: {
                router_ta_mint_searcher => 0.0,
            }
        }
    );

    // user token balances
    assert_all_token_balances!(
        &mut svm,
        token_user,
        {
            associated: {
                searcher.pubkey() => 0.0,
                user.pubkey() => 10.0,
                get_express_relay_metadata_key() => 0.0,
                express_relay_metadata.fee_receiver_relayer => 0.0,
            },
            raw: {
                router_ta_mint_user => 0.0,
            }
        }
    );


    // searcher token fee
    let swap_args = SwapV2Args {
        deadline:              svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:       token_searcher.get_amount_with_decimals(1.),
        amount_user:           token_user.get_amount_with_decimals(1.),
        referral_fee_ppm:      30_000_000,
        fee_token:             FeeToken::Searcher,
        swap_platform_fee_ppm: 20_000_000,
    };
    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_searcher,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: SwapParamOverride {
            ..Default::default()
        },
        relayer_signer: relayer_signer.pubkey(),
    });
    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap();

    // searcher token balances
    assert_all_token_balances!(
        &mut svm,
        token_searcher,
        {
            associated: {
                searcher.pubkey() => 9.0,
                user.pubkey() => 0.5,
                get_express_relay_metadata_key() => 0.16,
                express_relay_metadata.fee_receiver_relayer => 0.04,
            },
            raw: {
                router_ta_mint_searcher => 0.3,
            }
        }
    );

    // user token balances
    assert_all_token_balances!(
        &mut svm,
        token_user,
        {
            associated: {
                searcher.pubkey() => 1.0,
                user.pubkey() => 9.0,
                get_express_relay_metadata_key() => 0.0,
                express_relay_metadata.fee_receiver_relayer => 0.0,
            },
            raw: {
                router_ta_mint_user => 0.0,
            }
        }
    );
}

#[test]
fn test_swap_v2_mint_user() {
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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    // searcher token balances
    assert_all_token_balances!(
        &mut svm,
        token_searcher,
        {
            associated: {
                searcher.pubkey() => 10.0,
                user.pubkey() => 0.0,
                get_express_relay_metadata_key() => 0.0,
                express_relay_metadata.fee_receiver_relayer => 0.0,
            },
            raw: {
                router_ta_mint_searcher => 0.0,
            }
        }
    );


    // user token balances
    assert_all_token_balances!(
        &mut svm,
        token_user,
        {
            associated: {
                searcher.pubkey() => 0.0,
                user.pubkey() => 10.0,
                get_express_relay_metadata_key() => 0.0,
                express_relay_metadata.fee_receiver_relayer => 0.0,
            },
            raw: {
                router_ta_mint_user => 0.0,
            }
        }
    );

    let swap_args = SwapV2Args {
        deadline:              svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:       token_searcher.get_amount_with_decimals(1.),
        amount_user:           token_user.get_amount_with_decimals(1.),
        referral_fee_ppm:      15_000_000,
        fee_token:             FeeToken::User,
        swap_platform_fee_ppm: 20_000_000,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });
    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap();

    // searcher token balances
    assert_all_token_balances!(
        &mut svm,
        token_searcher,
        {
            associated: {
                searcher.pubkey() => 9.,
                user.pubkey() => 1.,
                get_express_relay_metadata_key() => 0.,
                express_relay_metadata.fee_receiver_relayer => 0.,
            },
            raw: {
                router_ta_mint_searcher => 0.,
            }
        }
    );

    // user token balances
    assert_all_token_balances!(
        &mut svm,
        token_user,
        {
            associated: {
                searcher.pubkey() => 0.65,
                user.pubkey() => 9.0,
                get_express_relay_metadata_key() => 0.16,
                express_relay_metadata.fee_receiver_relayer => 0.04,
            },
            raw: {
                router_ta_mint_user => 0.15,
            }
        }
    );
}

#[test]
fn test_swap_v2_mint_user_ppm_fee() {
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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    token_searcher.airdrop(&mut svm, &searcher.pubkey(), 99990.);
    token_user.airdrop(&mut svm, &user.pubkey(), 99990.);

    // searcher token balances
    assert_all_token_balances!(
        &mut svm,
        token_searcher,
        {
            associated: {
                searcher.pubkey() => 100000.0,
                user.pubkey() => 0.0,
                get_express_relay_metadata_key() => 0.0,
                express_relay_metadata.fee_receiver_relayer => 0.0,
            },
            raw: {
                router_ta_mint_searcher => 0.0,
            }
        }
    );


    // user token balances
    assert_all_token_balances!(
        &mut svm,
        token_user,
        {
            associated: {
                searcher.pubkey() => 0.0,
                user.pubkey() => 100000.0,
                get_express_relay_metadata_key() => 0.0,
                express_relay_metadata.fee_receiver_relayer => 0.0,
            },
            raw: {
                router_ta_mint_user => 0.0,
            }
        }
    );

    let swap_args = SwapV2Args {
        deadline:              svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:       token_searcher.get_amount_with_decimals(10000.),
        amount_user:           token_user.get_amount_with_decimals(10000.),
        referral_fee_ppm:      1589,
        fee_token:             FeeToken::User,
        swap_platform_fee_ppm: 2222,
    };

    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_user,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: Default::default(),
        relayer_signer: relayer_signer.pubkey(),
    });
    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap();

    // searcher token balances
    assert_all_token_balances!(
        &mut svm,
        token_searcher,
        {
            associated: {
                searcher.pubkey() => 90000.,
                user.pubkey() => 10000.,
                get_express_relay_metadata_key() => 0.,
                express_relay_metadata.fee_receiver_relayer => 0.,
            },
            raw: {
                router_ta_mint_searcher => 0.,
            }
        }
    );

    // user token balances
    assert_all_token_balances!(
        &mut svm,
        token_user,
        {
            associated: {
                searcher.pubkey() => 9999.618900,
                user.pubkey() => 90000.0,
                get_express_relay_metadata_key() => 0.177760,
                express_relay_metadata.fee_receiver_relayer => 0.044440,
            },
            raw: {
                router_ta_mint_user => 0.158900,
            }
        }
    );
}

#[test]
fn test_swap_v2_mint_searcher_ppm_fee() {
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
    } = setup_swap(Default::default());

    let express_relay_metadata = get_express_relay_metadata(&mut svm);

    token_searcher.airdrop(&mut svm, &searcher.pubkey(), 99990.);
    token_user.airdrop(&mut svm, &user.pubkey(), 99990.);

    // searcher token balances

    // searcher token balances
    assert_all_token_balances!(
        &mut svm,
        token_searcher,
        {
            associated: {
                searcher.pubkey() => 100000.0,
                user.pubkey() => 0.0,
                get_express_relay_metadata_key() => 0.0,
                express_relay_metadata.fee_receiver_relayer => 0.0,
            },
            raw: {
                router_ta_mint_searcher => 0.0,
            }
        }
    );


    // user token balances
    assert_all_token_balances!(
        &mut svm,
        token_user,
        {
            associated: {
                searcher.pubkey() => 0.0,
                user.pubkey() => 100000.0,
                get_express_relay_metadata_key() => 0.0,
                express_relay_metadata.fee_receiver_relayer => 0.0,
            },
            raw: {
                router_ta_mint_user => 0.0,
            }
        }
    );

    // searcher token fee
    let swap_args = SwapV2Args {
        deadline:              svm.get_sysvar::<Clock>().unix_timestamp,
        amount_searcher:       token_searcher.get_amount_with_decimals(10000.),
        amount_user:           token_user.get_amount_with_decimals(10000.),
        referral_fee_ppm:      1589,
        fee_token:             FeeToken::Searcher,
        swap_platform_fee_ppm: 2222,
    };
    let instructions = build_swap_instructions(SwapParams {
        searcher: searcher.pubkey(),
        user: user.pubkey(),
        router_fee_receiver_ta: router_ta_mint_searcher,
        fee_receiver_relayer: express_relay_metadata.fee_receiver_relayer,
        token_searcher: token_searcher.clone(),
        token_user: token_user.clone(),
        swap_args,
        overrides: SwapParamOverride {
            ..Default::default()
        },
        relayer_signer: relayer_signer.pubkey(),
    });
    submit_transaction(
        &mut svm,
        &instructions,
        &searcher,
        &[&searcher, &user, &relayer_signer],
    )
    .unwrap();

    // searcher token balances
    assert_all_token_balances!(
        &mut svm,
        token_searcher,
        {
            associated: {
                searcher.pubkey() => 90000.0,
                user.pubkey() => 9999.618900,
                get_express_relay_metadata_key() => 0.177760,
                express_relay_metadata.fee_receiver_relayer => 0.044440,
            },
            raw: {
                router_ta_mint_searcher => 0.158900,
            }
        }
    );

    // user token balances
    assert_all_token_balances!(
        &mut svm,
        token_user,
        {
            associated: {
                searcher.pubkey() => 10000.,
                user.pubkey() => 90000.,
                get_express_relay_metadata_key() => 0.,
                express_relay_metadata.fee_receiver_relayer => 0.,
            },
            raw: {
                router_ta_mint_user => 0.,
            }
        }
    );
}
