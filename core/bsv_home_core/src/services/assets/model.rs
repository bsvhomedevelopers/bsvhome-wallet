use primitives::{Asset, AssetId, AssetType, Chain, RecentActivityType};
use strum::IntoEnumIterator;
use swapper::AssetList as SwapAssetList;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetList {
    Buy,
    Sell,
    Swap,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeAssetNetworkDestination {
    Asset { asset: Asset },
    Assets { chain: Chain },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAssetAction {
    Open,
    Send,
    Receive,
    Buy,
    Sell,
    SwapPay,
    SwapReceive,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeSelectAssetType {
    Send,
    Receive,
    ReceiveCollection,
    Buy,
    SwapPay,
    SwapReceive { pay_asset_id: Option<AssetId> },
    Manage,
    PriceAlert,
    Deposit,
    Withdraw,
    WalletSearch,
    WalletSearchResults,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeSelectRowAction {
    Navigate,
    Toggle,
    Select,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeSelectAssetScope {
    Wallet,
    AllAssets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAssetRowTitle {
    Asset,
    Network,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAssetRowSubtitle {
    Network,
    Price,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAssetRowTrailing {
    Balance,
    Toggle,
    Copy,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeAssetRow {
    pub title: BsvHomeAssetRowTitle,
    pub shows_symbol: bool,
    pub subtitle: BsvHomeAssetRowSubtitle,
    pub trailing: BsvHomeAssetRowTrailing,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeSelectAssetFlow {
    pub row: BsvHomeAssetRow,
    pub row_action: BsvHomeSelectRowAction,
    pub action: Option<BsvHomeAssetAction>,
    pub scope: BsvHomeSelectAssetScope,
    pub filters: Vec<BsvHomeAssetFilter>,
    pub enables_price_alert: bool,
    pub network_search: bool,
    pub chain_filter: bool,
    pub recents: bool,
    pub popular_section: bool,
    pub balance_filter: bool,
    pub add_custom_token: bool,
    pub deposit_asset_display: bool,
}

#[uniffi::export]
impl BsvHomeSelectAssetType {
    pub fn flow(&self) -> BsvHomeSelectAssetFlow {
        super::rules::select_asset_flow(self.clone(), None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAssetFilter {
    Enabled,
    Buyable,
    Sellable,
    Swappable,
    HasBalance,
    HasAvailableBalance,
    ChainsOrAssetIds { chains: Vec<Chain>, asset_ids: Vec<AssetId> },
}

impl BsvHomeAssetFilter {
    pub fn asset_ids(asset_ids: Vec<AssetId>) -> Self {
        Self::ChainsOrAssetIds { chains: Vec::new(), asset_ids }
    }
}

impl From<SwapAssetList> for BsvHomeAssetFilter {
    fn from(assets: SwapAssetList) -> Self {
        Self::ChainsOrAssetIds {
            chains: assets.chains,
            asset_ids: assets.asset_ids,
        }
    }
}

#[uniffi::export]
impl BsvHomeAssetAction {
    pub fn filters(&self) -> Vec<BsvHomeAssetFilter> {
        match self {
            Self::Open => Vec::new(),
            Self::Send => vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::HasBalance],
            Self::Receive => vec![BsvHomeAssetFilter::Enabled],
            Self::Buy => vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Buyable],
            Self::Sell => vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Sellable],
            Self::SwapPay => vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Swappable, BsvHomeAssetFilter::HasAvailableBalance],
            Self::SwapReceive => vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Swappable],
        }
    }

    pub fn recent_activity_types(&self) -> Vec<RecentActivityType> {
        match self {
            Self::SwapPay | Self::SwapReceive => vec![RecentActivityType::SwapSelect, RecentActivityType::Swap],
            Self::Open | Self::Send | Self::Receive | Self::Buy | Self::Sell => RecentActivityType::iter().collect(),
        }
    }
}

impl BsvHomeAssetAction {
    pub fn recent_activity_type(&self, asset: &Asset) -> Option<RecentActivityType> {
        match self {
            Self::Open => Some(match asset.asset_type {
                AssetType::PERPETUAL => RecentActivityType::Perpetual,
                _ => RecentActivityType::Search,
            }),
            Self::Send => None,
            Self::Receive => Some(RecentActivityType::Receive),
            Self::Buy => Some(RecentActivityType::FiatBuy),
            Self::Sell => Some(RecentActivityType::FiatSell),
            Self::SwapPay | Self::SwapReceive => Some(RecentActivityType::SwapSelect),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Asset, AssetType, BsvHomeAssetAction, BsvHomeAssetFilter, RecentActivityType};
    use primitives::Chain;

    #[test]
    fn test_every_recorded_recent_type_is_shown_by_the_same_action() {
        let asset = Asset::from_chain(Chain::Ethereum);
        let perpetual = Asset {
            asset_type: AssetType::PERPETUAL,
            ..Asset::from_chain(Chain::HyperCore)
        };
        let actions = [
            BsvHomeAssetAction::Open,
            BsvHomeAssetAction::Send,
            BsvHomeAssetAction::Receive,
            BsvHomeAssetAction::Buy,
            BsvHomeAssetAction::Sell,
            BsvHomeAssetAction::SwapPay,
            BsvHomeAssetAction::SwapReceive,
        ];
        for action in actions {
            for asset in [&asset, &perpetual] {
                if let Some(recorded) = action.recent_activity_type(asset) {
                    assert!(action.recent_activity_types().contains(&recorded), "{action:?}");
                }
            }
        }
        assert_eq!(BsvHomeAssetAction::Send.recent_activity_type(&asset), None);
        assert_eq!(BsvHomeAssetAction::Open.recent_activity_type(&perpetual), Some(RecentActivityType::Perpetual));
        assert_eq!(BsvHomeAssetAction::Open.recent_activity_type(&asset), Some(RecentActivityType::Search));
        assert!(!BsvHomeAssetAction::SwapPay.recent_activity_types().contains(&RecentActivityType::Receive));
    }

    #[test]
    fn test_action_filters_gate_on_the_balance_each_action_can_spend() {
        assert_eq!(BsvHomeAssetAction::Send.filters(), vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::HasBalance]);
        assert_eq!(
            BsvHomeAssetAction::SwapPay.filters(),
            vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Swappable, BsvHomeAssetFilter::HasAvailableBalance]
        );
        assert_eq!(BsvHomeAssetAction::SwapReceive.filters(), vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Swappable]);
        assert_eq!(BsvHomeAssetAction::Buy.filters(), vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Buyable]);
        assert_eq!(BsvHomeAssetAction::Sell.filters(), vec![BsvHomeAssetFilter::Enabled, BsvHomeAssetFilter::Sellable]);
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeWalletSearchLimits {
    pub assets: u32,
    pub fetch: u32,
    pub perpetuals: u32,
    pub nfts: u32,
    pub results: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeHeaderButtonKind {
    Send,
    Receive,
    Buy,
    Swap,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeHeaderButton {
    pub kind: BsvHomeHeaderButtonKind,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeHeaderActions {
    WatchOnly,
    Buttons { buttons: Vec<BsvHomeHeaderButton> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAssetEmptyAction {
    Buy,
    Swap,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeAssetDetailsState {
    pub is_view_only: bool,
    pub header_actions: BsvHomeHeaderActions,
    pub shows_banners: bool,
    pub shows_manage: bool,
    pub shows_resources: bool,
    pub shows_price_alerts: bool,
    pub shows_price_change: bool,
    pub shows_earn: bool,
    pub empty_transactions_action: Option<BsvHomeAssetEmptyAction>,
}
