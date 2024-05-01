use anchor_lang::{system_program, prelude::*};
use solana_program_test::ProgramTest;
use solana_sdk::{account::Account, instruction::Instruction, signature::Keypair, transaction::Transaction, signer::Signer};
use anchor_lang::{ToAccountMetas, InstructionData};
use express_relay::{state::SEED_METADATA, InitializeArgs, SetRelayerArgs, accounts::{Initialize, SetRelayer}};

pub async fn initialize(mut program_test: ProgramTest, admin: Pubkey, relayer_signer: Pubkey, relayer_fee_receiver: Pubkey, split_protocol_default: u64, split_relayer: u64) -> Account {
    let payer = Keypair::new();
    program_test.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;
    let system_program_pk = system_program::ID;

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
            admin: admin,
            relayer_signer: relayer_signer,
            relayer_fee_receiver: relayer_fee_receiver,
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

    let express_relay_metadata_acc = program_context
        .banks_client
        .get_account(express_relay_metadata)
        .await
        .unwrap()
        .unwrap();

    return express_relay_metadata_acc;
}

pub async fn set_relayer(mut program_test: ProgramTest, admin: Keypair, relayer_signer: Pubkey, relayer_fee_receiver: Pubkey) -> Account {
    program_test.add_account(
        admin.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;

    let set_relayer_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::SetRelayer {
            _data: SetRelayerArgs {}
        }.data(),
        accounts: SetRelayer {
            admin: admin.pubkey(),
            express_relay_metadata: express_relay_metadata,
            relayer_signer: relayer_signer,
            relayer_fee_receiver: relayer_fee_receiver,
        }
        .to_account_metas(None),
    };

    let mut program_context = program_test.start_with_context().await;

    let mut set_relayer_tx = Transaction::new_with_payer(&[set_relayer_ix],Some(&admin.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    set_relayer_tx.partial_sign(&[&admin], recent_blockhash);
    program_context
        .banks_client
        .process_transaction(set_relayer_tx)
        .await
        .unwrap();

    let express_relay_metadata_acc = program_context
        .banks_client
        .get_account(express_relay_metadata)
        .await
        .unwrap()
        .unwrap();

    return express_relay_metadata_acc;
}
