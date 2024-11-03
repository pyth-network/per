use std::collections::HashMap;
use litesvm::types::{FailedTransactionMetadata, SimulatedTransactionInfo};
use solana_client::rpc_response::{Response, RpcResult, RpcSimulateTransactionResult};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client::rpc_client::SerializableTransaction;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use tokio::sync::RwLock;
use {
    crate::state::ChainStoreSvm,
    litesvm::{
        types::TransactionResult,
        LiteSVM,
    },
    solana_sdk::{
        account::{
            AccountSharedData,
            ReadableAccount,
        },
        account_utils::StateMut,
        bpf_loader_upgradeable::UpgradeableLoaderState,
        commitment_config::CommitmentConfig,
        transaction::VersionedTransaction,
    },
    std::{
        sync::Arc,
        time::Instant,
    },
};

pub struct Simulator {
    sender: RpcClient,
    receiver: RpcClient,
    pending_txs: RwLock<Vec<(VersionedTransaction, Instant)>>,
}

struct AccountsConfig {
    accounts: HashMap<Pubkey, Account>,
    programs: HashMap<Pubkey, Account>,
}

impl AccountsConfig {
    fn new() -> Self {
        Self { accounts: Default::default(), programs: Default::default() }
    }

    fn apply(&self, svm: &mut LiteSVM) {
        for (key, account) in self.accounts.iter() {
            svm.set_account(key.clone(), account.clone()).unwrap();
        }
        for (key, account) in self.programs.iter() {
            svm.add_program(key.clone(), &(account.data()
                [UpgradeableLoaderState::size_of_programdata_metadata()..]));
        }
    }
}

impl Simulator {
    pub fn new(sender: RpcClient, receiver: RpcClient) -> Self {
        Self { sender, receiver, pending_txs: Default::default() }
    }

    pub async fn fetch_pending_and_remove_old_txs(&self) -> Vec<VersionedTransaction> {
        let now = Instant::now();
        let mut pending_txs = self.pending_txs.write().await;
        pending_txs.retain(|(_, time)| now.duration_since(*time).as_secs() < 5);
        pending_txs.iter().map(|(tx, _)| tx.clone()).collect()
    }

    pub async fn send_transaction(
        &self,
        tx: &VersionedTransaction,
    ) -> solana_client::client_error::Result<Signature> {
        let now = Instant::now();
        self.pending_txs.write().await.push((tx.clone(),now));

        self.sender.send_transaction(tx).await
    }

    async fn fetch_tx_accounts(&self, transactions: &[VersionedTransaction]) -> RpcResult<AccountsConfig> {
        let keys = transactions
            .iter()
            .flat_map(|tx| tx.message.static_account_keys())
            .cloned()
            .collect::<Vec<_>>();
        let accounts_with_context = self
            .receiver
            .get_multiple_accounts_with_commitment(&keys, CommitmentConfig::processed())
            .await?;
        let accounts = accounts_with_context.value;
        let mut accounts_config = AccountsConfig::new();
        let mut programs_to_fetch = vec![];
        for (account_key, account) in keys.iter().zip(accounts.iter()) {
            if let Some(account) = account {
                if let Ok(UpgradeableLoaderState::Program {
                              programdata_address,
                          }) = account.state()
                {
                    programs_to_fetch.push((account_key.clone(), programdata_address));
                } else {
                    accounts_config.accounts.insert(account_key.clone(), account.clone());
                }
                // TODO: handle lookup tables
            } else {
                // it's ok to not have an account (this account is created by the transaction)
            }
        }

        let program_accounts = self
            .receiver
            .get_multiple_accounts_with_commitment(
                &programs_to_fetch.iter().map(|(_, programdata_address)| programdata_address.clone()).collect::<Vec<_>>(),
                CommitmentConfig::processed(),
            )
            .await?
            .value;
        for ((program_key, _), program_account) in programs_to_fetch.iter().zip(program_accounts.iter()) {
            if let Some(program_account) = program_account {
                accounts_config.programs.insert(program_key.clone(), program_account.clone());
            } else {
                // it's not ok to point to a non-existent program. TODO: handle this case
            }
        }
        RpcResult::Ok(Response { value: accounts_config, context: accounts_with_context.context })
    }

    pub async fn simulate_transaction(&self, transaction: &VersionedTransaction) -> RpcResult<Result<SimulatedTransactionInfo, FailedTransactionMetadata>> {
        let accounts_config_with_context = self.fetch_tx_accounts(&[transaction.clone()]).await?;
        let accounts_config = accounts_config_with_context.value;
        let pending_txs = self.fetch_pending_and_remove_old_txs().await;
        let mut svm = LiteSVM::new()
            .with_sigverify(false)
            .with_blockhash_check(false)
            .with_transaction_history(0);
        accounts_config.apply(&mut svm);

        pending_txs.into_iter().for_each(|tx|{
            let _ = svm.send_transaction(tx);
        });
        let res = svm.simulate_transaction(transaction.clone());
        RpcResult::Ok(Response { value: res, context: accounts_config_with_context.context })
    }

    pub async fn run(&self, tx: VersionedTransaction) {
        let keys = tx.message.static_account_keys();
        let res = self
            .receiver
            .get_multiple_accounts_with_commitment(keys, CommitmentConfig::confirmed())
            .await
            .unwrap()
            .value;

        let mut more_to_fetch = vec![];
        let mut program_addrs = vec![];
        for (account_key, account) in keys.iter().zip(res.iter()) {
            if let Some(account) = account {
                let data = <solana_sdk::account::Account as Into<AccountSharedData>>::into(
                    account.clone(),
                );
                if let Ok(UpgradeableLoaderState::Program {
                    programdata_address,
                }) = data.state()
                {
                    more_to_fetch.push(programdata_address);
                    program_addrs.push(account_key.clone());
                }
            }
        }

        let res2 = self
            .receiver
            .get_multiple_accounts_with_commitment(&more_to_fetch, CommitmentConfig::confirmed())
            .await
            .unwrap()
            .value;
        println!("more_to_fetch: {:?}", more_to_fetch);
        println!("res2: {:?}", res2);
        let mut svm = LiteSVM::new()
            .with_sigverify(false)
            .with_blockhash_check(false)
            .with_transaction_history(0);

        let now = Instant::now();

        program_addrs.iter().zip(&res2).for_each(|(key, account)| {
            svm.add_program(
                key.clone(),
                &(account.as_ref().unwrap().data()
                    [UpgradeableLoaderState::size_of_programdata_metadata()..]),
            );
        });
        keys.iter().zip(&res).for_each(|(key, account)| {
            if program_addrs.contains(key) {
                return;
            }
            if let Some(account) = account {
                if !account.executable {
                    println!("set_account: {:?}", key);
                    svm.set_account(key.clone(), account.clone()).unwrap();
                }
            }
        });
        // Code block to measure.
        for _ in 0..1000 {
            keys.iter().zip(&res).for_each(|(key, account)| {
                if program_addrs.contains(key) {
                    return;
                }
                if let Some(account) = account {
                    if !account.executable {
                        svm.set_account(key.clone(), account.clone()).unwrap();
                    }
                }
            });

            let res = svm.send_transaction(tx.clone());
            match res {
                TransactionResult::Ok(_) => {}
                TransactionResult::Err(err) => {
                    println!("Transaction failed: {:?}", err);
                }
            }
        }


        let elapsed = now.elapsed();
        println!("Elapsed: {:.2?}", elapsed);
        println!("res: {:?}", res);
    }
}
