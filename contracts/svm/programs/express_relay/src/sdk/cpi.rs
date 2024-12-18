use {
    crate::{
        __cpi_client_accounts_check_permission::CheckPermission,
        cpi::check_permission,
    },
    anchor_lang::prelude::*,
};

/// Makes a CPI call to the `CheckPermission` instruction in the Express Relay program.
/// Permissioning takes the form of a `SubmitBid` instruction with matching permission and router accounts.
/// Returns the fees paid to the router in the matching instructions.
pub fn check_permission_cpi<'info>(
    check_permission_accounts: CheckPermission<'info>,
    express_relay_program: AccountInfo<'info>,
) -> Result<u64> {
    let result = check_permission(CpiContext::new(
        express_relay_program.to_account_info(),
        check_permission_accounts,
    ))?;
    let fees_router = result.get();
    Ok(fees_router)
}
