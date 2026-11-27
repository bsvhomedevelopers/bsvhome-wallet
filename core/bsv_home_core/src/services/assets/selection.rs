use std::sync::Arc;

use primitives::currency::Currency;
use primitives::{Asset, AssetBasic, AssetId, Chain, NFTData, Wallet, WalletType};

use super::model::{BsvHomeAssetAction, BsvHomeSelectAssetFlow, BsvHomeSelectAssetType, BsvHomeWalletSearchLimits};
use super::rules;
use crate::services::chain::rules as chain_rules;
use crate::services::nft::BsvHomeNftItem;
use crate::services::nft::rules as nft_rules;

use crate::services::balance::BsvHomeBalanceService;
use crate::services::error::BsvHomeServiceError;
use crate::services::perpetual::BsvHomePerpetualService;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::price_alert::BsvHomePriceAlertService;
use crate::services::search::{BsvHomeSearchScope, BsvHomeSearchService};
use crate::services::swap::BsvHomeSwapService;
use crate::services::transfer::BsvHomeRecentActivityService;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomeAssetSelectionService {
    search: Arc<BsvHomeSearchService>,
    balances: Arc<BsvHomeBalanceService>,
    price_alerts: Arc<BsvHomePriceAlertService>,
    recent_activity: Arc<BsvHomeRecentActivityService>,
    preferences: Arc<BsvHomePreferencesService>,
    perpetuals: Arc<BsvHomePerpetualService>,
    session: Arc<BsvHomeWalletSessionService>,
    swap: Arc<BsvHomeSwapService>,
}

#[uniffi::export]
impl BsvHomeAssetSelectionService {
    #[uniffi::constructor]
    pub fn new(
        search: Arc<BsvHomeSearchService>,
        balances: Arc<BsvHomeBalanceService>,
        price_alerts: Arc<BsvHomePriceAlertService>,
        recent_activity: Arc<BsvHomeRecentActivityService>,
        preferences: Arc<BsvHomePreferencesService>,
        perpetuals: Arc<BsvHomePerpetualService>,
        session: Arc<BsvHomeWalletSessionService>,
        swap: Arc<BsvHomeSwapService>,
    ) -> Self {
        Self {
            search,
            balances,
            price_alerts,
            recent_activity,
            preferences,
            perpetuals,
            session,
            swap,
        }
    }

    pub fn flow(&self, select_type: BsvHomeSelectAssetType) -> BsvHomeSelectAssetFlow {
        let swap_receive_assets = match &select_type {
            BsvHomeSelectAssetType::SwapReceive { pay_asset_id: Some(pay_asset_id) } => Some(self.swap.supported_assets(pay_asset_id.clone())),
            _ => None,
        };
        rules::select_asset_flow_with_environment(select_type, swap_receive_assets, self.session.environment)
    }

    pub fn wallet_search_limits(&self, query: String) -> BsvHomeWalletSearchLimits {
        rules::wallet_search_limits(&query)
    }

    pub fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }

    pub fn filter_chains(&self, wallet: Wallet) -> Vec<Chain> {
        chain_rules::wallet_chains_by_rank(&self.session.environment.visible_wallet(wallet))
    }

    pub fn supports_tokens(&self, wallet: Option<Wallet>) -> bool {
        self.session.environment.allowed_asset_ids().is_none() && wallet.is_some_and(|wallet| !rules::token_chains(&self.session.environment.visible_wallet(wallet)).is_empty())
    }

    pub fn search_collections(&self, data: Vec<NFTData>, query: String) -> Vec<BsvHomeNftItem> {
        if self.session.environment.allowed_asset_ids().is_some() {
            return Vec::new();
        }
        nft_rules::search_collections(data, &query)
    }

    pub fn show_perpetuals(&self, wallet_type: WalletType, chains: Vec<Chain>) -> bool {
        self.preferences.show_perpetuals(wallet_type, chains)
    }

    pub async fn search_assets(&self, query: String) -> Result<Vec<AssetBasic>, BsvHomeServiceError> {
        self.search.search_assets(self.session.current_wallet().await?, query).await
    }

    pub async fn search(&self, query: String, scope: BsvHomeSearchScope) -> Result<bool, BsvHomeServiceError> {
        self.search.search(self.session.current_wallet().await?, query, scope).await
    }

    pub fn search_key(&self, query: String, scope: BsvHomeSearchScope) -> String {
        scope.search_key(query.trim())
    }

    pub async fn set_assets_enabled(&self, asset_ids: Vec<AssetId>, enabled: bool) -> Result<(), BsvHomeServiceError> {
        self.balances.set_assets_enabled(self.session.current_wallet_id()?, asset_ids, enabled).await
    }

    pub async fn set_asset_pinned(&self, asset_id: AssetId, pinned: bool) -> Result<(), BsvHomeServiceError> {
        self.balances.set_asset_pinned(self.session.current_wallet_id()?, asset_id, pinned).await
    }

    pub async fn set_perpetual_pinned(&self, perpetual_id: String, pinned: bool) -> Result<(), BsvHomeServiceError> {
        self.perpetuals.set_pinned(perpetual_id, pinned).await
    }

    pub async fn add_recent(&self, action: BsvHomeAssetAction, asset: Asset) -> Result<(), BsvHomeServiceError> {
        self.session.environment.require_asset(&asset.id)?;
        self.recent_activity.add_recent(action, asset).await
    }

    pub async fn set_price_alert(&self, asset_id: AssetId, enabled: bool) -> Result<(), BsvHomeServiceError> {
        self.session.environment.require_asset(&asset_id)?;
        self.price_alerts.set_auto_alert(asset_id, enabled).await
    }
}
