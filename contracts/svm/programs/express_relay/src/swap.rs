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

pub struct SendSwapFees<'info> {
    pub router_fee:                     u64,
    pub relayer_fee:                    u64,
    pub express_relay_fee:              u64,
    pub router_fee_receiver_ta:         InterfaceAccount<'info, TokenAccount>,
    pub relayer_fee_receiver_ata:       InterfaceAccount<'info, TokenAccount>,
    pub express_relay_fee_receiver_ata: InterfaceAccount<'info, TokenAccount>,
    pub express_relay_metadata:         Account<'info, ExpressRelayMetadata>,
    pub from:                           InterfaceAccount<'info, TokenAccount>,
    pub authority:                      Signer<'info>,
    pub mint:                           InterfaceAccount<'info, Mint>,
    pub token_program:                  Interface<'info, TokenInterface>,
}

impl<'info> SendSwapFees<'info> {
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
        self.transfer_fee(&self.router_fee_receiver_ta, self.router_fee)?;
        self.transfer_fee(&self.relayer_fee_receiver_ata, self.relayer_fee)?;
        self.transfer_fee(&self.express_relay_fee_receiver_ata, self.express_relay_fee)?;
        Ok(())
    }
}

pub struct SwapFees {
    pub router_fee:        u64,
    pub relayer_fee:       u64,
    pub express_relay_fee: u64,
    pub remaining_amount:  u64,
}
impl ExpressRelayMetadata {
    pub fn compute_swap_fees(&self, referral_fee_bps: u64, amount: u64) -> Result<SwapFees> {
        let total_fee = amount
            .checked_mul(referral_fee_bps)
            .ok_or(ProgramError::ArithmeticOverflow)?
            / FEE_SPLIT_PRECISION;
        let platform_fee = total_fee
            .checked_mul(self.swap_platform_fee)
            .ok_or(ProgramError::ArithmeticOverflow)?
            / FEE_SPLIT_PRECISION;
        let relayer_fee = platform_fee
            .checked_mul(self.split_relayer)
            .ok_or(ProgramError::ArithmeticOverflow)?
            / FEE_SPLIT_PRECISION;

        let remaining_amount = amount
            .checked_sub(total_fee)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        let router_fee = total_fee
            .checked_sub(platform_fee)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        let express_relay_fee = platform_fee
            .checked_sub(relayer_fee)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        Ok(SwapFees {
            router_fee,
            relayer_fee,
            express_relay_fee,
            remaining_amount,
        })
    }
}
