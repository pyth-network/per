use anchor_lang::{prelude::*, system_program};
use anchor_spl::token;
use solana_program_test::ProgramTestContext;
use solana_sdk::{account::Account, instruction::Instruction, signature::Keypair, signer::Signer, sysvar::instructions::id as sysvar_instructions_id, transaction::Transaction};
use anchor_lang::{ToAccountMetas, InstructionData};
use express_relay::{accounts::{CheckPermission, Initialize, Permission, SetProtocolSplit, SetAdmin, SetRelayer, SetSplits, WithdrawFees}, state::{SEED_CONFIG_PROTOCOL, SEED_METADATA}, InitializeArgs, PermissionArgs, SetProtocolSplitArgs, SetSplitsArgs};

// TODO: write helpers for failure cases as well
// TODO: failure initialize--fee splits too high

// TODO: failure set_relayer--invalid admin

// TODO: failure set_splits--invalid admin
// TODO: failure set_splits--fee splits too high

// TODO: failure set_protocol_split--invalid admin
// TODO: failure set_protocol_split--invalid protocol config
// TODO: failure set_protocol_split--fee splits too high

// TODO: failure permission--invalid relayer signer
// TODO: failure permission--invalid relayer fee receiver
// TODO: failure permission--CPI
// TODO: failure permission--passed deadline
// TODO: failure permission--insufficient funds in searcher account (needs to be greater than rent)
// TODO: failure check_permission--expected different permission key
// TODO: failure check_permission--expected different protocol key
// TODO: failure check_permisison--no permission instruction in tx

// TODO: failure withdraw_fees--invalid admin

