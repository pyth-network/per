use {
    litesvm::{
        types::{
            FailedTransactionMetadata,
            SimulatedTransactionInfo,
        },
        LiteSVM,
    },
    solana_client::{
        rpc_config::RpcSendTransactionConfig,
        rpc_response::{
            Response,
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
        bpf_loader_upgradeable::UpgradeableLoaderState,
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::Signature,
        transaction::VersionedTransaction,
    },
    std::{
        collections::HashMap,
        time::Instant,
    },
    tokio::sync::RwLock,
};

pub struct Simulator {
    sender:      RpcClient,
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

impl Simulator {
    pub fn new(sender: RpcClient, receiver: RpcClient) -> Self {
        Self {
            sender,
            receiver,
            pending_txs: Default::default(),
        }
    }

    pub async fn fetch_pending_and_remove_old_txs(&self) -> Vec<VersionedTransaction> {
        let now = Instant::now();
        let mut pending_txs = self.pending_txs.write().await;
        pending_txs.retain(|(_, time)| now.duration_since(*time).as_secs() < 5);
        //TODO: remove pending txs when they are processed
        pending_txs.iter().map(|(tx, _)| tx.clone()).collect()
    }

    pub async fn send_transaction(
        &self,
        tx: &VersionedTransaction,
    ) -> solana_client::client_error::Result<Signature> {
        let now = Instant::now();
        self.pending_txs.write().await.push((tx.clone(), now));
        self.sender
            .send_transaction_with_config(
                tx,
                RpcSendTransactionConfig {
                    skip_preflight:       true,
                    preflight_commitment: None,
                    encoding:             None,
                    max_retries:          Some(0),
                    min_context_slot:     None,
                },
            )
            .await
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
                    programs_to_fetch.push((*account_key, programdata_address));
                } else {
                    accounts_config
                        .accounts
                        .insert(*account_key, account.clone());
                }
                // TODO: handle lookup tables
            } else {
                // it's ok to not have an account (this account is created by the transaction)
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
