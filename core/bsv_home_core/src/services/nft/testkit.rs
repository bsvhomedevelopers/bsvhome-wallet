use std::sync::Mutex;

use primitives::{NFTAssetData, NFTAssetId, NFTData, WalletId};

use super::BsvHomeNftStore;
use crate::services::error::BsvHomeServiceError;

#[derive(Default)]
pub struct MemoryNftStore {
    pub cached: Option<NFTAssetData>,
    pub added: Mutex<Vec<NFTAssetData>>,
}

#[async_trait::async_trait]
impl BsvHomeNftStore for MemoryNftStore {
    async fn save_nfts(&self, _wallet_id: WalletId, _data: Vec<NFTData>) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }
    async fn get_asset_data(&self, _asset_id: NFTAssetId) -> Result<Option<NFTAssetData>, BsvHomeServiceError> {
        Ok(self.cached.clone())
    }
    async fn save_asset(&self, data: NFTAssetData) -> Result<(), BsvHomeServiceError> {
        self.added.lock().unwrap().push(data);
        Ok(())
    }
}
