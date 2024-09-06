use {
    crate::helpers::helpers_general::{
        create_and_fund_kp,
        create_and_submit_tx,
        fund_pk,
    },
    anchor_lang::{
        solana_program::sysvar::instructions as sysvar_instructions,
        InstructionData,
        ToAccountMetas,
    },
    express_relay::sdk::test_helpers::create_initialize_express_relay_ix,
    solana_program_test::{
        anchor_processor,
        ProgramTest,
        ProgramTestContext,
    },
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
    },
};

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
