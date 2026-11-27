use async_trait::async_trait;
use chain_traits::ChainState;
use std::error::Error;

use bsv_home_client::Client;
use primitives::NodeSyncStatus;

use crate::provider::state_mapper;
use crate::rpc::TronProvider;

#[async_trait]
impl<C: Client> ChainState for TronProvider<C> {
    async fn get_chain_id(&self) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
        Ok(Some(state_mapper::map_chain_id(&self.get_genesis_block_id().await?)?))
    }

    async fn get_block_latest_number(&self) -> Result<u64, Box<dyn Error + Send + Sync>> {
        Ok(self.get_latest_block().await? as u64)
    }

    async fn get_node_status(&self) -> Result<NodeSyncStatus, Box<dyn Error + Send + Sync>> {
        let latest_block = self.get_block_latest_number().await? as i64;
        state_mapper::map_node_status(latest_block)
    }
}

#[cfg(all(test, feature = "chain_integration_tests"))]
mod chain_integration_tests {
    use primitives::Chain;

    use super::*;
    use crate::provider::testkit::{create_shasta_test_client, create_test_client};
    use chain_traits::{ChainBalances, ChainToken, ChainTransactions, TransactionsRequest};
    use primitives::{AssetId, asset_constants::TRON_SHASTA_USDT_TOKEN_ID};

    #[tokio::test]
    async fn test_get_chain_id() {
        let tron_client = create_test_client();

        let chain_id = tron_client.get_chain_id().await.unwrap();

        assert_eq!(chain_id.as_deref(), Some(Chain::Tron.network_id()));
    }

    #[tokio::test]
    async fn test_get_block_latest_number() {
        let tron_client = create_test_client();

        let latest_block = tron_client.get_block_latest_number().await.unwrap();

        // Latest block should be a positive number
        assert!(latest_block > 0);
    }

    #[tokio::test]
    async fn test_get_node_status() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tron_client = create_test_client();
        let node_status = tron_client.get_node_status().await?;

        assert!(node_status.in_sync);
        assert!(node_status.latest_block_number.is_some());
        assert!(node_status.latest_block_number.unwrap_or(0) > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_shasta_network_token_and_history() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = create_shasta_test_client();
        let (chain_id, token, balance, history) = futures::try_join!(
            client.get_chain_id(),
            ChainToken::get_token_data(&client, TRON_SHASTA_USDT_TOKEN_ID.to_string()),
            client.get_balance_coin(TRON_SHASTA_USDT_TOKEN_ID.to_string()),
            client.get_transactions_by_address(TransactionsRequest::new(TRON_SHASTA_USDT_TOKEN_ID.to_string(), 2))
        )?;

        assert_eq!(chain_id.as_deref(), Some(Chain::TronShasta.network_id()));
        assert_eq!(token.id, AssetId::from_token(Chain::TronShasta, TRON_SHASTA_USDT_TOKEN_ID));
        assert_eq!(token.decimals, 6);
        assert_eq!(balance.asset_id, AssetId::from_chain(Chain::TronShasta));
        let requests = history.transaction_requests().ok_or("Shasta history must return transaction requests")?;
        assert!(!requests.is_empty());
        assert_eq!(requests.iter().map(|request| request.chain).collect::<Vec<_>>(), vec![Chain::TronShasta; requests.len()]);
        Ok(())
    }
}
