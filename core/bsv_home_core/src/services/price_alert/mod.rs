pub mod rules;
pub mod store;
#[cfg(test)]
pub(crate) mod testkit;

use crate::services::error::BsvHomeServiceError;
use std::sync::Arc;

use primitives::{AssetId, Currency, PriceAlert};

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::services::banner::BsvHomeNotificationPermissions;
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::device::BsvHomeDeviceService;
use crate::services::preferences::BsvHomePreferencesService;

pub use store::BsvHomePriceAlertStore;

#[derive(uniffi::Object)]
pub struct BsvHomePriceAlertService {
    api: Arc<BsvHomeDeviceApiClient>,
    preferences: Arc<BsvHomePreferencesService>,
    store: Arc<dyn BsvHomePriceAlertStore>,
    device: Arc<BsvHomeDeviceService>,
    permissions: Arc<dyn BsvHomeNotificationPermissions>,
    environment: BsvHomeRuntimeEnvironment,
}

#[uniffi::export]
impl BsvHomePriceAlertService {
    #[uniffi::constructor]
    pub fn new(
        api: Arc<BsvHomeDeviceApiClient>,
        preferences: Arc<BsvHomePreferencesService>,
        store: Arc<dyn BsvHomePriceAlertStore>,
        device: Arc<BsvHomeDeviceService>,
        permissions: Arc<dyn BsvHomeNotificationPermissions>,
    ) -> Self {
        Self::new_with_environment(api, preferences, store, device, permissions, BsvHomeRuntimeEnvironment::Production)
    }

    #[uniffi::constructor]
    pub fn new_with_environment(
        api: Arc<BsvHomeDeviceApiClient>,
        preferences: Arc<BsvHomePreferencesService>,
        store: Arc<dyn BsvHomePriceAlertStore>,
        device: Arc<BsvHomeDeviceService>,
        permissions: Arc<dyn BsvHomeNotificationPermissions>,
        environment: BsvHomeRuntimeEnvironment,
    ) -> Self {
        Self {
            api,
            preferences,
            store,
            device,
            permissions,
            environment,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.preferences.is_price_alerts_enabled()
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<(), BsvHomeServiceError> {
        if self.is_enabled() == enabled {
            return Ok(());
        }
        if enabled {
            if !self.permissions.request_permissions_or_open_settings().await? {
                return Ok(());
            }
            self.preferences.set_push_notifications_enabled(true)?;
        }
        self.preferences.set_price_alerts_enabled(enabled)?;
        self.device.synchronize().await.map(|_| ())
    }

    pub fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }

    pub async fn enable_price_alert(&self, alert: PriceAlert) -> Result<(), BsvHomeServiceError> {
        self.add_price_alerts(vec![alert]).await?;
        self.set_enabled(true).await
    }

    pub async fn set_auto_alert(&self, asset_id: AssetId, enabled: bool) -> Result<(), BsvHomeServiceError> {
        let alert = PriceAlert::new_auto(asset_id, self.get_currency());
        match enabled {
            true => self.enable_price_alert(alert).await,
            false => self.delete_price_alerts(vec![alert]).await,
        }
    }

    pub async fn sync(&self, asset_id: Option<AssetId>) -> Result<(), BsvHomeServiceError> {
        if let Some(asset_id) = &asset_id {
            self.require_asset(asset_id)?;
        }
        let remote = self
            .api
            .client
            .get_price_alerts(asset_id.as_ref().map(ToString::to_string))
            .await
            .map_err(BsvHomeApiError::from)?;
        let remote = match &asset_id {
            Some(asset_id) => remote.into_iter().filter(|alert| alert.asset_id == *asset_id).collect(),
            None => remote,
        };
        let remote = remote.into_iter().filter(|alert| self.allows_asset(&alert.asset_id)).collect();
        let local = self
            .store
            .get_price_alerts(asset_id)
            .await?
            .into_iter()
            .filter(|alert| self.allows_asset(&alert.asset_id))
            .collect();
        let changes = rules::reconcile(local, remote);
        if changes.delete_ids.is_empty() && changes.alerts.is_empty() {
            return Ok(());
        }
        self.store.update_price_alerts(changes.alerts, changes.delete_ids).await
    }

    pub async fn delete_price_alerts(&self, alerts: Vec<PriceAlert>) -> Result<(), BsvHomeServiceError> {
        for alert in &alerts {
            self.require_asset(&alert.asset_id)?;
        }
        self.store.update_price_alerts(Vec::new(), alerts.iter().map(|alert| alert.id()).collect()).await?;
        self.api.client.delete_price_alerts(alerts).await.map_err(BsvHomeApiError::from)?;
        Ok(())
    }
}

impl BsvHomePriceAlertService {
    pub async fn add_price_alerts(&self, alerts: Vec<PriceAlert>) -> Result<(), BsvHomeServiceError> {
        for alert in &alerts {
            self.require_asset(&alert.asset_id)?;
        }
        self.store.update_price_alerts(alerts.clone(), Vec::new()).await?;
        self.api.client.add_price_alerts(alerts).await.map_err(BsvHomeApiError::from)?;
        Ok(())
    }

