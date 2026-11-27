use std::sync::Arc;

use primitives::node_config::NodeRegion;
use primitives::{Chain, Latency};

use super::model::{BsvHomeAddNodeError, BsvHomeNodeCheck, BsvHomeNodeSelection, BsvHomeNodeStatusState};
use super::rules;
use crate::gateway::{BsvHomeGateway, GatewayError};
use crate::services::chain::rules as chain_rules;
use crate::services::error::BsvHomeServiceError;
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::node::BsvHomeNodeService;

#[derive(uniffi::Object)]
pub struct BsvHomeChainSettingsService {
    nodes: Arc<BsvHomeNodeService>,
    explorer: Arc<BsvHomeExplorerService>,
    gateway: Arc<BsvHomeGateway>,
}

#[uniffi::export]
impl BsvHomeChainSettingsService {
    #[uniffi::constructor]
    pub fn new(nodes: Arc<BsvHomeNodeService>, explorer: Arc<BsvHomeExplorerService>, gateway: Arc<BsvHomeGateway>) -> Self {
        Self { nodes, explorer, gateway }
    }

    pub fn chains(&self, query: String) -> Vec<Chain> {
        chain_rules::matching_chains(
            chain_rules::chains_by_rank().into_iter().filter(|chain| self.gateway.allows_chain(*chain)).collect(),
            &query,
        )
    }

    pub fn explorers(&self, chain: Chain) -> Vec<String> {
        if !self.gateway.allows_chain(chain) {
            return Vec::new();
        }
        self.explorer.get_explorers(chain)
    }

    pub fn explorer_name(&self, chain: Chain) -> String {
        if !self.gateway.allows_chain(chain) {
            return String::new();
        }
        self.explorer.get_explorer_name(chain)
    }

    pub fn set_explorer_name(&self, chain: Chain, name: String) -> Result<(), BsvHomeServiceError> {
        self.gateway.require_chain(chain)?;
        self.explorer.set_explorer_name(chain, name)
    }

    pub async fn nodes(&self, chain: Chain) -> Result<Vec<BsvHomeNodeSelection>, BsvHomeServiceError> {
        self.gateway.require_chain(chain)?;
        let nodes = self.nodes.get_nodes(chain).await?;
        let selected_url = self.nodes.selected_node(chain).url;
        Ok(rules::node_selections(self.nodes.sorted_nodes(chain, nodes), &selected_url))
    }

    pub async fn select_node(&self, chain: Chain, url: String) -> Result<(), BsvHomeServiceError> {
        self.gateway.require_chain(chain)?;
        self.nodes.select_node(chain, url).await
    }

    pub fn can_delete_node(&self, chain: Chain, url: String) -> bool {
        self.gateway.allows_chain(chain) && self.nodes.can_delete_node(chain, url)
    }

    pub async fn delete_node(&self, chain: Chain, url: String) -> Result<(), BsvHomeServiceError> {
        self.gateway.require_chain(chain)?;
        self.nodes.delete_node(chain, url).await
    }

    pub async fn add_node(&self, chain: Chain, url: String) -> Result<(), BsvHomeServiceError> {
        self.gateway.require_chain(chain)?;
        self.nodes.add_node(chain, url.clone()).await?;
        self.nodes.select_node(chain, url).await
    }

    pub fn node_flag(&self, url: String) -> Option<String> {
        NodeRegion::from_url(&url).map(|region| region.flag().to_string())
    }

    pub async fn node_status(&self, chain: Chain, url: String) -> BsvHomeNodeStatusState {
        if !self.gateway.allows_chain(chain) {
            return BsvHomeNodeStatusState::Error;
        }
        match self.gateway.get_node_status(chain, &url).await {
            Ok(status) if status.latest_block_number > 0 => BsvHomeNodeStatusState::Result {
                latest_block_number: status.latest_block_number,
                latency: Latency::from_milliseconds(status.latency_ms),
            },
            Ok(_) | Err(_) => BsvHomeNodeStatusState::Error,
        }
    }

    pub fn node_check_debounce_milliseconds(&self) -> u64 {
        rules::node_check_debounce_milliseconds()
    }

