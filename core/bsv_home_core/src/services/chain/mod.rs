mod environment;
pub mod rules;

pub use environment::BsvHomeRuntimeEnvironment;

use primitives::Chain;

use crate::wallet_connect::{wallet_connect_chain, wallet_connect_namespace, wallet_connect_reference};

#[derive(Default, uniffi::Object)]
pub struct BsvHomeChainService {
    environment: BsvHomeRuntimeEnvironment,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wowpay_chain_selectors_filter_retained_networks_and_queries() {
        let service = BsvHomeChainService::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
        assert_eq!(service.get_chains(String::new()), vec![Chain::BitcoinSv]);
        assert_eq!(service.get_chains("trx".into()), Vec::<Chain>::new());
        assert_eq!(service.get_chains(" ETH ".into()), Vec::<Chain>::new());
        assert_eq!(service.get_chains("bsv".into()), vec![Chain::BitcoinSv]);
        assert_eq!(
            service.get_matching_chains(vec![Chain::Ethereum, Chain::BitcoinSv, Chain::TronShasta, Chain::Tron], String::new()),
            vec![Chain::BitcoinSv]
        );
        assert_eq!(BsvHomeChainService::new().get_chains(String::new()), rules::chains_by_rank());
    }
}

#[uniffi::export]
impl BsvHomeChainService {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self::new_with_environment(BsvHomeRuntimeEnvironment::Production)
    }

    #[uniffi::constructor]
    pub fn new_with_environment(environment: BsvHomeRuntimeEnvironment) -> Self {
        Self { environment }
    }

    pub fn get_chains(&self, query: String) -> Vec<Chain> {
        rules::matching_chains(
            rules::chains_by_rank()
                .into_iter()
                .filter(|chain| self.environment.allows_chain(*chain))
                .chain(self.environment.available_chains().into_iter().filter(|chain| chain.is_testnet()))
                .collect(),
            &query,
        )
    }

    pub fn get_matching_chains(&self, chains: Vec<Chain>, query: String) -> Vec<Chain> {
        rules::matching_chains(chains.into_iter().filter(|chain| self.environment.allows_chain(*chain)).collect(), &query)
    }

    pub fn caip2_namespace(&self, chain: Chain) -> Option<String> {
        wallet_connect_namespace(chain)
    }

    pub fn caip2_reference(&self, chain: Chain) -> Option<String> {
        wallet_connect_reference(chain)
    }

    pub fn chain_from_caip2(&self, chain_id: String) -> Option<Chain> {
        wallet_connect_chain(chain_id)
    }
}
