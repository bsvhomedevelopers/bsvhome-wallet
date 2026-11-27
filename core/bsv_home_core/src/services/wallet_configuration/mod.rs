pub mod rules;

use crate::services::error::BsvHomeServiceError;
use primitives::WalletId;
use std::sync::Arc;

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::services::banner::{BsvHomeBannerStore, rules as banner_rules};

use crate::services::wallet_preferences::BsvHomeWalletPreferencesService;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomeWalletConfigurationService {
    api: Arc<BsvHomeDeviceApiClient>,
    banners: Arc<dyn BsvHomeBannerStore>,
    preferences: Arc<BsvHomeWalletPreferencesService>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomeWalletConfigurationService {
    #[uniffi::constructor]
    pub fn new(
        api: Arc<BsvHomeDeviceApiClient>,
        banners: Arc<dyn BsvHomeBannerStore>,
        preferences: Arc<BsvHomeWalletPreferencesService>,
        session: Arc<BsvHomeWalletSessionService>,
    ) -> Self {
        Self {
            api,
            banners,
            preferences,
            session,
        }
    }

    pub async fn sync(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        if !self.session.environment.uses_bsv_home_hosted_services() {
            return Ok(());
        }
        if self.preferences.is_wallet_configuration_completed(wallet_id.clone())? {
            return Ok(());
        }
        let wallet = self.session.get_wallet(wallet_id.clone()).await?.ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: "wallet not found".to_string(),
        })?;
        if !wallet.accounts.iter().any(|account| account.chain.supports_bsv_home_backend()) {
            return self.preferences.set_wallet_configuration_completed(wallet_id);
        }
        let result = self.api.client.get_wallet_configuration(wallet_id.id()).await.map_err(BsvHomeApiError::from)?;
        for key in rules::multi_signature_banners(&wallet_id, &result.configuration) {
            let state = banner_rules::default_state(key.event);
            self.banners.set_state(key, state).await?;
        }
        self.preferences.set_wallet_configuration_completed(wallet_id)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use futures::executor::block_on;
    use primitives::{Account, Chain, Wallet, WalletId};

    use super::BsvHomeWalletConfigurationService;
    use crate::api::BsvHomeDeviceApiClient;
    use crate::gateway::EmptyPreferences;
    use crate::services::banner::testkit::MemoryBannerStore;
    use crate::services::chain::BsvHomeRuntimeEnvironment;
    use crate::services::device::BsvHomeDeviceKeyService;
    use crate::services::wallet::testkit::MemoryWalletStore;
    use crate::services::wallet_preferences::{BsvHomeWalletPreferencesService, testkit::MemoryWalletPreferencesStore};
    use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
    use crate::testkit::TestAlienProvider;

    #[test]
    fn test_wowpay_wallet_configuration_preserves_hidden_accounts_and_completion_state_without_requests() {
        block_on(async {
            let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, "bsv"), Account::mock(Chain::Tron, "retained")]);
            let saved = serde_json::to_value(&wallet).unwrap();
            let wallets = Arc::new(MemoryWalletStore {
                wallets: Mutex::new(vec![wallet.clone()]),
                ..Default::default()
            });
            let provider = Arc::new(TestAlienProvider::with_status(503));
            let banners = Arc::new(MemoryBannerStore::default());
            let preferences = Arc::new(BsvHomeWalletPreferencesService::new(Arc::new(MemoryWalletPreferencesStore::default())));
            let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
                Arc::new(MemoryWalletSessionStore::default()),
                wallets.clone(),
                BsvHomeRuntimeEnvironment::WowPay,
            ));
            let service = BsvHomeWalletConfigurationService::new(
                Arc::new(BsvHomeDeviceApiClient::new(
                    provider.clone(),
                    Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
                )),
                banners.clone(),
                preferences.clone(),
                session,
            );
            service.sync(wallet.id.clone()).await.unwrap();
            assert_eq!(provider.requested_paths(), Vec::<String>::new());
            assert_eq!(preferences.is_wallet_configuration_completed(wallet.id).unwrap(), false);
            assert_eq!(serde_json::to_value(&wallets.wallets.lock().unwrap()[0]).unwrap(), saved);
            assert_eq!(banners.writes.lock().unwrap().len(), 0);
        });
    }

    #[test]
    fn test_unsupported_backend_wallet_configuration_skips_and_missing_or_failed_requests_stay_pending() {
        block_on(async {
            for (accounts, expected_paths) in [
                (vec![Account::mock(Chain::BitcoinSv, "bsv")], vec![]),
                (vec![Account::mock(Chain::TronShasta, "testnet")], vec![]),
                (
                    vec![Account::mock(Chain::BitcoinSv, "bsv"), Account::mock(Chain::Tron, "mainnet")],
                    vec!["/v2/devices/wallet_configuration".to_string()],
                ),
            ] {
                let wallet = Wallet::mock_with_accounts(accounts);
                let provider = Arc::new(TestAlienProvider::with_status(503));
                let banners = Arc::new(MemoryBannerStore::default());
                let preferences = Arc::new(BsvHomeWalletPreferencesService::new(Arc::new(MemoryWalletPreferencesStore::default())));
                let session = Arc::new(BsvHomeWalletSessionService::new(
                    Arc::new(MemoryWalletSessionStore::default()),
                    Arc::new(MemoryWalletStore {
                        wallets: Mutex::new(vec![wallet.clone()]),
                        ..Default::default()
                    }),
                ));
                let service = BsvHomeWalletConfigurationService::new(
                    Arc::new(BsvHomeDeviceApiClient::new(
                        provider.clone(),
                        Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
                    )),
                    banners.clone(),
                    preferences.clone(),
                    session,
                );
                assert_eq!(service.sync(wallet.id.clone()).await.is_ok(), expected_paths.is_empty());
                assert_eq!(preferences.is_wallet_configuration_completed(wallet.id).unwrap(), expected_paths.is_empty());
                let missing_wallet = WalletId::Multicoin("missing".into());
                assert!(service.sync(missing_wallet.clone()).await.is_err());
                assert!(!preferences.is_wallet_configuration_completed(missing_wallet).unwrap());
                assert_eq!(provider.requested_paths(), expected_paths);
                assert_eq!(banners.states.lock().unwrap().len(), 0);
            }
        });
    }
}
