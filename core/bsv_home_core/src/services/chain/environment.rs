use primitives::known_assets::TRON_SHASTA_USDT;
use primitives::{Asset, AssetId, Chain, TRANSACTIONS_LIMIT, Wallet, WalletType};

use crate::services::error::BsvHomeServiceError;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeRuntimeEnvironment {
    #[default]
    Production,
    LocalDevelopment,
    WowPay,
}

impl BsvHomeRuntimeEnvironment {
    pub fn allows_chain(self, chain: Chain) -> bool {
        match self {
            Self::Production => !chain.is_testnet(),
            Self::LocalDevelopment => !chain.is_testnet() || cfg!(debug_assertions),
            Self::WowPay => chain == Chain::BitcoinSv,
        }
    }

    pub fn allows_asset(self, asset_id: &AssetId) -> bool {
        match self {
            Self::Production | Self::LocalDevelopment => self.allows_chain(asset_id.chain),
            Self::WowPay => *asset_id == Chain::BitcoinSv.as_asset_id(),
        }
    }

    pub fn require_asset(self, asset_id: &AssetId) -> Result<(), BsvHomeServiceError> {
        self.require_chain(asset_id.chain)?;
        if self.allows_asset(asset_id) {
            Ok(())
        } else {
            Err(BsvHomeServiceError::Unsupported {
                msg: format!("asset unavailable in this environment: {asset_id}"),
            })
        }
    }

    pub fn require_chain(self, chain: Chain) -> Result<(), BsvHomeServiceError> {
        if self.allows_chain(chain) {
            Ok(())
        } else {
            Err(BsvHomeServiceError::Unsupported {
                msg: format!("network unavailable in this environment: {chain}"),
            })
        }
    }

    pub fn supports_test_accounts(self, wallet_type: WalletType) -> bool {
        match wallet_type {
            WalletType::Multicoin | WalletType::Single => self.allows_chain(Chain::TronShasta),
            WalletType::PrivateKey | WalletType::View => false,
        }
    }

    pub fn development_assets(self, wallet: &Wallet) -> Vec<Asset> {
        if !self.supports_test_accounts(wallet.wallet_type) || wallet.account(Chain::TronShasta).is_none() {
            return Vec::new();
        }
        vec![Asset::from_chain(Chain::TronShasta), TRON_SHASTA_USDT.clone()]
    }

    pub fn development_asset_ids(self, wallet: &Wallet) -> Vec<AssetId> {
        self.development_assets(wallet).into_iter().map(|asset| asset.id).collect()
    }
}

#[uniffi::export]
impl BsvHomeRuntimeEnvironment {
    pub fn asset_details_transactions_limit(self) -> u32 {
        match self {
            Self::Production | Self::LocalDevelopment => TRANSACTIONS_LIMIT as u32,
            Self::WowPay => 200,
        }
    }

    pub fn uses_bsv_receipt_events(self) -> bool {
        match self {
            Self::Production | Self::LocalDevelopment => false,
            Self::WowPay => true,
        }
    }

    pub fn uses_bsv_home_hosted_services(self) -> bool {
        match self {
            Self::Production | Self::LocalDevelopment => true,
            Self::WowPay => false,
        }
    }

    pub fn available_chains(self) -> Vec<Chain> {
        match self {
            Self::Production | Self::LocalDevelopment => Chain::all_with_testnets().into_iter().filter(|chain| self.allows_chain(*chain)).collect(),
            Self::WowPay => vec![Chain::BitcoinSv],
        }
    }

    pub fn allowed_asset_ids(self) -> Option<Vec<AssetId>> {
        match self {
            Self::Production | Self::LocalDevelopment => None,
            Self::WowPay => Some(vec![Chain::BitcoinSv.as_asset_id()]),
        }
    }

    pub fn visible_wallet(self, mut wallet: Wallet) -> Wallet {
        let mnemonic = self.supports_test_accounts(wallet.wallet_type);
        wallet
            .accounts
            .retain(|account| self.allows_chain(account.chain) && (!account.chain.is_testnet() || mnemonic));
        wallet
    }

