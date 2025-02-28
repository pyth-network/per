use {
    crate::{
        error::ErrorCode,
        state::ExpressRelayMetadata,
        token::transfer_token_if_needed,
        FeeToken,
        Swap,
        SwapArgs,
        FEE_SPLIT_PRECISION,
    },
    anchor_lang::{
        accounts::interface_account::InterfaceAccount,
        prelude::*,
    },
    anchor_spl::token_interface::TokenAccount,
};

pub struct PostFeeSwapArgs {
    pub amount_searcher_after_fees: u64,
    pub amount_user_after_fees:     u64,
}


impl<'info> Swap<'info> {
    pub fn compute_swap_fees<'a>(
        &'a self,
        args: &SwapArgs,
    ) -> Result<(TransferSwapFeeArgs<'info, 'a>, PostFeeSwapArgs)> {
        match args.fee_token {
            FeeToken::Searcher => {
                let SwapFeesWithRemainingAmount {
                    fees,
                    remaining_amount,
                } = self
                    .express_relay_metadata
                    .compute_swap_fees(args.referral_fee_bps, args.amount_searcher)?;
                Ok((
                    TransferSwapFeeArgs {
                        fees,
                        from: &self.searcher_ta_mint_searcher,
                        authority: &self.searcher,
                    },
                    PostFeeSwapArgs {
                        amount_searcher_after_fees: remaining_amount,
                        amount_user_after_fees:     args.amount_user,
                    },
                ))
            }
            FeeToken::User => {
                let SwapFeesWithRemainingAmount {
                    fees,
                    remaining_amount,
                } = self
                    .express_relay_metadata
                    .compute_swap_fees(args.referral_fee_bps, args.amount_user)?;
                Ok((
                    TransferSwapFeeArgs {
                        fees,
                        from: &self.user_ata_mint_user,
                        authority: &self.user,
                    },
                    PostFeeSwapArgs {
                        amount_searcher_after_fees: args.amount_searcher,
                        amount_user_after_fees:     remaining_amount,
                    },
                ))
            }
        }
    }

    pub fn transfer_swap_fees_cpi<'a>(&self, args: &TransferSwapFeeArgs<'info, 'a>) -> Result<()> {
        self.transfer_swap_fee_cpi(args.fees.router_fee, &self.router_fee_receiver_ta, args)?;
        self.transfer_swap_fee_cpi(args.fees.relayer_fee, &self.relayer_fee_receiver_ata, args)?;
        self.transfer_swap_fee_cpi(
            args.fees.express_relay_fee,
            &self.express_relay_fee_receiver_ata,
            args,
        )?;
        Ok(())
    }

    fn transfer_swap_fee_cpi<'a>(
        &self,
        fee: u64,
        receiver_ta: &InterfaceAccount<'info, TokenAccount>,
        args: &TransferSwapFeeArgs<'info, 'a>,
    ) -> Result<()> {
        transfer_token_if_needed(
            args.from,
            receiver_ta,
            &self.token_program_fee,
            args.authority,
            &self.mint_fee,
            fee,
        )?;
        Ok(())
    }

    pub fn check_enough_balances(&self, args: &SwapArgs) -> Result<()> {
        require_gte!(
            self.searcher_ta_mint_searcher.amount,
            args.amount_searcher,
            ErrorCode::SearcherInsufficientBalance
        );
        // We want this check to be the last
        require_gte!(
            self.user_ata_mint_user.amount,
            args.amount_user,
            ErrorCode::UserInsufficientBalance
        );
        Ok(())
    }
}

pub struct TransferSwapFeeArgs<'info, 'a> {
    pub fees:      SwapFees,
    pub from:      &'a InterfaceAccount<'info, TokenAccount>,
    pub authority: &'a Signer<'info>,
}

pub struct SwapFeesWithRemainingAmount {
    pub fees:             SwapFees,
    pub remaining_amount: u64,
}

pub struct SwapFees {
    pub router_fee:        u64,
    pub relayer_fee:       u64,
    pub express_relay_fee: u64,
}
impl ExpressRelayMetadata {
    pub fn compute_swap_fees(
        &self,
        referral_fee_bps: u16,
        amount: u64,
    ) -> Result<SwapFeesWithRemainingAmount> {
        if u64::from(referral_fee_bps) > FEE_SPLIT_PRECISION {
            return Err(ErrorCode::InvalidReferralFee.into());
        }
        let router_fee = amount
            .checked_mul(referral_fee_bps.into())
            .ok_or(ProgramError::ArithmeticOverflow)?
            / FEE_SPLIT_PRECISION;
        let platform_fee = amount
            .checked_mul(self.swap_platform_fee_bps)
            .ok_or(ProgramError::ArithmeticOverflow)?
            / FEE_SPLIT_PRECISION;
        let relayer_fee = platform_fee
            .checked_mul(self.split_relayer)
            .ok_or(ProgramError::ArithmeticOverflow)?
            / FEE_SPLIT_PRECISION;

        let remaining_amount = amount
            .checked_sub(router_fee)
            .ok_or(ProgramError::ArithmeticOverflow)?
            .checked_sub(platform_fee)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        let express_relay_fee = platform_fee
            .checked_sub(relayer_fee)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        Ok(SwapFeesWithRemainingAmount {
            fees: SwapFees {
                router_fee,
                relayer_fee,
                express_relay_fee,
            },
            remaining_amount,
        })
    }
}