    pub async fn check_node(&self, chain: Chain, url: String) -> Result<BsvHomeNodeCheck, BsvHomeAddNodeError> {
        self.gateway.require_chain(chain).map_err(|error| GatewayError::PlatformError { msg: error.to_string() })?;
        let url = rules::node_url(&url).ok_or(BsvHomeAddNodeError::InvalidUrl)?;
        Ok(self.gateway.check_node(chain, &url).await?)
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use primitives::node::{Node, NodeState};

    use super::*;
    use crate::gateway::EmptyPreferences;
    use crate::services::chain::BsvHomeRuntimeEnvironment;
    use crate::services::node::testkit::MemoryNodeStore;
    use crate::services::preferences::{BsvHomePreferencesService, BsvHomePreferencesStore, testkit::MemoryPreferencesStore};
    use crate::testkit::TestAlienProvider;

    struct Testkit {
        service: BsvHomeChainSettingsService,
        store: Arc<MemoryNodeStore>,
        preferences: Arc<MemoryPreferencesStore>,
        provider: Arc<TestAlienProvider>,
    }

    fn testkit(environment: BsvHomeRuntimeEnvironment) -> Testkit {
        let store = Arc::new(MemoryNodeStore::default());
        let preferences = Arc::new(MemoryPreferencesStore::default());
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let service = BsvHomeChainSettingsService::new(
            Arc::new(BsvHomeNodeService::new(store.clone(), preferences.clone())),
            Arc::new(BsvHomeExplorerService::new(Arc::new(BsvHomePreferencesService::new(preferences.clone())))),
            Arc::new(BsvHomeGateway::new_with_environment(
                provider.clone(),
                preferences.clone(),
                Arc::new(EmptyPreferences),
                environment,
            )),
        );
        Testkit {
            service,
            store,
            preferences,
            provider,
        }
    }

    #[test]
    fn test_wowpay_chain_settings_reject_hidden_networks_before_store_or_transport_access() {
        block_on(async {
            let kit = testkit(BsvHomeRuntimeEnvironment::WowPay);
            let legacy = Node {
                url: "https://legacy.example".into(),
                status: NodeState::Active,
                priority: 0,
            };
            kit.store.nodes.lock().unwrap().push(legacy.clone());
            for chain in [Chain::Ethereum, Chain::Tron, Chain::TronShasta] {
                kit.preferences.set(format!("node_{chain}"), legacy.url.clone()).unwrap();
                kit.preferences.set(format!("explorer_name_{chain}"), "legacy explorer".into()).unwrap();
            }
            let saved = kit.preferences.values.lock().unwrap().clone();
            for chain in [Chain::Ethereum, Chain::Tron, Chain::TronShasta] {
                assert!(matches!(kit.service.nodes(chain).await, Err(BsvHomeServiceError::Unsupported { .. })));
                assert!(matches!(
                    kit.service.select_node(chain, legacy.url.clone()).await,
                    Err(BsvHomeServiceError::Unsupported { .. })
                ));
                assert!(matches!(
                    kit.service.add_node(chain, "https://new.example".into()).await,
                    Err(BsvHomeServiceError::Unsupported { .. })
                ));
                assert!(matches!(
                    kit.service.delete_node(chain, legacy.url.clone()).await,
                    Err(BsvHomeServiceError::Unsupported { .. })
                ));
                assert!(matches!(
                    kit.service.set_explorer_name(chain, "new explorer".into()),
                    Err(BsvHomeServiceError::Unsupported { .. })
                ));
                assert!(kit.service.explorers(chain).is_empty());
                assert_eq!(kit.service.explorer_name(chain), "");
                assert!(!kit.service.can_delete_node(chain, legacy.url.clone()));
                assert_eq!(kit.service.node_status(chain, legacy.url.clone()).await, BsvHomeNodeStatusState::Error);
                assert!(matches!(kit.service.check_node(chain, legacy.url.clone()).await, Err(BsvHomeAddNodeError::Gateway(_))));
            }
            assert_eq!(
                serde_json::to_value(kit.store.nodes.lock().unwrap().clone()).unwrap(),
                serde_json::to_value(vec![legacy]).unwrap()
            );
            assert!(kit.store.requests.lock().unwrap().is_empty());
            assert_eq!(*kit.preferences.values.lock().unwrap(), saved);
            assert!(kit.provider.requested_paths().is_empty());
        });
    }

    #[test]
    fn test_wowpay_chain_settings_keep_allowed_node_and_explorer_operations() {
        block_on(async {
            let kit = testkit(BsvHomeRuntimeEnvironment::WowPay);
            for chain in [Chain::BitcoinSv] {
                let default_url = kit.service.nodes(chain).await.unwrap().into_iter().find(|node| node.is_selected).unwrap().url;
                let url = format!("https://{chain}.custom.example");
                kit.service.add_node(chain, url.clone()).await.unwrap();
                assert!(kit.service.can_delete_node(chain, url.clone()));
                assert!(kit.service.nodes(chain).await.unwrap().iter().any(|node| node.url == url && node.is_selected));
                kit.service.select_node(chain, default_url.clone()).await.unwrap();
                assert!(kit.service.nodes(chain).await.unwrap().iter().any(|node| node.url == default_url && node.is_selected));
                let name = kit.service.explorers(chain).into_iter().next().unwrap();
                kit.service.set_explorer_name(chain, name.clone()).unwrap();
                assert_eq!(kit.service.explorer_name(chain), name);
                kit.service.select_node(chain, url.clone()).await.unwrap();
                kit.service.delete_node(chain, url.clone()).await.unwrap();
                let remaining = kit.service.nodes(chain).await.unwrap();
                assert!(!remaining.iter().any(|node| node.url == url));
                assert!(remaining.iter().any(|node| node.url == default_url && node.is_selected));
            }
            assert!(kit.provider.requested_paths().is_empty());
        });
    }

    #[test]
    fn test_production_chain_settings_preserve_other_mainnet_settings() {
        block_on(async {
            let kit = testkit(BsvHomeRuntimeEnvironment::Production);
            let url = "https://ethereum.custom.example".to_string();
            kit.service.add_node(Chain::Ethereum, url.clone()).await.unwrap();
            assert!(kit.service.nodes(Chain::Ethereum).await.unwrap().iter().any(|node| node.url == url && node.is_selected));
            let name = kit.service.explorers(Chain::Ethereum).into_iter().next().unwrap();
            kit.service.set_explorer_name(Chain::Ethereum, name.clone()).unwrap();
            assert_eq!(kit.service.explorer_name(Chain::Ethereum), name);
            kit.service.delete_node(Chain::Ethereum, url).await.unwrap();
        });
    }
}
