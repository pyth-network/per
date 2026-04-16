use {
    anchor_lang::error::ErrorCode as AnchorErrorCode,
    anchor_spl::{
        associated_token::{
            get_associated_token_address_with_program_id,
            spl_associated_token_account::instruction::create_associated_token_account_idempotent,
        },
        token::spl_token,
    },
    solana_sdk::{
        instruction::InstructionError,
        signature::Keypair,
        signer::Signer,
    },
    testing::{
        express_relay::{
            helpers::get_express_relay_metadata_key,
            withdraw_spl_fees::withdraw_spl_fees_instruction,
        },
        helpers::{
            assert_custom_error,
            generate_and_fund_key,
            submit_transaction,
        },
        setup::setup,
        token::Token,
    },
};

#[test]
fn test_withdraw_spl_fees() {
    let setup_result = setup(None).expect("setup failed");
    let mut svm = setup_result.svm;
    let admin = setup_result.admin;

    let express_relay_metadata = get_express_relay_metadata_key();
    let fee_receiver_admin = generate_and_fund_key(&mut svm);
    let fee_token = Token::create_mint(&mut svm, spl_token::ID, 6);

    let express_relay_fee_receiver_ata = get_associated_token_address_with_program_id(
        &express_relay_metadata,
        &fee_token.mint,
        &fee_token.token_program,
    );
    let fee_receiver_admin_ata = get_associated_token_address_with_program_id(
        &fee_receiver_admin.pubkey(),
        &fee_token.mint,
        &fee_token.token_program,
    );

    fee_token.airdrop(&mut svm, &express_relay_metadata, 3.5);

    let create_admin_ata_ix = create_associated_token_account_idempotent(
        &admin.pubkey(),
        &fee_receiver_admin.pubkey(),
        &fee_token.mint,
        &fee_token.token_program,
    );
    let withdraw_ix = withdraw_spl_fees_instruction(
        &admin,
        express_relay_fee_receiver_ata,
        fee_receiver_admin_ata,
        fee_token.mint,
        fee_token.token_program,
    );
    submit_transaction(
        &mut svm,
        &[create_admin_ata_ix, withdraw_ix],
        &admin,
        &[&admin],
    )
    .unwrap();

    assert!(Token::token_balance_matches(
        &mut svm,
        &express_relay_fee_receiver_ata,
        0,
    ));
    assert!(Token::token_balance_matches(
        &mut svm,
        &fee_receiver_admin_ata,
        fee_token.get_amount_with_decimals(3.5),
    ));
}

#[test]
fn test_withdraw_spl_fees_fail_wrong_admin() {
    let setup_result = setup(None).expect("setup failed");

    let mut svm = setup_result.svm;
    let wrong_admin = generate_and_fund_key(&mut svm);

    let express_relay_metadata = get_express_relay_metadata_key();
    let fee_receiver_admin = Keypair::new();
    let fee_token = Token::create_mint(&mut svm, spl_token::ID, 6);
    let express_relay_fee_receiver_ata = get_associated_token_address_with_program_id(
        &express_relay_metadata,
        &fee_token.mint,
        &fee_token.token_program,
    );
    let fee_receiver_admin_ata = get_associated_token_address_with_program_id(
        &fee_receiver_admin.pubkey(),
        &fee_token.mint,
        &fee_token.token_program,
    );
    fee_token.airdrop(&mut svm, &express_relay_metadata, 1.0);
    let create_admin_ata_ix = create_associated_token_account_idempotent(
        &wrong_admin.pubkey(),
        &fee_receiver_admin.pubkey(),
        &fee_token.mint,
        &fee_token.token_program,
    );
    submit_transaction(
        &mut svm,
        &[create_admin_ata_ix],
        &wrong_admin,
        &[&wrong_admin],
    )
    .unwrap();

    let withdraw_ix = withdraw_spl_fees_instruction(
        &wrong_admin,
        express_relay_fee_receiver_ata,
        fee_receiver_admin_ata,
        fee_token.mint,
        fee_token.token_program,
    );
    let tx_result = submit_transaction(&mut svm, &[withdraw_ix], &wrong_admin, &[&wrong_admin])
        .expect_err("Transaction should have failed");

    assert_custom_error(
        tx_result.err,
        0,
        InstructionError::Custom(AnchorErrorCode::ConstraintHasOne.into()),
    );
}
