use std::sync::Arc;

use primitives::{Deeplink, PlatformStore, WalletId};

use crate::services::device::BsvHomeDevicePlatform;
use crate::services::error::BsvHomeServiceError;
use crate::services::perpetual::BsvHomePerpetualService;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::transaction_state::BsvHomeTransactionStateStore;
use crate::services::wallet_preferences::BsvHomeWalletPreferencesService;

#[derive(uniffi::Object)]
pub struct BsvHomeDeveloperService {
    platform: Arc<dyn BsvHomeDevicePlatform>,
    preferences: Arc<BsvHomePreferencesService>,
    wallet_preferences: Arc<BsvHomeWalletPreferencesService>,
    transactions: Arc<dyn BsvHomeTransactionStateStore>,
    perpetual: Arc<BsvHomePerpetualService>,
}

#[uniffi::export]
impl BsvHomeDeveloperService {
    #[uniffi::constructor]
    pub fn new(
        platform: Arc<dyn BsvHomeDevicePlatform>,
        preferences: Arc<BsvHomePreferencesService>,
        wallet_preferences: Arc<BsvHomeWalletPreferencesService>,
        transactions: Arc<dyn BsvHomeTransactionStateStore>,
        perpetual: Arc<BsvHomePerpetualService>,
    ) -> Self {
        Self {
            platform,
            preferences,
            wallet_preferences,
            transactions,
            perpetual,
        }
    }

    pub async fn device_id(&self) -> Result<String, BsvHomeServiceError> {
        self.platform.device_id().await
    }

    pub async fn push_token(&self) -> Result<String, BsvHomeServiceError> {
        self.platform.push_token().await
    }

    pub async fn platform_store(&self) -> Result<PlatformStore, BsvHomeServiceError> {
        Ok(self.platform.device_info().await?.platform_store)
    }

    pub async fn clear_pending_transactions(&self) -> Result<(), BsvHomeServiceError> {
        for pending in self.transactions.get_pending_transactions().await? {
            self.transactions.delete_transaction(pending.wallet.id, pending.transaction.id).await?;
        }
        Ok(())
    }

    pub fn reset_transactions_timestamp(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        self.wallet_preferences.reset_transactions_timestamp(wallet_id)
    }

    pub fn delete_wallet_preferences(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        self.wallet_preferences.delete_preferences(wallet_id)
    }

    pub fn clear_preferences(&self) -> Result<(), BsvHomeServiceError> {
        self.preferences.clear()
    }

    pub async fn clear_perpetual_markets(&self) -> Result<(), BsvHomeServiceError> {
        self.perpetual.clear_markets().await
    }

    pub fn deeplink_url(&self, deeplink: Deeplink) -> String {
        deeplink.to_bsv_home_url()
    }
}
