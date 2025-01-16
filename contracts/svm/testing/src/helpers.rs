use {
    litesvm::types::TransactionResult,
    solana_sdk::{
        instruction::{
            Instruction,
            InstructionError,
        },
        native_token::LAMPORTS_PER_SOL,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        sysvar::clock::Clock,
        transaction::{
            Transaction,
            TransactionError::{
                self,
            },
        },
    },
};
pub const TX_FEE: u64 = 10_000; // TODO: make this programmatic? FeeStructure is currently private field within LiteSVM

#[allow(clippy::result_large_err)]
pub fn submit_transaction(
    svm: &mut litesvm::LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> TransactionResult {
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&payer.pubkey()),
        signers,
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
}

pub fn generate_and_fund_key(svm: &mut litesvm::LiteSVM) -> Keypair {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();
    svm.airdrop(&pubkey, 10 * LAMPORTS_PER_SOL).unwrap();
    keypair
}

pub fn get_balance(svm: &litesvm::LiteSVM, pubkey: &Pubkey) -> u64 {
    svm.get_balance(pubkey).unwrap_or(0)
}

pub fn warp_to_unix(svm: &mut litesvm::LiteSVM, unix_timestamp: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = unix_timestamp + 1;
    svm.set_sysvar(&clock);
}

pub fn assert_custom_error(
    error: TransactionError,
    instruction_index: u8,
    instruction_error: InstructionError,
) {
    match error {
        TransactionError::InstructionError(index, error_variant) => {
            assert_eq!(index, instruction_index);
            assert_eq!(error_variant, instruction_error);
        }
        _ => panic!("Unexpected error variant"),
    }
}
