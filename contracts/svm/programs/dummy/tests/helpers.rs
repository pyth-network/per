use {
    anchor_lang::{
        solana_program::sysvar::instructions as sysvar_instructions,
        InstructionData,
        ToAccountMetas,
    },
    express_relay::sdk::test_helpers::create_initialize_express_relay_ix,
    solana_program_test::{
        anchor_processor,
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
    program_test.add_account(
        kp.pubkey(),
        Account {
            lamports: balance,
            ..Account::default()
        },
    );
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

pub struct SetupInfo {
    pub program_test_context: ProgramTestContext,
    pub payer:                Keypair,
    pub relayer_signer:       Keypair,
    pub fee_receiver_relayer: Keypair,
}

pub async fn setup(router: Pubkey) -> SetupInfo {
    let mut program_test = ProgramTest::new("dummy", dummy::ID, anchor_processor!(dummy::entry));
    program_test.add_program(
        "express_relay",
        express_relay::id(),
        anchor_processor!(express_relay::entry),
    );

    let payer = create_and_fund_kp(&mut program_test, 1_000_000_000);
    let admin = create_and_fund_kp(&mut program_test, 1_000_000_000);
    let relayer_signer = create_and_fund_kp(&mut program_test, 1_000_000_000);
    let fee_receiver_relayer = create_and_fund_kp(&mut program_test, 1_000_000_000);
    fund_pk(router, &mut program_test, 1_000_000_000);

    let mut program_test_context = program_test.start_with_context().await;

    let ix_initialize_express_relay = create_initialize_express_relay_ix(
        payer.pubkey(),
        admin.pubkey(),
        relayer_signer.pubkey(),
        fee_receiver_relayer.pubkey(),
    );

    create_and_submit_tx(
        &mut program_test_context,
        &[ix_initialize_express_relay],
        &payer,
        &[&payer],
    )
    .await
    .unwrap();

    SetupInfo {
        program_test_context,
        payer,
        relayer_signer,
        fee_receiver_relayer,
    }
}

pub fn create_do_nothing_ix(payer: Pubkey, permission: Pubkey, router: Pubkey) -> Instruction {
    Instruction {
        program_id: dummy::id(),
        data:       dummy::instruction::DoNothing {}.data(),
        accounts:   dummy::accounts::DoNothing {
            payer,
            express_relay: express_relay::id(),
            sysvar_instructions: sysvar_instructions::id(),
            permission,
            router,
        }
        .to_account_metas(None),
    }
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
