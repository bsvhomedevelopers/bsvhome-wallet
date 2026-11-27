pub mod autoclose;
pub mod details;
pub mod model;
pub mod rules;
pub mod store;
pub mod stream;
#[cfg(test)]
pub(crate) mod testkit;

use crate::services::error::BsvHomeServiceError;
use crate::services::failures::record;
use model::{BsvHomePerpetualConnection, BsvHomePerpetualRefreshFailure, BsvHomePerpetualRefreshStep};
use std::sync::Arc;

use bsv_home_hypercore::models::websocket::HyperliquidSocketMessage;
use bsv_home_hypercore::provider::websocket_mapper::{diff_clearinghouse_positions, diff_open_orders_positions, parse_websocket_data};
use chrono::Utc;
use primitives::perpetual::PerpetualBalance;
use primitives::portfolio::PerpetualPortfolio;
use primitives::{AssetId, Chain, ChartPeriod, PerpetualAccountMode, PerpetualModifyConfirmData, PerpetualProvider, Wallet, WalletId};
use std::collections::HashMap;

use crate::config::perpetual_config::PRICES_UPDATE_INTERVAL_SECONDS;
use crate::services::preferences::BsvHomePreferencesService;

pub use autoclose::{BsvHomeAutocloseField, BsvHomeAutocloseModify};
pub use details::BsvHomePerpetualDetailsService;
pub use model::{
    BsvHomeAutocloseSummary, BsvHomeMarketsRefreshTrigger, BsvHomePerpetualPositionAction, BsvHomePerpetualPositionKind, BsvHomePerpetualSocketUpdate, BsvHomePerpetualTransferData,
};
pub use store::BsvHomePerpetualStore;

