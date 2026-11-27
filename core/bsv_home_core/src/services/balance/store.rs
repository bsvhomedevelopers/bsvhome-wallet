use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{AssetId, WalletId};

use super::model::{BsvHomeAssetBalance, BsvHomeBalanceRecord};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeBalanceStore: Send + Sync {
    async fn get_available_balances(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>) -> Result<Vec<BsvHomeAssetBalance>, BsvHomeServiceError>;
    async fn update_balances(&self, wallet_id: WalletId, balances: Vec<BsvHomeBalanceRecord>) -> Result<(), BsvHomeServiceError>;
    async fn get_enabled_asset_ids(&self, wallet_id: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError>;
    async fn set_assets_enabled(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>, enabled: bool) -> Result<(), BsvHomeServiceError>;
    async fn set_asset_pinned(&self, wallet_id: WalletId, asset_id: AssetId, pinned: bool) -> Result<(), BsvHomeServiceError>;
}
