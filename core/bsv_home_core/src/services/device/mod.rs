pub mod keys;
pub mod platform;
pub mod rules;
pub mod signer;
#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests;

use crate::services::error::BsvHomeServiceError;
use std::sync::Arc;

use primitives::{AddressChains, Chain, Device, DeviceLocale, WalletId};

pub use keys::BsvHomeDeviceKeyService;
pub use platform::{BsvHomeDeviceInfo, BsvHomeDevicePlatform};
pub use signer::BsvHomeDeviceRequestSigner;

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::subscription::{BsvHomeSubscriptionService, rules as subscription_rules};
use crate::services::wallet::BsvHomeWalletStore;
use bsv_home_api::WalletRequestPreflight;
use bsv_home_client::ClientError;
use futures::lock::Mutex;
use std::sync::Weak;

#[derive(uniffi::Object)]
pub struct BsvHomeDeviceService {
    api: Arc<BsvHomeDeviceApiClient>,
    subscriptions: Arc<BsvHomeSubscriptionService>,
    wallet_store: Arc<dyn BsvHomeWalletStore>,
    platform: Arc<dyn BsvHomeDevicePlatform>,
    preferences: Arc<BsvHomePreferencesService>,
    sync_lock: Mutex<()>,
    environment: BsvHomeRuntimeEnvironment,
}

#[uniffi::export]
impl BsvHomeDeviceService {
    #[uniffi::constructor]
    pub fn new(
        api: Arc<BsvHomeDeviceApiClient>,
        subscriptions: Arc<BsvHomeSubscriptionService>,
        wallet_store: Arc<dyn BsvHomeWalletStore>,
        platform: Arc<dyn BsvHomeDevicePlatform>,
        preferences: Arc<BsvHomePreferencesService>,
    ) -> Self {
        Self::new_with_environment(api, subscriptions, wallet_store, platform, preferences, BsvHomeRuntimeEnvironment::Production)
    }

    #[uniffi::constructor]
    pub fn new_with_environment(
        api: Arc<BsvHomeDeviceApiClient>,
        subscriptions: Arc<BsvHomeSubscriptionService>,
        wallet_store: Arc<dyn BsvHomeWalletStore>,
        platform: Arc<dyn BsvHomeDevicePlatform>,
        preferences: Arc<BsvHomePreferencesService>,
        environment: BsvHomeRuntimeEnvironment,
    ) -> Self {
        Self {
            api,
            subscriptions,
            wallet_store,
            platform,
            preferences,
            sync_lock: Mutex::new(()),
            environment,
        }
    }

    pub async fn synchronize_if_needed(&self) -> Result<(), BsvHomeServiceError> {
        if !self.environment.uses_bsv_home_hosted_services() {
            return Ok(());
        }
        let device = self.current_device().await?;
        if !self.needs_sync(device.clone()).await? {
            return Ok(());
        }
        let _guard = self.sync_lock.lock().await;
        if self.needs_sync(device.clone()).await? {
            self.sync(device).await?;
        }
        Ok(())
    }
}

impl BsvHomeDeviceService {
    pub async fn set_push_enabled(&self, enabled: bool) -> Result<(), BsvHomeServiceError> {
        self.preferences.set_push_notifications_declined(!enabled)?;
        self.preferences.set_push_notifications_enabled(enabled)?;
        self.synchronize_if_needed().await
    }

    pub async fn synchronize(&self) -> Result<Device, BsvHomeServiceError> {
        let _guard = self.sync_lock.lock().await;
        self.sync(self.current_device().await?).await
    }

    pub async fn is_registered(&self) -> Result<bool, BsvHomeServiceError> {
        Ok(self.preferences.is_device_registered())
    }
}

impl BsvHomeDeviceService {
    fn require_hosted_services(&self) -> Result<(), BsvHomeServiceError> {
        if !self.environment.uses_bsv_home_hosted_services() {
            return Err(BsvHomeServiceError::Unsupported {
                msg: "BsvHome hosted services are unavailable in this environment".into(),
            });
        }
        Ok(())
    }

    async fn current_device(&self) -> Result<Device, BsvHomeServiceError> {
        self.require_hosted_services()?;
        let info = self.platform.device_info().await?;
        let is_push_enabled = self.platform.is_push_enabled().await?;
        let token = match is_push_enabled {
            true => self.platform.push_token().await?,
            false => String::new(),
        };
        Ok(Device {
            id: self.platform.device_id().await?,
            platform: info.platform,
            platform_store: info.platform_store,
            os: info.os,
            model: info.model,
            token,
            locale: DeviceLocale::from_locale_identifier(&info.locale_identifier),
            version: info.version,
            currency: self.platform.get_currency().await?,
            is_push_enabled,
            is_price_alerts_enabled: Some(self.preferences.is_price_alerts_enabled()),
            subscriptions_version: 0,
        })
    }

