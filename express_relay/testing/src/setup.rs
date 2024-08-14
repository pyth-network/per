use crate::helpers::LAMPORTS_PER_SOL;
use crate::{express_relay::initialize::get_initialize_instruction as get_initialize_express_relay_instruction, helpers::submit_transaction};
use crate::dummy::initialize::get_initialize_instruction as get_initialize_dummy_instruction;
use solana_sdk::{signature::Keypair, signer::Signer};

pub struct SetupParams {
    pub split_protocol_default: u64,
    pub split_relayer: u64,
}

pub struct SetupResult {
    pub svm: litesvm::LiteSVM,
    pub payer: Keypair,
    pub admin: Keypair,
    pub relayer_signer: Keypair,
    pub fee_receiver_relayer: Keypair,
    pub split_protocol_default: u64,
    pub split_relayer: u64,
    pub searcher: Keypair,
}

pub fn setup(params: SetupParams) -> SetupResult {
    let SetupParams {
        split_protocol_default,
        split_relayer,
    } = params;

    let mut svm = litesvm::LiteSVM::new();
    svm.add_program_from_file(
        express_relay::ID,
        "../target/deploy/express_relay.so",
    )
    .unwrap();
    svm.add_program_from_file(
        dummy::ID,
        "../target/deploy/dummy.so",
    ).unwrap();

    let payer = Keypair::new();
    let admin = Keypair::new();
    let relayer_signer = Keypair::new();
    let fee_receiver_relayer = Keypair::new();

    svm.airdrop(&payer.pubkey(), 10*LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&admin.pubkey(), 10*LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&relayer_signer.pubkey(), 10*LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&fee_receiver_relayer.pubkey(), 1*LAMPORTS_PER_SOL).unwrap();

    let searcher = Keypair::new();

    svm.airdrop(&searcher.pubkey(), 20*LAMPORTS_PER_SOL).unwrap();

    let initialize_express_relay_ix = get_initialize_express_relay_instruction(
        &payer,
        admin.pubkey(),
        relayer_signer.pubkey(),
        fee_receiver_relayer.pubkey(),
        split_protocol_default,
        split_relayer
    );

    submit_transaction(&mut svm, &[initialize_express_relay_ix], &payer, &[&payer]).expect("Initialize express relay tx failed unexpectedly");

    let initialize_dummy_ix = get_initialize_dummy_instruction(
        &payer,
    );

    submit_transaction(&mut svm, &[initialize_dummy_ix], &payer, &[&payer]).expect("Initialize dummy tx failed unexpectedly");

    return SetupResult {
        svm,
        payer,
        admin,
        relayer_signer,
        fee_receiver_relayer,
        split_protocol_default,
        split_relayer,
        searcher,
    };
}
