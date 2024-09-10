use {
    crate::{
        get_matching_instructions,
        ConfigRouter,
        ExpressRelayMetadata,
        PermissionInfo,
        SubmitBidArgs,
        FEE_SPLIT_PRECISION,
    },
    anchor_lang::prelude::*,
};

// Returns the total fees paid to the router for a given permission and router within the current transaction
pub fn get_fees_paid_to_router(
    sysvar_instructions: AccountInfo,
    permission: AccountInfo,
    router: AccountInfo,
    router_config: AccountInfo,
    express_relay_metadata: Account<ExpressRelayMetadata>,
) -> Result<u64> {
    let mut total_fees = 0u64;
    let dataa = &mut &**router_config.try_borrow_data()?;
    let split_router = match ConfigRouter::try_deserialize(dataa) {
        Ok(config_router) => config_router.split,
        Err(_) => express_relay_metadata.split_router_default,
    };
    let matching_ixs = get_matching_instructions(
        sysvar_instructions,
        Some(PermissionInfo {
            permission: *permission.key,
            router:     *router.key,
        }),
    )?;
    for ix in matching_ixs {
        let submit_bid_args = SubmitBidArgs::try_from_slice(&ix.data[8..]).map_err(|_| {
            ProgramError::BorshIoError("Failed to deserialize SubmitBidArgs".to_string())
        })?;
        let bid_amount = submit_bid_args.bid_amount;
        let fee = bid_amount * split_router / FEE_SPLIT_PRECISION;
        total_fees += fee;
    }

    Ok(total_fees)
}
