use anchor_lang::{system_program, prelude::*};
use solana_program_test::{ProgramTest, tokio};
use solana_sdk::{account::Account, instruction::Instruction, signature::Keypair, transaction::Transaction, signer::Signer};
use anchor_lang::{ToAccountMetas, InstructionData};
use express_relay::{state::{SEED_METADATA, ExpressRelayMetadata}, InitializeArgs, accounts::Initialize};

#[tokio::test]
async fn test_initialize() {
    let mut program_test = ProgramTest::new(
        "express_relay",
        express_relay::id(),
        None,
    );

    let payer = Keypair::new();
    program_test.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;
    let admin = Keypair::new();
    let relayer_signer = Keypair::new();
    let relayer_fee_receiver = Keypair::new();
    let system_program_pk = system_program::ID;

    let split_protocol_default = 200_000_000_000_000_000;
    let split_relayer = 50_000_000_000_000_000;

    let initalize_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::Initialize {
            data: InitializeArgs {
                split_protocol_default: split_protocol_default,
                split_relayer: split_relayer,
            }
        }.data(),
        accounts: Initialize {
            payer: payer.pubkey(),
            express_relay_metadata: express_relay_metadata,
            admin: admin.pubkey(),
            relayer_signer: relayer_signer.pubkey(),
            relayer_fee_receiver: relayer_fee_receiver.pubkey(),
            system_program: system_program_pk,
        }
        .to_account_metas(None),
    };

    let mut program_context = program_test.start_with_context().await;

    let mut initalize_tx = Transaction::new_with_payer(&[initalize_ix],Some(&payer.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    initalize_tx.partial_sign(&[&payer], recent_blockhash);
    program_context
        .banks_client
        .process_transaction(initalize_tx)
        .await
        .unwrap();

    let express_relay_metadata_post = program_context
        .banks_client
        .get_account(express_relay_metadata)
        .await
        .unwrap()
        .unwrap();
    let express_relay_metadata_data = ExpressRelayMetadata::try_deserialize(&mut express_relay_metadata_post.data.as_ref()).unwrap();
    assert_eq!(express_relay_metadata_data.admin, admin.pubkey());
    assert_eq!(express_relay_metadata_data.relayer_signer, relayer_signer.pubkey());
    assert_eq!(express_relay_metadata_data.relayer_fee_receiver, relayer_fee_receiver.pubkey());
    assert_eq!(express_relay_metadata_data.split_protocol_default, split_protocol_default);
    assert_eq!(express_relay_metadata_data.split_relayer, split_relayer);
}
