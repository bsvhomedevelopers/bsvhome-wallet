use std::collections::HashMap;

use chain_traits::{ChainRequestClassifier, ChainTransactionDecode};
use primitives::{Chain, ChainRequest, ChainRequestType, ChainType};

trait BroadcastProvider: ChainRequestClassifier + ChainTransactionDecode {}

impl<T> BroadcastProvider for T where T: ChainRequestClassifier + ChainTransactionDecode {}

pub struct BroadcastProviders {
    providers: HashMap<Chain, Box<dyn BroadcastProvider>>,
}

impl BroadcastProviders {
    pub fn from_chains(chains: impl IntoIterator<Item = Chain>) -> Self {
        Self {
            providers: chains.into_iter().map(|chain| (chain, new_provider(chain))).collect(),
        }
    }

    fn get_provider(&self, chain: Chain) -> Option<&dyn BroadcastProvider> {
        self.providers.get(&chain).map(|provider| provider.as_ref())
    }

    pub fn classify_request(&self, chain: Chain, request: ChainRequest<'_>) -> ChainRequestType {
        self.get_provider(chain).map_or(ChainRequestType::Unknown, |provider| provider.classify_request(request))
    }

    pub fn decode_transaction_broadcast(&self, chain: Chain, response: &[u8]) -> Option<String> {
        self.get_provider(chain).and_then(|provider| provider.decode_transaction_broadcast_bytes(response))
    }
}

fn new_provider(chain: Chain) -> Box<dyn BroadcastProvider> {
    match chain.chain_type() {
        ChainType::Bitcoin => Box::new(bsv_home_bitcoin::provider::BroadcastProvider),
        ChainType::Ethereum => Box::new(bsv_home_evm::provider::BroadcastProvider),
        ChainType::Solana => Box::new(bsv_home_solana::provider::BroadcastProvider),
        ChainType::Cosmos => Box::new(bsv_home_cosmos::provider::BroadcastProvider),
        ChainType::Ton => Box::new(bsv_home_ton::provider::BroadcastProvider),
        ChainType::Tron => Box::new(bsv_home_tron::provider::BroadcastProvider),
        ChainType::Aptos => Box::new(bsv_home_aptos::provider::BroadcastProvider),
        ChainType::Sui => Box::new(bsv_home_sui::provider::BroadcastProvider),
        ChainType::Xrp => Box::new(bsv_home_xrp::provider::BroadcastProvider),
        ChainType::Near => Box::new(bsv_home_near::provider::BroadcastProvider),
        ChainType::Stellar => Box::new(bsv_home_stellar::provider::BroadcastProvider),
        ChainType::Algorand => Box::new(bsv_home_algorand::provider::BroadcastProvider),
        ChainType::Polkadot => Box::new(bsv_home_polkadot::provider::BroadcastProvider),
        ChainType::Cardano => Box::new(bsv_home_cardano::provider::BroadcastProvider),
        ChainType::HyperCore => Box::new(bsv_home_hypercore::provider::BroadcastProvider),
    }
}
