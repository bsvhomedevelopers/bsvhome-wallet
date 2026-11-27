use std::sync::Arc;

use bsv_home_algorand::rpc::{AlgorandClient, AlgorandProvider};
use bsv_home_aptos::rpc::client::AptosClient;
use bsv_home_bitcoin::rpc::client::BitcoinClient;
use bsv_home_bsc::BscStakingClient;
use bsv_home_cardano::rpc::client::CardanoClient;
use bsv_home_cosmos::rpc::client::CosmosClient;
use bsv_home_everstake::EverstakeStakingClient;
use bsv_home_evm::rpc::{EthereumClient, EthereumProvider};
use bsv_home_hypercore::rpc::client::HyperCoreClient;
use bsv_home_jsonrpc::grpc::AlienGrpcTransport;
use bsv_home_monad::MonadStakingClient;
use bsv_home_near::rpc::{NearClient, NearProvider};
use bsv_home_optimism::OptimismGasOracle;
use bsv_home_polkadot::rpc::{PolkadotClient, PolkadotProvider};
use bsv_home_solana::rpc::{SolanaClient, SolanaProvider};
use bsv_home_stellar::rpc::client::StellarClient;
use bsv_home_sui::rpc::{SuiClient, SuiProvider};
use bsv_home_tempo::TempoProvider;
use bsv_home_ton::rpc::client::TonClient;
use bsv_home_tron::rpc::{TronProvider, client::TronClient, trongrid::client::TronGridClient};
use bsv_home_xrp::rpc::XrpClient;
use chain_traits::ChainTraits;
use primitives::{BitcoinChain, Chain, ChainType, EVMChain, chain_cosmos::CosmosChain};

use super::{GatewayError, PreferencesWrapper, SecureStoreWrapper};
use crate::alien::{AlienProvider, AlienProviderWrapper, new_alien_client};
use crate::network::JsonRpcClient;
use crate::services::chain::{BsvHomeRuntimeEnvironment, rules as chain_rules};
use crate::services::preferences::{BsvHomePreferencesStore, BsvHomeSecureStore};

pub struct ChainClientFactory {
    bsv_scheduler: Arc<bsv_home_bsv::rpc::BsvRequestScheduler>,
    alien: Arc<dyn AlienProvider>,
    bsv_alien: Arc<dyn AlienProvider>,
    preferences: Arc<dyn BsvHomePreferencesStore>,
    secure_preferences: Arc<dyn BsvHomeSecureStore>,
    pub(crate) environment: BsvHomeRuntimeEnvironment,
}

impl ChainClientFactory {
    pub(crate) fn bsv_client(&self) -> bsv_home_bsv::rpc::BsvClient<crate::alien::AlienClient> {
        self.bsv_read_scope().1
    }

    pub(crate) fn bsv_read_scope(&self) -> (String, bsv_home_bsv::rpc::BsvClient<crate::alien::AlienClient>) {
        let url = crate::services::node::node_url(self.preferences.as_ref(), Chain::BitcoinSv);
        let client = bsv_home_bsv::rpc::BsvClient::with_scheduler(new_alien_client(url.clone(), self.bsv_alien.clone()), self.bsv_scheduler.clone());
        (url, client)
    }

    pub fn new(alien: Arc<dyn AlienProvider>, preferences: Arc<dyn BsvHomePreferencesStore>, secure_preferences: Arc<dyn BsvHomeSecureStore>) -> Self {
        Self::new_with_environment(alien, preferences, secure_preferences, BsvHomeRuntimeEnvironment::Production)
    }

    pub fn new_with_environment(
        alien: Arc<dyn AlienProvider>,
        preferences: Arc<dyn BsvHomePreferencesStore>,
        secure_preferences: Arc<dyn BsvHomeSecureStore>,
        environment: BsvHomeRuntimeEnvironment,
    ) -> Self {
        Self::new_with_bsv_provider(alien.clone(), alien, preferences, secure_preferences, environment)
    }

    pub(crate) fn new_with_bsv_provider(
        alien: Arc<dyn AlienProvider>,
        bsv_alien: Arc<dyn AlienProvider>,
        preferences: Arc<dyn BsvHomePreferencesStore>,
        secure_preferences: Arc<dyn BsvHomeSecureStore>,
        environment: BsvHomeRuntimeEnvironment,
    ) -> Self {
        Self {
            bsv_scheduler: bsv_home_bsv::rpc::BsvRequestScheduler::shared(),
            alien,
            bsv_alien,
            preferences,
            secure_preferences,
            environment,
        }
    }

