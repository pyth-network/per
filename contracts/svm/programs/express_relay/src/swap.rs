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
    anchor_spl::{
        associated_token::get_associated_token_address,
        token_interface::{
            Mint,
            TokenAccount,
            TokenInterface,
        },
    },
};

pub struct PostFeeSwapArgs<'info, 'a> {
    pub input_after_fees:  u64,
    pub output_after_fees: u64,
    pub send_swap_fees:    SendSwapFees<'info, 'a>,
}

impl<'info> Swap<'info> {
    pub fn prepare_swap_fees<'a>(&'a self, args: &SwapArgs) -> Result<PostFeeSwapArgs<'info, 'a>> {
        match args.fee_token {
            FeeToken::Input => {
                let SwapFees {
                    express_relay_fee,
                    relayer_fee,
                    router_fee,
                    remaining_amount,
                } = self
                    .express_relay_metadata
                    .compute_swap_fees(args.referral_fee_bps, args.amount_input)?;
                Ok(PostFeeSwapArgs {
                    input_after_fees:  remaining_amount,
                    output_after_fees: args.amount_output,
                    send_swap_fees:    SendSwapFees {
                        router:                 ReceiverAndFee::new(
                            &self.router_fee_receiver_ta,
                            router_fee,
                        ),
                        relayer:                ReceiverAndFee::new(
                            &self.relayer_fee_receiver_ata,
                            relayer_fee,
                        ),
                        express_relay:          ReceiverAndFee::new(
                            &self.express_relay_fee_receiver_ata,
                            express_relay_fee,
                        ),
                        express_relay_metadata: &self.express_relay_metadata,
                        from:                   &self.searcher_input_ta,
                        authority:              &self.searcher,
                        mint:                   &self.mint_input,
                        token_program:          &self.token_program_input,
                    },
                })
            }
            FeeToken::Output => {
                let SwapFees {
                    express_relay_fee,
                    relayer_fee,
                    router_fee,
                    remaining_amount,
                } = self
                    .express_relay_metadata
                    .compute_swap_fees(args.referral_fee_bps, args.amount_output)?;
                Ok(PostFeeSwapArgs {
                    input_after_fees:  args.amount_input,
                    output_after_fees: remaining_amount,
                    send_swap_fees:    SendSwapFees {
                        router:                 ReceiverAndFee::new(
                            &self.router_fee_receiver_ta,
                            router_fee,
                        ),
                        relayer:                ReceiverAndFee::new(
                            &self.relayer_fee_receiver_ata,
                            relayer_fee,
                        ),
                        express_relay:          ReceiverAndFee::new(
                            &self.express_relay_fee_receiver_ata,
                            express_relay_fee,
                        ),
                        express_relay_metadata: &self.express_relay_metadata,
                        from:                   &self.trader_output_ata,
                        authority:              &self.trader,
                        mint:                   &self.mint_output,
                        token_program:          &self.token_program_output,
                    },
                })
            }
        }
    }
}

pub struct ReceiverAndFee<'info, 'a> {
    receiver_ta: &'a InterfaceAccount<'info, TokenAccount>,
    fee:         u64,
}

impl<'info, 'a> ReceiverAndFee<'info, 'a> {
    pub fn new(receiver_ta: &'a InterfaceAccount<'info, TokenAccount>, fee: u64) -> Self {
        Self { receiver_ta, fee }
    }

    pub fn check_receiver_token_account(
        &self,
        mint: &InterfaceAccount<'info, Mint>,
        token_program: &Interface<'info, TokenInterface>,
    ) -> Result<()> {
        require_eq!(self.receiver_ta.mint, mint.key(), ErrorCode::InvalidMint);
        require_eq!(
            *self.receiver_ta.to_account_info().owner,
            token_program.key(),
            ErrorCode::InvalidTokenProgram
        );

        Ok(())
    }

    pub fn check_receiver_associated_token_account(
        &self,
        owner: &Pubkey,
        mint: &InterfaceAccount<'info, Mint>,
        token_program: &Interface<'info, TokenInterface>,
    ) -> Result<()> {
        require_eq!(
            self.receiver_ta.key(),
            get_associated_token_address(owner, &mint.key()),
            ErrorCode::InvalidAta
        );
        self.check_receiver_token_account(mint, token_program)?;
        Ok(())
    }
}

pub struct SendSwapFees<'info, 'a> {
    pub router:                 ReceiverAndFee<'info, 'a>,
    pub relayer:                ReceiverAndFee<'info, 'a>,
    pub express_relay:          ReceiverAndFee<'info, 'a>,
    pub express_relay_metadata: &'a Account<'info, ExpressRelayMetadata>,
    pub from:                   &'a InterfaceAccount<'info, TokenAccount>,
    pub authority:              &'a Signer<'info>,
    pub mint:                   &'a InterfaceAccount<'info, Mint>,
    pub token_program:          &'a Interface<'info, TokenInterface>,
}

impl<'info, 'a> SendSwapFees<'info, 'a> {
    pub fn check_receiver_token_accounts(&self) -> Result<()> {
        self.router
            .check_receiver_token_account(self.mint, self.token_program)?;
        self.relayer.check_receiver_associated_token_account(
            &self.express_relay_metadata.fee_receiver_relayer,
            self.mint,
            self.token_program,
        )?;
        self.express_relay.check_receiver_associated_token_account(
            &self.express_relay_metadata.key(),
            self.mint,
            self.token_program,
        )?;
        Ok(())
    }

    fn transfer_fee(&self, fee_receiver: &ReceiverAndFee<'info, 'a>) -> Result<()> {
        transfer_token_if_needed(
            self.from,
            fee_receiver.receiver_ta,
            self.token_program,
            self.authority,
            self.mint,
            fee_receiver.fee,
        )?;
        Ok(())
    }

    pub fn transfer_fees(&self) -> Result<()> {
        self.transfer_fee(&self.router)?;
        self.transfer_fee(&self.relayer)?;
        self.transfer_fee(&self.express_relay)?;
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
