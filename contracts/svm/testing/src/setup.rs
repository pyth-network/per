use {
    crate::{
        dummy::do_nothing::do_nothing_instruction,
        express_relay::{
            initialize::initialize_instruction as initialize_express_relay_instruction,
            swap::{
                swap_instruction,
                ReferralFeeInfo,
                TokenAccounts,
                TokenInfo,
            },
        },
        helpers::{
            create_mint,
            generate_and_fund_key,
            initialize_ata,
            mint_tokens,
            submit_transaction,
        },
    },
    anchor_spl::{
        associated_token::get_associated_token_address,
        token::ID as spl_token,
    },
    express_relay::{
        state::SEED_SWAP,
        SwapArgs,
    },
    solana_sdk::{
        instruction::Instruction,
        native_token::LAMPORTS_PER_SOL,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        transaction::TransactionError,
    },
};

pub struct SetupParams {
    pub split_router_default: u64,
    pub split_relayer:        u64,
}

pub struct SetupResult {
    pub svm:                  litesvm::LiteSVM,
    pub payer:                Keypair,
    pub admin:                Keypair,
    pub relayer_signer:       Keypair,
    pub fee_receiver_relayer: Keypair,
    pub split_router_default: u64,
    pub split_relayer:        u64,
    pub searcher:             Keypair,
}

pub fn setup(params: SetupParams) -> Result<SetupResult, TransactionError> {
    let SetupParams {
        split_router_default,
        split_relayer,
    } = params;

    let mut svm = litesvm::LiteSVM::new();
    svm.add_program_from_file(express_relay::ID, "../target/deploy/express_relay.so")
        .unwrap();
    svm.add_program_from_file(dummy::ID, "../target/deploy/dummy.so")
        .unwrap();

    let payer = generate_and_fund_key(&mut svm);
    let admin = generate_and_fund_key(&mut svm);
    let relayer_signer = generate_and_fund_key(&mut svm);
    let fee_receiver_relayer = generate_and_fund_key(&mut svm);

    let searcher = generate_and_fund_key(&mut svm);

    let initialize_express_relay_ix = initialize_express_relay_instruction(
        &payer,
        admin.pubkey(),
        relayer_signer.pubkey(),
        fee_receiver_relayer.pubkey(),
        split_router_default,
        split_relayer,
    );

    let tx_result_express_relay =
        submit_transaction(&mut svm, &[initialize_express_relay_ix], &payer, &[&payer]);
    match tx_result_express_relay {
        Ok(_) => (),
        Err(e) => return Err(e.err),
    };

    Ok(SetupResult {
        svm,
        payer,
        admin,
        relayer_signer,
        fee_receiver_relayer,
        split_router_default,
        split_relayer,
        searcher,
    })
}

// TODO: perhaps refactor to return Swap Info separately from the BidInfo (i.e. trader, tas, mints?)
pub struct BidInfo {
    pub svm:                  litesvm::LiteSVM,
    pub relayer_signer:       Keypair,
    pub searcher:             Keypair,
    pub fee_receiver_relayer: Keypair,
    pub router:               Pubkey,
    pub permission_key:       Pubkey,
    pub bid_amount:           u64,
    pub deadline:             i64,
    pub ixs:                  Vec<Instruction>,
    pub trader:               Option<Keypair>,
    pub tas_searcher:         Option<TokenAccounts>,
    pub tas_trader:           Option<TokenAccounts>,
    pub tas_router:           Option<TokenAccounts>,
}

pub const SPLIT_ROUTER_DEFAULT: u64 = 4000;
pub const SPLIT_RELAYER: u64 = 2000;

pub struct SwapInfo {
    pub token_info_input:        TokenInfo,
    pub token_info_output:       TokenInfo,
    pub token_accounts_searcher: TokenAccounts,
    pub token_accounts_trader:   TokenAccounts,
    pub token_accounts_router:   TokenAccounts,
    pub referral_fee_info:       ReferralFeeInfo,
}

pub enum IxsType {
    Dummy,
    Swap(SwapArgs),
}

pub fn setup_bid(ixs_types: IxsType) -> BidInfo {
    let setup_result = setup(SetupParams {
        split_router_default: SPLIT_ROUTER_DEFAULT,
        split_relayer:        SPLIT_RELAYER,
    })
    .expect("setup failed");

    let mut svm = setup_result.svm;
    let relayer_signer = setup_result.relayer_signer;
    let searcher = setup_result.searcher;
    let mut trader: Option<Keypair> = None;
    let mut tas_searcher: Option<TokenAccounts> = None;
    let mut tas_trader: Option<TokenAccounts> = None;
    let mut tas_router: Option<TokenAccounts> = None;
    let fee_receiver_relayer = setup_result.fee_receiver_relayer;
    let mut permission_key = Keypair::new().pubkey();
    let router = Keypair::new().pubkey();
    let bid_amount = LAMPORTS_PER_SOL;
    let deadline: i64 = 100_000_000_000;
    let ixs = match ixs_types {
        IxsType::Dummy => [do_nothing_instruction(&searcher, permission_key, router)].to_vec(),
        IxsType::Swap(swap_args) => {
            trader = Some(generate_and_fund_key(&mut svm));
            let mint_input = Keypair::new();
            let mint_output = Keypair::new();
            let mint_input_pk = mint_input.pubkey();
            let mint_output_pk = mint_output.pubkey();
            let token_program_input = spl_token;
            let token_program_output = spl_token;
            let swap_info = setup_swap_info(
                mint_input,
                mint_output,
                swap_args.amount_input,
                swap_args.amount_output,
                token_program_input,
                token_program_output,
                searcher.pubkey(),
                trader.as_ref().unwrap().pubkey(),
                router,
                swap_args.referral_fee_input,
                swap_args.referral_fee_ppm,
            );
            setup_tokens(
                &mut svm,
                &swap_info,
                &searcher,
                &trader.as_ref().unwrap().pubkey(),
                &router,
            );
            permission_key = Pubkey::find_program_address(
                &[
                    SEED_SWAP,
                    trader.as_ref().unwrap().pubkey().as_ref(),
                    mint_input_pk.as_ref(),
                    mint_output_pk.as_ref(),
                    &swap_args.nonce.to_le_bytes(),
                ],
                &express_relay::id(),
            )
            .0;

            let ixs_swap = [swap_instruction(
                &searcher,
                trader.as_ref().unwrap(),
                router,
                permission_key,
                swap_info.token_info_input,
                swap_info.token_info_output,
                &swap_info.token_accounts_searcher,
                &swap_info.token_accounts_trader,
                &swap_info.token_accounts_router,
                swap_info.referral_fee_info,
                swap_args.nonce,
            )]
            .to_vec();
            tas_searcher = Some(swap_info.token_accounts_searcher);
            tas_trader = Some(swap_info.token_accounts_trader);
            tas_router = Some(swap_info.token_accounts_router);
            ixs_swap
        }
    };

    BidInfo {
        svm,
        relayer_signer,
        searcher,
        fee_receiver_relayer,
        router,
        permission_key,
        bid_amount,
        deadline,
        ixs,
        trader,
        tas_searcher,
        tas_trader,
        tas_router,
    }
}

