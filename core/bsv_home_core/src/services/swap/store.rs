use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{AssetId, Chain, WalletId};

use super::model::BsvHomeSwapPair;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeSwapStore: Send + Sync {
    async fn get_swap_pairs(&self, wallet_id: WalletId) -> Result<Vec<BsvHomeSwapPair>, BsvHomeServiceError>;
    async fn get_recent_asset_ids(&self, wallet_id: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError>;
    async fn get_pay_asset_ids(&self, wallet_id: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError>;
    async fn get_receive_asset_ids(&self, wallet_id: WalletId, chains: Vec<Chain>, asset_ids: Vec<AssetId>) -> Result<Vec<AssetId>, BsvHomeServiceError>;
}
