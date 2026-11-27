use std::collections::{HashMap, HashSet};

use bsv_home_client::Client;
use chain_traits::ChainTransaction;
use primitives::{Address, Chain, Transaction, TransactionIdRequest};

use crate::{
    BsvAddress,
    rpc::{BsvClient, ProviderError, Unspent},
};

#[cfg(test)]
#[path = "wallet_tests.rs"]
mod tests;

impl<C: Client> BsvClient<C> {
    pub async fn address_activity(&self, address: &str) -> Result<(bool, bool), ProviderError> {
        let activity = self.address_activities(&[address.to_string()]).await?.pop().ok_or("BSV address activity missing")?;
        Ok(activity)
    }

    pub async fn address_activities(&self, addresses: &[String]) -> Result<Vec<(bool, bool)>, ProviderError> {
        let histories = self.histories(addresses).await?;
        let mut activities = Vec::with_capacity(histories.len());
        let mut transactions = HashMap::new();
        for (address, history) in histories {
            let owner = BsvAddress::from_str(&address)?;
            let mut has_history = false;
            let mut has_confirmed_history = false;
            for entry in history {
                if !transactions.contains_key(&entry.tx_hash) {
                    transactions.insert(entry.tx_hash.clone(), self.transaction_info(&entry.tx_hash).await?);
                }
                let transaction = transactions.get(&entry.tx_hash).ok_or("BSV history transaction missing")?;
                let mut belongs = false;
                for output in &transaction.vout {
                    belongs |= output.belongs_to(&owner)?;
                }
                if !belongs && !transaction.is_coinbase() {
                    for input in &transaction.vin {
                        let txid = input.txid.as_ref().ok_or("BSV history input transaction missing")?;
                        let index = input.vout.ok_or("BSV history input output index missing")?;
                        let previous = self.transaction_info(txid).await?;
                        belongs |= previous.output(index)?.belongs_to(&owner)?;
                    }
                }
                if !belongs {
                    return Err("BSV history transaction does not belong to requested address".into());
                }
                has_history = true;
                if transaction.confirmations.is_some_and(|count| count > 0) && transaction.blockheight.is_some_and(|height| height > 0) {
                    has_confirmed_history = true;
                }
            }
            activities.push((has_history, has_confirmed_history));
        }
        Ok(activities)
    }

    pub async fn wallet_native_unspent(&self, addresses: &[String]) -> Result<Vec<(String, Unspent)>, ProviderError> {
        validate_wallet_addresses(addresses)?;
        let mut outputs = Vec::new();
        let mut outpoints = HashSet::new();
        for address in addresses {
            for output in self.native_unspent(address).await? {
                if !outpoints.insert((output.tx_hash.clone(), output.tx_pos)) {
                    return Err("duplicate BSV wallet outpoint".into());
                }
                outputs.push((address.clone(), output));
            }
        }
        Ok(outputs)
    }

    pub async fn wallet_transactions(&self, addresses: &[String], _limit: usize) -> Result<Vec<Transaction>, ProviderError> {
        let mut ids = HashSet::new();
        let mut requests = Vec::new();
        for (_, history) in self.histories(addresses).await? {
            for entry in history {
                if ids.insert(entry.tx_hash.clone()) {
                    requests.push(TransactionIdRequest::new(Chain::BitcoinSv, entry.tx_hash, entry.height.filter(|height| *height > 0)));
                }
            }
        }
        let mut transactions = Vec::with_capacity(requests.len());
        for request in requests {
            let transaction = self.get_transaction_by_hash(request).await?.ok_or("BSV wallet history transaction missing")?;
            transactions.push(super::transactions_mapper::finalize_wallet(transaction, addresses)?);
        }
        transactions.sort_by(|left, right| right.created_at.cmp(&left.created_at).then_with(|| left.hash().cmp(right.hash())));
        Ok(transactions)
    }
}

fn validate_wallet_addresses(addresses: &[String]) -> Result<(), ProviderError> {
    if addresses.is_empty() {
        return Err("BSV wallet requires at least one owned address".into());
    }
    let mut owners = HashSet::new();
    for address in addresses {
        BsvAddress::from_str(address)?;
        if !owners.insert(address) {
            return Err("duplicate BSV wallet address".into());
        }
    }
    Ok(())
}
