use anchor_lang::{InstructionData, ToAccountMetas};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer, system_program, sysvar::instructions::id as sysvar_instructions_id};
use express_relay::{accounts::SubmitBid, SubmitBidArgs};

use super::helpers::{get_express_relay_metadata_key, get_protocol_config_key};

pub fn get_bid_instructions(
    relayer_signer: &Keypair,
    searcher: &Keypair,
    protocol: Pubkey,
    fee_receiver_relayer: Pubkey,
    fee_receiver_protocol: Pubkey,
    permission: Pubkey,
    bid_amount: u64,
    deadline: i64,
    ixs: &[Instruction],
) -> Vec<Instruction> {
    let express_relay_metadata = get_express_relay_metadata_key();
    let protocol_config = get_protocol_config_key(protocol);

    let submit_bid_ix = Instruction {
        program_id: express_relay::id(),
        data: express_relay::instruction::SubmitBid {
            data: SubmitBidArgs {
                deadline,
                bid_amount,
            }
        }.data(),
        accounts: SubmitBid {
            relayer_signer: relayer_signer.pubkey(),
            searcher: searcher.pubkey(),
            permission: permission,
            protocol: protocol,
            protocol_config: protocol_config,
            fee_receiver_relayer: fee_receiver_relayer,
            fee_receiver_protocol: fee_receiver_protocol,
            express_relay_metadata: express_relay_metadata,
            system_program: system_program::ID,
            sysvar_instructions: sysvar_instructions_id(),
        }.to_account_metas(None),
    };

    return [&[submit_bid_ix], ixs].concat().iter().map(|ix| ix.clone()).collect();
}
