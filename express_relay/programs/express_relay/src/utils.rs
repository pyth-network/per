use anchor_lang::{prelude::*, system_program::{transfer, Transfer}};
use crate::{
    error::ErrorCode,
    state::*,
    SubmitBid,
};

pub fn validate_fee_split(split: u64) -> Result<()> {
    if split > FEE_SPLIT_PRECISION {
        return err!(ErrorCode::FeeSplitLargerThanPrecision);
    }
    Ok(())
}

pub fn transfer_lamports(
    from: &AccountInfo,
    to: &AccountInfo,
    amount: u64,
) -> Result<()> {
    **from.try_borrow_mut_lamports()? -= amount;
    **to.try_borrow_mut_lamports()? += amount;
    Ok(())
}

pub fn transfer_lamports_cpi<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
    system_program: AccountInfo<'info>,
) -> Result<()> {
    let cpi_accounts = Transfer {
        from: from.clone(),
        to: to.clone(),
    };

    transfer(CpiContext::new(system_program, cpi_accounts), amount)?;

    Ok(())
}

pub fn validate_pda(pda: &Pubkey, program_id: &Pubkey, seeds: &[&[u8]]) -> Result<()> {
    let (pda_calculated, _) = Pubkey::find_program_address(seeds, program_id);
    if pda != &pda_calculated {
        return err!(ErrorCode::InvalidPDAProvided);
    }

    Ok(())
}

pub fn handle_bid_payment(ctx: Context<SubmitBid>, bid_amount: u64) -> Result<()> {
    let searcher = &ctx.accounts.searcher;
    let rent_searcher = Rent::get()?.minimum_balance(searcher.to_account_info().data_len());
    if bid_amount + rent_searcher > searcher.lamports() {
        return err!(ErrorCode::InsufficientSearcherFunds);
    }

    let express_relay_metadata = &ctx.accounts.express_relay_metadata;
    let split_relayer = express_relay_metadata.split_relayer;
    let split_protocol_default = express_relay_metadata.split_protocol_default;

    let split_protocol: u64;
    let protocol_config = &ctx.accounts.protocol_config;
    let protocol_config_account_info = protocol_config.to_account_info();
    // validate the protocol config account struct in program logic bc it may be uninitialized
    // only validate if the account has data
    if protocol_config_account_info.data_len() > 0 {
        let account_data = &mut &**protocol_config_account_info.try_borrow_data()?;
        let protocol_config_data = ConfigProtocol::try_deserialize(account_data)?;
        split_protocol = protocol_config_data.split;
    } else {
        split_protocol = split_protocol_default;
    }

    let fee_protocol = bid_amount * split_protocol / FEE_SPLIT_PRECISION;
    if fee_protocol > bid_amount {
        // this error should never be reached due to fee split checks, but kept as a matter of defensive programming
        return err!(ErrorCode::FeesHigherThanBid);
    }

    let fee_relayer = bid_amount.saturating_sub(fee_protocol) * split_relayer / FEE_SPLIT_PRECISION;
    if fee_relayer.checked_add(fee_protocol).ok_or(ProgramError::ArithmeticOverflow)? > bid_amount {
        // this error should never be reached due to fee split checks, but kept as a matter of defensive programming
        return err!(ErrorCode::FeesHigherThanBid);
    }

    let protocol = &ctx.accounts.protocol;
    let fee_receiver_protocol = &ctx.accounts.fee_receiver_protocol;
    if protocol.executable {
        // validate the protocol fee receiver address as pda if protocol is a program
        validate_pda(fee_receiver_protocol.key, protocol.key, &[SEED_EXPRESS_RELAY_FEES])?;
    } else {
        // if protocol is non-executable pubkey, protocol fee receiver address should = protocol address
        assert_eq!(protocol.key, fee_receiver_protocol.key);
    }

    let balance_fee_receiver_protocol = fee_receiver_protocol.lamports();
    let rent_fee_receiver_protocol = Rent::get()?.minimum_balance(0);
    if balance_fee_receiver_protocol+fee_protocol < rent_fee_receiver_protocol {
        return err!(ErrorCode::InsufficientProtocolFeeReceiverRent);
    }

    let fee_receiver_relayer = &ctx.accounts.fee_receiver_relayer;
    let balance_fee_receiver_relayer = fee_receiver_relayer.lamports();
    let rent_fee_receiver_relayer = Rent::get()?.minimum_balance(0);
    if balance_fee_receiver_relayer+fee_relayer < rent_fee_receiver_relayer {
        return err!(ErrorCode::InsufficientRelayerFeeReceiverRent);
    }

    transfer_lamports_cpi(
        &searcher.to_account_info(),
        &fee_receiver_protocol.to_account_info(),
        fee_protocol,
        ctx.accounts.system_program.to_account_info()
    )?;
    transfer_lamports_cpi(
        &searcher.to_account_info(),
        &fee_receiver_relayer.to_account_info(),
        fee_relayer,
        ctx.accounts.system_program.to_account_info()
    )?;
    transfer_lamports_cpi(
        &searcher.to_account_info(),
        &express_relay_metadata.to_account_info(),
        bid_amount.saturating_sub(fee_protocol).saturating_sub(fee_relayer),
        ctx.accounts.system_program.to_account_info()
    )?;

    Ok(())
}
