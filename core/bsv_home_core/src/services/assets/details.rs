use futures::TryFutureExt;
use std::sync::Arc;

use crate::services::wallet::addresses::{BsvHomeBsvAddressPool, BsvHomeBsvAddressSnapshot};
use primitives::{Asset, AssetId, AssetMetaData, BannerEvent, Chain, Deeplink, PriceAlert, VerificationStatus, Wallet, WalletId};

use crate::deeplink::BsvHomeDeeplinkService;
use crate::services::balance::{BsvHomeAssetBalance, BsvHomeBalanceService};
use crate::services::banner::{BsvHomeBannerAction, BsvHomeBannerContent, BsvHomeBannerKey, BsvHomeBannerService};
use crate::services::error::BsvHomeServiceError;
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::price_alert::BsvHomePriceAlertService;
use crate::services::stream::BsvHomeStreamSubscriptionService;
use crate::services::swap::{BsvHomeSwapPairSuggestion, BsvHomeSwapService};
use crate::services::transactions::BsvHomeTransactionsService;
use crate::services::wallet_session::BsvHomeWalletSessionService;
use primitives::BlockExplorerLink;

use crate::services::failures::{StepFailure, record};

use super::{BsvHomeAssetDetailsState, BsvHomeAssetNetworkDestination, BsvHomeAssetsService, rules};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAssetRefreshStep {
    AddPrices,
    SyncAsset,
    SyncAssociations,
    UpdateBalances,
    SyncTransactions,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeAssetRefreshFailure {
    pub step: BsvHomeAssetRefreshStep,
    pub message: String,
}

impl StepFailure for BsvHomeAssetRefreshFailure {
    type Step = BsvHomeAssetRefreshStep;

    fn new(step: BsvHomeAssetRefreshStep, message: String) -> Self {
        Self { step, message }
    }
}

#[derive(uniffi::Object)]
pub struct BsvHomeAssetDetailsService {
    assets: Arc<BsvHomeAssetsService>,
    balances: Arc<BsvHomeBalanceService>,
    transactions: Arc<BsvHomeTransactionsService>,
    banners: Arc<BsvHomeBannerService>,
    swap: Arc<BsvHomeSwapService>,
    explorer: Arc<BsvHomeExplorerService>,
    price_alerts: Arc<BsvHomePriceAlertService>,
    stream: Arc<BsvHomeStreamSubscriptionService>,
    deeplinks: Arc<BsvHomeDeeplinkService>,
    session: Arc<BsvHomeWalletSessionService>,
    addresses: Arc<BsvHomeBsvAddressPool>,
}

#[uniffi::export]
impl BsvHomeAssetDetailsService {
    #[uniffi::constructor]
    pub fn new(
        assets: Arc<BsvHomeAssetsService>,
        balances: Arc<BsvHomeBalanceService>,
        transactions: Arc<BsvHomeTransactionsService>,
        banners: Arc<BsvHomeBannerService>,
        swap: Arc<BsvHomeSwapService>,
        explorer: Arc<BsvHomeExplorerService>,
        price_alerts: Arc<BsvHomePriceAlertService>,
        stream: Arc<BsvHomeStreamSubscriptionService>,
        deeplinks: Arc<BsvHomeDeeplinkService>,
        session: Arc<BsvHomeWalletSessionService>,
        addresses: Arc<BsvHomeBsvAddressPool>,
    ) -> Self {
        Self {
            assets,
            balances,
            transactions,
            banners,
            swap,
            explorer,
            price_alerts,
            stream,
            deeplinks,
            session,
            addresses,
        }
    }

    pub async fn prepare_bsv_addresses(&self, wallet_id: WalletId) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.addresses.prepare(wallet_id).await
    }
    pub async fn bsv_addresses(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvAddressSnapshot>, BsvHomeServiceError> {
        self.addresses.snapshot(wallet_id).await
    }
    pub async fn add_bsv_address(&self, wallet_id: WalletId) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.addresses.add_address(wallet_id).await
    }
    pub async fn select_bsv_address(&self, wallet_id: WalletId, index: u32) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.addresses.select_address(wallet_id, index).await
    }
    pub fn bsv_scan_generation(&self, wallet_id: WalletId) -> Result<u64, BsvHomeServiceError> {
        self.addresses.scan_generation(wallet_id)
    }
    pub async fn sync_bsv_addresses(&self, wallet_id: WalletId, generation: u64) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.addresses.sync_at_generation(wallet_id, generation, false).await
    }
    pub async fn continue_bsv_scan(&self, wallet_id: WalletId, generation: u64) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.addresses.sync_at_generation(wallet_id, generation, true).await
    }

    pub fn cancel_bsv_scan(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        self.addresses.cancel_scan(wallet_id)
    }

    pub async fn refresh(&self, asset_id: AssetId) -> Vec<BsvHomeAssetRefreshFailure> {
        let mut failures = Vec::new();
        let wallet_id = match self.session.current_wallet_id() {
            Ok(wallet_id) => wallet_id,
            Err(error) => {
                return vec![
                    BsvHomeAssetRefreshFailure::new(BsvHomeAssetRefreshStep::UpdateBalances, error.to_string()),
                    BsvHomeAssetRefreshFailure::new(BsvHomeAssetRefreshStep::SyncTransactions, error.to_string()),
                ];
            }
        };
        record(&mut failures, BsvHomeAssetRefreshStep::AddPrices, self.stream.add_prices(vec![asset_id.clone()])).await;

        let associations = match self.assets.sync_asset(asset_id.clone()).await {
            Ok(asset) => asset.associations.into_iter().map(|association| association.asset_id).collect(),
            Err(error) => {
                failures.push(BsvHomeAssetRefreshFailure::new(BsvHomeAssetRefreshStep::SyncAsset, error.to_string()));
                Vec::new()
            }
        };
        if !associations.is_empty() {
            record(
                &mut failures,
                BsvHomeAssetRefreshStep::SyncAssociations,
                self.assets.sync_missing_assets(associations).map_ok(|_| ()),
            )
            .await;
        }

        record(
            &mut failures,
            BsvHomeAssetRefreshStep::UpdateBalances,
            self.balances.update(wallet_id.clone(), vec![asset_id.clone()]),
        )
        .await;
        record(
            &mut failures,
            BsvHomeAssetRefreshStep::SyncTransactions,
            self.transactions.sync_wallet(wallet_id, Some(asset_id)),
        )
        .await;
        failures
    }

    pub async fn sync_transactions(&self, asset_id: Option<AssetId>) -> Result<(), BsvHomeServiceError> {
        self.transactions.sync_wallet(self.session.current_wallet_id()?, asset_id).await
    }

    pub async fn update_balances(&self, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        self.balances.update(self.session.current_wallet_id()?, asset_ids).await
    }

    pub async fn set_asset_pinned(&self, asset_id: AssetId, pinned: bool) -> Result<(), BsvHomeServiceError> {
        self.balances.set_asset_pinned(self.session.current_wallet_id()?, asset_id, pinned).await
    }

    pub async fn set_assets_enabled(&self, asset_ids: Vec<AssetId>, enabled: bool) -> Result<(), BsvHomeServiceError> {
        self.balances.set_assets_enabled(self.session.current_wallet_id()?, asset_ids, enabled).await
    }

    pub async fn add_prices(&self, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        self.stream.add_prices(asset_ids).await
    }

    pub fn banner_content(&self, event: BannerEvent, asset: Option<Asset>) -> BsvHomeBannerContent {
        self.banners.banner_content(event, asset)
    }

    pub async fn apply_banner_action(&self, key: BsvHomeBannerKey, action: BsvHomeBannerAction) -> Result<(), BsvHomeServiceError> {
        self.banners.apply_action(key, action).await
    }

    pub fn network_destination(&self, asset_id: AssetId) -> Option<BsvHomeAssetNetworkDestination> {
        rules::network_destination(&asset_id)
    }

    pub fn verification_status(&self, asset: Asset, rank: i32) -> Option<VerificationStatus> {
        rules::verification_status(&asset, rank)
    }

    pub fn state(
        &self,
        wallet: Wallet,
        chain: Chain,
        metadata: AssetMetaData,
        balance: BsvHomeAssetBalance,
        banner_events: Vec<BannerEvent>,
        price: Option<f64>,
        price_alerts: Vec<PriceAlert>,
    ) -> BsvHomeAssetDetailsState {
        let mut state = rules::details_state(wallet.wallet_type, chain, &metadata, &balance, &banner_events, price, price_alerts);
        if let Some(actions) = rules::bsv_wallet_header_actions(&wallet, chain, &banner_events, self.assets.environment()) {
            state.header_actions = actions;
        }
        state
    }

    pub fn swap_pair(&self, asset_id: AssetId, has_balance: bool) -> Result<BsvHomeSwapPairSuggestion, BsvHomeServiceError> {
        self.swap.pair_for_asset(asset_id, has_balance)
    }

    pub fn explorer_name(&self, chain: Chain) -> String {
        self.explorer.get_explorer_name(chain)
    }

    pub fn address_url(&self, chain: Chain, address: String) -> BlockExplorerLink {
        self.explorer.get_address_url(chain, address)
    }

    pub fn token_url(&self, chain: Chain, address: String) -> Option<BlockExplorerLink> {
        self.explorer.get_token_url(chain, address)
    }

    pub async fn set_price_alert(&self, asset_id: AssetId, enabled: bool) -> Result<(), BsvHomeServiceError> {
        self.price_alerts.set_auto_alert(asset_id, enabled).await
    }

    pub async fn sync_price_alerts(&self, asset_id: Option<AssetId>) -> Result<(), BsvHomeServiceError> {
        if asset_id.as_ref().is_some_and(|asset| !asset.chain.supports_bsv_home_backend()) {
            return Ok(());
        }
        self.price_alerts.sync(asset_id).await
    }

    pub fn deeplink_url(&self, deeplink: Deeplink) -> String {
        self.deeplinks.build_url(deeplink)
    }

    pub fn deeplink_bsv_home_url(&self, deeplink: Deeplink) -> String {
        self.deeplinks.build_bsv_home_url(deeplink)
    }
}
