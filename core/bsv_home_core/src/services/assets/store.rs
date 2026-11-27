use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{Asset, AssetBasic, AssetFull, AssetId, WalletId};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeAssetStore: Send + Sync {
    async fn get_asset_ids(&self, asset_ids: Vec<AssetId>) -> Result<Vec<AssetId>, BsvHomeServiceError>;
    async fn get_assets(&self, asset_ids: Vec<AssetId>) -> Result<Vec<Asset>, BsvHomeServiceError>;
    async fn save_assets(&self, assets: Vec<AssetBasic>) -> Result<(), BsvHomeServiceError>;
    async fn save_asset(&self, asset: AssetFull) -> Result<(), BsvHomeServiceError>;
    async fn add_missing_balances(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError>;
    async fn add_balances(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>, enabled: bool) -> Result<(), BsvHomeServiceError>;
    async fn set_buyable_assets(&self, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError>;
    async fn set_sellable_assets(&self, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError>;
    async fn set_swappable_assets(&self, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError>;
    async fn set_stakeable_assets(&self, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError>;
}
