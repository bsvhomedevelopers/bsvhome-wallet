use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::Chain;
use primitives::node::Node;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeNodeStore: Send + Sync {
    async fn get_nodes(&self, chain: Chain) -> Result<Vec<Node>, BsvHomeServiceError>;
    async fn add_node(&self, chain: Chain, node: Node) -> Result<(), BsvHomeServiceError>;
    async fn delete_node(&self, chain: Chain, url: String) -> Result<(), BsvHomeServiceError>;
}