    pub async fn create(&self, chain: Chain) -> Result<Arc<dyn ChainTraits>, GatewayError> {
        let url = crate::services::node::node_url(self.preferences.as_ref(), chain);
        self.create_with_url(chain, url).await
    }

    pub async fn create_with_url(&self, chain: Chain, url: String) -> Result<Arc<dyn ChainTraits>, GatewayError> {
        self.environment
            .require_chain(chain)
            .map_err(|error| GatewayError::PlatformError { msg: error.to_string() })?;
        let alien_client = new_alien_client(url.clone(), self.alien.clone());
        if chain.is_testnet() {
            let client = TronClient::new_with_chain(chain, alien_client.clone());
            let indexer = TronGridClient::new_with_chain(chain, alien_client, String::new());
            let provider: Arc<dyn ChainTraits> = Arc::new(TronProvider::new(client, Box::new(indexer.clone()), Box::new(indexer)));
            let network_id = provider
                .get_chain_id()
                .await
                .map_err(super::map_network_error)?
                .ok_or_else(|| GatewayError::PlatformError {
                    msg: "test node did not report its network identity".into(),
                })?;
            if let Some(network_id) = chain_rules::mismatched_network_id(chain, Some(&network_id)) {
                return Err(GatewayError::NetworkIdMismatch {
                    chain: chain.to_string(),
                    network_id,
                });
            }
            return Ok(provider);
        }
        match chain.chain_type() {
            ChainType::HyperCore => {
                let preferences = Arc::new(PreferencesWrapper {
                    preferences: self.preferences.clone(),
                });
                let secure_preferences = Arc::new(SecureStoreWrapper {
                    store: self.secure_preferences.clone(),
                });
                Ok(Arc::new(HyperCoreClient::new_with_preferences(alien_client, preferences, secure_preferences)))
            }
            ChainType::Bitcoin => match chain {
                Chain::BitcoinSv => Ok(Arc::new(bsv_home_bsv::rpc::BsvClient::with_scheduler(
                    new_alien_client(url, self.bsv_alien.clone()),
                    self.bsv_scheduler.clone(),
                ))),
                _ => Ok(Arc::new(BitcoinClient::new(alien_client, BitcoinChain::from_chain(chain).unwrap()))),
            },
            ChainType::Cardano => Ok(Arc::new(CardanoClient::new(alien_client))),
            ChainType::Stellar => Ok(Arc::new(StellarClient::new(alien_client))),
            ChainType::Sui => Ok(Arc::new(SuiProvider::new_rpc_only(SuiClient::new_with_transport(
                url,
                Arc::new(AlienGrpcTransport::new(Arc::new(AlienProviderWrapper::new(self.alien.clone())))),
            )))),
            ChainType::Xrp => Ok(Arc::new(XrpClient::new(JsonRpcClient::new(alien_client.clone())))),
            ChainType::Algorand => Ok(Arc::new(AlgorandProvider::new_rpc_only(AlgorandClient::new(alien_client)))),
            ChainType::Near => Ok(Arc::new(NearProvider::new_rpc_only(NearClient::new(JsonRpcClient::new(alien_client))))),
            ChainType::Aptos => Ok(Arc::new(AptosClient::new(alien_client))),
            ChainType::Cosmos => Ok(Arc::new(CosmosClient::new(CosmosChain::from_chain(chain).unwrap(), alien_client))),
            ChainType::Ton => Ok(Arc::new(TonClient::new(alien_client))),
            ChainType::Tron => Ok(Arc::new(TronProvider::new_rpc_only(TronClient::new(alien_client)))),
            ChainType::Polkadot => Ok(Arc::new(PolkadotProvider::new_rpc_only(PolkadotClient::new(alien_client)))),
            ChainType::Solana => {
                let client = JsonRpcClient::new(alien_client.clone());
                Ok(Arc::new(SolanaProvider::new_rpc_only(SolanaClient::new(client))))
            }
            ChainType::Ethereum => {
                let evm_chain = EVMChain::from_chain(chain).unwrap();
                let client = EthereumClient::new(JsonRpcClient::new(alien_client), evm_chain);
                let provider = TempoProvider::new_or_else(client, |client| {
                    if evm_chain.is_opstack() {
                        Box::new(EthereumProvider::new_rpc_only_with_provider(client.clone(), Box::new(OptimismGasOracle::new(client))))
                    } else {
                        Box::new(match evm_chain {
                            EVMChain::Ethereum => EthereumProvider::new_rpc_only_with_provider(client.clone(), Box::new(EverstakeStakingClient::new(client, None))),
                            EVMChain::Monad => EthereumProvider::new_rpc_only_with_provider(client.clone(), Box::new(MonadStakingClient::new(client))),
                            EVMChain::SmartChain => EthereumProvider::new_rpc_only_with_provider(client.clone(), Box::new(BscStakingClient::new(client))),
                            _ => EthereumProvider::new_rpc_only(client),
                        })
                    }
                });
                Ok(Arc::from(provider))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::preferences::EmptyPreferences;
    use crate::testkit::TestAlienProvider;
    use futures::executor::block_on;

    #[test]
    fn test_get_is_token_address_matches_chain_config() {
        let factory = ChainClientFactory::new(Arc::new(TestAlienProvider::with_status(404)), Arc::new(EmptyPreferences {}), Arc::new(EmptyPreferences {}));
        for chain in Chain::all() {
            let client = block_on(factory.create(chain)).unwrap();
            let is_token_supported = chain.default_asset_type().is_some();
            match chain.chain_type().mock_token_id() {
                Some(token_id) => assert_eq!(
                    client.get_is_token_address(token_id),
                    is_token_supported,
                    "{chain}: get_is_token_address({token_id}) disagrees with default_asset_type in chain config"
                ),
                None => assert!(!is_token_supported, "{chain} declares token support in chain config but has no token id fixture"),
            }
        }
    }

    #[test]
    fn test_shasta_factory_production_rejects_direct_entry_before_http() {
        let alien = Arc::new(TestAlienProvider::with_status(200));
        let factory = ChainClientFactory::new(alien.clone(), Arc::new(EmptyPreferences {}), Arc::new(EmptyPreferences {}));
        let error = block_on(factory.create_with_url(Chain::TronShasta, "https://shasta.example".to_string())).err().unwrap();

        assert_eq!(error.to_string(), "Platform error: network unavailable in this environment: tronshasta");
        assert_eq!(alien.requested_paths(), Vec::<String>::new());
    }

    #[test]
    fn test_shasta_factory_requires_the_matching_genesis_identity() {
        let responses = [
            (
                r#"{"blockID":"00000000000000001ebf88508a03865c71d452e25f4d51194196a1d22b6653dc"}"#,
                "Node reports network 0x2b6653dc for tronshasta",
            ),
            (r#"{"blockID":""}"#, "Network error: Tron genesis block id is too short"),
            (r#"{}"#, "Network error: missing field `blockID` at line 1 column 2"),
        ];

        for (body, message) in responses {
            let alien = Arc::new(TestAlienProvider::with_json(200, body));
            let factory = ChainClientFactory::new_with_environment(
                alien.clone(),
                Arc::new(EmptyPreferences {}),
                Arc::new(EmptyPreferences {}),
                BsvHomeRuntimeEnvironment::LocalDevelopment,
            );
            let error = block_on(factory.create_with_url(Chain::TronShasta, "https://shasta.example".to_string())).err().unwrap();

            if cfg!(debug_assertions) {
                assert_eq!(error.to_string(), message);
                assert_eq!(alien.requested_paths(), vec!["https://shasta.example/wallet/getblockbynum?num=0"]);
            } else {
                assert_eq!(error.to_string(), "Platform error: network unavailable in this environment: tronshasta");
                assert_eq!(alien.requested_paths(), Vec::<String>::new());
            }
        }
    }

    #[test]
    fn test_shasta_factory_preserves_chain_after_identity_validation() {
        let alien = Arc::new(TestAlienProvider::with_json(
            200,
            r#"{"blockID":"0000000000000000de1aa88295e1fcf982742f773e0419c5a9c134c994a9059e"}"#,
        ));
        let factory = ChainClientFactory::new_with_environment(
            alien.clone(),
            Arc::new(EmptyPreferences {}),
            Arc::new(EmptyPreferences {}),
            BsvHomeRuntimeEnvironment::LocalDevelopment,
        );
        let result = block_on(factory.create_with_url(Chain::TronShasta, "https://shasta.example".to_string()));

        if cfg!(debug_assertions) {
            assert_eq!(result.unwrap().get_chain(), Chain::TronShasta);
            assert_eq!(alien.requested_paths(), vec!["https://shasta.example/wallet/getblockbynum?num=0"]);
        } else {
            assert_eq!(result.err().unwrap().to_string(), "Platform error: network unavailable in this environment: tronshasta");
            assert_eq!(alien.requested_paths(), Vec::<String>::new());
        }
    }
}
