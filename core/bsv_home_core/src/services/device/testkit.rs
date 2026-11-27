use async_trait::async_trait;
use primitives::{Currency, Device, Platform, PlatformStore};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{BsvHomeDeviceInfo, BsvHomeDevicePlatform};
use crate::services::error::BsvHomeServiceError;

pub(crate) struct MemoryDevicePlatform;

#[derive(Default)]
pub(crate) struct UnavailableDevicePlatform {
    pub calls: AtomicUsize,
}

impl UnavailableDevicePlatform {
    fn unavailable(&self) -> BsvHomeServiceError {
        self.calls.fetch_add(1, Ordering::SeqCst);
        BsvHomeServiceError::Platform {
            msg: "device platform unavailable".into(),
        }
    }
}

#[async_trait]
impl BsvHomeDevicePlatform for UnavailableDevicePlatform {
    async fn device_id(&self) -> Result<String, BsvHomeServiceError> {
        Err(self.unavailable())
    }
    async fn device_info(&self) -> Result<BsvHomeDeviceInfo, BsvHomeServiceError> {
        Err(self.unavailable())
    }
    async fn push_token(&self) -> Result<String, BsvHomeServiceError> {
        Err(self.unavailable())
    }
    async fn is_push_enabled(&self) -> Result<bool, BsvHomeServiceError> {
        Err(self.unavailable())
    }
    async fn get_currency(&self) -> Result<Currency, BsvHomeServiceError> {
        Err(self.unavailable())
    }
}

#[async_trait]
impl BsvHomeDevicePlatform for MemoryDevicePlatform {
    async fn device_id(&self) -> Result<String, BsvHomeServiceError> {
        Ok(Device::mock().id)
    }

    async fn device_info(&self) -> Result<BsvHomeDeviceInfo, BsvHomeServiceError> {
        Ok(BsvHomeDeviceInfo {
            platform: Platform::IOS,
            platform_store: PlatformStore::AppStore,
            os: "iOS".into(),
            model: "iPhone".into(),
            version: "1.0.0".into(),
            locale_identifier: "en".into(),
        })
    }

    async fn push_token(&self) -> Result<String, BsvHomeServiceError> {
        Ok(Device::mock().token)
    }

    async fn is_push_enabled(&self) -> Result<bool, BsvHomeServiceError> {
        Ok(true)
    }

    async fn get_currency(&self) -> Result<Currency, BsvHomeServiceError> {
        Ok(Currency::USD)
    }
}
