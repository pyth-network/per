use anchor_lang::{prelude::*, system_program};
use anchor_spl::{associated_token::get_associated_token_address, token};
use spl_associated_token_account::instruction::create_associated_token_account;
use solana_program_test::ProgramTestContext;
use solana_sdk::{account::Account, ed25519_instruction, instruction::Instruction, signature::Keypair, signer::Signer, sysvar::instructions::id as sysvar_instructions_id, transaction::Transaction};
use anchor_lang::{ToAccountMetas, InstructionData};
use express_relay::{state::{SEED_METADATA, SEED_CONFIG_PROTOCOL, SEED_PERMISSION, SEED_EXPRESS_RELAY_FEES, SEED_AUTHORITY, SEED_SIGNATURE_ACCOUNTING, ExpressRelayMetadata}, InitializeArgs, SetRelayerArgs, SetSplitsArgs, PermissionArgs, DepermissionArgs, accounts::{Initialize, SetRelayer, SetSplits, Permission, Depermission}};
use std::str::FromStr;
use solana_program::{hash, system_instruction, ed25519_program};
use spl_token::instruction::{approve, sync_native};

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
    bid_amount: u64
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

    let valid_until: u64 = 69_420_000_000_000;
    let mut msg: [u8; 112] = [0; 32+32+32+8+8];
    msg[..32].copy_from_slice(&protocol.key().to_bytes());
    msg[32..64].copy_from_slice(&permission_id);
    msg[64..96].copy_from_slice(&searcher_payer.pubkey().key().to_bytes());
    msg[96..104].copy_from_slice(&bid_amount.to_le_bytes());
    msg[104..].copy_from_slice(&valid_until.to_le_bytes());
    let digest = hash::hash(&msg);
    let signature: [u8; 64] = searcher_payer.sign_message(digest.as_ref()).as_ref().try_into().unwrap();

    let signature_accounting = Pubkey::find_program_address(
        &[
            SEED_SIGNATURE_ACCOUNTING,
            &signature[..32],
            &signature[32..]
        ], &express_relay::id()).0;

    let permission_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::Permission {
            data:
                Box::new(
                    PermissionArgs {
                        permission_id: permission_id.clone(),
                        // bid_id: bid_id.clone(),
                        bid_amount: bid_amount.clone(),
                    }
                )
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
    let wsol_mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let wsol_ta_user = get_associated_token_address(&searcher_payer.pubkey(), &wsol_mint);
    let wsol_ta_express_relay = Pubkey::find_program_address(&[b"ata", wsol_mint.as_ref()], &express_relay::id()).0;
    let express_relay_authority = Pubkey::find_program_address(&[SEED_AUTHORITY], &express_relay::id()).0;

    // initialize the wsol_ta_user account with some lamports and approve the express_relay_authority to transfer
    if program_context.banks_client.get_balance(wsol_ta_user).await.unwrap() == 0 {
        let create_wsol_ta_user_ix = create_associated_token_account(
            &searcher_payer.pubkey(),
            &searcher_payer.pubkey(),
            &wsol_mint,
            &token::ID
        );

        let send_sol_wsol_ta_user_ix = system_instruction::transfer(
            &searcher_payer.pubkey(),
            &wsol_ta_user,
            5_000_000_000
        );

        let sync_native_ix = sync_native(
            &token::ID,
            &wsol_ta_user
        ).unwrap();

        let approve_ix = approve(
            &token::ID,
            &wsol_ta_user,
            &express_relay_authority,
            &searcher_payer.pubkey(),
            &[&searcher_payer.pubkey()],
            4_000_000_000
        ).unwrap();

        let mut tx_create_wsol_ta_user = Transaction::new_with_payer(
            &[
                create_wsol_ta_user_ix,
                send_sol_wsol_ta_user_ix,
                sync_native_ix,
                approve_ix
            ],
            Some(&searcher_payer.pubkey()));
        let recent_blockhash = program_context.last_blockhash.clone();

        tx_create_wsol_ta_user.partial_sign(&[&searcher_payer], recent_blockhash);
        program_context
            .banks_client
            .process_transaction(tx_create_wsol_ta_user)
            .await
            .unwrap();
    }

    print!("permission id before the ix: {:?}", permission_id);
    print!("valid until before the ix: {:?}", valid_until);
    print!("signature before the ix: {:?}", signature);
    print!("DOING THIS {:?}", express_relay::instruction::Depermission {
        data: Box::new(DepermissionArgs {
            permission_id,
            valid_until,
            signature
        })
    }.data());

    let depermission_ix = Instruction {
        program_id: express_relay::id(),
        data:
        express_relay::instruction::Depermission {
            data: Box::new(
                DepermissionArgs {
                permission_id,
                valid_until,
                signature
            })
        }.data(),
        accounts: Depermission {
            relayer_signer: relayer_signer.pubkey(),
            permission: permission,
            user: searcher_payer.pubkey(),
            protocol: protocol,
            protocol_fee_receiver: protocol_fee_receiver,
            relayer_fee_receiver: relayer_fee_receiver,
            protocol_config: protocol_config,
            express_relay_metadata: express_relay_metadata,
            wsol_mint: wsol_mint,
            wsol_ta_user: wsol_ta_user,
            wsol_ta_express_relay: wsol_ta_express_relay,
            express_relay_authority: express_relay_authority,
            signature_accounting: signature_accounting,
            token_program: token::ID,
            system_program: system_program::ID,
            sysvar_instructions: sysvar_instructions_id(),
        }
        .to_account_metas(None),
    };

    // TODO: use a library to construct this
    // let sigver_ix = ed25519_instruction::new_ed25519_instruction(
    //     searcher_payer,
    //     &digest,
    // );
    let mut data_ix_sigver: [u8; 112 + 32] = [0; 112 + 32];
    data_ix_sigver[..1].copy_from_slice(&[1u8]);
    data_ix_sigver[1..2].copy_from_slice(&[0u8]);
    data_ix_sigver[2..4].copy_from_slice(&[48u8, 0u8]);
    data_ix_sigver[4..6].copy_from_slice(&[255u8, 255u8]);
    data_ix_sigver[6..8].copy_from_slice(&[16u8, 0u8]);
    data_ix_sigver[8..10].copy_from_slice(&[255u8, 255u8]);
    data_ix_sigver[10..12].copy_from_slice(&[112u8, 0u8]);
    data_ix_sigver[12..14].copy_from_slice(&[32u8, 0u8]);
    data_ix_sigver[14..16].copy_from_slice(&[255u8, 255u8]);

    data_ix_sigver[16..48].copy_from_slice(&searcher_payer.pubkey().to_bytes());
    data_ix_sigver[48..112].copy_from_slice(&signature);
    data_ix_sigver[112..].copy_from_slice(digest.as_ref());

    let sigver_ix = Instruction {
        program_id: ed25519_program::id(),
        data: data_ix_sigver.into(),
        accounts: vec![]
    };

    let mut tx = Transaction::new_with_payer(
        &[
            permission_ix,
            sigver_ix,
            depermission_ix
        ],
        Some(&relayer_signer.pubkey()));
    let recent_blockhash = program_context.last_blockhash.clone();

    tx.partial_sign(&[&relayer_signer], recent_blockhash);
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
