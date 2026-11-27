mod broadcast_providers;
mod chain_providers;
mod provider_config;

use std::{collections::HashMap, sync::Arc};

use bsv_home_alchemy::{AlchemyApi, alchemy_url};
use bsv_home_algorand::rpc::{AlgorandClient, AlgorandIndexer, AlgorandProvider};
use bsv_home_aptos::rpc::AptosClient;
use bsv_home_bitcoin::rpc::client::BitcoinClient;
use bsv_home_bsc::BscStakingClient;
use bsv_home_cardano::rpc::CardanoClient;
use bsv_home_client::{ReqwestClient, retry_policy};
use bsv_home_cosmos::rpc::client::CosmosClient;
use bsv_home_everstake::{EverstakeClient, EverstakeStakingClient};
use bsv_home_evm::rpc::{EVMAssetBalanceProvider, EVMIndexer, EVMTransactionsByAddressProvider, EthereumClient, EthereumProvider};
use bsv_home_hypercore::rpc::client::HyperCoreClient;
use bsv_home_jsonrpc::client::JsonRpcClient;
use bsv_home_monad::MonadStakingClient;
use bsv_home_near::rpc::{NearClient, NearIndexer, NearProvider};
use bsv_home_polkadot::rpc::{PolkadotClient, PolkadotIndexer, PolkadotProvider};
use bsv_home_solana::rpc::{SolanaClient, SolanaIndexer, SolanaProvider};
use bsv_home_stellar::rpc::client::StellarClient;
use bsv_home_sui::rpc::{SuiClient, SuiIndexer, SuiProvider};
use bsv_home_tempo::TempoProvider;
use bsv_home_ton::rpc::TonClient;
use bsv_home_tron::rpc::{TronProvider, client::TronClient, trongrid::client::TronGridClient};
use bsv_home_xrp::rpc::XrpClient;
use chain_traits::ChainTraits;
use reqwest::Client;

use primitives::{BitcoinChain, Chain, ChainType, EVMChain, chain_cosmos::CosmosChain};
use settings::Settings;

pub use broadcast_providers::BroadcastProviders;
pub use chain_providers::ChainProviders;
pub use chain_traits::{TransactionFeeEstimate, TransactionFeeEstimates, TransactionIdRequest, TransactionsRequest, TransactionsResult};
pub use provider_config::ProviderConfig;

pub struct ProviderFactory {}

impl ProviderFactory {
    pub fn new_from_settings_with_user_agent(chain: Chain, settings: &Settings, user_agent: &str) -> Box<dyn ChainTraits> {
        let url = Self::get_chain_url(chain, settings);

        Self::new_provider(ProviderConfig::from_settings(chain, &url, settings), user_agent)
    }

    pub(crate) fn new_providers_with_user_agent(settings: &Settings, user_agent: &str) -> Vec<Box<dyn ChainTraits>> {
        Chain::all()
            .iter()
            .map(|chain| Self::new_from_settings_with_user_agent(*chain, settings, user_agent))
            .collect()
    }

    fn new_provider(config: ProviderConfig, user_agent: &str) -> Box<dyn ChainTraits> {
        let host = config.url.parse::<url::Url>().ok().and_then(|url| url.host_str().map(String::from)).unwrap_or_default();
        let retry = if config.chain == Chain::BitcoinSv {
            reqwest::retry::never()
        } else {
            retry_policy(host, 3)
        };
        let reqwest_client = bsv_home_client::builder().retry(retry).build().expect("Failed to build reqwest client");
        Self::build_provider(config, user_agent, reqwest_client)
    }

    pub fn new_provider_with_client(config: ProviderConfig, reqwest_client: Client) -> Box<dyn ChainTraits> {
        Self::build_provider(config, "", reqwest_client)
    }

