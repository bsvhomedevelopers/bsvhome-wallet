use std::sync::Arc;

use crate::services::banner::BsvHomeNotificationPermissions;
use crate::services::device::BsvHomeDeviceService;
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomePreferencesService;

#[derive(uniffi::Object)]
pub struct BsvHomeNotificationsService {
    device: Arc<BsvHomeDeviceService>,
    preferences: Arc<BsvHomePreferencesService>,
    permissions: Arc<dyn BsvHomeNotificationPermissions>,
}

#[uniffi::export]
impl BsvHomeNotificationsService {
    #[uniffi::constructor]
    pub fn new(device: Arc<BsvHomeDeviceService>, preferences: Arc<BsvHomePreferencesService>, permissions: Arc<dyn BsvHomeNotificationPermissions>) -> Self {
        Self { device, preferences, permissions }
    }

    pub fn is_enabled(&self) -> bool {
        self.preferences.is_push_notifications_enabled()
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<bool, BsvHomeServiceError> {
        if !enabled {
            self.device.set_push_enabled(false).await?;
            return Ok(false);
        }
        let granted = self.permissions.request_permissions_or_open_settings().await?;
        if granted {
            self.device.set_push_enabled(true).await?;
        }
        Ok(granted)
    }
}
