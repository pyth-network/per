use crate::helpers::generate_and_fund_key;
use crate::{express_relay::initialize::get_initialize_instruction as get_initialize_express_relay_instruction, helpers::submit_transaction};
use crate::dummy::initialize::get_initialize_instruction as get_initialize_dummy_instruction;
use solana_sdk::transaction::TransactionError;
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

pub fn setup(params: SetupParams) -> Result<SetupResult, TransactionError> {
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

    let payer = generate_and_fund_key(&mut svm);
    let admin = generate_and_fund_key(&mut svm);
    let relayer_signer = generate_and_fund_key(&mut svm);
    let fee_receiver_relayer = generate_and_fund_key(&mut svm);

    let searcher = generate_and_fund_key(&mut svm);

    let initialize_express_relay_ix = get_initialize_express_relay_instruction(
        &payer,
        admin.pubkey(),
        relayer_signer.pubkey(),
        fee_receiver_relayer.pubkey(),
        split_protocol_default,
        split_relayer
    );

    let tx_result_express_relay = submit_transaction(&mut svm, &[initialize_express_relay_ix], &payer, &[&payer]);
    match tx_result_express_relay {
        Ok(_) => (),
        Err(e) => return Err(e.err),
    };

    let initialize_dummy_ix = get_initialize_dummy_instruction(
        &payer,
    );

    let tx_result_dummy = submit_transaction(&mut svm, &[initialize_dummy_ix], &payer, &[&payer]);
    match tx_result_dummy {
        Ok(_) => (),
        Err(e) => return Err(e.err),
    };

    return Ok(
        SetupResult {
            svm,
            payer,
            admin,
            relayer_signer,
            fee_receiver_relayer,
            split_protocol_default,
            split_relayer,
            searcher,
        }
    );
}
