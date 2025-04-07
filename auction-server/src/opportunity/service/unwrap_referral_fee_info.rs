use {
    super::{
        ChainTypeSvm,
        Service,
    },
    crate::{
        api::RestError,
        config::ChainId,
        opportunity::entities::ReferralFeeInfo,
    },
};

impl Service<ChainTypeSvm> {
    /// Extracts router and referral_fee_bps from an option.
    /// If the option is None, it uses the express relay metadata address as the router.
    /// This is because no fees need to be paid, and the express relay metadata token account must already exist as per the program.
    pub fn unwrap_referral_fee_info(
        &self,
        referral_fee_info: Option<ReferralFeeInfo>,
        chain_id: &ChainId,
    ) -> Result<ReferralFeeInfo, RestError> {
        match referral_fee_info {
            Some(referral_fee_info) => Ok(referral_fee_info),
            None => {
                let config = self.get_config(chain_id)?;
                Ok(ReferralFeeInfo {
                    router:           Service::<ChainTypeSvm>::calculate_metadata_address(config),
                    referral_fee_bps: 0,
                })
            }
        }
    }
}
