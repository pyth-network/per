use {
    crate::auction::entities::Bid,
    futures::future::join_all,
    litesvm::{
        types::{
            FailedTransactionMetadata,
            SimulatedTransactionInfo,
        },
        LiteSVM,
    },
    solana_client::{
        client_error,
        rpc_response::{
            Response,
            RpcResponseContext,
            RpcResult,
        },
    },
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        account::{
            Account,
            ReadableAccount,
        },
        account_utils::StateMut,
        address_lookup_table::state::AddressLookupTable,
        bpf_loader_upgradeable::UpgradeableLoaderState,
        clock::Clock,
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::Signature,
        slot_hashes::SlotHashes,
        transaction::VersionedTransaction,
    },
    std::{
        collections::{
            HashMap,
            HashSet,
        },
        time::Instant,
    },
    time::{
        Duration,
        OffsetDateTime,
    },
    tokio::sync::RwLock,
};

pub struct Simulator {
    receiver:      RpcClient,
    pending_txs:   RwLock<Vec<(VersionedTransaction, Instant)>>,
    account_cache: RwLock<HashMap<Pubkey, (Account, Instant)>>,
}

struct AccountsConfig {
    accounts:      HashMap<Pubkey, Account>,
    programs_data: HashMap<Pubkey, Account>,
}

impl AccountsConfig {
    fn new() -> Self {
        Self {
            accounts:      Default::default(),
            programs_data: Default::default(),
        }
    }

    /// Adds all the accounts to the LiteSVM instance according to their type
    fn apply(&self, svm: &mut LiteSVM) {
        // Need to set the program executable data before the program accounts
        for (key, account) in self.programs_data.iter() {
            if let Err(e) = svm.set_account(*key, account.clone()) {
                tracing::error!("Failed to set program data for key {:?} {:?}", key, e);
            }
        }
        for (key, account) in self.accounts.iter() {
            if let Err(e) = svm.set_account(*key, account.clone()) {
                tracing::error!("Failed to set account for key {:?} {:?}", key, e);
            }
        }
    }
}

// TODO: Remove pending transactions if the submit bid deadline is reached
/// Maximum duration for a transaction to be considered pending without any confirmation on-chain
/// This value may differ from how long the auction server retries to send the transaction
const MAX_PENDING_DURATION: Duration = Duration::seconds(15);

/// Cache duration for accounts that are not expected to change frequently
/// (Program accounts, Lookup Table accounts)
const ACCOUNT_CACHE_DURATION: Duration = Duration::hours(1);

impl Simulator {
    pub fn new(receiver: RpcClient) -> Self {
        Self {
            receiver,
            pending_txs: Default::default(),
            account_cache: Default::default(),
        }
    }

    pub async fn fetch_pending_and_remove_old_txs(&self) -> Vec<VersionedTransaction> {
        let now = Instant::now();
        let mut pending_txs = self.pending_txs.write().await;
        pending_txs.retain(|(_, time)| now.duration_since(*time) < MAX_PENDING_DURATION);
        pending_txs.iter().map(|(tx, _)| tx.clone()).collect()
    }

    /// Adds a pending transaction to the simulator to be considered in the next simulations
    /// This function should be called when a transaction is submitted to the chain
    pub async fn add_pending_transaction(&self, tx: &VersionedTransaction) {
        let now = Instant::now();
        self.pending_txs.write().await.push((tx.clone(), now));
    }

    /// Removes a pending transaction from the simulator
    /// This function should be called when a transaction is confirmed on-chain
    pub async fn remove_pending_transaction(&self, sig: &Signature) {
        self.pending_txs.write().await.retain(|(tx, _)| {
            !tx.signatures
                .first()
                .map(|tx_sig| tx_sig.eq(sig))
                .unwrap_or(false)
        });
    }

    /// Tries to get accounts from cache, if any of them are not found, returns None.
    async fn try_get_accounts_from_cache(&self, keys: &[Pubkey]) -> Option<Vec<Account>> {
        let mut cache_result = vec![];
        let cache = self.account_cache.read().await;
        for key in keys.iter() {
            if let Some((account, update_time)) = cache.get(key) {
                if Instant::now().duration_since(*update_time) > ACCOUNT_CACHE_DURATION {
                    return None;
                }
                cache_result.push(account.clone());
            } else {
                return None;
            }
        }
        Some(cache_result)
    }

    /// Tries to get accounts from cache, if any of them are not found, fetches all of them from RPC
    /// and updates the cache.
    /// You should only use this function for accounts that are not expected to change frequently:
    /// - Program accounts
    /// - Lookup Table accounts
    async fn get_multiple_accounts_with_cache(
        &self,
        keys: &[Pubkey],
    ) -> client_error::Result<Vec<Option<Account>>> {
        if let Some(accounts) = self.try_get_accounts_from_cache(keys).await {
            return Ok(accounts.into_iter().map(Some).collect());
        }
        let result = self.get_multiple_accounts_chunked(keys).await?;
        let mut cache = self.account_cache.write().await;
        for (key, account) in keys.iter().zip(result.value.iter()) {
            if let Some(account) = account {
                cache.insert(*key, (account.clone(), Instant::now()));
            }
        }
        Ok(result.value)
    }

