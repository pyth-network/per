use {
    litesvm::{
        types::{
            FailedTransactionMetadata,
            SimulatedTransactionInfo,
        },
        LiteSVM,
    },
    solana_client::rpc_response::{
        Response,
        RpcResult,
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
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::Signature,
        slot_hashes::SlotHashes,
        transaction::VersionedTransaction,
    },
    std::{
        collections::HashMap,
        time::{
            Duration,
            Instant,
        },
    },
    tokio::sync::RwLock,
};

pub struct Simulator {
    receiver:    RpcClient,
    pending_txs: RwLock<Vec<(VersionedTransaction, Instant)>>,
}

struct AccountsConfig {
    accounts: HashMap<Pubkey, Account>,
    programs: HashMap<Pubkey, Account>,
}

impl AccountsConfig {
    fn new() -> Self {
        Self {
            accounts: Default::default(),
            programs: Default::default(),
        }
    }

    fn apply(&self, svm: &mut LiteSVM) {
        for (key, account) in self.accounts.iter() {
            svm.set_account(*key, account.clone()).unwrap();
        }
        for (key, account) in self.programs.iter() {
            svm.add_program(
                *key,
                &(account.data()[UpgradeableLoaderState::size_of_programdata_metadata()..]),
            );
        }
    }
}

const MAX_PENDING_DURATION: Duration = Duration::from_secs(15);


impl Simulator {
    pub fn new(receiver: RpcClient) -> Self {
        Self {
            receiver,
            pending_txs: Default::default(),
        }
    }

    pub async fn fetch_pending_and_remove_old_txs(&self) -> Vec<VersionedTransaction> {
        let now = Instant::now();
        let mut pending_txs = self.pending_txs.write().await;
        pending_txs.retain(|(_, time)| now.duration_since(*time) < MAX_PENDING_DURATION);
        pending_txs.iter().map(|(tx, _)| tx.clone()).collect()
    }


    pub async fn add_pending_transaction(&self, tx: &VersionedTransaction) {
        let now = Instant::now();
        self.pending_txs.write().await.push((tx.clone(), now));
    }

    pub async fn remove_pending_transaction(&self, sig: &Signature) {
        self.pending_txs
            .write()
            .await
            .retain(|tx| !tx.0.signatures[0].eq(sig));
    }

    async fn fetch_tx_accounts(
        &self,
        transactions: &[VersionedTransaction],
    ) -> RpcResult<AccountsConfig> {
        let keys = transactions
            .iter()
            .flat_map(|tx| tx.message.static_account_keys())
            .cloned()
            .collect::<Vec<_>>();
        let lookup_keys: HashMap<Pubkey, Vec<u8>> = transactions
            .iter()
            .flat_map(|tx| tx.message.address_table_lookups().unwrap_or_default())
            .map(|x| {
                (
                    x.account_key,
                    x.readonly_indexes
                        .iter()
                        .chain(x.writable_indexes.iter())
                        .cloned()
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let accounts_with_context = self
            .receiver
            .get_multiple_accounts_with_commitment(&keys, CommitmentConfig::processed())
            .await?;
        let slot = accounts_with_context.context.slot;
        let accounts = accounts_with_context.value;
        let mut accounts_config = AccountsConfig::new();
        let mut programs_to_fetch = vec![];
        let mut lookup_accounts_to_fetch: Vec<Pubkey> = vec![];

        for (account_key, account) in keys.iter().zip(accounts.iter()) {
            // it's ok to not have an account (this account is created by the transaction)
            if let Some(account) = account {
                if let Ok(UpgradeableLoaderState::Program {
                    programdata_address,
                }) = account.state()
                {
                    programs_to_fetch.push((*account_key, programdata_address));
                } else {
                    accounts_config
                        .accounts
                        .insert(*account_key, account.clone());
                }
                if let Some(indexes) = lookup_keys.get(account_key) {
                    if let Ok(table_data_deserialized) =
                        AddressLookupTable::deserialize(&account.data)
                    {
                        if let Ok(addrs) =
                            table_data_deserialized.lookup(slot, indexes, &SlotHashes::default())
                        {
                            lookup_accounts_to_fetch.extend(addrs.iter());
                        }
                    }
                }
            }
        }

        //TODO: handle caching for programs already fetched

        let program_accounts = self
            .receiver
            .get_multiple_accounts_with_commitment(
                &programs_to_fetch
                    .iter()
                    .map(|(_, programdata_address)| *programdata_address)
                    .collect::<Vec<_>>(),
                CommitmentConfig::processed(),
            )
            .await?
            .value;
        for ((program_key, _), program_account) in
            programs_to_fetch.iter().zip(program_accounts.iter())
        {
            if let Some(program_account) = program_account {
                accounts_config
                    .programs
                    .insert(*program_key, program_account.clone());
            } else {
                // it's not ok to point to a non-existent program. TODO: handle this case
            }
        }
        Ok(Response {
            value:   accounts_config,
            context: accounts_with_context.context,
        })
    }

    pub async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> RpcResult<Result<SimulatedTransactionInfo, FailedTransactionMetadata>> {
        let accounts_config_with_context = self.fetch_tx_accounts(&[transaction.clone()]).await?;
        let accounts_config = accounts_config_with_context.value;
        let pending_txs = self.fetch_pending_and_remove_old_txs().await;
        let mut svm = LiteSVM::new()
            .with_sigverify(false)
            .with_blockhash_check(false)
            .with_transaction_history(0);
        accounts_config.apply(&mut svm);

        pending_txs.into_iter().for_each(|tx| {
            let _ = svm.send_transaction(tx);
        });
        let res = svm.simulate_transaction(transaction.clone());
        Ok(Response {
            value:   res,
            context: accounts_config_with_context.context,
        })
    }
}