    pub fn allows_wallet(self, wallet: &Wallet) -> bool {
        let identity_allowed = wallet
            .id
            .chain()
            .is_none_or(|chain| self.allows_chain(chain) && (!chain.is_testnet() || self.supports_test_accounts(wallet.wallet_type)));
        match self {
            Self::Production | Self::LocalDevelopment => identity_allowed,
            Self::WowPay => identity_allowed && wallet.accounts.iter().any(|account| self.allows_chain(account.chain)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::Account;
    use primitives::known_assets::TRON_USDT;

    #[test]
    fn test_runtime_environment_rejects_test_network_by_default() {
        let production = BsvHomeRuntimeEnvironment::Production;
        assert_eq!(production.available_chains(), Chain::all());
        assert!(production.allows_chain(Chain::Tron));
        assert!(!production.allows_chain(Chain::TronShasta));
        assert!(production.require_chain(Chain::TronShasta).is_err());
        assert_eq!(BsvHomeRuntimeEnvironment::LocalDevelopment.allows_chain(Chain::TronShasta), cfg!(debug_assertions));
        assert!(!BsvHomeRuntimeEnvironment::LocalDevelopment.supports_test_accounts(WalletType::PrivateKey));
        assert!(!BsvHomeRuntimeEnvironment::LocalDevelopment.supports_test_accounts(WalletType::View));
        assert_eq!(production.asset_details_transactions_limit() as usize, TRANSACTIONS_LIMIT);
        assert_eq!(BsvHomeRuntimeEnvironment::LocalDevelopment.asset_details_transactions_limit() as usize, TRANSACTIONS_LIMIT);
    }

    #[test]
    fn test_wowpay_asset_scope_rejects_same_symbol_tokens_and_retained_networks() {
        let environment = BsvHomeRuntimeEnvironment::WowPay;
        assert_eq!(environment.asset_details_transactions_limit(), 200);
        let allowed = environment.allowed_asset_ids().unwrap();
        for asset_id in &allowed {
            assert_eq!(environment.require_asset(asset_id), Ok(()));
        }
        let hidden = [
            Chain::Ethereum.as_asset_id(),
            Chain::Tron.as_asset_id(),
            TRON_USDT.id.clone(),
            Chain::TronShasta.as_asset_id(),
            TRON_SHASTA_USDT.id.clone(),
            AssetId::from_token(Chain::Tron, "other-contract"),
            AssetId::from_token(Chain::BitcoinSv, "mnee"),
        ];
        for asset_id in hidden {
            assert!(!environment.allows_asset(&asset_id));
            assert!(environment.require_asset(&asset_id).is_err());
        }
        assert_eq!(environment.available_chains(), vec![Chain::BitcoinSv]);
        assert_eq!(environment.development_assets(&Wallet::mock()), Vec::<Asset>::new());
        assert_eq!(BsvHomeRuntimeEnvironment::Production.allowed_asset_ids(), None);
        assert_eq!(BsvHomeRuntimeEnvironment::LocalDevelopment.allowed_asset_ids(), None);
        assert!(BsvHomeRuntimeEnvironment::Production.allows_asset(&AssetId::from_token(Chain::Tron, "other-contract")));
    }

    #[test]
    fn test_runtime_environment_hides_retained_test_accounts_for_ineligible_wallets() {
        let saved = Wallet::mock_with_accounts(vec![Account::mock(Chain::Tron, "mainnet"), Account::mock(Chain::TronShasta, "testnet")]);
        let production = BsvHomeRuntimeEnvironment::Production.visible_wallet(saved.clone());

        assert_eq!(saved.accounts.len(), 2);
        assert_eq!(production.accounts.iter().map(|account| account.chain).collect::<Vec<_>>(), vec![Chain::Tron]);
        assert_eq!(BsvHomeRuntimeEnvironment::Production.development_asset_ids(&saved), Vec::new());

        for wallet_type in [WalletType::Multicoin, WalletType::Single, WalletType::PrivateKey, WalletType::View] {
            let wallet = Wallet { wallet_type, ..saved.clone() };
            let visible = BsvHomeRuntimeEnvironment::LocalDevelopment.visible_wallet(wallet);
            let expected = match wallet_type {
                WalletType::Multicoin | WalletType::Single if cfg!(debug_assertions) => vec![Chain::Tron, Chain::TronShasta],
                WalletType::Multicoin | WalletType::Single | WalletType::PrivateKey | WalletType::View => vec![Chain::Tron],
            };

            assert_eq!(visible.accounts.iter().map(|account| account.chain).collect::<Vec<_>>(), expected);
            assert_eq!(
                BsvHomeRuntimeEnvironment::LocalDevelopment.development_asset_ids(&visible).len(),
                if expected.len() == 2 { 2 } else { 0 }
            );
        }
    }
}