    /// Fetches multiple accounts from the RPC in chunks
    /// There is no guarantee that all the accounts will be fetched with the same slot
    async fn get_multiple_accounts_chunked(
        &self,
        keys: &[Pubkey],
    ) -> RpcResult<Vec<Option<Account>>> {
        let mut result = vec![];
        let mut context_with_min_slot: Option<RpcResponseContext> = None;
        const MAX_RPC_ACCOUNT_LIMIT: usize = 100;
        // Ensure at least one call is made, even if keys is empty
        let key_chunks = if keys.is_empty() {
            vec![&[][..]]
        } else {
            keys.chunks(MAX_RPC_ACCOUNT_LIMIT).collect()
        };

        // Process chunks in parallel
        let chunk_results = join_all(key_chunks.into_iter().map(|chunk| {
            self.receiver
                .get_multiple_accounts_with_commitment(chunk, CommitmentConfig::processed())
        }))
        .await;
        for chunk_result in chunk_results {
            let chunk_result = chunk_result?;
            result.extend(chunk_result.value);
            if context_with_min_slot.is_none()
                || context_with_min_slot.as_ref().unwrap().slot > chunk_result.context.slot
            {
                context_with_min_slot = Some(chunk_result.context);
            }
        }
        Ok(Response {
            value:   result,
            context: context_with_min_slot.unwrap(), // Safe because we ensured at least one call was made
        })
    }

    async fn resolve_lookup_addresses(
        &self,
        transactions: &[&VersionedTransaction],
    ) -> client_error::Result<Vec<Pubkey>> {
        let mut lookup_table_keys: HashMap<Pubkey, HashSet<u8>> = HashMap::default();
        transactions
            .iter()
            .flat_map(|tx| tx.message.address_table_lookups().unwrap_or_default())
            .for_each(|x| {
                lookup_table_keys.entry(x.account_key).or_default().extend(
                    x.readonly_indexes
                        .iter()
                        .chain(x.writable_indexes.iter())
                        .cloned()
                        .collect::<Vec<_>>(),
                )
            });

        let accs = self
            .get_multiple_accounts_with_cache(
                &lookup_table_keys.keys().cloned().collect::<Vec<_>>(),
            )
            .await?;
        let mut results = vec![];
        for ((lookup_table_address, indexes), lookup_table_account) in
            lookup_table_keys.iter().zip(accs.iter())
        {
            results.push(*lookup_table_address);
            if let Some(lookup_account) = lookup_table_account {
                if let Ok(table_data_deserialized) =
                    AddressLookupTable::deserialize(&lookup_account.data)
                {
                    if let Ok(addrs) = table_data_deserialized.lookup(
                        table_data_deserialized.meta.last_extended_slot + 1,
                        &indexes.iter().cloned().collect::<Vec<_>>(),
                        &SlotHashes::default(),
                    ) {
                        results.extend(addrs.iter());
                    }
                }
            }
        }
        Ok(results)
    }

    /// Fetches all the accounts needed for simulating the transactions via RPC
    /// Uses the account cache to avoid fetching programs and lookup tables multiple times
    /// Returns an AccountsConfig struct that can be used to initialize the LiteSVM instance
    #[tracing::instrument(skip_all, fields(slot))]
    async fn fetch_tx_accounts_via_rpc(
        &self,
        transactions: &[&VersionedTransaction],
    ) -> RpcResult<AccountsConfig> {
        let mut keys = transactions
            .iter()
            .flat_map(|tx| tx.message.static_account_keys())
            .cloned()
            .collect::<HashSet<_>>();

        keys.extend(self.resolve_lookup_addresses(transactions).await?);
        let keys = keys.into_iter().collect::<Vec<_>>();

        let accounts_with_context = self.get_multiple_accounts_chunked(&keys).await?;
        tracing::Span::current().record("slot", accounts_with_context.context.slot);
        let accounts = accounts_with_context.value;
        let mut accounts_config = AccountsConfig::new();
        let mut program_data_addresses = vec![];

        for (account_key, account) in keys.iter().zip(accounts.iter()) {
            // it's ok to not have an account (this account is created by the transaction)
            if let Some(account) = account {
                if account.owner == solana_sdk::bpf_loader_upgradeable::id() {
                    if let Ok(UpgradeableLoaderState::Program {
                        programdata_address,
                    }) = account.state()
                    {
                        program_data_addresses.push(programdata_address);
                    }
                }
                accounts_config
                    .accounts
                    .insert(*account_key, account.clone());
            }
        }

        let program_datas = self
            .get_multiple_accounts_with_cache(&program_data_addresses)
            .await?;
        for (program_data_address, program_data_account) in
            program_data_addresses.iter().zip(program_datas.iter())
        {
            if let Some(program_data_account) = program_data_account {
                accounts_config
                    .programs_data
                    .insert(*program_data_address, program_data_account.clone());
            }
        }

        Ok(Response {
            value:   accounts_config,
            context: accounts_with_context.context,
        })
    }