    async fn needs_sync(&self, device: Device) -> Result<bool, BsvHomeServiceError> {
        if !self.preferences.is_device_registered() {
            return Ok(true);
        }
        let Some(pushed) = self.preferences.get_pushed_device() else {
            return Ok(true);
        };
        let local = Device {
            subscriptions_version: self.preferences.get_subscriptions_version(),
            ..device
        };
        if rules::device_changed(&pushed, &local) {
            return Ok(true);
        }
        let signature = rules::subscriptions_signature(&self.wallet_store.get_wallets().await?);
        Ok(self.preferences.get_pushed_subscriptions() != Some(signature))
    }

    async fn sync(&self, device: Device) -> Result<Device, BsvHomeServiceError> {
        let signature = rules::subscriptions_signature(&self.wallet_store.get_wallets().await?);
        let mut version = self.preferences.get_subscriptions_version();
        let remote = self.get_or_create(&device).await.map_err(BsvHomeApiError::from)?;

        let signature_changed = self.preferences.get_pushed_subscriptions() != Some(signature.clone());
        if signature_changed || remote.subscriptions_version != version {
            self.subscriptions.sync().await?;
            if signature_changed {
                let next_version = remote.subscriptions_version.checked_add(1).ok_or_else(|| BsvHomeServiceError::Api {
                    msg: "BsvHome subscription version is exhausted".to_string(),
                })?;
                version = version.max(next_version);
                self.preferences.set_subscriptions_version(version)?;
            }
            self.preferences.set_pushed_subscriptions(signature)?;
        }

        let local = Device {
            subscriptions_version: version,
            ..device
        };
        let synced = if rules::device_changed(&remote, &local) {
            self.api.client.update_device(local.clone()).await.map_err(BsvHomeApiError::from)?
        } else {
            remote
        };
        self.preferences.set_pushed_device(&local)?;
        Ok(synced)
    }
}

impl BsvHomeDeviceService {
    async fn prepare_wallet(&self, wallet_id: &str, chain: Option<Chain>) -> Result<(), ClientError> {
        self.require_hosted_services().map_err(preflight_error)?;
        let wallet_id = WalletId::from_id(wallet_id).ok_or_else(|| ClientError::Serialization("invalid wallet request id".to_string()))?;
        let wallet = self
            .wallet_store
            .get_wallet(wallet_id)
            .await
            .map_err(preflight_error)?
            .ok_or_else(|| ClientError::Serialization("wallet request references an unknown local wallet".to_string()))?;
        if let Some(chain) = chain
            && (!chain.supports_bsv_home_backend() || wallet.account(chain).is_none())
        {
            return Err(ClientError::Serialization(format!("wallet request chain {chain} is unavailable for BsvHome subscriptions")));
        }
        let subscriptions = subscription_rules::wallet_subscriptions(&[wallet])
            .into_iter()
            .filter_map(|mut wallet| {
                wallet.subscriptions = wallet
                    .subscriptions
                    .into_iter()
                    .filter_map(|address| {
                        let chains = address
                            .chains
                            .into_iter()
                            .filter(|candidate| chain.is_none_or(|chain| *candidate == chain))
                            .collect::<Vec<_>>();
                        (!chains.is_empty()).then(|| AddressChains::new(address.address, chains))
                    })
                    .collect();
                (!wallet.subscriptions.is_empty()).then_some(wallet)
            })
            .collect::<Vec<_>>();
        if subscriptions.is_empty() {
            return Err(ClientError::Serialization("wallet has no BsvHome-supported subscriptions".to_string()));
        }
        let _guard = self.sync_lock.lock().await;
        let device = self.current_device().await.map_err(preflight_error)?;
        self.get_or_create(&device).await?;
        self.subscriptions.ensure_subscriptions(subscriptions).await
    }

    async fn get_or_create(&self, device: &Device) -> Result<Device, ClientError> {
        let registered = self.preferences.is_device_registered() || self.api.client.is_device_registered().await?;
        if registered {
            match self.api.client.get_device().await? {
                Some(remote) => {
                    self.preferences.set_device_registered(true).map_err(preflight_error)?;
                    return Ok(remote);
                }
                None => self.preferences.set_device_registered(false).map_err(preflight_error)?,
            }
        }
        let added = self
            .api
            .client
            .add_device(Device {
                subscriptions_version: 0,
                ..device.clone()
            })
            .await?;
        self.preferences.set_device_registered(true).map_err(preflight_error)?;
        Ok(added)
    }
}

#[derive(Debug)]
pub(crate) struct DeviceSyncPreflight {
    pub(crate) service: Weak<BsvHomeDeviceService>,
}

#[async_trait::async_trait]
impl WalletRequestPreflight for DeviceSyncPreflight {
    async fn prepare(&self, wallet_id: &str, chain: Option<Chain>) -> Result<(), ClientError> {
        let service = self
            .service
            .upgrade()
            .ok_or_else(|| ClientError::Network("device synchronization service is unavailable".to_string()))?;
        service.prepare_wallet(wallet_id, chain).await
    }
}

fn preflight_error(error: BsvHomeServiceError) -> ClientError {
    ClientError::Network(error.to_string())
}
