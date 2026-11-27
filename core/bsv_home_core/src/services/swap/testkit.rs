use async_trait::async_trait;
use primitives::{AssetId, Chain, WalletId};

use super::{BsvHomeSwapPair, BsvHomeSwapStore};
use crate::services::error::BsvHomeServiceError;

pub struct UnusedSwapStore;

#[async_trait]
impl BsvHomeSwapStore for UnusedSwapStore {
    async fn get_swap_pairs(&self, _wallet_id: WalletId) -> Result<Vec<BsvHomeSwapPair>, BsvHomeServiceError> {
        panic!("selection capabilities must not read swap pairs");
    }

    async fn get_recent_asset_ids(&self, _wallet_id: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        panic!("selection capabilities must not read recent swaps");
    }

    async fn get_pay_asset_ids(&self, _wallet_id: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        panic!("selection capabilities must not read swap pay candidates");
    }

    async fn get_receive_asset_ids(&self, _wallet_id: WalletId, _chains: Vec<Chain>, _asset_ids: Vec<AssetId>) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        panic!("selection capabilities must not read swap receive candidates");
    }
}
