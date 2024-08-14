use litesvm::types::TransactionResult;
use solana_sdk::{instruction::Instruction, signature::Keypair, signer::Signer, transaction::Transaction, pubkey::Pubkey};

pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
pub const TX_FEE: u64 = 10_000; // TODO: make this programmatic? FeeStructure is currently private field within LiteSVM

pub fn submit_transaction(svm: &mut litesvm::LiteSVM, ixs: &[Instruction], payer: &Keypair, signers: &[&Keypair]) -> TransactionResult {
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&payer.pubkey()),
        signers,
        svm.latest_blockhash(),
    );

    return svm.send_transaction(tx);
}

pub fn get_balance(svm: &litesvm::LiteSVM, pubkey: &Pubkey) -> u64 {
    return match svm.get_balance(pubkey) {
        Some(balance) => balance,
        None => 0,
    };
}
