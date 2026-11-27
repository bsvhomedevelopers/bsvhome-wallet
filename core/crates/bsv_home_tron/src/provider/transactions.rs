use async_trait::async_trait;
use chain_traits::{ChainBlockTransactions, ChainTransaction, TransactionIdRequest};
use std::error::Error;

use bsv_home_client::Client;
use primitives::Transaction;

use super::transactions_mapper::{map_transaction, map_transactions_by_block};
use crate::rpc::TronProvider;

#[async_trait]
impl<C: Client> ChainBlockTransactions for TronProvider<C> {
    async fn get_transactions_by_block(&self, block: u64) -> Result<Vec<Transaction>, Box<dyn Error + Sync + Send>> {
        let block_data = self.get_block_transactions(block).await?;
        if block_data.transactions.is_empty() {
            return Ok(vec![]);
        }

        let receipts = self.get_block_transactions_receipts(block).await?;
        Ok(map_transactions_by_block(self.get_chain(), block_data, receipts))
    }
}

#[async_trait]
impl<C: Client> ChainTransaction for TronProvider<C> {
    async fn get_transaction_by_hash(&self, request: TransactionIdRequest) -> Result<Option<Transaction>, Box<dyn Error + Sync + Send>> {
        let hash = request.hash;
        let Some(receipt) = self.get_transaction_receipt(hash.clone()).await? else {
            return Ok(None);
        };
        Ok(map_transaction(self.get_chain(), self.get_transaction(hash).await?, receipt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::TronClient;
    use bsv_home_client::{ClientError, testkit::MockClient};
    use chain_traits::{ChainTransactions, TransactionsRequest};
    use primitives::Chain;

    const TRANSACTIONS_RESPONSE: &str = include_str!("../../testdata/transactions_by_address.json");
    const TRC20_TRANSACTIONS_RESPONSE: &str = include_str!("../../testdata/trc20_transactions_by_address.json");
    const ADDRESS: &str = "TBKwjUtXVsX1r724C1V52nocBgtioDjx9u";
    const INCOMING_TRANSACTION_ID: &str = "d61c72e80f48b6d014301fc088f5d7f5a512d08b17cc6208d6b9993aeea12b0a";

    #[tokio::test]
    async fn test_get_transactions_by_address() {
        let client = TronProvider::mock(|path| {
            if path.contains("/transactions/trc20") {
                Ok(TRC20_TRANSACTIONS_RESPONSE.as_bytes().to_vec())
            } else {
                Ok(TRANSACTIONS_RESPONSE.as_bytes().to_vec())
            }
        });
        let result = client.get_transactions_by_address(TransactionsRequest::new(ADDRESS.to_string(), 4)).await.unwrap();
        let transactions = result.transaction_requests().unwrap();
        assert_eq!(transactions.len(), 4);
        assert!(transactions.iter().any(|transaction| transaction.hash == INCOMING_TRANSACTION_ID));
    }

    #[tokio::test]
    async fn test_shasta_history_preserves_network_and_incoming_token_transactions() {
        let client = TronProvider::mock_with_chain(Chain::TronShasta, |path| {
            if path.contains("/transactions/trc20") {
                Ok(TRC20_TRANSACTIONS_RESPONSE.as_bytes().to_vec())
            } else {
                Ok(TRANSACTIONS_RESPONSE.as_bytes().to_vec())
            }
        });

        let result = client.get_transactions_by_address(TransactionsRequest::new(ADDRESS.to_string(), 4)).await.unwrap();
        let transactions = result.transaction_requests().unwrap();

        assert_eq!(transactions.iter().map(|transaction| transaction.chain).collect::<Vec<_>>(), vec![Chain::TronShasta; 4]);
        assert_eq!(
            transactions.iter().find(|transaction| transaction.hash == INCOMING_TRANSACTION_ID),
            Some(&TransactionIdRequest::new(Chain::TronShasta, INCOMING_TRANSACTION_ID.to_string(), None))
        );
    }

    #[tokio::test]
    async fn test_transaction_details_preserve_shasta_identity() {
        let client = MockClient::new().with_get(|path| {
            if path.starts_with("/wallet/gettransactioninfobyid") {
                Ok(include_str!("../../testdata/transaction_coin_transfer_receipt.json").as_bytes().to_vec())
            } else if path.starts_with("/wallet/gettransactionbyid") {
                Ok(include_str!("../../testdata/transaction_coin_transfer.json").as_bytes().to_vec())
            } else {
                Err(ClientError::Http { status: 404, body: Vec::new() })
            }
        });
        let provider = TronProvider::new_rpc_only(TronClient::new_with_chain(Chain::TronShasta, client));
        let transaction = provider
            .get_transaction_by_hash(TransactionIdRequest::new(Chain::TronShasta, "hash".to_string(), None))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(transaction.id.chain, Chain::TronShasta);
        assert_eq!(transaction.asset_id.chain, Chain::TronShasta);
        assert_eq!(transaction.fee_asset_id.chain, Chain::TronShasta);
    }
}

#[cfg(all(test, feature = "chain_integration_tests"))]
mod chain_integration_tests {
    use super::*;
    use crate::provider::testkit::{TEST_ADDRESS, TEST_TRANSACTION_ID, create_test_client};
    use chain_traits::{ChainState, ChainTransactions, TransactionsRequest};

    #[tokio::test]
    async fn test_get_transactions_by_block() {
        let tron_client = create_test_client();

        let latest_block = tron_client.get_block_latest_number().await.unwrap();
        let block_number = latest_block - 25;
        let transactions = tron_client.get_transactions_by_block(block_number).await.unwrap();

        assert!(latest_block > 0);
        assert!(!transactions.is_empty());

        if let Some(transaction) = transactions.first() {
            assert!(!transaction.id.hash.is_empty());
        }
    }

    #[tokio::test]
    async fn test_get_transactions_by_address() {
        let tron_client = create_test_client();
        let result = tron_client
            .get_transactions_by_address(TransactionsRequest::new(TEST_ADDRESS.to_string(), 2))
            .await
            .unwrap();
        let transactions = result.transaction_requests().unwrap();
        assert!(!transactions.is_empty());
    }

    #[tokio::test]
    async fn test_get_transaction_by_hash() {
        let tron_client = create_test_client();
        let transaction = tron_client
            .get_transaction_by_hash(TransactionIdRequest::new(primitives::Chain::Tron, TEST_TRANSACTION_ID.to_string(), None))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(transaction.hash(), TEST_TRANSACTION_ID);
    }
}