use crate::gateway::BsvHomeGateway;
use crate::models::perpetual::BsvHomeChartCandleStick;
use crate::services::assets::BsvHomeAssetStore;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::price::BsvHomePriceService;
use crate::services::stream::rules::hyperliquid_account;
use crate::services::wallet_preferences::BsvHomeWalletPreferencesService;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomePerpetualService {
    gateway: Arc<BsvHomeGateway>,
    price: Arc<BsvHomePriceService>,
    store: Arc<dyn BsvHomePerpetualStore>,
    asset_store: Arc<dyn BsvHomeAssetStore>,
    preferences: Arc<BsvHomePreferencesService>,
    balance: Arc<BsvHomeBalanceService>,
    wallet_preferences: Arc<BsvHomeWalletPreferencesService>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomePerpetualService {
    #[uniffi::constructor]
    pub fn new(
        gateway: Arc<BsvHomeGateway>,
        price: Arc<BsvHomePriceService>,
        store: Arc<dyn BsvHomePerpetualStore>,
        asset_store: Arc<dyn BsvHomeAssetStore>,
        preferences: Arc<BsvHomePreferencesService>,
        balance: Arc<BsvHomeBalanceService>,
        wallet_preferences: Arc<BsvHomeWalletPreferencesService>,
        session: Arc<BsvHomeWalletSessionService>,
    ) -> Self {
        Self {
            gateway,
            price,
            store,
            asset_store,
            preferences,
            balance,
            wallet_preferences,
            session,
        }
    }

    pub fn autoclose_summary(&self, data: PerpetualModifyConfirmData) -> Option<BsvHomeAutocloseSummary> {
        rules::autoclose_summary(&data)
    }

    pub async fn refresh(&self, trigger: BsvHomeMarketsRefreshTrigger) -> Vec<BsvHomePerpetualRefreshFailure> {
        let (positions, markets) = futures::join!(self.sync_current_positions(), async {
            self.sync_markets_if_needed(Chain::HyperCore, trigger).await.map(|_| ())
        });
        let mut failures = Vec::new();
        record(&mut failures, BsvHomePerpetualRefreshStep::Positions, async { positions }).await;
        record(&mut failures, BsvHomePerpetualRefreshStep::Markets, async { markets }).await;
        failures
    }

    pub async fn sync_enablement(&self, wallet: Option<Wallet>, trigger: BsvHomeMarketsRefreshTrigger) -> Result<bool, BsvHomeServiceError> {
        if !self.preferences.is_perpetual_enabled() {
            self.clear_markets().await?;
            return Ok(false);
        }
        self.sync_markets_if_needed(Chain::HyperCore, trigger).await?;
        Ok(self.should_connect_perpetuals(wallet))
    }

    pub fn should_connect_perpetuals(&self, wallet: Option<Wallet>) -> bool {
        wallet.is_some_and(|wallet| rules::show_perpetuals(self.preferences.is_perpetual_enabled(), wallet.wallet_type, &wallet.chains()))
    }

    pub async fn set_pinned(&self, perpetual_id: String, pinned: bool) -> Result<(), BsvHomeServiceError> {
        self.store.set_pinned(vec![perpetual_id], pinned).await
    }

    pub async fn connection(&self, wallet: Wallet) -> Result<Option<BsvHomePerpetualConnection>, BsvHomeServiceError> {
        let Some(account) = hyperliquid_account(&wallet.accounts) else {
            return Ok(None);
        };
        let chain = account.chain;
        let address = account.address.clone();
        let mode = match self.sync_positions(wallet.id.clone(), chain, address.clone()).await {
            Ok(mode) => mode,
            Err(_) => self.account_mode(wallet.id, chain, address.clone()).await?,
        };
        Ok(Some(BsvHomePerpetualConnection { address, mode }))
    }
}

impl BsvHomePerpetualService {
    pub async fn sync_markets_if_needed(&self, chain: Chain, trigger: BsvHomeMarketsRefreshTrigger) -> Result<bool, BsvHomeServiceError> {
        if !trigger.should_sync_markets(self.markets_updated_at()?, Utc::now().timestamp()) {
            return Ok(false);
        }
        self.sync_markets(chain).await?;
        Ok(true)
    }

    pub async fn sync_current_positions(&self) -> Result<(), BsvHomeServiceError> {
        let wallet = self.session.current_wallet().await?;
        let Some(account) = hyperliquid_account(&wallet.accounts) else {
            return Ok(());
        };
        self.sync_positions(wallet.id, account.chain, account.address.clone()).await.map(|_| ())
    }

    pub async fn sync_markets(&self, chain: Chain) -> Result<(), BsvHomeServiceError> {
        let data = self.gateway.get_perpetuals_data(chain).await?;
        self.asset_store.save_assets(rules::perpetual_asset_basics(&data)).await?;
        self.store.save_perpetuals(data).await?;
        if let Some(price) = rules::collateral_price(chain) {
            self.price.update_prices(vec![price]).await?;
        }
        self.preferences.set_perpetual_markets_updated_at(Some(Utc::now().timestamp()))
    }

    pub async fn get_portfolio(&self, chain: Chain, address: String) -> Result<PerpetualPortfolio, BsvHomeServiceError> {
        Ok(self.gateway.get_perpetual_portfolio(chain, address).await?)
    }

    pub async fn apply_socket_message(&self, wallet_id: WalletId, mode: PerpetualAccountMode, data: Vec<u8>) -> Result<BsvHomePerpetualSocketUpdate, BsvHomeServiceError> {
        let message = parse_websocket_data(&data, mode).map_err(|error| BsvHomeServiceError::Core { msg: error.to_string() })?;
        match message {
            HyperliquidSocketMessage::AccountState { balance, positions } => {
                let existing = self.store.get_positions(wallet_id.clone(), PerpetualProvider::Hypercore).await?;
                let diff = diff_clearinghouse_positions(positions, existing);
                self.store.update_positions(wallet_id.clone(), diff.positions, diff.delete_position_ids).await?;
                if let Some(balance) = balance {
                    self.update_balance(wallet_id, balance).await?;
                }
                Ok(BsvHomePerpetualSocketUpdate::Applied)
            }
            HyperliquidSocketMessage::SpotState { balance } => {
                self.update_balance(wallet_id, balance).await?;
                Ok(BsvHomePerpetualSocketUpdate::Applied)
            }
            HyperliquidSocketMessage::OpenOrders { orders } => {
                let existing = self.store.get_positions(wallet_id.clone(), PerpetualProvider::Hypercore).await?;
                let diff = diff_open_orders_positions(&orders, existing);
                self.store.update_positions(wallet_id, diff.positions, diff.delete_position_ids).await?;
                Ok(BsvHomePerpetualSocketUpdate::Applied)
            }
            HyperliquidSocketMessage::Candle { candle } => Ok(BsvHomePerpetualSocketUpdate::Candle { candle }),
            HyperliquidSocketMessage::MarketData { market } => {
                self.store.update_market(market).await?;
                Ok(BsvHomePerpetualSocketUpdate::Applied)
            }
            HyperliquidSocketMessage::MarketPrices { prices } => {
                self.update_prices(prices).await?;
                Ok(BsvHomePerpetualSocketUpdate::Applied)
            }
            HyperliquidSocketMessage::SubscriptionResponse { subscription_type } => Ok(BsvHomePerpetualSocketUpdate::SubscriptionResponse { subscription_type }),
            HyperliquidSocketMessage::Error { message } => Ok(BsvHomePerpetualSocketUpdate::Error { message }),
            HyperliquidSocketMessage::Unknown => Ok(BsvHomePerpetualSocketUpdate::Unknown),
        }
    }
}

impl BsvHomePerpetualService {
    pub async fn sync_positions(&self, wallet_id: WalletId, chain: Chain, address: String) -> Result<PerpetualAccountMode, BsvHomeServiceError> {
        let (mode, summary) = futures::join!(self.account_mode(wallet_id.clone(), chain, address.clone()), self.gateway.get_positions(chain, address));
        let (mode, summary) = (mode?, summary?);
        let existing_ids = self.store.get_position_ids(wallet_id.clone(), provider(chain)?).await?;
        let delete_ids = rules::stale_position_ids(existing_ids, &summary.positions);
        self.store.update_positions(wallet_id.clone(), summary.positions, delete_ids).await?;
        self.update_balance(wallet_id, summary.balance).await?;
        Ok(mode)
    }

    pub async fn account_mode(&self, wallet_id: WalletId, chain: Chain, address: String) -> Result<PerpetualAccountMode, BsvHomeServiceError> {
        match self.gateway.get_perpetual_account_mode(chain, address).await {
            Ok(mode) => {
                self.wallet_preferences.set_perpetual_account_mode(wallet_id, mode)?;
                Ok(mode)
            }
            Err(_) => self.wallet_preferences.get_perpetual_account_mode(wallet_id),
        }
    }
}

impl BsvHomePerpetualService {
    pub fn markets_updated_at(&self) -> Result<Option<i64>, BsvHomeServiceError> {
        self.preferences.get_perpetual_markets_updated_at()
    }

    pub async fn get_candlesticks(&self, chain: Chain, symbol: String, period: ChartPeriod) -> Result<Vec<BsvHomeChartCandleStick>, BsvHomeServiceError> {
        Ok(self.gateway.get_perpetual_candlesticks(chain, symbol, period.as_ref().to_string()).await?)
    }

    pub async fn clear_markets(&self) -> Result<(), BsvHomeServiceError> {
        self.store.delete_perpetuals().await?;
        self.preferences.set_perpetual_markets_updated_at(None)
    }

    pub fn collateral_asset_id(&self, chain: Chain) -> Option<AssetId> {
        rules::collateral_asset_id(chain)
    }
}

impl BsvHomePerpetualService {
    pub async fn update_balance(&self, wallet_id: WalletId, balance: PerpetualBalance) -> Result<(), BsvHomeServiceError> {
        let update = rules::balance_update(&balance).map_err(|error| BsvHomeServiceError::Core { msg: error.to_string() })?;
        self.balance.update_balances(wallet_id, vec![update]).await
    }

    pub async fn update_prices(&self, prices: HashMap<String, f64>) -> Result<(), BsvHomeServiceError> {
        let now = Utc::now().timestamp();
        if !rules::prices_outdated(self.preferences.get_perpetual_prices_updated_at()?, now, PRICES_UPDATE_INTERVAL_SECONDS) {
            return Ok(());
        }
        self.store.update_prices(prices).await?;
        self.preferences.set_perpetual_prices_updated_at(Some(now))
    }
}

fn provider(chain: Chain) -> Result<PerpetualProvider, BsvHomeServiceError> {
    rules::provider(chain).ok_or_else(|| BsvHomeServiceError::Unsupported {
        msg: format!("perpetuals unsupported on {chain}"),
    })
}
