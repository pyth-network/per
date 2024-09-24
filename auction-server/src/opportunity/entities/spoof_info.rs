use ethers::types::{
    Address,
    U256,
};

#[derive(Clone)]
pub enum SpoofState {
    Spoofed {
        balance_slot:   U256,
        allowance_slot: U256,
    },
    UnableToSpoof,
}

#[derive(Clone)]
pub struct SpoofInfo {
    pub token: Address,
    pub state: SpoofState,
}
