use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{NFTAssetData, NFTAssetId, NFTData, WalletId};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeNftStore: Send + Sync {
    async fn save_nfts(&self, wallet_id: WalletId, data: Vec<NFTData>) -> Result<(), BsvHomeServiceError>;
    async fn get_asset_data(&self, asset_id: NFTAssetId) -> Result<Option<NFTAssetData>, BsvHomeServiceError>;
    async fn save_asset(&self, data: NFTAssetData) -> Result<(), BsvHomeServiceError>;
}
