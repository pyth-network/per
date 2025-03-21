use {
    async_trait::async_trait,
    ethers::{
        prelude::{
            gas_oracle::{
                GasOracleError,
                Result,
            },
            GasOracle,
        },
        providers::Middleware,
        types::{
            I256,
            U256,
        },
    },
};

// The default fee estimation logic in ethers.rs includes some hardcoded constants that do not
// work well in layer 2 networks because it lower bounds the priority fee at 3 gwei.
// Unfortunately this logic is not configurable in ethers.rs.
//
// See original logic here:
// https://github.com/gakonst/ethers-rs/blob/master/ethers-providers/src/rpc/provider.rs#L452

/// The default max priority fee per gas, used in case the base fee is within a threshold.
pub const EIP1559_FEE_ESTIMATION_DEFAULT_PRIORITY_FEE: u64 = 1_000_000;

/// The threshold max change/difference (in %) at which we will ignore the fee history values
/// under it.
pub const EIP1559_FEE_ESTIMATION_THRESHOLD_MAX_CHANGE: i64 = 200;

/// Gas oracle from a [`Middleware`] implementation such as an
/// Ethereum RPC provider.
#[derive(Clone, Debug)]
#[must_use]
pub struct EthProviderOracle<M: Middleware> {
    provider: M,
}

impl<M: Middleware> EthProviderOracle<M> {
    pub fn new(provider: M) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl<M: Middleware> GasOracle for EthProviderOracle<M>
where
    M::Error: 'static,
{
    async fn fetch(&self) -> Result<U256> {
        self.provider
            .get_gas_price()
            .await
            .map_err(|err| GasOracleError::ProviderError(Box::new(err)))
    }

    async fn estimate_eip1559_fees(&self) -> Result<(U256, U256)> {
        self.provider
            .estimate_eip1559_fees(Some(eip1559_default_estimator))
            .await
            .map_err(|err| GasOracleError::ProviderError(Box::new(err)))
    }
}

/// The default EIP-1559 fee estimator which is based on the work by [MyCrypto](https://github.com/MyCryptoHQ/MyCrypto/blob/master/src/services/ApiService/Gas/eip1559.ts).
pub fn eip1559_default_estimator(base_fee_per_gas: U256, rewards: Vec<Vec<U256>>) -> (U256, U256) {
    let max_priority_fee_per_gas = std::cmp::max(
        estimate_priority_fee(rewards),
        U256::from(EIP1559_FEE_ESTIMATION_DEFAULT_PRIORITY_FEE),
    );

    // Minimum of 4 blocks needed to 1.6 the base fee.
    let potential_max_fee = base_fee_per_gas * 16 / 10;
    (
        potential_max_fee + max_priority_fee_per_gas,
        max_priority_fee_per_gas,
    )
}

fn estimate_priority_fee(rewards: Vec<Vec<U256>>) -> U256 {
    let mut rewards: Vec<U256> = rewards
        .iter()
        .map(|r| r[0])
        .filter(|r| *r > U256::zero())
        .collect();
    if rewards.is_empty() {
        return U256::zero();
    }
    if rewards.len() == 1 {
        return rewards[0];
    }
    // Sort the rewards as we will eventually take the median.
    rewards.sort();

    // A copy of the same vector is created for convenience to calculate percentage change
    // between subsequent fee values.
    let rotated_rewards = {
        let mut rewards = rewards.clone();
        rewards.rotate_left(1);
        rewards
    };

    let mut percentage_change: Vec<I256> = rewards
        .iter()
        .zip(rotated_rewards.iter())
        .map(|(a, b)| {
            let a = I256::try_from(*a).expect("priority fee overflow");
            let b = I256::try_from(*b).expect("priority fee overflow");
            ((b - a) * 100) / a
        })
        .collect();
    percentage_change.pop();

    // Fetch the max of the percentage change, and that element's index.
    let max_change = percentage_change.iter().max().unwrap();
    let max_change_index = percentage_change
        .iter()
        .position(|&c| c == *max_change)
        .unwrap();

    // If we encountered a big change in fees at a certain position, then consider only
    // the values >= it.
    let values = if *max_change >= EIP1559_FEE_ESTIMATION_THRESHOLD_MAX_CHANGE.into()
        && (max_change_index >= (rewards.len() / 2))
    {
        rewards[max_change_index..].to_vec()
    } else {
        rewards
    };

    // Return the median.
    values[values.len() / 2]
}
