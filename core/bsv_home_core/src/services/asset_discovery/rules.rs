use std::collections::HashSet;

use primitives::{Account, AssetId, Chain};

use crate::services::collections::unique;

pub fn discoverable_asset_ids(asset_ids: Vec<String>, accounts: &[Account]) -> Vec<AssetId> {
    let chains: HashSet<Chain> = accounts.iter().map(|account| account.chain).filter(Chain::supports_bsv_home_backend).collect();
    unique(asset_ids.into_iter().filter_map(|id| AssetId::new(&id)).filter(|asset_id| chains.contains(&asset_id.chain)))
}

pub fn has_bsv_home_backend_accounts(accounts: &[Account]) -> bool {
    accounts.iter().any(|account| account.chain.supports_bsv_home_backend())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discoverable_asset_ids_keeps_wallet_chains_and_dedupes() {
        let account = Account {
            chain: Chain::Ethereum,
            address: "0xaddress".into(),
            derivation_path: "".into(),
            extended_public_key: None,
        };

        let asset_ids = discoverable_asset_ids(
            vec!["ethereum_0xusdc".into(), "ethereum_0xusdc".into(), "solana_usdc".into(), "not an id".into()],
            &[account],
        );

        assert_eq!(asset_ids, vec![AssetId::from_token(Chain::Ethereum, "0xusdc")]);
    }

    #[test]
    fn test_remote_discovery_excludes_unsupported_backend_accounts_and_asset_ids() {
        let testnet = Account::mock(Chain::TronShasta, "Tshared");
        let mainnet = Account::mock(Chain::Tron, "Tshared");
        let bsv = Account::mock(Chain::BitcoinSv, "bsv");
        assert_eq!(
            [
                has_bsv_home_backend_accounts(&[]),
                has_bsv_home_backend_accounts(std::slice::from_ref(&testnet)),
                has_bsv_home_backend_accounts(std::slice::from_ref(&bsv)),
                has_bsv_home_backend_accounts(&[mainnet.clone(), testnet.clone(), bsv.clone()]),
            ],
            [false, false, false, true]
        );
        assert_eq!(
            discoverable_asset_ids(
                vec![
                    Chain::Tron.as_asset_id().to_string(),
                    Chain::TronShasta.as_asset_id().to_string(),
                    Chain::BitcoinSv.as_asset_id().to_string()
                ],
                &[mainnet, testnet, bsv],
            ),
            vec![Chain::Tron.as_asset_id()]
        );
    }
}