    fn build_provider(config: ProviderConfig, user_agent: &str, reqwest_client: Client) -> Box<dyn ChainTraits> {
        let bsv_home_client = ReqwestClient::new_with_user_agent(config.url.clone(), reqwest_client, user_agent.to_string());
        let chain = config.chain;

        match chain.chain_type() {
            ChainType::Bitcoin => match chain {
                Chain::BitcoinSv => Box::new(bsv_home_bsv::rpc::BsvClient::new(bsv_home_client)),
                _ => Box::new(BitcoinClient::new(bsv_home_client, BitcoinChain::from_chain(chain).unwrap())),
            },
            ChainType::Ethereum => {
                let evm_chain = EVMChain::from_chain(chain).unwrap();
                let rpc_client = JsonRpcClient::new(bsv_home_client.clone());
                let client = EthereumClient::new(rpc_client, evm_chain);
                TempoProvider::new_or_else(client, |client| {
                    let everstake = EverstakeClient::new(bsv_home_client.clone().with_base_url(config.everstake_url.clone()));
                    let indexer = EVMIndexer::for_chain(
                        bsv_home_client
                            .clone()
                            .with_base_url(alchemy_url(chain, &config.indexers.alchemy.url, AlchemyApi::JsonRpc, &config.indexers.alchemy.key)),
                        bsv_home_client.clone().with_base_url(format!("{}/{}", config.indexers.ankr.url, config.indexers.ankr.key)),
                        config.indexers.blockscout.configure_client(bsv_home_client),
                        config.indexers.blockscout.key,
                        evm_chain,
                    );
                    if let Some(indexer) = indexer {
                        let indexer = Arc::new(indexer);
                        let transactions = Box::new(EVMTransactionsByAddressProvider::new(indexer.clone()));
                        let asset_balances = Box::new(EVMAssetBalanceProvider::new(indexer));
                        Box::new(match evm_chain {
                            EVMChain::Ethereum => {
                                EthereumProvider::new_with_provider(client.clone(), transactions, asset_balances, Box::new(EverstakeStakingClient::new(client, Some(everstake))))
                            }
                            EVMChain::Monad => EthereumProvider::new_with_provider(client.clone(), transactions, asset_balances, Box::new(MonadStakingClient::new(client))),
                            EVMChain::SmartChain => EthereumProvider::new_with_provider(client.clone(), transactions, asset_balances, Box::new(BscStakingClient::new(client))),
                            _ => EthereumProvider::new(client, transactions, asset_balances),
                        })
                    } else {
                        Box::new(match evm_chain {
                            EVMChain::Ethereum => EthereumProvider::new_rpc_only_with_provider(client.clone(), Box::new(EverstakeStakingClient::new(client, Some(everstake)))),
                            EVMChain::Monad => EthereumProvider::new_rpc_only_with_provider(client.clone(), Box::new(MonadStakingClient::new(client))),
                            EVMChain::SmartChain => EthereumProvider::new_rpc_only_with_provider(client.clone(), Box::new(BscStakingClient::new(client))),
                            _ => EthereumProvider::new_rpc_only(client),
                        })
                    }
                })
            }
            ChainType::Cardano => Box::new(CardanoClient::new(bsv_home_client)),
            ChainType::Cosmos => {
                let chain = CosmosChain::from_chain(chain).unwrap();
                Box::new(CosmosClient::new(chain, bsv_home_client))
            }
            ChainType::Aptos => Box::new(AptosClient::new(bsv_home_client)),
            ChainType::Sui => Box::new(SuiProvider::new(
                SuiClient::new(config.url),
                Box::new(SuiIndexer::new(config.indexers.sui.configure_client(bsv_home_client))),
            )),
            ChainType::Xrp => Box::new(XrpClient::new(JsonRpcClient::new(bsv_home_client))),
            ChainType::Algorand => Box::new(AlgorandProvider::new(
                AlgorandClient::new(bsv_home_client.clone()),
                Box::new(AlgorandIndexer::new(config.indexers.algorand.configure_client(bsv_home_client))),
            )),
            ChainType::Stellar => Box::new(StellarClient::new(bsv_home_client)),
            ChainType::Near => Box::new(NearProvider::new(
                NearClient::new(JsonRpcClient::new(bsv_home_client.clone())),
                Box::new(NearIndexer::new(
                    config.indexers.fastnear.neardata.configure_client(bsv_home_client.clone()),
                    config.indexers.fastnear.transfers.configure_client(bsv_home_client.clone()),
                    config.indexers.fastnear.tx.configure_client(bsv_home_client),
                )),
            )),
            ChainType::Polkadot => Box::new(PolkadotProvider::new(
                PolkadotClient::new(bsv_home_client.clone()),
                Box::new(PolkadotIndexer::new(
                    config
                        .indexers
                        .subscan
                        .configure_client(bsv_home_client)
                        .with_default_headers(HashMap::from([("x-api-key".to_string(), config.indexers.subscan.key)])),
                )),
            )),
            ChainType::Solana => Box::new(SolanaProvider::new(
                SolanaClient::new(JsonRpcClient::new(bsv_home_client.clone())),
                Box::new(SolanaIndexer::new(JsonRpcClient::new(bsv_home_client.with_base_url(alchemy_url(
                    chain,
                    &config.indexers.alchemy.url,
                    AlchemyApi::JsonRpc,
                    &config.indexers.alchemy.key,
                ))))),
            )),
            ChainType::Ton => Box::new(TonClient::new(bsv_home_client)),
            ChainType::Tron => {
                let trongrid = TronGridClient::new_with_chain(chain, config.indexers.trongrid.configure_client(bsv_home_client.clone()), config.indexers.trongrid.key);
                Box::new(TronProvider::new(
                    TronClient::new_with_chain(chain, bsv_home_client),
                    Box::new(trongrid.clone()),
                    Box::new(trongrid),
                ))
            }
            ChainType::HyperCore => Box::new(HyperCoreClient::new(bsv_home_client)),
        }
    }

