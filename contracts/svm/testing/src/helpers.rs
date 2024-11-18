use {
    litesvm::types::TransactionResult,
    solana_sdk::{
        instruction::{
            Instruction,
            InstructionError::Custom,
        },
        native_token::LAMPORTS_PER_SOL,
        program_pack::Pack,
        pubkey::Pubkey,
        signature::Keypair,
        signer::Signer,
        system_instruction,
        sysvar::clock::Clock,
        transaction::{
            Transaction,
            TransactionError::{
                self,
                InstructionError,
            },
        },
    },
    spl_associated_token_account::instruction as associated_token_instruction,
    spl_token::instruction as token_instruction,
};

pub const TX_FEE: u64 = 10_000; // TODO: make this programmatic? FeeStructure is currently private field within LiteSVM

pub fn submit_transaction(
    svm: &mut litesvm::LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> TransactionResult {
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&payer.pubkey()),
        signers,
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
}

pub fn generate_and_fund_key(svm: &mut litesvm::LiteSVM) -> Keypair {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();
    svm.airdrop(&pubkey, 10 * LAMPORTS_PER_SOL).unwrap();
    keypair
}

pub fn create_mint(
    svm: &mut litesvm::LiteSVM,
    mint: &Keypair,
    payer: &Keypair,
    token_program: &Pubkey,
) {
    let ix_create_account = system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        svm.minimum_balance_for_rent_exemption(spl_token::state::Mint::LEN),
        spl_token::state::Mint::LEN as u64,
        token_program,
    );
    let ix_create_mint =
        token_instruction::initialize_mint(token_program, &mint.pubkey(), &payer.pubkey(), None, 9)
            .unwrap();
    submit_transaction(
        svm,
        &[ix_create_account, ix_create_mint],
        payer,
        &[payer, mint],
    )
    .unwrap();
}

// TODO: for now, this is assuming the provided token account is an ATA
pub fn initialize_ata(
    svm: &mut litesvm::LiteSVM,
    mint: &Pubkey,
    owner: &Pubkey,
    token_program: &Pubkey,
    payer: &Keypair,
) {
    let ix_create_ata = associated_token_instruction::create_associated_token_account_idempotent(
        &payer.pubkey(),
        owner,
        mint,
        token_program,
    );
    submit_transaction(svm, &[ix_create_ata], payer, &[payer]).unwrap();
}

pub fn mint_tokens(
    svm: &mut litesvm::LiteSVM,
    mint: &Pubkey,
    token_account: &Pubkey,
    amount: u64,
    auth: &Keypair,
    token_program: &Pubkey,
) {
    let ix = token_instruction::mint_to(
        token_program,
        mint,
        token_account,
        &auth.pubkey(),
        &[&auth.pubkey()],
        amount,
    )
    .unwrap();
    submit_transaction(svm, &[ix], auth, &[auth]).unwrap();
}

pub fn get_balance(svm: &litesvm::LiteSVM, pubkey: &Pubkey) -> u64 {
    svm.get_balance(pubkey).unwrap_or(0)
}

pub fn get_spl_balance(svm: &litesvm::LiteSVM, ta_pubkey: &Pubkey) -> u64 {
    let account = svm.get_account(ta_pubkey).unwrap();
    let account_data = spl_token::state::Account::unpack(&account.data).unwrap();
    account_data.amount
}

pub fn warp_to_unix(svm: &mut litesvm::LiteSVM, unix_timestamp: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = unix_timestamp + 1;
    svm.set_sysvar(&clock);
}

pub fn assert_custom_error(error: TransactionError, instruction_index: u8, custom_error: u32) {
    match error {
        InstructionError(index, error_variant) => {
            assert_eq!(index, instruction_index);
            match error_variant {
                Custom(code) => {
                    assert_eq!(code, custom_error);
                }
                _ => panic!("Unexpected error code"),
            }
        }
        _ => panic!("Unexpected error variant"),
    }
}
