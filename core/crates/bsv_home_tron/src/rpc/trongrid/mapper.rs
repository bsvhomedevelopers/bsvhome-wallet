use std::{collections::HashSet, error::Error, str::FromStr};

use super::model::{TronGridAccount, TronGridTransaction};
use chain_traits::TransactionIdRequest;
use num_bigint::BigUint;
use primitives::{AssetBalance, AssetId, Chain};

pub struct TronGridMapper;

impl TronGridMapper {
    pub fn map_asset_balances(chain: Chain, account: TronGridAccount) -> Result<Vec<AssetBalance>, Box<dyn Error + Send + Sync>> {
        account
            .trc20
            .into_iter()
            .flat_map(|trc20_map| {
                trc20_map
                    .into_iter()
                    .map(|(contract_address, balance)| Ok(AssetBalance::new(AssetId::from(chain, Some(contract_address)), BigUint::from_str(&balance)?)))
            })
            .collect()
    }

    pub fn map_transaction_requests(chain: Chain, transactions: Vec<TronGridTransaction>, trc20_transactions: Vec<TronGridTransaction>, limit: usize) -> Vec<TransactionIdRequest> {
        let mut transactions = transactions.into_iter().chain(trc20_transactions).collect::<Vec<_>>();
        transactions.sort_unstable_by(|left, right| {
            right
                .block_timestamp
                .cmp(&left.block_timestamp)
                .then_with(|| left.transaction_id.cmp(&right.transaction_id))
        });

        let mut transaction_ids = HashSet::new();
        transactions
            .into_iter()
            .filter(|transaction| transaction_ids.insert(transaction.transaction_id.clone()))
            .take(limit)
            .map(|transaction| TransactionIdRequest::new(chain, transaction.transaction_id, None))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn transaction(transaction_id: &str, block_timestamp: u64) -> TronGridTransaction {
        TronGridTransaction {
            transaction_id: transaction_id.to_string(),
            block_timestamp,
        }
    }

    #[test]
    fn test_map_transaction_requests() {
        let transactions = vec![transaction("native", 30), transaction("duplicate", 20), transaction("oldest-native", 5)];
        let trc20_transactions = vec![transaction("incoming-token", 40), transaction("duplicate", 20), transaction("older-token", 10)];

        let requests = TronGridMapper::map_transaction_requests(Chain::Tron, transactions, trc20_transactions, 4);

        assert_eq!(
            requests.iter().map(|request| request.hash.as_str()).collect::<Vec<_>>(),
            vec!["incoming-token", "native", "duplicate", "older-token"]
        );
    }

    #[test]
    fn test_map_asset_balances_isolates_networks_and_rejects_invalid_balances() {
        let account = TronGridAccount {
            trc20: vec![HashMap::from([("token".to_string(), "1000000".to_string())])],
        };

        for chain in [Chain::Tron, Chain::TronShasta] {
            assert_eq!(
                TronGridMapper::map_asset_balances(chain, account.clone()).unwrap(),
                vec![AssetBalance::new(AssetId::from_token(chain, "token"), BigUint::from(1_000_000u64))]
            );
        }

        let invalid_account = TronGridAccount {
            trc20: vec![HashMap::from([("token".to_string(), "invalid".to_string())])],
        };
        assert!(TronGridMapper::map_asset_balances(Chain::TronShasta, invalid_account).is_err());
    }
}
