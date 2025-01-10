use {
    crate::error::ErrorCode,
    anchor_lang::prelude::*,
};

pub fn check_deadline(deadline: i64) -> Result<()> {
    if deadline < Clock::get()?.unix_timestamp {
        return err!(ErrorCode::DeadlinePassed);
    }
    Ok(())
}
