use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{AssetId, PriceAlert};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomePriceAlertStore: Send + Sync {
    async fn get_price_alerts(&self, asset_id: Option<AssetId>) -> Result<Vec<PriceAlert>, BsvHomeServiceError>;
    async fn update_price_alerts(&self, alerts: Vec<PriceAlert>, delete_ids: Vec<String>) -> Result<(), BsvHomeServiceError>;
}