    fn allows_asset(&self, asset_id: &AssetId) -> bool {
        self.environment.allows_asset(asset_id) && asset_id.chain.supports_bsv_home_backend()
    }

    fn require_asset(&self, asset_id: &AssetId) -> Result<(), BsvHomeServiceError> {
        self.environment.require_asset(asset_id)?;
        if !asset_id.chain.supports_bsv_home_backend() {
            return Err(BsvHomeServiceError::Unsupported {
                msg: "price alerts are unavailable for this network".into(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::executor::block_on;
    use primitives::currency::Currency;
    use primitives::{AssetId, Chain, PriceAlert, known_assets::TRON_SHASTA_USDT};

    use super::{BsvHomePriceAlertStore, rules::reconcile, testkit::service};
    use crate::services::error::BsvHomeServiceError;
    use crate::testkit::TestAlienProvider;

    fn alert(chain: Chain, price: Option<f64>) -> PriceAlert {
        PriceAlert {
            asset_id: AssetId::from_chain(chain),
            currency: Currency::USD,
            price,
            price_percent_change: None,
            price_direction: None,
            last_notified_at: None,
            identifier: String::new(),
        }
    }

    #[test]
    fn test_reconcile() {
        let local = vec![alert(Chain::Bitcoin, None), alert(Chain::Ethereum, None)];
        let remote = vec![alert(Chain::Bitcoin, None), alert(Chain::Solana, Some(1.0))];
        let changes = reconcile(local.clone(), remote.clone());
        assert_eq!(changes.delete_ids, vec![local[1].id()]);
        assert_eq!(changes.alerts.iter().map(PriceAlert::id).collect::<Vec<_>>(), vec![remote[1].id()]);

        let unchanged = reconcile(local.clone(), local.clone());
        assert!(unchanged.delete_ids.is_empty() && unchanged.alerts.is_empty());

        let changes = reconcile(Vec::new(), Vec::new());
        assert!(changes.delete_ids.is_empty() && changes.alerts.is_empty());
    }

    #[test]
    fn test_unsupported_backend_price_alert_mutations_and_sync_are_rejected_before_store_permissions_or_backend_access() {
        block_on(async {
            let mainnet = alert(Chain::Tron, Some(1.0));
            let retained = vec![mainnet.clone(), PriceAlert::new_auto(Chain::TronShasta.as_asset_id(), Currency::USD)];
            let provider = Arc::new(TestAlienProvider::with_status(503));
            let (service, store) = service(provider.clone(), retained.clone());
            for asset_id in [Chain::TronShasta.as_asset_id(), TRON_SHASTA_USDT.id.clone(), Chain::BitcoinSv.as_asset_id()] {
                let development = PriceAlert::new_auto(asset_id.clone(), Currency::USD);
                let errors = [
                    service.enable_price_alert(development.clone()).await.unwrap_err(),
                    service.set_auto_alert(asset_id.clone(), true).await.unwrap_err(),
                    service.set_auto_alert(asset_id.clone(), false).await.unwrap_err(),
                    service.add_price_alerts(vec![mainnet.clone(), development.clone()]).await.unwrap_err(),
                    service.delete_price_alerts(vec![mainnet.clone(), development]).await.unwrap_err(),
                    service.sync(Some(asset_id)).await.unwrap_err(),
                ];
                assert_eq!(
                    errors.iter().map(|error| matches!(error, BsvHomeServiceError::Unsupported { .. })).collect::<Vec<_>>(),
                    vec![true; 6]
                );
            }
            assert_eq!(
                serde_json::to_value(store.get_price_alerts(None).await.unwrap()).unwrap(),
                serde_json::to_value(retained).unwrap()
            );
            assert_eq!(store.updates.lock().unwrap().len(), 0);
            assert!(!service.is_enabled());
            assert!(!service.preferences.is_push_notifications_enabled());
            assert_eq!(provider.requested_paths(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_mainnet_price_alert_writes_continue_to_update_store_and_backend() {
        block_on(async {
            let provider = Arc::new(TestAlienProvider::with_json(200, "{}"));
            let (service, store) = service(provider.clone(), Vec::new());
            let mainnet = alert(Chain::Tron, Some(1.0));
            service.add_price_alerts(vec![mainnet.clone()]).await.unwrap();
            assert_eq!(
                serde_json::to_value(store.get_price_alerts(None).await.unwrap()).unwrap(),
                serde_json::to_value(vec![mainnet.clone()]).unwrap()
            );
            service.delete_price_alerts(vec![mainnet.clone()]).await.unwrap();
            assert!(store.get_price_alerts(None).await.unwrap().is_empty());
            assert_eq!(
                serde_json::to_value(&*store.updates.lock().unwrap()).unwrap(),
                serde_json::to_value(vec![(vec![mainnet.clone()], Vec::<String>::new()), (Vec::<PriceAlert>::new(), vec![mainnet.id()])]).unwrap()
            );
            assert_eq!(provider.requested_paths().len(), 2);
        });
    }

    #[test]
    fn test_global_price_alert_sync_ignores_unsupported_assets_and_preserves_retained_alerts() {
        block_on(async {
            let mainnet = alert(Chain::Tron, Some(1.0));
            let retained = vec![
                PriceAlert::new_auto(Chain::TronShasta.as_asset_id(), Currency::USD),
                PriceAlert::new_auto(Chain::BitcoinSv.as_asset_id(), Currency::USD),
            ];
            let remote = vec![
                mainnet.clone(),
                PriceAlert::new_auto(TRON_SHASTA_USDT.id.clone(), Currency::USD),
                PriceAlert::new_auto(Chain::BitcoinSv.as_asset_id(), Currency::USD),
            ];
            let provider = Arc::new(TestAlienProvider::with_json(200, &serde_json::to_string(&remote).unwrap()));
            let (service, store) = service(provider.clone(), retained.clone());
            service.sync(None).await.unwrap();
            assert_eq!(
                serde_json::to_value(&*store.updates.lock().unwrap()).unwrap(),
                serde_json::to_value(vec![(vec![mainnet.clone()], Vec::<String>::new())]).unwrap()
            );
            assert_eq!(
                serde_json::to_value(store.get_price_alerts(None).await.unwrap()).unwrap(),
                serde_json::to_value([retained, vec![mainnet]].concat()).unwrap()
            );
            assert_eq!(provider.requested_paths().len(), 1);
        });
    }

    #[test]
    fn test_wowpay_price_alert_sync_preserves_hidden_records_and_rejects_new_hidden_writes() {
        block_on(async {
            let bsv = PriceAlert::new_auto(Chain::BitcoinSv.as_asset_id(), Currency::USD);
            let counterfeit = PriceAlert::new_auto(AssetId::from_token(Chain::Tron, "counterfeit-usdt"), Currency::USD);
            let hidden = vec![
                PriceAlert::new_auto(Chain::Ethereum.as_asset_id(), Currency::USD),
                counterfeit.clone(),
                PriceAlert::new_auto(Chain::Tron.as_asset_id(), Currency::USD),
                PriceAlert::new_auto(primitives::known_assets::TRON_USDT.id.clone(), Currency::USD),
            ];
            let provider = Arc::new(TestAlienProvider::with_json(200, &serde_json::to_string(&vec![bsv.clone(), counterfeit]).unwrap()));
            let (mut service, store) = service(provider.clone(), hidden.clone());
            service.environment = crate::services::chain::BsvHomeRuntimeEnvironment::WowPay;
            for alert in hidden.iter().chain([&bsv]) {
                assert!(service.add_price_alerts(vec![alert.clone()]).await.is_err());
                assert!(service.delete_price_alerts(vec![alert.clone()]).await.is_err());
            }
            assert!(store.updates.lock().unwrap().is_empty());
            assert!(provider.requested_paths().is_empty());
            service.sync(None).await.unwrap();
            assert_eq!(
                serde_json::to_value(&*store.updates.lock().unwrap()).unwrap(),
                serde_json::to_value(Vec::<(Vec<PriceAlert>, Vec<String>)>::new()).unwrap()
            );
            assert_eq!(
                serde_json::to_value(store.get_price_alerts(None).await.unwrap()).unwrap(),
                serde_json::to_value(hidden).unwrap()
            );
            assert_eq!(provider.requested_paths().len(), 1);
        });
    }
}
