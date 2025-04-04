use {
    super::helpers::get_express_relay_metadata_key,
    crate::token::Token,
    anchor_lang::{
        InstructionData,
        ToAccountMetas,
    },
    anchor_spl::associated_token::{
        get_associated_token_address_with_program_id,
        spl_associated_token_account::instruction::create_associated_token_account_idempotent,
    },
    express_relay::{
        accounts::{
            self,
        },
        instruction::{
            Swap,
            SwapV2,
        },
        FeeToken,
        SwapArgs,
        SwapV2Args,
    },
    solana_sdk::{
        instruction::Instruction,
        pubkey::Pubkey,
    },
};

#[derive(Default)]
pub struct SwapParamOverride {
    pub user_ata_mint_user:             Option<Pubkey>,
    pub mint_fee:                       Option<Pubkey>,
    pub searcher_ta_mint_searcher:      Option<Pubkey>,
    pub searcher_ta_mint_user:          Option<Pubkey>,
    pub express_relay_fee_receiver_ata: Option<Pubkey>,
    pub relayer_fee_receiver_ata:       Option<Pubkey>,
}

pub trait AnyVersionSwapArgs: Sized + Send + Sync {
    fn get_fee_token(&self) -> FeeToken;

    fn into_swap_instruction_data(self) -> Vec<u8>;
}

impl AnyVersionSwapArgs for SwapArgs {
    fn get_fee_token(&self) -> FeeToken {
        self.fee_token
    }

    fn into_swap_instruction_data(self) -> Vec<u8> {
        Swap { data: self }.data()
    }
}

impl AnyVersionSwapArgs for SwapV2Args {
    fn get_fee_token(&self) -> FeeToken {
        self.fee_token
    }

    fn into_swap_instruction_data(self) -> Vec<u8> {
        SwapV2 { data: self }.data()
    }
}

pub struct SwapParams<Args: AnyVersionSwapArgs = SwapArgs> {
    pub searcher:               Pubkey,
    pub user:                   Pubkey,
    pub router_fee_receiver_ta: Pubkey,
    pub fee_receiver_relayer:   Pubkey,
    pub token_searcher:         Token,
    pub token_user:             Token,
    pub swap_args:              Args,
    pub relayer_signer:         Pubkey,
    /// Overrides from default behavior that may result in an invalid instruction
    /// and are meant to be used for testing.
    pub overrides:              SwapParamOverride,
}

/// Builds a swap instruction.
pub fn create_swap_instruction(swap_params: SwapParams<impl AnyVersionSwapArgs>) -> Instruction {
    let SwapParams {
        searcher,
        user,
        router_fee_receiver_ta,
        fee_receiver_relayer,
        swap_args,
        relayer_signer,
        token_searcher,
        token_user,
        overrides:
            SwapParamOverride {
                searcher_ta_mint_searcher,
                searcher_ta_mint_user,
                user_ata_mint_user: user_ata_mint_user_override,
                mint_fee: mint_fee_override,
                express_relay_fee_receiver_ata,
                relayer_fee_receiver_ata,
            },
    } = swap_params;
    let express_relay_metadata = get_express_relay_metadata_key();

    let mint_fee = mint_fee_override.unwrap_or(match swap_args.get_fee_token() {
        FeeToken::Searcher => token_searcher.mint,
        FeeToken::User => token_user.mint,
    });

    let token_program_searcher = token_searcher.token_program;
    let token_program_user = token_user.token_program;

    let token_program_fee = match swap_args.get_fee_token() {
        FeeToken::Searcher => token_program_searcher,
        FeeToken::User => token_program_user,
    };
    let accounts_submit_bid = accounts::Swap {
        searcher,
        user,
        searcher_ta_mint_searcher: searcher_ta_mint_searcher.unwrap_or(
            get_associated_token_address_with_program_id(
                &searcher,
                &token_searcher.mint,
                &token_program_searcher,
            ),
        ),
        searcher_ta_mint_user: searcher_ta_mint_user.unwrap_or(
            get_associated_token_address_with_program_id(
                &searcher,
                &token_user.mint,
                &token_program_user,
            ),
        ),
        user_ata_mint_searcher: get_associated_token_address_with_program_id(
            &user,
            &token_searcher.mint,
            &token_program_searcher,
        ),
        user_ata_mint_user: user_ata_mint_user_override.unwrap_or(
            get_associated_token_address_with_program_id(
                &user,
                &token_user.mint,
                &token_program_user,
            ),
        ),
        router_fee_receiver_ta,
        relayer_fee_receiver_ata: relayer_fee_receiver_ata.unwrap_or(
            get_associated_token_address_with_program_id(
                &fee_receiver_relayer,
                &mint_fee,
                &token_program_fee,
            ),
        ),
        express_relay_fee_receiver_ata: express_relay_fee_receiver_ata.unwrap_or(
            get_associated_token_address_with_program_id(
                &express_relay_metadata,
                &mint_fee,
                &token_program_fee,
            ),
        ),
        mint_searcher: token_searcher.mint,
        mint_user: token_user.mint,
        mint_fee,
        token_program_searcher,
        token_program_user,
        token_program_fee,
        express_relay_metadata,
        relayer_signer,
    }
    .to_account_metas(None);

    let data = swap_args.into_swap_instruction_data();
    Instruction {
        program_id: express_relay::ID,
        accounts: accounts_submit_bid,
        data,
    }
}

/// Builds a set of instructions to perform a swap, including creating the associated token accounts.
pub fn build_swap_instructions(
    swap_params: SwapParams<impl AnyVersionSwapArgs>,
) -> Vec<Instruction> {
    let SwapParams {
        searcher,
        user,
        fee_receiver_relayer,
        token_searcher,
        token_user,
        swap_args,
        overrides:
            SwapParamOverride {
                searcher_ta_mint_user,
                mint_fee: mint_fee_override,
                ..
            },
        ..
    } = &swap_params;
    let mut instructions: Vec<Instruction> = vec![];

    let token_program_searcher = token_searcher.token_program;
    let token_program_user = token_user.token_program;
    let mint_fee = mint_fee_override.unwrap_or(match swap_args.get_fee_token() {
        FeeToken::Searcher => token_searcher.mint,
        FeeToken::User => token_user.mint,
    });
    let token_program_fee = match swap_args.get_fee_token() {
        FeeToken::Searcher => token_program_searcher,
        FeeToken::User => token_program_user,
    };

    if searcher_ta_mint_user.is_none() {
        instructions.push(create_associated_token_account_idempotent(
            searcher,
            searcher,
            &token_user.mint,
            &token_program_user,
        ));
    }
    instructions.push(create_associated_token_account_idempotent(
        searcher,
        user,
        &token_searcher.mint,
        &token_program_searcher,
    ));
    instructions.push(create_associated_token_account_idempotent(
        searcher,
        fee_receiver_relayer,
        &mint_fee,
        &token_program_fee,
    ));
    instructions.push(create_associated_token_account_idempotent(
        searcher,
        &get_express_relay_metadata_key(),
        &mint_fee,
        &token_program_fee,
    ));


    instructions.push(create_swap_instruction(swap_params));

    instructions
}
