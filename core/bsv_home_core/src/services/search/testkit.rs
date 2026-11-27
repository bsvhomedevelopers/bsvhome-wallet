use async_trait::async_trait;
use primitives::{AssetId, AssetList};

use super::BsvHomeSearchStore;
use crate::services::error::BsvHomeServiceError;

pub struct UnusedSearchStore;

#[async_trait]
impl BsvHomeSearchStore for UnusedSearchStore {
    async fn set_assets(&self, _key: String, _asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        panic!("selection capabilities must not write search assets");
    }

    async fn set_perpetuals(&self, _key: String, _perpetual_ids: Vec<String>) -> Result<(), BsvHomeServiceError> {
        panic!("selection capabilities must not write search perpetuals");
    }

    async fn set_lists(&self, _key: String, _lists: Vec<AssetList>) -> Result<(), BsvHomeServiceError> {
        panic!("selection capabilities must not write search lists");
    }
}
