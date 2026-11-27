use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::currency::Currency;
use primitives::{AssetId, AssetMarket, AssetPrice, FiatRate};

use super::model::BsvHomePriceUpdate;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomePriceStore: Send + Sync {
    async fn get_prices(&self, asset_ids: Vec<AssetId>) -> Result<Vec<AssetPrice>, BsvHomeServiceError>;
    async fn get_rate(&self, currency: Currency) -> Result<Option<FiatRate>, BsvHomeServiceError>;
    async fn get_rates(&self) -> Result<Vec<FiatRate>, BsvHomeServiceError>;
    async fn save_rates(&self, rates: Vec<FiatRate>) -> Result<(), BsvHomeServiceError>;
    async fn save_prices(&self, currency: Currency, prices: Vec<BsvHomePriceUpdate>) -> Result<(), BsvHomeServiceError>;
    async fn convert_prices(&self, currency: Currency, rate: f64) -> Result<(), BsvHomeServiceError>;
    async fn save_market(&self, asset_id: AssetId, market: AssetMarket) -> Result<(), BsvHomeServiceError>;
}
