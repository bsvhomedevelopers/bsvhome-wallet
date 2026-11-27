use std::sync::Mutex;

use primitives::{InAppNotification, WalletId};

use super::BsvHomeNotificationStore;
use crate::services::error::BsvHomeServiceError;

#[derive(Default)]
pub struct MemoryNotificationStore {
    pub unread: bool,
    pub saved: Mutex<Vec<InAppNotification>>,
}

#[async_trait::async_trait]
impl BsvHomeNotificationStore for MemoryNotificationStore {
    async fn save_notifications(&self, notifications: Vec<InAppNotification>) -> Result<(), BsvHomeServiceError> {
        self.saved.lock().unwrap().extend(notifications);
        Ok(())
    }

    async fn has_unread_notifications(&self, _wallet_id: WalletId) -> Result<bool, BsvHomeServiceError> {
        Ok(self.unread)
    }
}
