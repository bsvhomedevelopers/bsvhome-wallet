use primitives::currency::Currency;
use primitives::{Platform, PlatformStore};

use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeDeviceInfo {
    pub platform: Platform,
    pub platform_store: PlatformStore,
    pub os: String,
    pub model: String,
    pub version: String,
    pub locale_identifier: String,
}

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeDevicePlatform: Send + Sync {
    async fn device_id(&self) -> Result<String, BsvHomeServiceError>;
    async fn device_info(&self) -> Result<BsvHomeDeviceInfo, BsvHomeServiceError>;
    async fn push_token(&self) -> Result<String, BsvHomeServiceError>;
    async fn is_push_enabled(&self) -> Result<bool, BsvHomeServiceError>;
    async fn get_currency(&self) -> Result<Currency, BsvHomeServiceError>;
}
