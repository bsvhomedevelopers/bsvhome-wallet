use crate::config::docs::DocsUrl;
use crate::models::custom_types::BsvHomeBigInt;
use primitives::{Asset, AssetId, Banner, BannerEvent, BannerState, Chain, Wallet, WalletId};

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBannerContext {
    pub wallet: Option<Wallet>,
    pub asset: Option<Asset>,
    pub is_stakeable: bool,
    pub has_stake_balance: bool,
    pub has_available_balance: bool,
    pub is_asset_activated: bool,
    pub asset_rank_score: Option<i32>,
    pub is_wallet_empty: bool,
}

#[uniffi::export]
impl BsvHomeBannerContext {
    pub fn visible_banners(&self, stored: Vec<Banner>) -> Vec<Banner> {
        super::rules::visible_banners(stored, self)
    }
}

impl BsvHomeBannerContext {
    pub fn wallet(wallet: Wallet, is_wallet_empty: bool) -> Self {
        Self {
            wallet: Some(wallet),
            asset: None,
            is_stakeable: false,
            has_stake_balance: false,
            has_available_balance: false,
            is_asset_activated: true,
            asset_rank_score: None,
            is_wallet_empty,
        }
    }

    pub(super) fn asset_id(&self) -> Option<AssetId> {
        self.asset.as_ref().map(|asset| asset.id.clone())
    }

    pub(super) fn banner(&self, item: BsvHomeBannerItem) -> Banner {
        Banner {
            wallet_id: self.wallet.as_ref().map(|wallet| wallet.id.clone()),
            asset: self.asset.clone(),
            event: item.event,
            state: item.state,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BsvHomeBannerItem {
    pub event: BannerEvent,
    pub state: BannerState,
    pub asset_id: Option<AssetId>,
}

enum BannerScope {
    Asset,
    Chain,
    Wallet,
}

impl BsvHomeBannerItem {
    fn scope(&self) -> BannerScope {
        match self.event {
            BannerEvent::AccountBlockedMultiSignature => BannerScope::Chain,
            BannerEvent::Onboarding => BannerScope::Wallet,
            BannerEvent::Stake | BannerEvent::AccountActivation | BannerEvent::ActivateAsset | BannerEvent::SuspiciousAsset | BannerEvent::TradePerpetuals => BannerScope::Asset,
        }
    }

    pub(super) fn applies_to_asset(&self, asset_id: &AssetId) -> bool {
        match self.scope() {
            BannerScope::Asset => self.asset_id.as_ref() == Some(asset_id),
            BannerScope::Chain => self.asset_id.as_ref().is_some_and(|id| id.chain == asset_id.chain),
            BannerScope::Wallet => false,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBannerKey {
    pub wallet_id: Option<WalletId>,
    pub asset_id: Option<AssetId>,
    pub event: BannerEvent,
}

#[uniffi::export]
impl BsvHomeBannerKey {
    pub fn identifier(&self) -> String {
        [
            self.wallet_id.as_ref().map(WalletId::id),
            self.asset_id.as_ref().map(ToString::to_string),
            Some(self.event.as_ref().to_string()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("_")
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeBannerIcon {
    MoneyBag,
    Network { chain: Chain },
    Warning,
    Suspicious,
    Bitcoin,
    Perpetuals,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeBannerAmount {
    pub value: BsvHomeBigInt,
    pub decimals: i32,
    pub symbol: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeBannerTitle {
    Stake { asset_name: String },
    AccountActivation,
    Warning,
    ActivateAsset,
    SuspiciousAsset,
    Onboarding,
    TradePerpetuals,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeBannerDescription {
    Stake { asset_symbol: String },
    AccountActivation { network_name: String, fee: BsvHomeBannerAmount },
    MultiSignatureBlocked { network_name: String },
    ActivateAsset { asset_symbol: String, network_name: String },
    SuspiciousAsset,
    Onboarding,
    TradePerpetuals,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeBannerLink {
    Docs { item: DocsUrl },
    External { url: String },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeBannerContent {
    pub icon: Option<BsvHomeBannerIcon>,
    pub title: Option<BsvHomeBannerTitle>,
    pub description: Option<BsvHomeBannerDescription>,
    pub link: Option<BsvHomeBannerLink>,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum BsvHomeBannerAction {
    Event { event: BannerEvent },
    Button,
    Close,
}

impl BsvHomeBannerAction {
    pub fn is_dismissal(&self) -> bool {
        matches!(self, Self::Close)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::Chain;

    #[test]
    fn test_only_the_close_action_dismisses_a_banner() {
        assert!(BsvHomeBannerAction::Close.is_dismissal());
        assert!(!BsvHomeBannerAction::Button.is_dismissal());
        assert!(!BsvHomeBannerAction::Event { event: BannerEvent::Stake }.is_dismissal());
    }

    #[test]
    fn test_banner_identifier() {
        let key = |wallet_id: Option<&str>, asset_id: Option<AssetId>, event: BannerEvent| BsvHomeBannerKey {
            wallet_id: wallet_id.map(|id| WalletId::Multicoin(id.to_string())),
            asset_id,
            event,
        };

        assert_eq!(
            key(Some("wallet-1"), Some(AssetId::from_chain(Chain::Bitcoin)), BannerEvent::Stake).identifier(),
            "multicoin_wallet-1_bitcoin_stake"
        );
        assert_eq!(key(None, None, BannerEvent::SuspiciousAsset).identifier(), "suspiciousAsset");
        assert_eq!(key(Some("wallet-1"), None, BannerEvent::Onboarding).identifier(), "multicoin_wallet-1_onboarding");
        assert_eq!(
            key(None, Some(AssetId::from_chain(Chain::Ethereum)), BannerEvent::ActivateAsset).identifier(),
            "ethereum_activateAsset"
        );
    }
}
