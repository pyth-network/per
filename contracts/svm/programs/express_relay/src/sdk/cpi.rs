use {
    crate::{
        __cpi_client_accounts_check_permission::CheckPermission,
        cpi::check_permission,
    },
    anchor_lang::prelude::*,
};

pub fn check_permission_cpi<'info>(
    check_permission_accounts: CheckPermission<'info>,
    express_relay_program: AccountInfo<'info>,
) -> Result<(u16, u64)> {
    let result = check_permission(CpiContext::new(
        express_relay_program.to_account_info(),
        check_permission_accounts,
    ))?;
    let (n_bid_ixs, fees) = result.get();

    Ok((n_bid_ixs, fees))
}
