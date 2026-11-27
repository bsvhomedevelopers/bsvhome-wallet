use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use primitives::{Account, Chain, Wallet, WalletType};
use tempfile::TempDir;

use super::BsvHomeAppStartService;
use crate::api::{BsvHomeApiClient, BsvHomeDeviceApiClient};
use crate::gateway::EmptyPreferences;
use crate::keystore::BsvHomeKeystore;
use crate::services::assets::{BsvHomeAssetStore, testkit::MemoryAssetStore};
use crate::services::balance::BsvHomeBalanceService;
use crate::services::banner::{BsvHomeBannerService, rules as banner_rules, testkit::MemoryBannerStore};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::config::BsvHomeConfigService;
use crate::services::device::{BsvHomeDeviceKeyService, BsvHomeDeviceService, testkit::MemoryDevicePlatform};
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::file::testkit::NoopFileStore;
use crate::services::localization::testkit::EnglishLocalizer;
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::stream::testkit::SubscriptionTestkit;
use crate::services::subscription::BsvHomeSubscriptionService;
use crate::services::transactions::testkit::{TEST_BSV_ADDRESS, TransactionsTestkit};
use crate::services::wallet::{
    BsvHomeWalletService,
    addresses::testkit::pool,
    testkit::{MemoryAddressStore, MemoryKeystorePassword},
};
use crate::services::wallet_configuration::BsvHomeWalletConfigurationService;
use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
use crate::testkit::TestAlienProvider;

struct AppStartTestkit {
    service: BsvHomeAppStartService,
    provider: Arc<TestAlienProvider>,
    banners: Arc<MemoryBannerStore>,
    assets: Arc<MemoryAssetStore>,
    wallet: Wallet,
    _directory: TempDir,
}

impl AppStartTestkit {
    fn new(environment: BsvHomeRuntimeEnvironment) -> Self {
        let wallet = Wallet {
            wallet_type: WalletType::View,
            ..Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, TEST_BSV_ADDRESS)])
        };
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let kit = TransactionsTestkit::new_with_environment(wallet.clone(), provider.clone(), environment);
        let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
            Arc::new(MemoryWalletSessionStore {
                current: Mutex::new(Some(wallet.id.clone())),
            }),
            kit.wallet_store.clone(),
            environment,
        ));
        let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
        let api = Arc::new(BsvHomeApiClient::new(provider.clone()));
        let device_api = Arc::new(BsvHomeDeviceApiClient::new(
            provider.clone(),
            Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
        ));
        let device = Arc::new(BsvHomeDeviceService::new(
            device_api.clone(),
            Arc::new(BsvHomeSubscriptionService::new(device_api.clone(), kit.wallet_store.clone())),
            kit.wallet_store.clone(),
            Arc::new(MemoryDevicePlatform),
            preferences.clone(),
        ));
        let subscriptions = SubscriptionTestkit::new(&[], &[]);
        let addresses = pool(kit.gateway.clone(), kit.wallet_store.clone());
        let balances = Arc::new(BsvHomeBalanceService::new(
            kit.gateway,
            kit.wallet_store.clone(),
            kit.asset_store.clone(),
            subscriptions.balances,
            kit.assets.clone(),
            Arc::new(subscriptions.service),
            addresses.clone(),
        ));
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new_with_environment(directory.path().to_string_lossy().into_owned(), environment).unwrap();
        let wallet_service = Arc::new(BsvHomeWalletService::new(
            keystore,
            Arc::new(MemoryKeystorePassword::default()),
            kit.wallet_store,
            session.clone(),
            preferences.clone(),
            Arc::new(NoopFileStore),
            kit.wallet_preferences.clone(),
            Arc::new(BsvHomeExplorerService::new(preferences.clone())),
            Arc::new(MemoryAddressStore::default()),
            Arc::new(EnglishLocalizer),
            addresses,
        ));
        let banners = Arc::new(MemoryBannerStore::default());
        Self {
            service: BsvHomeAppStartService::new(
                Arc::new(BsvHomeConfigService::new(api, preferences)),
                Arc::new(BsvHomeBannerService::new(banners.clone())),
                kit.assets,
                balances,
                Arc::new(BsvHomeWalletConfigurationService::new(device_api, banners.clone(), kit.wallet_preferences, session)),
                wallet_service,
                device,
            ),
            provider,
            banners,
            assets: kit.asset_store,
            wallet,
            _directory: directory,
        }
    }
}

#[test]
fn test_wowpay_app_start_keeps_local_banners_without_hosted_requests() {
    block_on(async {
        let kit = AppStartTestkit::new(BsvHomeRuntimeEnvironment::WowPay);
        assert_eq!(kit.service.run().await, vec![]);
        assert_eq!(kit.provider.requested_paths(), Vec::<String>::new());
        assert_eq!(
            kit.banners
                .writes
                .lock()
                .unwrap()
                .iter()
                .map(|keys| keys.iter().map(|key| key.identifier()).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            vec![banner_rules::setup_keys().iter().map(|key| key.identifier()).collect::<Vec<_>>()],
        );
    });
}

#[test]
fn test_production_app_start_preserves_hosted_config_and_device_requests() {
    block_on(async {
        let kit = AppStartTestkit::new(BsvHomeRuntimeEnvironment::Production);
        let failures = kit.service.run().await;
        assert_eq!(failures.len(), 3);
        let mut paths = kit.provider.requested_paths();
        paths.sort();
        assert_eq!(paths, vec!["/v1/config", "/v1/config", "/v2/devices/is_registered"]);
    });
}

#[test]
fn test_wowpay_wallet_initialization_preserves_default_assets_and_balances() {
    block_on(async {
        let kit = AppStartTestkit::new(BsvHomeRuntimeEnvironment::WowPay);
        kit.service.initialize_wallet_assets(kit.wallet.clone()).await.unwrap();
        let bsv = Chain::BitcoinSv.as_asset_id();
        assert_eq!(kit.assets.get_asset_ids(vec![bsv.clone(), Chain::Tron.as_asset_id()]).await.unwrap(), vec![bsv.clone()]);
        assert_eq!(*kit.assets.added_balances.lock().unwrap(), vec![(kit.wallet.id, vec![bsv], true)]);
        assert_eq!(kit.provider.requested_paths(), Vec::<String>::new());
    });
}
