use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use primitives::{AssetId, Currency, PriceAlert};

use super::{BsvHomePriceAlertService, BsvHomePriceAlertStore};
use crate::api::BsvHomeDeviceApiClient;
use crate::gateway::EmptyPreferences;
use crate::services::banner::BsvHomeNotificationPermissions;
use crate::services::device::{BsvHomeDeviceInfo, BsvHomeDeviceKeyService, BsvHomeDevicePlatform, BsvHomeDeviceService};
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::subscription::BsvHomeSubscriptionService;
use crate::services::wallet::testkit::MemoryWalletStore;
use crate::testkit::TestAlienProvider;

#[derive(Default)]
pub struct MemoryPriceAlertStore {
    pub alerts: Mutex<Vec<PriceAlert>>,
    pub updates: Mutex<Vec<(Vec<PriceAlert>, Vec<String>)>>,
}

#[async_trait]
impl BsvHomePriceAlertStore for MemoryPriceAlertStore {
    async fn get_price_alerts(&self, asset_id: Option<AssetId>) -> Result<Vec<PriceAlert>, BsvHomeServiceError> {
        Ok(self
            .alerts
            .lock()
            .unwrap()
            .iter()
            .filter(|alert| asset_id.as_ref().is_none_or(|asset_id| alert.asset_id == *asset_id))
            .cloned()
            .collect())
    }

    async fn update_price_alerts(&self, alerts: Vec<PriceAlert>, delete_ids: Vec<String>) -> Result<(), BsvHomeServiceError> {
        self.updates.lock().unwrap().push((alerts.clone(), delete_ids.clone()));
        let mut stored = self.alerts.lock().unwrap();
        stored.retain(|alert| !delete_ids.contains(&alert.id()) && alerts.iter().all(|new| new.id() != alert.id()));
        stored.extend(alerts);
        Ok(())
    }
}

struct UnexpectedNotifications;

#[async_trait]
impl BsvHomeNotificationPermissions for UnexpectedNotifications {
    async fn request_permissions_or_open_settings(&self) -> Result<bool, BsvHomeServiceError> {
        panic!("price alert rejection must precede notification permissions");
    }
}

#[async_trait]
impl BsvHomeDevicePlatform for UnexpectedNotifications {
    async fn device_id(&self) -> Result<String, BsvHomeServiceError> {
        panic!("asset alert writes must not synchronize the device");
    }
    async fn device_info(&self) -> Result<BsvHomeDeviceInfo, BsvHomeServiceError> {
        panic!("asset alert writes must not synchronize the device");
    }
    async fn push_token(&self) -> Result<String, BsvHomeServiceError> {
        panic!("asset alert writes must not synchronize the device");
    }
    async fn is_push_enabled(&self) -> Result<bool, BsvHomeServiceError> {
        panic!("asset alert writes must not synchronize the device");
    }
    async fn get_currency(&self) -> Result<Currency, BsvHomeServiceError> {
        panic!("asset alert writes must not synchronize the device");
    }
}

pub(crate) fn service(provider: Arc<TestAlienProvider>, alerts: Vec<PriceAlert>) -> (BsvHomePriceAlertService, Arc<MemoryPriceAlertStore>) {
    let api = Arc::new(BsvHomeDeviceApiClient::new(provider, Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences)))));
    let wallets = Arc::new(MemoryWalletStore::default());
    let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
    let notifications = Arc::new(UnexpectedNotifications);
    let device = Arc::new(BsvHomeDeviceService::new(
        api.clone(),
        Arc::new(BsvHomeSubscriptionService::new(api.clone(), wallets.clone())),
        wallets,
        notifications.clone(),
        preferences.clone(),
    ));
    let store = Arc::new(MemoryPriceAlertStore {
        alerts: Mutex::new(alerts),
        ..Default::default()
    });
    (BsvHomePriceAlertService::new(api, preferences, store.clone(), device, notifications), store)
}
