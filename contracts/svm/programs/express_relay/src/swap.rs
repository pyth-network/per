use {
    crate::{
        error::ErrorCode,
        state::ExpressRelayMetadata,
        token::check_receiver_and_transfer_token_if_needed,
        FeeToken,
        Swap,
        SwapArgs,
        SwapV2Args,
        FEE_SPLIT_PRECISION,
    },
    anchor_lang::{
        accounts::interface_account::InterfaceAccount,
        error::ErrorCode as AnchorErrorCode,
        prelude::*,
    },
    anchor_spl::token_interface::TokenAccount,
};

pub struct PostFeeSwapArgs {
    pub amount_searcher_after_fees: u64,
    pub amount_user_after_fees:     u64,
}


impl<'info> Swap<'info> {
    pub fn convert_to_v2(&self, args: &SwapArgs) -> SwapV2Args {
        SwapV2Args {
            deadline:              args.deadline,
            amount_searcher:       args.amount_searcher,
            amount_user:           args.amount_user,
            fee_token:             args.fee_token,
            referral_fee_bps:      args.referral_fee_bps,
            swap_platform_fee_bps: self.express_relay_metadata.swap_platform_fee_bps,
        }
    }
    pub fn compute_swap_fees<'a>(
        &'a self,
        args: &SwapV2Args,
    ) -> Result<(TransferSwapFeeArgs<'info, 'a>, PostFeeSwapArgs)> {
        match args.fee_token {
            FeeToken::Searcher => {
                let SwapFeesWithRemainingAmount {
                    fees,
                    remaining_amount,
                } = self.express_relay_metadata.compute_swap_fees(
                    args.referral_fee_bps,
                    args.swap_platform_fee_bps,
                    args.amount_searcher,
                )?;
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
                } = self.express_relay_metadata.compute_swap_fees(
                    args.referral_fee_bps,
                    args.swap_platform_fee_bps,
                    args.amount_user,
                )?;
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
        self.transfer_swap_fee_cpi(
            args.fees.router_fee,
            &self.router_fee_receiver_ta,
            None,
            args,
        )?;
        self.transfer_swap_fee_cpi(
            args.fees.relayer_fee,
            &self.relayer_fee_receiver_ata,
            Some(&self.express_relay_metadata.fee_receiver_relayer),
            args,
        )?;
        self.transfer_swap_fee_cpi(
            args.fees.express_relay_fee,
            &self.express_relay_fee_receiver_ata,
            Some(self.express_relay_metadata.to_account_info().key),
            args,
        )?;
        Ok(())
    }

    fn transfer_swap_fee_cpi<'a>(
        &self,
        fee: u64,
        receiver_ta: &UncheckedAccount<'info>,
        receiver: Option<&Pubkey>,
        args: &TransferSwapFeeArgs<'info, 'a>,
    ) -> Result<()> {
        check_receiver_and_transfer_token_if_needed(
            args.from,
            receiver_ta,
            receiver,
            &self.token_program_fee,
            args.authority,
            &self.mint_fee,
            fee,
        )?;
        Ok(())
    }

    pub fn check_enough_balances(&self, args: &SwapV2Args) -> Result<()> {
        require_gte!(
            self.searcher_ta_mint_searcher.amount,
            args.amount_searcher,
            ErrorCode::InsufficientSearcherFunds
        );
        require_gte!(
            self.user_ata_mint_user.amount,
            args.amount_user,
            ErrorCode::InsufficientUserFunds
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
    pub fn check_relayer_signer(&self, relayer_signer: &Pubkey) -> Result<()> {
        if !self.relayer_signer.eq(relayer_signer)
            && !self.secondary_relayer_signer.eq(relayer_signer)
        {
            return Err(AnchorErrorCode::ConstraintHasOne.into());
        }
        Ok(())
    }

    pub fn compute_swap_fees_with_default_platform_fee(
        &self,
        referral_fee_bps: u16,
        amount: u64,
    ) -> Result<SwapFeesWithRemainingAmount> {
        self.compute_swap_fees(referral_fee_bps, self.swap_platform_fee_bps, amount)
    }
    pub fn compute_swap_fees(
        &self,
        referral_fee_bps: u16,
        swap_platform_fee_bps: u64,
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
            .checked_mul(swap_platform_fee_bps)
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
