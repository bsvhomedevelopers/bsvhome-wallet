use primitives::{AssetId, Chain, Wallet};

use super::model::BsvHomeMemoWarning;
use crate::config::chain::is_memo_supported;
use crate::services::chain::BsvHomeRuntimeEnvironment;

pub fn memo_warning(chain: Chain) -> BsvHomeMemoWarning {
    if !is_memo_supported(chain) {
        return BsvHomeMemoWarning::NotSupported;
    }
    match chain {
        Chain::Xrp => BsvHomeMemoWarning::DestinationTag,
        _ => BsvHomeMemoWarning::Memo,
    }
}

pub fn network_asset_ids(asset_id: AssetId, associations: Vec<AssetId>, wallet: &Wallet, environment: BsvHomeRuntimeEnvironment) -> Vec<AssetId> {
    let mut asset_ids: Vec<AssetId> = Vec::new();
    for candidate in std::iter::once(asset_id).chain(associations) {
        if environment.allows_asset(&candidate) && wallet.account(candidate.chain).is_some() && !asset_ids.contains(&candidate) {
            asset_ids.push(candidate);
        }
    }
    asset_ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::Account;
    use primitives::known_assets::{TRON_SHASTA_USDT, TRON_USDT};

    #[test]
    fn test_network_asset_ids_keeps_wallet_chains_in_order_without_duplicates() {
        let ethereum_usdc = AssetId::from_token(Chain::Ethereum, "0xusdc");
        let base_usdc = AssetId::from_token(Chain::Base, "0xusdc");
        let solana_usdc = AssetId::from_token(Chain::Solana, "usdc");

        let asset_ids = network_asset_ids(
            ethereum_usdc.clone(),
            vec![base_usdc.clone(), solana_usdc, ethereum_usdc.clone()],
            &Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Ethereum, Chain::Base], "address")),
            BsvHomeRuntimeEnvironment::Production,
        );

        assert_eq!(asset_ids, vec![ethereum_usdc, base_usdc]);
    }

    #[test]
    fn test_memo_warning_names_the_field_each_chain_uses() {
        assert_eq!(memo_warning(Chain::Xrp), BsvHomeMemoWarning::DestinationTag);
        assert_eq!(memo_warning(Chain::Cosmos), BsvHomeMemoWarning::Memo);
        assert_eq!(memo_warning(Chain::Ton), BsvHomeMemoWarning::Memo);
        assert_eq!(memo_warning(Chain::Ethereum), BsvHomeMemoWarning::NotSupported);
        assert_eq!(memo_warning(Chain::Bitcoin), BsvHomeMemoWarning::NotSupported);
    }

    #[test]
    fn test_wowpay_receive_associations_require_exact_assets_and_a_wallet_account() {
        let usdt = TRON_USDT.id.clone();
        let bsv = Chain::BitcoinSv.as_asset_id();
        let candidates = vec![
            Chain::Ethereum.as_asset_id(),
            AssetId::from_token(Chain::Tron, "same-symbol-usdt"),
            TRON_SHASTA_USDT.id.clone(),
            bsv.clone(),
            usdt.clone(),
        ];
        let wallet = Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Ethereum, Chain::Tron, Chain::TronShasta, Chain::BitcoinSv], "address"));
        assert_eq!(network_asset_ids(usdt.clone(), candidates.clone(), &wallet, BsvHomeRuntimeEnvironment::WowPay), vec![bsv]);
        let tron = Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Tron], "address"));
        assert_eq!(network_asset_ids(usdt.clone(), candidates, &tron, BsvHomeRuntimeEnvironment::WowPay), Vec::<AssetId>::new());
    }
}