    #[tracing::instrument(skip_all)]
    fn setup_lite_svm(&self, accounts_config_with_context: &Response<AccountsConfig>) -> LiteSVM {
        let mut svm = LiteSVM::new()
            .with_sigverify(false)
            .with_blockhash_check(false)
            .with_transaction_history(0);
        // this is necessary for correct lookup table access
        // otherwise 0 = slot < table.last_extended_slot
        svm.warp_to_slot(accounts_config_with_context.context.slot);
        // we grab the timestamp after fetching the accounts to maximize chance of timestamp exceeds any timestamps stored in fetched accounts
        self.warp_to_timestamp(&mut svm, OffsetDateTime::now_utc().unix_timestamp());
        accounts_config_with_context.value.apply(&mut svm);
        svm
    }

    /// Warps the LiteSVM object clock to the given timestamp
    /// This is necessary because LiteSVM does not natively support warping to a timestamp
    fn warp_to_timestamp(&self, svm: &mut LiteSVM, timestamp: i64) {
        let mut clock = svm.get_sysvar::<Clock>();
        clock.unix_timestamp = timestamp;
        svm.set_sysvar(&clock);
    }


    #[allow(clippy::result_large_err)]
    fn check_rent_exemption(
        svm: &LiteSVM,
        simulation_result: Result<SimulatedTransactionInfo, FailedTransactionMetadata>,
    ) -> Result<SimulatedTransactionInfo, FailedTransactionMetadata> {
        let tx_info = simulation_result?;
        for (pubkey, data) in &tx_info.post_accounts {
            // Ignore if lamports are zero since the account may be closed in the transaction
            if 0 < data.lamports()
                && data.lamports() < svm.minimum_balance_for_rent_exemption(data.data().len())
            {
                let mut metadata = tx_info.meta.clone();
                metadata
                    .logs
                    .push(format!("Insufficient Funds For Rent: {}", pubkey));
                return Err(FailedTransactionMetadata {
                    //TODO: account_index is not correct, we should find it from the transaction
                    // the meta logs reflect a successful transaction which is inconsistent with the error
                    err:  solana_sdk::transaction::TransactionError::InsufficientFundsForRent {
                        account_index: 0,
                    },
                    meta: metadata,
                });
            }
        }
        Ok(tx_info)
    }

    /// Simulates a transaction with the current state of the chain
    /// applying pending transactions beforehand. The simulation is done locally via fetching
    /// all the necessary accounts from the RPC.
    /// Simulation happens even if some of the pending transactions are failed.
    /// Returns the simulation result and the context of the accounts fetched.
    pub async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> RpcResult<Result<SimulatedTransactionInfo, FailedTransactionMetadata>> {
        let pending_txs = self.fetch_pending_and_remove_old_txs().await;
        let txs_to_fetch = pending_txs
            .iter()
            .chain(std::iter::once(transaction))
            .collect::<Vec<_>>();
        let accounts_config_with_context = self.fetch_tx_accounts_via_rpc(&txs_to_fetch).await?;
        let mut svm = self.setup_lite_svm(&accounts_config_with_context);

        pending_txs.into_iter().for_each(|tx| {
            let _ = svm.send_transaction(tx);
        });
        let res = svm.simulate_transaction(transaction.clone());
        let res = Self::check_rent_exemption(&svm, res);
        Ok(Response {
            value:   res,
            context: accounts_config_with_context.context,
        })
    }

    /// Given a list of bids, tries to find the optimal set of bids that can be submitted to the chain
    /// considering the current state of the chain and the pending transactions.
    /// Right now, for simplicity, the method assume the bids are sorted, and tries to submit them in order
    /// and only return the ones that are successfully submitted.
    pub async fn optimize_bids(&self, bids_sorted: &[Bid]) -> RpcResult<Vec<Bid>> {
        let pending_txs = self.fetch_pending_and_remove_old_txs().await;
        let txs_to_fetch = pending_txs
            .iter()
            .chain(bids_sorted.iter().map(|bid| &bid.chain_data.transaction))
            .collect::<Vec<_>>();
        let accounts_config_with_context = self.fetch_tx_accounts_via_rpc(&txs_to_fetch).await?;
        let mut svm = self.setup_lite_svm(&accounts_config_with_context);

        pending_txs.into_iter().for_each(|tx| {
            let _ = svm.send_transaction(tx);
        });
        let mut res = vec![];
        for bid in bids_sorted {
            if svm
                .send_transaction(bid.chain_data.transaction.clone())
                .is_ok()
            {
                res.push(bid.clone());
            }
        }
        Ok(Response {
            value:   res,
            context: accounts_config_with_context.context,
        })
    }
}