// as default, this method sets all token accounts to ATAs.
pub fn setup_swap_info(
    mint_input: Keypair,
    mint_output: Keypair,
    amount_input: u64,
    amount_output: u64,
    token_program_input: Pubkey,
    token_program_output: Pubkey,
    searcher: Pubkey,
    trader: Pubkey,
    router: Pubkey,
    referral_fee_input: bool,
    referral_fee_ppm: u64,
) -> SwapInfo {
    let mint_input_pk = mint_input.pubkey();
    let mint_output_pk = mint_output.pubkey();

    let token_info_input = TokenInfo {
        mint:    mint_input,
        amount:  amount_input,
        program: token_program_input,
    };
    let token_info_output = TokenInfo {
        mint:    mint_output,
        amount:  amount_output,
        program: token_program_output,
    };
    let token_accounts_searcher = TokenAccounts {
        input:  get_associated_token_address(&searcher, &mint_input_pk),
        output: get_associated_token_address(&searcher, &mint_output_pk),
    };
    let token_accounts_trader = TokenAccounts {
        input:  get_associated_token_address(&trader, &mint_input_pk),
        output: get_associated_token_address(&trader, &mint_output_pk),
    };
    let token_accounts_router = TokenAccounts {
        input:  get_associated_token_address(&router, &mint_input_pk),
        output: get_associated_token_address(&router, &mint_output_pk),
    };
    let referral_fee_info = ReferralFeeInfo {
        input: referral_fee_input,
        ppm:   referral_fee_ppm,
    };

    SwapInfo {
        token_info_input,
        token_info_output,
        token_accounts_searcher,
        token_accounts_trader,
        token_accounts_router,
        referral_fee_info,
    }
}

pub fn setup_tokens(
    svm: &mut litesvm::LiteSVM,
    swap_info: &SwapInfo,
    searcher: &Keypair,
    trader: &Pubkey,
    router: &Pubkey,
) {
    let searcher_ta_info_input = TokenAccountInfo {
        token_account: swap_info.token_accounts_searcher.input,
        owner:         searcher.pubkey(),
    };
    let trader_ta_info_input = TokenAccountInfo {
        token_account: swap_info.token_accounts_trader.input,
        owner:         *trader,
    };
    let router_ta_info_input = TokenAccountInfo {
        token_account: swap_info.token_accounts_router.input,
        owner:         *router,
    };
    setup_token(
        svm,
        &swap_info.token_info_input,
        searcher,
        &searcher_ta_info_input,
        &trader_ta_info_input,
        &router_ta_info_input,
    );

    let searcher_ta_info_output = TokenAccountInfo {
        token_account: swap_info.token_accounts_searcher.output,
        owner:         searcher.pubkey(),
    };
    let trader_ta_info_output = TokenAccountInfo {
        token_account: swap_info.token_accounts_trader.output,
        owner:         *trader,
    };
    let router_ta_info_output = TokenAccountInfo {
        token_account: swap_info.token_accounts_router.output,
        owner:         *router,
    };
    setup_token(
        svm,
        &swap_info.token_info_output,
        searcher,
        &searcher_ta_info_output,
        &trader_ta_info_output,
        &router_ta_info_output,
    );
}

pub struct TokenAccountInfo {
    pub token_account: Pubkey,
    pub owner:         Pubkey,
}

pub fn setup_token(
    svm: &mut litesvm::LiteSVM,
    token_info: &TokenInfo,
    searcher: &Keypair,
    searcher_ta_info: &TokenAccountInfo,
    trader_ta_info: &TokenAccountInfo,
    router_ta_info: &TokenAccountInfo,
) {
    create_mint(svm, &token_info.mint, searcher, &token_info.program);
    for info in [searcher_ta_info, trader_ta_info, router_ta_info].iter() {
        initialize_ata(
            svm,
            &token_info.mint.pubkey(),
            &info.owner,
            &token_info.program,
            searcher,
        );
        mint_tokens(
            svm,
            &token_info.mint.pubkey(),
            &info.token_account,
            token_info.amount * 10,
            searcher,
            &token_info.program,
        );
    }
}
