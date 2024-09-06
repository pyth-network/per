use {
    solana_program_test::{
        BanksClientError,
        ProgramTest,
        ProgramTestContext,
    },
    solana_sdk::{
        account::Account,
        commitment_config::CommitmentLevel,
        instruction::{
            Instruction,
            InstructionError::Custom,
        },
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        transaction::{
            Transaction,
            TransactionError::{
                self,
                InstructionError,
            },
        },
    },
};

pub fn fund_pk(pk: Pubkey, program_test: &mut ProgramTest, balance: u64) {
    program_test.add_account(
        pk,
        Account {
            lamports: balance,
            ..Account::default()
        },
    );
}

pub fn create_and_fund_kp(program_test: &mut ProgramTest, balance: u64) -> Keypair {
    let kp = Keypair::new();
    fund_pk(kp.pubkey(), program_test, balance);
    kp
}

pub async fn create_and_submit_tx(
    program_test_context: &mut ProgramTestContext,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> Result<(), BanksClientError> {
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&payer.pubkey()),
        signers,
        program_test_context
            .get_new_latest_blockhash()
            .await
            .unwrap(),
    );

    program_test_context
        .banks_client
        .process_transaction_with_commitment(tx, CommitmentLevel::Processed)
        .await
}

pub fn assert_custom_error(error: TransactionError, instruction_index: u8, custom_error: u32) {
    match error {
        InstructionError(index, error_variant) => {
            assert_eq!(index, instruction_index);
            match error_variant {
                Custom(code) => {
                    assert_eq!(code, custom_error);
                }
                _ => panic!("Unexpected error code"),
            }
        }
        _ => panic!("Unexpected error variant"),
    }
}
