use anchor_lang::{system_program, prelude::*};
use solana_program_test::ProgramTestContext;
use solana_sdk::{account::Account, instruction::Instruction, signature::Keypair, transaction::Transaction, signer::Signer, sysvar::instructions::id as sysvar_instructions_id};
use anchor_lang::{ToAccountMetas, InstructionData};
use express_relay::{state::{SEED_METADATA, SEED_CONFIG_PROTOCOL, SEED_PERMISSION, SEED_EXPRESS_RELAY_FEES, ExpressRelayMetadata}, InitializeArgs, SetRelayerArgs, SetSplitsArgs, PermissionArgs, DepermissionArgs, accounts::{Initialize, SetRelayer, SetSplits, Permission, Depermission}};

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

pub async fn express_relay_tx(
    program_context: &mut ProgramTestContext,
    searcher_payer: Keypair,
    relayer_signer: Keypair,
    protocol: Pubkey,
    permission_id: [u8; 32],
    bid_id: [u8; 16],
    bid_amount: u64,
    instruction: Instruction
) -> (u64, u64, u64) {
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;

    let express_relay_metadata_acc_pre = program_context
        .banks_client
        .get_account(express_relay_metadata)
        .await
        .unwrap()
        .unwrap();
    let express_relay_metadata_data_pre = ExpressRelayMetadata::try_deserialize(&mut express_relay_metadata_acc_pre.data.as_ref()).unwrap();

    let relayer_fee_receiver = express_relay_metadata_data_pre.relayer_fee_receiver;

    let protocol_config = Pubkey::find_program_address(&[SEED_CONFIG_PROTOCOL, &protocol.to_bytes()], &express_relay::id()).0;

    let permission = Pubkey::find_program_address(&[SEED_PERMISSION, &protocol.to_bytes(), &permission_id], &express_relay::id()).0;

    let permission_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::Permission {
            data: PermissionArgs {
                permission_id: permission_id.clone(),
                bid_id: bid_id.clone(),
                bid_amount: bid_amount.clone(),
            }
        }.data(),
        accounts: Permission {
            relayer_signer: relayer_signer.pubkey(),
            permission: permission,
            protocol: protocol,
            express_relay_metadata: express_relay_metadata,
            system_program: system_program::ID,
            sysvar_instructions: sysvar_instructions_id(),
        }
        .to_account_metas(None),
    };

    let protocol_fee_receiver = Pubkey::find_program_address(&[SEED_EXPRESS_RELAY_FEES], &protocol).0;
    let depermission_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::Depermission {
            _data: DepermissionArgs {
                permission_id: permission_id.clone(),
                bid_id: bid_id,
            }
        }.data(),
        accounts: Depermission {
            relayer_signer: relayer_signer.pubkey(),
            permission: permission,
            protocol: protocol,
            protocol_fee_receiver: protocol_fee_receiver,
            relayer_fee_receiver: relayer_fee_receiver,
            protocol_config: protocol_config,
            express_relay_metadata: express_relay_metadata,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    };

    let mut tx = Transaction::new_with_payer(
        &[
            permission_ix,
            instruction,
            depermission_ix
        ],
        Some(&searcher_payer.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    tx.partial_sign(&[&searcher_payer, &relayer_signer], recent_blockhash);
    program_context
        .banks_client
        .process_transaction(tx)
        .await
        .unwrap();

    let permission_balance = program_context
        .banks_client
        .get_balance(permission)
        .await
        .unwrap();

    let relayer_fee_receiver_balance = program_context
        .banks_client
        .get_balance(relayer_fee_receiver)
        .await
        .unwrap();

    let protocol_balance = program_context
        .banks_client
        .get_balance(protocol_fee_receiver)
        .await
        .unwrap();

    return (permission_balance, relayer_fee_receiver_balance, protocol_balance);
}