    pub fn get_chain_config(chain: Chain, settings: &Settings) -> &settings::Chain {
        match chain {
            Chain::Bitcoin => &settings.chains.bitcoin,
            Chain::BitcoinCash => &settings.chains.bitcoincash,
            Chain::BitcoinSv => &settings.chains.bitcoinsv,
            Chain::Litecoin => &settings.chains.litecoin,
            Chain::Ethereum => &settings.chains.ethereum,
            Chain::SmartChain => &settings.chains.smartchain,
            Chain::Solana => &settings.chains.solana,
            Chain::Polygon => &settings.chains.polygon,
            Chain::Thorchain => &settings.chains.thorchain,
            Chain::Mayachain => &settings.chains.mayachain,
            Chain::Cosmos => &settings.chains.cosmos,
            Chain::Osmosis => &settings.chains.osmosis,
            Chain::Arbitrum => &settings.chains.arbitrum,
            Chain::Ton => &settings.chains.ton,
            Chain::Tron => &settings.chains.tron,
            Chain::TronShasta => &settings.chains.tronshasta,
            Chain::Doge => &settings.chains.doge,
            Chain::Zcash => &settings.chains.zcash,
            Chain::Optimism => &settings.chains.optimism,
            Chain::Aptos => &settings.chains.aptos,
            Chain::Base => &settings.chains.base,
            Chain::AvalancheC => &settings.chains.avalanchec,
            Chain::Sui => &settings.chains.sui,
            Chain::Xrp => &settings.chains.xrp,
            Chain::OpBNB => &settings.chains.opbnb,
            Chain::Fantom => &settings.chains.fantom,
            Chain::Gnosis => &settings.chains.gnosis,
            Chain::Celestia => &settings.chains.celestia,
            Chain::Injective => &settings.chains.injective,
            Chain::Sei => &settings.chains.sei,
            Chain::SeiEvm => &settings.chains.seievm,
            Chain::Manta => &settings.chains.manta,
            Chain::Blast => &settings.chains.blast,
            Chain::Noble => &settings.chains.noble,
            Chain::ZkSync => &settings.chains.zksync,
            Chain::Linea => &settings.chains.linea,
            Chain::Mantle => &settings.chains.mantle,
            Chain::Celo => &settings.chains.celo,
            Chain::Near => &settings.chains.near,
            Chain::World => &settings.chains.world,
            Chain::Plasma => &settings.chains.plasma,
            Chain::Stellar => &settings.chains.stellar,
            Chain::Sonic => &settings.chains.sonic,
            Chain::Algorand => &settings.chains.algorand,
            Chain::Polkadot => &settings.chains.polkadot,
            Chain::Cardano => &settings.chains.cardano,
            Chain::Abstract => &settings.chains.abstract_chain,
            Chain::Berachain => &settings.chains.berachain,
            Chain::Ink => &settings.chains.ink,
            Chain::Unichain => &settings.chains.unichain,
            Chain::Hyperliquid => &settings.chains.hyperliquid,
            Chain::HyperCore => &settings.chains.hypercore,
            Chain::Monad => &settings.chains.monad,
            Chain::XLayer => &settings.chains.xlayer,
            Chain::Robinhood => &settings.chains.robinhood,
            Chain::Stable => &settings.chains.stable,
            Chain::Tempo => &settings.chains.tempo,
        }
    }

    pub fn get_chain_endpoints(settings: &Settings) -> HashMap<Chain, String> {
        Chain::all()
            .into_iter()
            .map(|chain| (chain, Self::get_chain_url(chain, settings)))
            .filter(|(_, url)| !url.is_empty())
            .collect()
    }

    pub fn get_chain_url(chain: Chain, settings: &Settings) -> String {
        if chain.is_testnet() || settings.dynode.url.is_empty() {
            Self::get_chain_config(chain, settings).url.clone()
        } else {
            format!("{}/{}", settings.dynode.url, chain.as_ref())
        }
    }
}
