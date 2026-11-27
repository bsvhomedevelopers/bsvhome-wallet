mod rules;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use primitives::known_assets::HYPERCORE_PERPETUAL_USDC;
use primitives::{Asset, AssetFiatValue, AssetId, Banner, BannerEvent, Chain, Currency, FiatRate, PerpetualBalance, TotalFiatValue, Wallet};

use crate::services::asset_discovery::BsvHomeAssetDiscoveryService;
use crate::services::assets::BsvHomeAssetsService;
use crate::services::assets::model::BsvHomeHeaderActions;
use crate::services::assets::rules::bsv_wallet_header_actions;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::balance::rules as balance_rules;
use crate::services::banner::{BsvHomeBannerAction, BsvHomeBannerContent, BsvHomeBannerContext, BsvHomeBannerKey, BsvHomeBannerService};
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::wallet_preferences::{BsvHomeDiscoveryStep, BsvHomeWalletPreferencesService};
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeWalletHomeViewState {
    pub total_value: TotalFiatValue,
    pub shows_pnl: bool,
    pub header_actions: BsvHomeHeaderActions,
    pub show_collections: bool,
    pub visible_banners: Vec<Banner>,
}

#[derive(uniffi::Object)]
pub struct BsvHomeWalletHomeService {
    balances: Arc<BsvHomeBalanceService>,
    assets: Arc<BsvHomeAssetsService>,
    discovery: Arc<BsvHomeAssetDiscoveryService>,
    banners: Arc<BsvHomeBannerService>,
    wallet_preferences: Arc<BsvHomeWalletPreferencesService>,
    preferences: Arc<BsvHomePreferencesService>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomeWalletHomeService {
    #[uniffi::constructor]
    pub fn new(
        balances: Arc<BsvHomeBalanceService>,
        assets: Arc<BsvHomeAssetsService>,
        discovery: Arc<BsvHomeAssetDiscoveryService>,
        banners: Arc<BsvHomeBannerService>,
        wallet_preferences: Arc<BsvHomeWalletPreferencesService>,
        preferences: Arc<BsvHomePreferencesService>,
        session: Arc<BsvHomeWalletSessionService>,
    ) -> Self {
        Self {
            balances,
            assets,
            discovery,
            banners,
            wallet_preferences,
            preferences,
            session,
        }
    }

    pub fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }

    pub fn available_chains(&self) -> Vec<Chain> {
        self.session.environment.available_chains()
    }

    pub fn development_asset_ids(&self, wallet: Wallet) -> Vec<AssetId> {
        self.session.environment.development_asset_ids(&wallet)
    }

    pub fn convert_total_value(&self, value: f64, currency: Currency, rates: Vec<FiatRate>) -> Option<f64> {
        rules::convert_total_value(value, currency, rates)
    }

    pub fn view_state(
        &self,
        wallet: Wallet,
        balances: Vec<AssetFiatValue>,
        perpetual: Option<PerpetualBalance>,
        banners: Vec<Banner>,
        is_wallet_empty: bool,
    ) -> BsvHomeWalletHomeViewState {
        let wallet = self.session.environment.visible_wallet(wallet);
        let chains = wallet.chains();
        let total_value = self.total_fiat_value(balances, perpetual);
        let banner_events = banners.iter().map(|banner| banner.event).collect::<Vec<_>>();
        BsvHomeWalletHomeViewState {
            shows_pnl: balance_rules::shows_pnl(&total_value),
            header_actions: bsv_wallet_header_actions(&wallet, Chain::BitcoinSv, &banner_events, self.assets.environment())
                .unwrap_or_else(|| rules::header_actions(wallet.wallet_type, &chains, rules::header_buttons_enabled(&banners))),
            show_collections: self.preferences.show_collections(wallet.wallet_type, chains),
            visible_banners: BsvHomeBannerContext::wallet(wallet, is_wallet_empty).visible_banners(banners),
            total_value,
        }
    }

    pub async fn update_balances(&self, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        self.balances.update(self.session.current_wallet_id()?, asset_ids).await
    }

    pub fn shows_initial_loading(&self) -> Result<bool, BsvHomeServiceError> {
        let wallet_id = self.session.current_wallet_id()?;
        let completed = self.wallet_preferences.is_initial_load_completed(wallet_id.clone(), BsvHomeDiscoveryStep::Assets)?;
        Ok(rules::shows_initial_loading(completed, self.wallet_preferences.get_assets_timestamp(wallet_id)))
    }

    pub async fn refresh(&self) -> Result<(), BsvHomeServiceError> {
        let wallet_id = self.session.current_wallet_id()?;
        let (balances, discovery, prices) = futures::join!(
            self.balances.update_enabled_display_balances(wallet_id.clone()),
            self.discovery.discover(wallet_id.clone()),
            async {
                let asset_ids = self.balances.enabled_asset_ids(wallet_id).await?;
                self.assets.sync_defillama_prices(&asset_ids).await.map(|_| ())
            }
        );
        balances?;
        discovery?;
        prices
    }

    pub async fn set_asset_pinned(&self, asset_id: AssetId, pinned: bool) -> Result<(), BsvHomeServiceError> {
        self.balances.set_asset_pinned(self.session.current_wallet_id()?, asset_id, pinned).await
    }

    pub async fn set_assets_enabled(&self, asset_ids: Vec<AssetId>, enabled: bool) -> Result<(), BsvHomeServiceError> {
        self.balances.set_assets_enabled(self.session.current_wallet_id()?, asset_ids, enabled).await
    }

    pub fn banner_content(&self, event: BannerEvent, asset: Option<Asset>) -> BsvHomeBannerContent {
        self.banners.banner_content(event, asset)
    }

    pub async fn apply_banner_action(&self, key: BsvHomeBannerKey, action: BsvHomeBannerAction) -> Result<(), BsvHomeServiceError> {
        self.banners.apply_action(key, action).await
    }
}

impl BsvHomeWalletHomeService {
    fn total_fiat_value(&self, balances: Vec<AssetFiatValue>, perpetual: Option<PerpetualBalance>) -> TotalFiatValue {
        balance_rules::total_fiat_value(&rules::wallet_balances(balances, perpetual.filter(|_| self.includes_perpetual_collateral())))
    }

    fn includes_perpetual_collateral(&self) -> bool {
        self.session.environment.allows_asset(&HYPERCORE_PERPETUAL_USDC.id)
            && self
                .session
                .get_current_wallet_id()
                .ok()
                .flatten()
                .is_some_and(|wallet_id| self.wallet_preferences.includes_perpetual_collateral(wallet_id))
    }
}
