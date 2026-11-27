use std::sync::Mutex;

use primitives::Chain;
use primitives::node::Node;

use super::BsvHomeNodeStore;
use crate::services::error::BsvHomeServiceError;

#[derive(Default)]
pub struct MemoryNodeStore {
    pub nodes: Mutex<Vec<Node>>,
    pub requests: Mutex<Vec<Chain>>,
}

#[async_trait::async_trait]
impl BsvHomeNodeStore for MemoryNodeStore {
    async fn get_nodes(&self, chain: Chain) -> Result<Vec<Node>, BsvHomeServiceError> {
        self.requests.lock().unwrap().push(chain);
        Ok(self.nodes.lock().unwrap().clone())
    }
    async fn add_node(&self, chain: Chain, node: Node) -> Result<(), BsvHomeServiceError> {
        self.requests.lock().unwrap().push(chain);
        self.nodes.lock().unwrap().push(node);
        Ok(())
    }
    async fn delete_node(&self, chain: Chain, url: String) -> Result<(), BsvHomeServiceError> {
        self.requests.lock().unwrap().push(chain);
        self.nodes.lock().unwrap().retain(|node| node.url != url);
        Ok(())
    }
}
