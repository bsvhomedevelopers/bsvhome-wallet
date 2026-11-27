pub mod rules;

use crate::services::error::BsvHomeServiceError;
use bsv_home_client::ClientError;
use primitives::WalletSubscription;
use std::sync::Arc;

use crate::services::wallet::BsvHomeWalletStore;

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};

#[derive(uniffi::Object)]
pub struct BsvHomeSubscriptionService {
    api: Arc<BsvHomeDeviceApiClient>,
    store: Arc<dyn BsvHomeWalletStore>,
}

impl BsvHomeSubscriptionService {
    pub(crate) async fn ensure_subscriptions(&self, subscriptions: Vec<WalletSubscription>) -> Result<(), ClientError> {
        let remote = self.api.client.get_subscriptions().await?;
        let additions = rules::subscription_changes(subscriptions, remote).to_add;
        if !additions.is_empty() {
            self.api.client.add_subscriptions(additions).await?;
        }
        Ok(())
    }
}

#[uniffi::export]
impl BsvHomeSubscriptionService {
    #[uniffi::constructor]
    pub fn new(api: Arc<BsvHomeDeviceApiClient>, store: Arc<dyn BsvHomeWalletStore>) -> Self {
        Self { api, store }
    }

    pub async fn sync(&self) -> Result<bool, BsvHomeServiceError> {
        let local = rules::wallet_subscriptions(&self.store.get_wallets().await?);
        let remote = self.api.client.get_subscriptions().await.map_err(BsvHomeApiError::from)?;
        let changes = rules::subscription_changes(local, remote);
        if changes.is_empty() {
            return Ok(false);
        }
        if !changes.to_add.is_empty() {
            self.api.client.add_subscriptions(changes.to_add).await.map_err(BsvHomeApiError::from)?;
        }
        if !changes.to_delete.is_empty() {
            self.api.client.delete_subscriptions(changes.to_delete).await.map_err(BsvHomeApiError::from)?;
        }
        Ok(true)
    }
}
