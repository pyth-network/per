use {
    crate::{
        express_relay::{
            initialize::initialize_instruction as initialize_express_relay_instruction,
            set_secondary_relayer::set_secondary_relayer_instruction,
        },
        helpers::{
            generate_and_fund_key,
            submit_transaction,
        },
    },
    solana_sdk::{
        signature::Keypair,
        signer::Signer,
        transaction::TransactionError,
    },
};

pub const SPLIT_ROUTER_DEFAULT: u64 = 4000;
pub const SPLIT_RELAYER: u64 = 2000;

pub struct SetupParams {
    pub split_router_default: u64,
    pub split_relayer:        u64,
}

pub struct SetupResult {
    pub svm:                      litesvm::LiteSVM,
    pub payer:                    Keypair,
    pub admin:                    Keypair,
    pub relayer_signer:           Keypair,
    pub secondary_relayer_signer: Keypair,
    pub fee_receiver_relayer:     Keypair,
    pub split_router_default:     u64,
    pub split_relayer:            u64,
    pub searcher:                 Keypair,
}

impl Default for SetupParams {
    fn default() -> Self {
        Self {
            split_router_default: SPLIT_ROUTER_DEFAULT,
            split_relayer:        SPLIT_RELAYER,
        }
    }
}

pub fn setup(params: Option<SetupParams>) -> Result<SetupResult, TransactionError> {
    let SetupParams {
        split_router_default,
        split_relayer,
    } = params.unwrap_or_default();

    let mut svm = litesvm::LiteSVM::new();
    svm.add_program_from_file(express_relay::ID, "../target/deploy/express_relay.so")
        .unwrap();
    svm.add_program_from_file(dummy::ID, "../target/deploy/dummy.so")
        .unwrap();

    let payer = generate_and_fund_key(&mut svm);
    let admin = generate_and_fund_key(&mut svm);
    let relayer_signer = generate_and_fund_key(&mut svm);
    let secondary_relayer_signer = generate_and_fund_key(&mut svm);
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

    let set_secondary_relayer_ix =
        set_secondary_relayer_instruction(&admin, secondary_relayer_signer.pubkey());

    let tx_result_express_relay =
        submit_transaction(&mut svm, &[initialize_express_relay_ix], &payer, &[&payer]);
    let tx_result_secondary_relayer =
        submit_transaction(&mut svm, &[set_secondary_relayer_ix], &admin, &[&admin]);

    match tx_result_express_relay.and(tx_result_secondary_relayer) {
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
        secondary_relayer_signer,
    })
}
