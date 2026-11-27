use std::sync::Arc;

use primitives::chart::ChartCandleUpdate;
use primitives::{Asset, AssetId, Chain, ChartPeriod, Currency, Perpetual, PerpetualPosition};

use super::model::{BsvHomePerpetualPositionAction, BsvHomePerpetualPositionKind};
use super::{BsvHomePerpetualService, rules};
use crate::models::perpetual::{BsvHomeChartCandleStick, BsvHomePerpetualSubscription};
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::transactions::BsvHomeTransactionsService;
use crate::services::transfer::BsvHomeTransferData;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomePerpetualDetailsService {
    perpetuals: Arc<BsvHomePerpetualService>,
    transactions: Arc<BsvHomeTransactionsService>,
    preferences: Arc<BsvHomePreferencesService>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomePerpetualDetailsService {
    #[uniffi::constructor]
    pub fn new(
        perpetuals: Arc<BsvHomePerpetualService>,
        transactions: Arc<BsvHomeTransactionsService>,
        preferences: Arc<BsvHomePreferencesService>,
        session: Arc<BsvHomeWalletSessionService>,
    ) -> Self {
        Self {
            perpetuals,
            transactions,
            preferences,
            session,
        }
    }

    pub fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }

    pub fn position_action(
        &self,
        perpetual: Perpetual,
        asset: Asset,
        position: Option<PerpetualPosition>,
        kind: BsvHomePerpetualPositionKind,
    ) -> Result<BsvHomePerpetualPositionAction, BsvHomeServiceError> {
        rules::position_action(&perpetual, &asset, position, kind)
    }

    pub fn close_transfer(&self, perpetual: Perpetual, asset: Asset, position: Option<PerpetualPosition>) -> Result<BsvHomeTransferData, BsvHomeServiceError> {
        rules::close_transfer(&perpetual, &asset, position)
    }

    pub fn chart_period(&self) -> ChartPeriod {
        self.preferences.get_perpetual_chart_period()
    }

    pub fn set_chart_period(&self, period: ChartPeriod) -> Result<(), BsvHomeServiceError> {
        self.preferences.set_perpetual_chart_period(period)
    }

    pub fn candle_subscription(&self, perpetual: Perpetual, period: ChartPeriod) -> BsvHomePerpetualSubscription {
        BsvHomePerpetualSubscription::Candle {
            symbol: rules::symbol(&perpetual),
            interval: rules::candle_interval(&period).to_string(),
        }
    }

    pub fn market_subscription(&self, perpetual: Perpetual) -> BsvHomePerpetualSubscription {
        BsvHomePerpetualSubscription::MarketData {
            symbol: rules::symbol(&perpetual),
        }
    }

    pub async fn candlesticks(&self, perpetual: Perpetual, period: ChartPeriod) -> Result<Vec<BsvHomeChartCandleStick>, BsvHomeServiceError> {
        self.perpetuals.get_candlesticks(Chain::HyperCore, rules::symbol(&perpetual), period).await
    }

    pub fn apply_candle_update(
        &self,
        candles: Vec<BsvHomeChartCandleStick>,
        update: ChartCandleUpdate,
        perpetual: Perpetual,
        period: ChartPeriod,
    ) -> Option<Vec<BsvHomeChartCandleStick>> {
        rules::apply_candle_update(candles, update, &perpetual, &period)
    }

    pub async fn sync_positions(&self) -> Result<(), BsvHomeServiceError> {
        self.perpetuals.sync_current_positions().await
    }

    pub async fn sync_transactions(&self, asset_id: AssetId) -> Result<(), BsvHomeServiceError> {
        self.transactions.sync_wallet(self.session.current_wallet_id()?, Some(asset_id)).await
    }
}
