use std::sync::Arc;
use std::time::Instant;
use litesvm::LiteSVM;
use litesvm::types::TransactionResult;
use solana_sdk::account::{AccountSharedData, ReadableAccount};
use solana_sdk::bpf_loader_upgradeable::UpgradeableLoaderState;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::transaction::VersionedTransaction;
use solana_sdk::account_utils::StateMut;
use crate::state::ChainStoreSvm;

pub struct Simulator {
    store: Arc<ChainStoreSvm>,
}

impl Simulator {
    pub fn new(store: Arc<ChainStoreSvm>) -> Self {
        Self {
            store
        }
    }

    pub async fn run(&self, tx: VersionedTransaction) {
        let keys = tx.message.static_account_keys();
        let res = self.store.client.get_multiple_accounts_with_commitment(keys, CommitmentConfig::confirmed()).await.unwrap().value;

        let mut more_to_fetch = vec![];
        let mut program_addrs = vec![];
        for (account_key, account) in keys.iter().zip(res.iter()) {
            if let Some(account) = account {
                let data = <solana_sdk::account::Account as Into<AccountSharedData>>::into(account.clone());
                if let Ok(UpgradeableLoaderState::Program {
                              programdata_address
                          }) = data.state() {
                    more_to_fetch.push(programdata_address);
                    program_addrs.push(account_key.clone());
                }
            }
        }

        let res2 = self.store.client.get_multiple_accounts_with_commitment(&more_to_fetch, CommitmentConfig::confirmed()).await.unwrap().value;
        println!("more_to_fetch: {:?}", more_to_fetch);
        println!("res2: {:?}", res2);
        let mut svm = LiteSVM::new().with_sigverify(false).with_blockhash_check(false).with_transaction_history(0);

        let now = Instant::now();

        program_addrs.iter().zip(&res2).for_each(|(key, account)| {
            svm.add_program(key.clone(), &(account.as_ref().unwrap().data()[UpgradeableLoaderState::size_of_programdata_metadata()..]));
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