pub async fn initialize(program_context: &mut ProgramTestContext, payer: &Keypair, admin: Pubkey, relayer_signer: Pubkey, fee_receiver_relayer: Pubkey, split_protocol_default: u64, split_relayer: u64) -> Account {
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
            fee_receiver_relayer: fee_receiver_relayer,
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

pub async fn set_admin(program_context: &mut ProgramTestContext, admin: Keypair, admin_new: Pubkey) -> Account {
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;

    let set_admin_ix = Instruction {
        program_id: express_relay::id(),
        data: express_relay::instruction::SetAdmin {}.data(),
        accounts: SetAdmin {
            admin: admin.pubkey(),
            express_relay_metadata: express_relay_metadata,
            admin_new: admin_new,
        }.to_account_metas(None),
    };

    let mut set_admin_tx = Transaction::new_with_payer(&[set_admin_ix],Some(&admin.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    set_admin_tx.partial_sign(&[&admin], recent_blockhash);
    program_context
        .banks_client
        .process_transaction(set_admin_tx)
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

pub async fn set_relayer(program_context: &mut ProgramTestContext, admin: Keypair, relayer_signer: Pubkey, fee_receiver_relayer: Pubkey) -> Account {
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;

    let set_relayer_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::SetRelayer {}.data(),
        accounts: SetRelayer {
            admin: admin.pubkey(),
            express_relay_metadata: express_relay_metadata,
            relayer_signer: relayer_signer,
            fee_receiver_relayer: fee_receiver_relayer,
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

pub async fn set_protocol_split(program_context: &mut ProgramTestContext, admin: Keypair, protocol: Pubkey, split_protocol: u64) -> (Account, Account) {
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;

    let protocol_config = Pubkey::find_program_address(&[SEED_CONFIG_PROTOCOL, protocol.as_ref()], &express_relay::id()).0;

    let set_protocol_split_ix = Instruction {
        program_id: express_relay::id(),
        data: express_relay::instruction::SetProtocolSplit {
            data: SetProtocolSplitArgs {
                split_protocol: split_protocol,
            }
        }.data(),
        accounts: SetProtocolSplit {
            admin: admin.pubkey(),
            protocol_config: protocol_config,
            express_relay_metadata: express_relay_metadata,
            protocol: protocol,
            system_program: system_program::ID,
        }.to_account_metas(None)
    };

    let mut set_protocol_split_tx = Transaction::new_with_payer(&[set_protocol_split_ix],Some(&admin.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    set_protocol_split_tx.partial_sign(&[&admin], recent_blockhash);
    program_context
        .banks_client
        .process_transaction(set_protocol_split_tx)
        .await
        .unwrap();

    let protocol_config_acc = program_context
        .banks_client
        .get_account(protocol_config)
        .await
        .unwrap()
        .unwrap();

    let express_relay_metadata_acc = program_context
        .banks_client
        .get_account(express_relay_metadata)
        .await
        .unwrap()
        .unwrap();

    return (protocol_config_acc, express_relay_metadata_acc);
}

pub async fn withdraw_fees(program_context: &mut ProgramTestContext, admin: Keypair, fee_receiver_admin: Pubkey) -> (u64, u64) {
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;

    let withdraw_fees_ix = Instruction {
        program_id: express_relay::id(),
        data: express_relay::instruction::WithdrawFees {}.data(),
        accounts: WithdrawFees {
            admin: admin.pubkey(),
            fee_receiver_admin: fee_receiver_admin,
            express_relay_metadata: express_relay_metadata
        }.to_account_metas(None)
    };

    let mut withdraw_fees_tx = Transaction::new_with_payer(&[withdraw_fees_ix],Some(&admin.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    withdraw_fees_tx.partial_sign(&[&admin], recent_blockhash);
    program_context
        .banks_client
        .process_transaction(withdraw_fees_tx)
        .await
        .unwrap();

    let balance_express_relay_metadata = program_context
        .banks_client
        .get_balance(express_relay_metadata)
        .await
        .unwrap();

    let balance_fee_receiver_admin = program_context
        .banks_client
        .get_balance(fee_receiver_admin)
        .await
        .unwrap();

    return (balance_express_relay_metadata, balance_fee_receiver_admin);
}

pub async fn permission(
    program_context: &mut ProgramTestContext,
    relayer_signer: Keypair,
    searcher: Keypair,
    protocol: Pubkey,
    fee_receiver_relayer: Pubkey,
    fee_receiver_protocol: Pubkey,
    permission: Pubkey,
    bid_amount: u64
) -> (bool, u64, u64, u64, u64) {
    let express_relay_metadata = Pubkey::find_program_address(&[SEED_METADATA], &express_relay::id()).0;
    let protocol_config = Pubkey::find_program_address(&[SEED_CONFIG_PROTOCOL, protocol.as_ref()], &express_relay::id()).0;

    let permission_ix = Instruction {
        program_id: express_relay::id(),
        data: express_relay::instruction::Permission {
            data: PermissionArgs {
                deadline: 1_000_000_000_000_000,
                bid_amount: bid_amount,
            }
        }.data(),
        accounts: Permission {
            relayer_signer: relayer_signer.pubkey(),
            searcher: searcher.pubkey(),
            permission: permission,
            protocol: protocol,
            protocol_config: protocol_config,
            fee_receiver_relayer: fee_receiver_relayer,
            fee_receiver_protocol: fee_receiver_protocol,
            express_relay_metadata: express_relay_metadata,
            system_program: system_program::ID,
            token_program: token::ID,
            sysvar_instructions: sysvar_instructions_id(),
        }.to_account_metas(None)
    };

    let check_permission_ix = Instruction {
        program_id: express_relay::id(),
        data: express_relay::instruction::CheckPermission {}.data(),
        accounts: CheckPermission {
            sysvar_instructions: sysvar_instructions_id(),
            permission: permission,
            protocol: protocol,
        }.to_account_metas(None)
    };

    let mut permission_tx = Transaction::new_with_payer(&[permission_ix, check_permission_ix],Some(&relayer_signer.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    permission_tx.partial_sign(&[&relayer_signer, &searcher], recent_blockhash);
    let tx_success =  match program_context
        .banks_client
        .process_transaction(permission_tx)
        .await {
            Ok(_) => true,
            Err(_) => false,
        };

    let balance_express_relay_metadata = program_context.banks_client.get_balance(express_relay_metadata).await.unwrap();
    let balance_fee_receiver_relayer = program_context.banks_client.get_balance(fee_receiver_relayer).await.unwrap();
    let balance_fee_receiver_protocol = program_context.banks_client.get_balance(fee_receiver_protocol).await.unwrap();
    let balance_searcher = program_context.banks_client.get_balance(searcher.pubkey()).await.unwrap();

    return (tx_success, balance_express_relay_metadata, balance_fee_receiver_relayer, balance_fee_receiver_protocol, balance_searcher);
}
