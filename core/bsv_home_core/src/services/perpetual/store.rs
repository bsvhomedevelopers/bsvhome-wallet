use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::perpetual::PerpetualData;
use primitives::{PerpetualMarketData, PerpetualPosition, PerpetualProvider, WalletId};
use std::collections::HashMap;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomePerpetualStore: Send + Sync {
    async fn save_perpetuals(&self, data: Vec<PerpetualData>) -> Result<(), BsvHomeServiceError>;
    async fn set_pinned(&self, perpetual_ids: Vec<String>, pinned: bool) -> Result<(), BsvHomeServiceError>;
    async fn delete_perpetuals(&self) -> Result<(), BsvHomeServiceError>;
    async fn get_positions(&self, wallet_id: WalletId, provider: PerpetualProvider) -> Result<Vec<PerpetualPosition>, BsvHomeServiceError>;
    async fn get_position_ids(&self, wallet_id: WalletId, provider: PerpetualProvider) -> Result<Vec<String>, BsvHomeServiceError>;
    async fn update_positions(&self, wallet_id: WalletId, positions: Vec<PerpetualPosition>, delete_ids: Vec<String>) -> Result<(), BsvHomeServiceError>;
    async fn update_market(&self, market: PerpetualMarketData) -> Result<(), BsvHomeServiceError>;
    async fn update_prices(&self, prices: HashMap<String, f64>) -> Result<(), BsvHomeServiceError>;
}
