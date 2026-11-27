use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::BannerState;

use super::model::BsvHomeBannerKey;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeBannerStore: Send + Sync {
    async fn get_state(&self, key: BsvHomeBannerKey) -> Result<Option<BannerState>, BsvHomeServiceError>;
    async fn set_state(&self, key: BsvHomeBannerKey, state: BannerState) -> Result<(), BsvHomeServiceError>;
    async fn add_banners(&self, keys: Vec<BsvHomeBannerKey>, state: BannerState) -> Result<(), BsvHomeServiceError>;
}
