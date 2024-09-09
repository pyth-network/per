use {
    crate::id as EXPRESS_RELAY_PID,
    anchor_lang::{
        prelude::*,
        Discriminator,
    },
};

// Helper method to create a CPI to the Express Relay program to check permission for a given permission key and router
pub fn check_permission<'info>(
    sysvar_instructions: AccountInfo<'info>,
    permission: AccountInfo<'info>,
    router: AccountInfo<'info>,
) -> Result<()> {
    let data = &crate::instruction::CheckPermission::DISCRIMINATOR.to_vec();

    anchor_lang::solana_program::program::invoke(
        &anchor_lang::solana_program::instruction::Instruction {
            program_id: EXPRESS_RELAY_PID(),
            accounts:   vec![
                AccountMeta::new_readonly(*sysvar_instructions.key, false),
                AccountMeta::new_readonly(*permission.key, false),
                AccountMeta::new_readonly(*router.key, false),
            ],
            data:       data.clone(),
        },
        &[sysvar_instructions, permission, router],
    )
    .map_or_else(|e| Err(Into::into(e)), |_| Ok(()))
}
