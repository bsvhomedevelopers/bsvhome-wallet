pub mod store;
#[cfg(test)]
pub(crate) mod testkit;

use crate::services::error::BsvHomeServiceError;
use primitives::WalletId;
use primitives::unix_seconds;
use std::sync::Arc;

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};

pub use store::BsvHomeNotificationStore;

use crate::services::wallet_preferences::BsvHomeWalletPreferencesService;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomeNotificationService {
    api: Arc<BsvHomeDeviceApiClient>,
    store: Arc<dyn BsvHomeNotificationStore>,
    preferences: Arc<BsvHomeWalletPreferencesService>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomeNotificationService {
    #[uniffi::constructor]
    pub fn new(
        api: Arc<BsvHomeDeviceApiClient>,
        store: Arc<dyn BsvHomeNotificationStore>,
        preferences: Arc<BsvHomeWalletPreferencesService>,
        session: Arc<BsvHomeWalletSessionService>,
    ) -> Self {
        Self { api, store, preferences, session }
    }

    pub async fn open(&self) -> Result<(), BsvHomeServiceError> {
        let wallet_id = self.session.current_wallet_id()?;
        self.sync(wallet_id.clone()).await?;
        if self.store.has_unread_notifications(wallet_id).await? {
            self.api.client.mark_notifications_read().await.map_err(BsvHomeApiError::from)?;
        }
        Ok(())
    }
}

impl BsvHomeNotificationService {
    async fn sync(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        let started_at = unix_seconds().map_err(|error| BsvHomeServiceError::Core { msg: error.to_string() })?;
        let from_timestamp = self.preferences.get_notifications_timestamp(wallet_id.clone());
        let notifications = self.api.client.get_notifications(from_timestamp).await.map_err(BsvHomeApiError::from)?;
        self.store.save_notifications(notifications).await?;
        self.preferences.set_notifications_timestamp(wallet_id, started_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::EmptyPreferences;
    use crate::services::device::BsvHomeDeviceKeyService;
    use crate::services::wallet::testkit::MemoryWalletStore;
    use crate::services::wallet_preferences::testkit::MemoryWalletPreferencesStore;
    use crate::services::wallet_session::testkit::MemoryWalletSessionStore;
    use crate::testkit::TestAlienProvider;
    use testkit::MemoryNotificationStore;

    async fn open(unread: bool) -> Vec<String> {
        let provider = Arc::new(TestAlienProvider::with_json(200, "[]"));
        let api = Arc::new(BsvHomeDeviceApiClient::new(
            provider.clone(),
            Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
        ));
        let store = Arc::new(MemoryNotificationStore { unread, ..Default::default() });
        let preferences = Arc::new(BsvHomeWalletPreferencesService::new(Arc::new(MemoryWalletPreferencesStore::default())));
        let session = Arc::new(BsvHomeWalletSessionService::new(
            Arc::new(MemoryWalletSessionStore::default()),
            Arc::new(MemoryWalletStore::default()),
        ));
        session.set_current_wallet_id(Some(WalletId::Multicoin("wallet".to_string()))).unwrap();
        BsvHomeNotificationService::new(api, store, preferences, session).open().await.unwrap();
        provider.requested_paths()
    }

    #[test]
    fn test_open_marks_read_only_when_unread() {
        futures::executor::block_on(async {
            assert_eq!(open(false).await, vec!["/v2/devices/notifications?from_timestamp=0"]);
            assert_eq!(open(true).await, vec!["/v2/devices/notifications?from_timestamp=0", "/v2/devices/notifications/read"]);
        });
    }
}
