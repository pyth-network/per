use anchor_lang::{system_program, prelude::*};
use solana_program_test::ProgramTestContext;
use solana_sdk::{account::Account, instruction::Instruction, signature::Keypair, transaction::Transaction, signer::Signer};
use anchor_lang::{ToAccountMetas, InstructionData};
use express_relay::{state::SEED_METADATA, InitializeArgs, SetRelayerArgs, SetSplitsArgs, accounts::{Initialize, SetRelayer, SetSplits}};

pub async fn initialize(program_context: &mut ProgramTestContext, payer: Keypair, admin: Pubkey, relayer_signer: Pubkey, relayer_fee_receiver: Pubkey, split_protocol_default: u64, split_relayer: u64) -> Account {
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;
    let system_program_pk = system_program::ID;

    let initialize_ix = Instruction {
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

    let mut initialize_tx = Transaction::new_with_payer(&[initialize_ix],Some(&payer.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    initialize_tx.partial_sign(&[&payer], recent_blockhash);
    program_context
        .banks_client
        .process_transaction(initialize_tx)
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

pub async fn set_relayer(program_context: &mut ProgramTestContext, admin: Keypair, relayer_signer: Pubkey, relayer_fee_receiver: Pubkey) -> Account {
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

pub async fn set_splits(program_context: &mut ProgramTestContext, admin: Keypair, split_protocol_default: u64, split_relayer: u64) -> Account {
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;

    let set_splits_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::SetSplits {
            data: SetSplitsArgs {
                split_protocol_default: split_protocol_default,
                split_relayer: split_relayer,
            }
        }.data(),
        accounts: SetSplits {
            admin: admin.pubkey(),
            express_relay_metadata: express_relay_metadata,
        }
        .to_account_metas(None),
    };

    let mut set_splits_tx = Transaction::new_with_payer(&[set_splits_ix],Some(&admin.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    set_splits_tx.partial_sign(&[&admin], recent_blockhash);
    program_context
        .banks_client
        .process_transaction(set_splits_tx)
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
