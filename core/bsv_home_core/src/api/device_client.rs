use std::sync::Arc;

use bsv_home_api::BsvHomeDeviceApiClient as DeviceApiClient;

use crate::services::device::{BsvHomeDeviceKeyService, BsvHomeDeviceService, DeviceSyncPreflight};

use crate::alien::{AlienError, AlienProvider, AlienProviderWrapper};
use crate::config::public::API_URL;

#[derive(Debug, uniffi::Object)]
pub struct BsvHomeDeviceApiClient {
    pub(crate) client: DeviceApiClient<AlienError>,
}

#[uniffi::export]
impl BsvHomeDeviceApiClient {
    #[uniffi::constructor]
    pub fn new(provider: Arc<dyn AlienProvider>, device_key: Arc<BsvHomeDeviceKeyService>) -> Self {
        Self {
            client: DeviceApiClient::new(API_URL.to_string(), Arc::new(AlienProviderWrapper::new(provider)), device_key),
        }
    }
}

#[uniffi::export]
impl BsvHomeDeviceApiClient {
    pub fn set_device_sync_preflight(&self, device: Arc<BsvHomeDeviceService>) {
        self.client.set_preflight(Arc::new(DeviceSyncPreflight { service: Arc::downgrade(&device) }));
    }
}
