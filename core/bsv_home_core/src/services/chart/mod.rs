pub mod model;
pub mod rules;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use primitives::currency::Currency;
use primitives::{Asset, AssetId, AssetLink, AssetMarket, ChartDateValue, ChartPeriod, PriceAlert};

use crate::api::{BsvHomeApiClient, BsvHomeApiError};
use crate::services::error::BsvHomeServiceError;
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::price::BsvHomePriceService;
use crate::services::price_alert::BsvHomePriceAlertService;

pub use model::{BsvHomeAssetMarketRow, BsvHomeChartData, BsvHomeChartHeader, BsvHomeChartSection, BsvHomeChartValueType};

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeChart {
    pub values: Vec<ChartDateValue>,
    pub base_value: f64,
    pub current: Option<BsvHomeChartCurrent>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeChartCurrent {
    pub date: DateTime<Utc>,
    pub value: f64,
    pub change_percentage: f64,
}

#[uniffi::export]
pub fn price_chart_data(chart: BsvHomeChart) -> Option<BsvHomeChartData> {
    rules::price_chart_data(chart)
}

#[uniffi::export]
pub fn chart_header(value_type: BsvHomeChartValueType, base: f64, value: f64, shows_secondary_value: bool) -> BsvHomeChartHeader {
    rules::header(value_type, base, value, None, shows_secondary_value)
}

#[derive(uniffi::Object)]
pub struct BsvHomeChartService {
    api: Arc<BsvHomeApiClient>,
    price: Arc<BsvHomePriceService>,
    preferences: Arc<BsvHomePreferencesService>,
    price_alerts: Arc<BsvHomePriceAlertService>,
    explorer: Arc<BsvHomeExplorerService>,
}

#[uniffi::export]
impl BsvHomeChartService {
    #[uniffi::constructor]
    pub fn new(
        api: Arc<BsvHomeApiClient>,
        price: Arc<BsvHomePriceService>,
        preferences: Arc<BsvHomePreferencesService>,
        price_alerts: Arc<BsvHomePriceAlertService>,
        explorer: Arc<BsvHomeExplorerService>,
    ) -> Self {
        Self {
            api,
            price,
            preferences,
            price_alerts,
            explorer,
        }
    }

    pub fn sections(&self, asset: Asset, price: Option<f64>, market: Option<AssetMarket>, price_alerts: Vec<PriceAlert>, links: Vec<AssetLink>) -> Vec<BsvHomeChartSection> {
        let contract_explorer = asset.id.token_id.clone().and_then(|token_id| self.explorer.get_token_url(asset.id.chain, token_id));
        rules::chart_sections(&asset, price, market.as_ref(), price_alerts, links, contract_explorer)
    }

    pub fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }

    pub fn chart_period(&self) -> ChartPeriod {
        self.preferences.get_chart_period()
    }

    pub fn set_chart_period(&self, period: ChartPeriod) -> Result<(), BsvHomeServiceError> {
        self.preferences.set_chart_period(period)
    }

    pub async fn sync_charts(&self, asset_id: AssetId, period: ChartPeriod) -> Result<BsvHomeChart, BsvHomeServiceError> {
        if asset_id.chain == primitives::Chain::BitcoinSv {
            // TODO(BSV-API): enable BSV market data after the backend contract is supported.
            return Err(BsvHomeServiceError::Unsupported {
                msg: "BSV_API_UNSUPPORTED: charts".to_string(),
            });
        }
        let currency = self.get_currency();
        let charts = self.api.client.get_charts(asset_id.clone(), period).await.map_err(BsvHomeApiError::from)?;
        if let Some(market) = charts.market {
            self.price.update_market(asset_id.clone(), market).await?;
        }
        let rate = self.price.rate(currency.clone()).await?.ok_or(BsvHomeServiceError::InvalidInput {
            msg: format!("unknown currency: {currency}"),
        })?;
        let latest = self.price.prices(vec![asset_id]).await?.into_iter().next();
        let values = rules::converted_values(charts.prices, rate.rate);
        let base_value = rules::base_value(&values);
        let current = rules::current_value(&values, latest, Utc::now(), period, base_value);
        Ok(BsvHomeChart { values, base_value, current })
    }

    pub async fn sync_price_alerts(&self, asset_id: AssetId) -> Result<(), BsvHomeServiceError> {
        self.price_alerts.sync(Some(asset_id)).await
    }
}
