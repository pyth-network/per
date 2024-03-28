/// This module contains functions to spoof the balance and allowance of an address in an
/// ERC20 token.
/// This is used to alter the balance and allowance of an address in a token to verify
/// the opportunities in a simulation environment.
/// The spoofing is done by finding the storage slot of the balance and allowance of an address
/// This approach is just a heuristic and will not work for all tokens, specially if the token
/// has a custom storage layout or logic to calculate the balance or allowance
/// Finding the storage slot is done by brute forcing the storage slots (only the first 32 slots)
/// and checking if the output of the balance or allowance is the expected value
use ethers::addressbook::Address;
use {
    crate::{
        opportunity_adapter::ERC20,
        state::SpoofInfo,
    },
    anyhow::anyhow,
    ethers::{
        core::rand,
        prelude::{
            spoof,
            Bytes,
            LocalWallet,
            Provider,
            RawCall,
            Signer,
            H256,
            U256,
        },
        providers::Http,
        utils::keccak256,
    },
    std::sync::Arc,
};

/// Calculate the storage key for the balance of an address in an ERC20 token. This is used to spoof the balance
///
/// # Arguments
///
/// * `owner`: The address of the owner of the balance
/// * `balance_slot`: The slot where the balance mapping is located inside the contract storage
pub fn calculate_balance_storage_key(owner: Address, balance_slot: U256) -> H256 {
    let mut buffer: [u8; 64] = [0; 64];
    buffer[12..32].copy_from_slice(owner.as_bytes());
    balance_slot.to_big_endian(buffer[32..64].as_mut());
    keccak256(Bytes::from(buffer)).into()
}

/// Calculate the storage key for the allowance of an spender for an address in an ERC20 token.
/// This is used to spoof the allowance
///
/// # Arguments
///
/// * `owner`: The address of the owner where the allowance is calculated
/// * `spender`: The address of the spender where the allowance is calculated
/// * `allowance_slot`: The slot where the allowance mapping is located inside the contract storage
pub fn calculate_allowance_storage_key(
    owner: Address,
    spender: Address,
    allowance_slot: U256,
) -> H256 {
    let mut buffer_spender: [u8; 64] = [0; 64];
    buffer_spender[12..32].copy_from_slice(owner.as_bytes());
    allowance_slot.to_big_endian(buffer_spender[32..64].as_mut());
    let spender_slot = keccak256(Bytes::from(buffer_spender));

    let mut buffer_allowance: [u8; 64] = [0; 64];
    buffer_allowance[12..32].copy_from_slice(spender.as_bytes());
    buffer_allowance[32..64].copy_from_slice(&spender_slot);
    keccak256(Bytes::from(buffer_allowance)).into()
}

const MAX_SLOT_FOR_BRUTEFORCE: i32 = 32;

/// Find the balance slot of an ERC20 token that can be used to spoof the balance of an address
/// Returns an error if no slot is found or if the network calls fail
///
/// # Arguments
///
/// * `token`: ERC20 token address
/// * `client`: Client to interact with the blockchain
async fn find_spoof_balance_slot(
    token: Address,
    client: Arc<Provider<Http>>,
) -> anyhow::Result<U256> {
    let contract = ERC20::new(token, client.clone());
    let fake_owner = LocalWallet::new(&mut rand::thread_rng());
    for balance_slot in 0..MAX_SLOT_FOR_BRUTEFORCE {
        let tx = contract.balance_of(fake_owner.address()).tx;
        let mut state = spoof::State::default();
        let balance_storage_key =
            calculate_balance_storage_key(fake_owner.address(), balance_slot.into());
        let value: [u8; 32] = rand::random();
        state
            .account(token)
            .store(balance_storage_key, value.into());
        let result = client.call_raw(&tx).state(&state).await?;
        if result == Bytes::from(value) {
            return Ok(balance_slot.into());
        }
    }
    Err(anyhow!("Could not find balance slot"))
}

/// Find the allowance slot of an ERC20 token that can be used to spoof the allowance of an address
/// Returns an error if no slot is found or if the network calls fail
///
/// # Arguments
///
/// * `token`: ERC20 token address
/// * `client`: Client to interact with the blockchain
async fn find_spoof_allowance_slot(
    token: Address,
    client: Arc<Provider<Http>>,
) -> anyhow::Result<U256> {
    let contract = ERC20::new(token, client.clone());
    let fake_owner = LocalWallet::new(&mut rand::thread_rng());
    let fake_spender = LocalWallet::new(&mut rand::thread_rng());

    for allowance_slot in 0..MAX_SLOT_FOR_BRUTEFORCE {
        let tx = contract
            .allowance(fake_owner.address(), fake_spender.address())
            .tx;
        let mut state = spoof::State::default();
        let allowance_storage_key = calculate_allowance_storage_key(
            fake_owner.address(),
            fake_spender.address(),
            allowance_slot.into(),
        );
        let value: [u8; 32] = rand::random();
        state
            .account(token)
            .store(allowance_storage_key, value.into());
        let result = client.call_raw(&tx).state(&state).await?;
        if result == Bytes::from(value) {
            return Ok(allowance_slot.into());
        }
    }
    Err(anyhow!("Could not find allowance slot"))
}

/// Find the spoof info for an ERC20 token. This includes the balance slot and the allowance slot
/// Returns an error if no balance or allowance slot is found
/// # Arguments
///
/// * `token`: ERC20 token address
/// * `client`: Client to interact with the blockchain
pub async fn find_spoof_info(
    token: Address,
    client: Arc<Provider<Http>>,
) -> anyhow::Result<SpoofInfo> {
    let balance_slot = find_spoof_balance_slot(token, client.clone()).await?;
    let allowance_slot = find_spoof_allowance_slot(token, client.clone()).await?;
    Ok(SpoofInfo::Spoofed {
        balance_slot,
        allowance_slot,
    })
}
