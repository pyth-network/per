use {
    crate::{
        state::ExpressRelayMetadata,
        token::{
            check_receiver_associated_token_account,
            check_receiver_token_account,
            transfer_token_if_needed,
        },
        FEE_SPLIT_PRECISION,
    },
    anchor_lang::{
        accounts::interface_account::InterfaceAccount,
        prelude::*,
    },
    anchor_spl::token_interface::{
        Mint,
        TokenAccount,
        TokenInterface,
    },
};

pub struct SendSwapFee<'info> {
    pub fee_router:                     u64,
    pub fee_relayer:                    u64,
    pub fee_express_relay:              u64,
    pub router_fee_receiver_ta:         InterfaceAccount<'info, TokenAccount>,
    pub relayer_fee_receiver_ata:       InterfaceAccount<'info, TokenAccount>,
    pub express_relay_fee_receiver_ata: InterfaceAccount<'info, TokenAccount>,
    pub express_relay_metadata:         Account<'info, ExpressRelayMetadata>,
    pub from:                           InterfaceAccount<'info, TokenAccount>,
    pub authority:                      Signer<'info>,
    pub mint:                           InterfaceAccount<'info, Mint>,
    pub token_program:                  Interface<'info, TokenInterface>,
}

impl<'info> SendSwapFee<'info> {
    pub fn check_receiver_token_accounts(&self) -> Result<()> {
        check_receiver_token_account(
            &self.router_fee_receiver_ta,
            &self.mint,
            &self.token_program,
        )?;
        check_receiver_associated_token_account(
            &self.relayer_fee_receiver_ata,
            &self.express_relay_metadata.relayer_signer,
            &self.mint,
            &self.token_program,
        )?;
        check_receiver_associated_token_account(
            &self.express_relay_fee_receiver_ata,
            &self.express_relay_metadata.key(),
            &self.mint,
            &self.token_program,
        )?;
        Ok(())
    }


    pub fn transfer_fee(
        &self,
        fee_receiver: &InterfaceAccount<'info, TokenAccount>,
        fee: u64,
    ) -> Result<()> {
        transfer_token_if_needed(
            &self.from,
            fee_receiver,
            &self.token_program,
            &self.authority,
            &self.mint,
            fee,
        )?;
        Ok(())
    }

    pub fn transfer_fees(&self) -> Result<()> {
        self.transfer_fee(&self.router_fee_receiver_ta, self.fee_router)?;
        self.transfer_fee(&self.relayer_fee_receiver_ata, self.fee_relayer)?;
        self.transfer_fee(&self.express_relay_fee_receiver_ata, self.fee_express_relay)?;
        Ok(())
    }
}

pub struct SwapFees {
    pub fee_router:        u64,
    pub fee_relayer:       u64,
    pub fee_express_relay: u64,
    pub remaining_amount:  u64,
}
impl ExpressRelayMetadata {
    pub fn compute_swap_fees(&self, referral_fee_bps: u64, amount: u64) -> Result<SwapFees> {
        let fee_total = amount
            .checked_mul(referral_fee_bps)
            .ok_or(ProgramError::ArithmeticOverflow)?
            / FEE_SPLIT_PRECISION;
        let fee_platform = fee_total
            .checked_mul(self.swap_platform_fee)
            .ok_or(ProgramError::ArithmeticOverflow)?
            / FEE_SPLIT_PRECISION;
        let fee_relayer = fee_platform
            .checked_mul(self.split_relayer)
            .ok_or(ProgramError::ArithmeticOverflow)?
            / FEE_SPLIT_PRECISION;

        let remaining_amount = amount
            .checked_sub(fee_total)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        let fee_router = fee_total
            .checked_sub(fee_platform)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        let fee_express_relay = fee_platform
            .checked_sub(fee_relayer)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        Ok(SwapFees {
            fee_router,
            fee_relayer,
            fee_express_relay,
            remaining_amount,
        })
    }
}
