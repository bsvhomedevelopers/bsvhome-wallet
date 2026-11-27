use super::testkit::{MemoryDevicePlatform, UnavailableDevicePlatform};
use super::*;
use crate::alien::{AlienError, AlienHttpMethod, AlienProvider, AlienResponse, AlienTarget};
use crate::config::public::API_URL;
use crate::gateway::EmptyPreferences;
use crate::services::preferences::testkit::MemoryPreferencesStore;
use crate::services::wallet::testkit::MemoryWalletStore;
use async_trait::async_trait;
use futures::executor::block_on;
use primitives::{Account, TransactionsResponse, Wallet, WalletSubscription, WalletSubscriptionChains};
use std::slice::from_ref;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::Ordering;

#[derive(Debug)]
struct DeviceProvider {
    device: StdMutex<Device>,
    subscriptions: StdMutex<Vec<WalletSubscriptionChains>>,
    additions: StdMutex<Vec<WalletSubscription>>,
    requests: StdMutex<Vec<(AlienHttpMethod, String)>>,
    registration_status: u16,
    subscription_status: u16,
    update_status: u16,
}

impl Default for DeviceProvider {
    fn default() -> Self {
        Self {
            device: StdMutex::new(Device::mock()),
            subscriptions: StdMutex::new(Vec::new()),
            additions: StdMutex::new(Vec::new()),
            requests: StdMutex::new(Vec::new()),
            registration_status: 200,
            subscription_status: 200,
            update_status: 200,
        }
    }
}

fn response<T: serde::Serialize>(value: T) -> Arc<AlienResponse> {
    Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&value).unwrap()))
}

#[async_trait]
impl AlienProvider for DeviceProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        assert!(target.headers.as_ref().unwrap().contains_key("Authorization"));
        let path = target.url.strip_prefix(API_URL).unwrap().split('?').next().unwrap().to_string();
        self.requests.lock().unwrap().push((target.method, path.clone()));
        match (target.method, path.as_str()) {
            (AlienHttpMethod::Get, "/v2/devices") => {
                if self.registration_status != 200 {
                    return Ok(Arc::new(AlienResponse::new(Some(self.registration_status), Vec::new())));
                }
                Ok(response(self.device.lock().unwrap().clone()))
            }
            (AlienHttpMethod::Put, "/v2/devices") => {
                if self.update_status != 200 {
                    return Ok(Arc::new(AlienResponse::new(Some(self.update_status), Vec::new())));
                }
                let device: Device = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                *self.device.lock().unwrap() = device.clone();
                Ok(response(device))
            }
            (AlienHttpMethod::Get, "/v2/devices/subscriptions") => Ok(response(self.subscriptions.lock().unwrap().clone())),
            (AlienHttpMethod::Post, "/v2/devices/subscriptions") => {
                let additions: Vec<WalletSubscription> = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                self.additions.lock().unwrap().extend(additions.clone());
                if self.subscription_status != 200 {
                    return Ok(Arc::new(AlienResponse::new(Some(self.subscription_status), Vec::new())));
                }
                self.subscriptions.lock().unwrap().extend(additions.into_iter().map(|wallet| WalletSubscriptionChains {
                    wallet_id: wallet.wallet_id,
                    chains: wallet.subscriptions.into_iter().flat_map(|address| address.chains).collect(),
                }));
                Ok(response(()))
            }
            (AlienHttpMethod::Get, "/v2/devices/transactions") => Ok(response(TransactionsResponse::new(Vec::new(), Vec::new()))),
            _ => panic!("unexpected device request"),
        }
    }
}

fn service(provider: Arc<DeviceProvider>, wallets: Vec<Wallet>) -> (Arc<BsvHomeDeviceService>, Arc<BsvHomePreferencesService>) {
    let api = Arc::new(BsvHomeDeviceApiClient::new(provider, Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences)))));
    let store = Arc::new(MemoryWalletStore {
        wallets: StdMutex::new(wallets),
        ..Default::default()
    });
    let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
    preferences.set_device_registered(true).unwrap();
    preferences.set_price_alerts_enabled(true).unwrap();
    let service = Arc::new(BsvHomeDeviceService::new(
        api.clone(),
        Arc::new(BsvHomeSubscriptionService::new(api.clone(), store.clone())),
        store,
        Arc::new(MemoryDevicePlatform),
        preferences.clone(),
    ));
    api.set_device_sync_preflight(service.clone());
    (service, preferences)
}

fn mixed_wallet() -> Wallet {
    Wallet::mock_with_accounts(vec![Account::mock(Chain::Tron, "Tfixture"), Account::mock(Chain::BitcoinSv, "bsv-fixture")])
}

#[test]
fn test_wowpay_device_sync_stops_before_platform_credentials_and_hosted_requests() {
    block_on(async {
        let provider = Arc::new(DeviceProvider::default());
        let platform = Arc::new(UnavailableDevicePlatform::default());
        let api = Arc::new(BsvHomeDeviceApiClient::new(
            provider.clone(),
            Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
        ));
        let wallets = Arc::new(MemoryWalletStore {
            wallets: StdMutex::new(vec![mixed_wallet()]),
            ..Default::default()
        });
        let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
        let service = BsvHomeDeviceService::new_with_environment(
            api.clone(),
            Arc::new(BsvHomeSubscriptionService::new(api, wallets.clone())),
            wallets,
            platform.clone(),
            preferences.clone(),
            BsvHomeRuntimeEnvironment::WowPay,
        );

        service.synchronize_if_needed().await.unwrap();
        service.set_push_enabled(true).await.unwrap();
        assert_eq!(
            service.synchronize().await.unwrap_err(),
            BsvHomeServiceError::Unsupported {
                msg: "BsvHome hosted services are unavailable in this environment".into()
            }
        );
        assert_eq!(service.prepare_wallet(&mixed_wallet().id.id(), None).await.is_err(), true);
        assert_eq!(platform.calls.load(Ordering::SeqCst), 0);
        assert_eq!(*provider.requests.lock().unwrap(), vec![]);
        assert_eq!(preferences.is_device_registered(), false);
        assert_eq!(preferences.get_pushed_device().is_none(), true);
    });
}

#[test]
fn test_explicit_production_device_sync_keeps_default_hosted_registration() {
    block_on(async {
        for explicit in [false, true] {
            let provider = Arc::new(DeviceProvider::default());
            let (mut device, preferences) = service(provider.clone(), vec![mixed_wallet()]);
            if explicit {
                device = Arc::new(BsvHomeDeviceService::new_with_environment(
                    device.api.clone(),
                    device.subscriptions.clone(),
                    device.wallet_store.clone(),
                    device.platform.clone(),
                    preferences,
                    BsvHomeRuntimeEnvironment::Production,
                ));
            }
            device.synchronize_if_needed().await.unwrap();
            assert_eq!(
                *provider.requests.lock().unwrap(),
                vec![
                    (AlienHttpMethod::Get, "/v2/devices".into()),
                    (AlienHttpMethod::Get, "/v2/devices/subscriptions".into()),
                    (AlienHttpMethod::Post, "/v2/devices/subscriptions".into()),
                    (AlienHttpMethod::Put, "/v2/devices".into()),
                ]
            );
        }
    });
}

#[test]
fn test_wallet_preflight_is_add_only_and_scoped_without_publishing_full_checkpoint() {
    block_on(async {
        let wallet = mixed_wallet();
        let unrelated = Wallet {
            id: WalletId::Multicoin("unrelated".into()),
            ..Wallet::mock_with_accounts(vec![Account::mock(Chain::Ethereum, "0xfixture")])
        };
        let remote = WalletSubscriptionChains {
            wallet_id: unrelated.id.clone(),
            chains: vec![Chain::Ethereum, Chain::Bitcoin],
        };
        let provider = Arc::new(DeviceProvider {
            subscriptions: StdMutex::new(vec![remote.clone()]),
            update_status: 500,
            ..Default::default()
        });
        let (service, preferences) = service(provider.clone(), vec![wallet.clone(), unrelated]);
        preferences.set_subscriptions_version(7).unwrap();
        preferences.set_pushed_subscriptions("previous-checkpoint".into()).unwrap();
        service
            .api
            .client
            .get_transactions(wallet.id.id(), Some(Chain::Tron.as_asset_id().to_string()), 0)
            .await
            .unwrap();
        assert_eq!(provider.additions.lock().unwrap().clone(), subscription_rules::wallet_subscriptions(from_ref(&wallet)));
        assert_eq!(provider.subscriptions.lock().unwrap()[0], remote);
        assert_eq!(preferences.get_subscriptions_version(), 7);
        assert_eq!(preferences.get_pushed_subscriptions(), Some("previous-checkpoint".into()));
        assert_eq!(
            provider.requests.lock().unwrap().clone(),
            vec![
                (AlienHttpMethod::Get, "/v2/devices".into()),
                (AlienHttpMethod::Get, "/v2/devices/subscriptions".into()),
                (AlienHttpMethod::Post, "/v2/devices/subscriptions".into()),
                (AlienHttpMethod::Get, "/v2/devices/transactions".into()),
            ]
        );
    });
}

#[test]
fn test_required_subscription_and_registration_failures_block_history() {
    block_on(async {
        for (registration_status, subscription_status, expected_requests) in [(403, 200, 1), (200, 400, 3)] {
            let wallet = mixed_wallet();
            let provider = Arc::new(DeviceProvider {
                registration_status,
                subscription_status,
                ..Default::default()
            });
            let (service, preferences) = service(provider.clone(), vec![wallet.clone()]);
            let error = service
                .api
                .client
                .get_transactions(wallet.id.id(), Some(Chain::Tron.as_asset_id().to_string()), 0)
                .await
                .unwrap_err();
            let expected_status = if registration_status != 200 { registration_status } else { subscription_status };
            match error {
                ClientError::Http { status, .. } => assert_eq!(status, expected_status),
                error => panic!("unexpected error: {error}"),
            }
            assert_eq!(provider.requests.lock().unwrap().len(), expected_requests);
            assert_eq!(preferences.get_pushed_subscriptions(), None);
        }
    });
}

#[test]
fn test_asset_history_proceeds_when_its_chain_is_ready_but_wallet_history_requires_all_chains() {
    block_on(async {
        let wallet = Wallet::mock_with_accounts(vec![
            Account::mock(Chain::Tron, "Tfixture"),
            Account::mock(Chain::Ethereum, "0xfixture"),
            Account::mock(Chain::BitcoinSv, "bsv-fixture"),
        ]);
        let provider = Arc::new(DeviceProvider {
            subscriptions: StdMutex::new(vec![WalletSubscriptionChains {
                wallet_id: wallet.id.clone(),
                chains: vec![Chain::Tron],
            }]),
            subscription_status: 400,
            update_status: 500,
            ..Default::default()
        });
        let (service, preferences) = service(provider.clone(), vec![wallet.clone()]);
        service
            .api
            .client
            .get_transactions(wallet.id.id(), Some(Chain::Tron.as_asset_id().to_string()), 0)
            .await
            .unwrap();
        assert_eq!(provider.additions.lock().unwrap().len(), 0);
        assert!(service.api.client.get_transactions(wallet.id.id(), None, 0).await.is_err());
        let additions = provider.additions.lock().unwrap();
        assert_eq!(additions.len(), 1);
        assert_eq!(additions[0].subscriptions, vec![AddressChains::new("0xfixture".into(), vec![Chain::Ethereum])]);
        assert_eq!(preferences.get_pushed_subscriptions(), None);
        assert_eq!(provider.requests.lock().unwrap().iter().filter(|(_, path)| path == "/v2/devices/transactions").count(), 1);
    });
}

#[test]
fn test_empty_bsv_home_subscription_checkpoint_converges_for_a_bsv_only_wallet() {
    block_on(async {
        let provider = Arc::new(DeviceProvider::default());
        let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, "bsv-fixture")]);
        let (service, preferences) = service(provider.clone(), vec![wallet]);
        service.synchronize_if_needed().await.unwrap();
        assert_eq!(preferences.get_pushed_subscriptions(), Some(String::new()));
        assert_eq!(provider.additions.lock().unwrap().len(), 0);
        let requests = provider.requests.lock().unwrap().len();
        service.synchronize_if_needed().await.unwrap();
        assert_eq!(provider.requests.lock().unwrap().len(), requests);
    });
}

#[test]
fn test_preflight_rejects_unknown_wallet_missing_chain_and_unsupported_chain_before_egress() {
    block_on(async {
        let wallet = mixed_wallet();
        let provider = Arc::new(DeviceProvider::default());
        let (service, _) = service(provider.clone(), vec![wallet.clone()]);
        for (wallet_id, chain) in [
            ("multicoin_missing".to_string(), Chain::Tron),
            (wallet.id.id(), Chain::Ethereum),
            (wallet.id.id(), Chain::BitcoinSv),
        ] {
            assert!(service.prepare_wallet(&wallet_id, Some(chain)).await.is_err());
        }
        assert_eq!(provider.requests.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_subscription_checkpoint_survives_profile_failure_without_repeated_version_increment() {
    block_on(async {
        let wallet = mixed_wallet();
        let provider = Arc::new(DeviceProvider {
            update_status: 500,
            ..Default::default()
        });
        let (service, preferences) = service(provider.clone(), vec![wallet.clone()]);
        assert!(service.synchronize_if_needed().await.is_err());
        let version = preferences.get_subscriptions_version();
        assert_eq!(version, 2);
        assert_eq!(preferences.get_pushed_subscriptions(), Some(rules::subscriptions_signature(&[wallet])));
        assert!(preferences.get_pushed_device().is_none());
        assert!(service.synchronize_if_needed().await.is_err());
        assert_eq!(preferences.get_subscriptions_version(), version);
        assert_eq!(provider.additions.lock().unwrap().len(), 1);
    });
}

#[test]
fn test_exhausted_remote_subscription_version_returns_error_without_publishing_a_checkpoint() {
    block_on(async {
        let provider = Arc::new(DeviceProvider {
            device: StdMutex::new(Device {
                subscriptions_version: i32::MAX,
                ..Device::mock()
            }),
            ..Default::default()
        });
        let (service, preferences) = service(provider, vec![mixed_wallet()]);
        assert_eq!(
            service.synchronize_if_needed().await.unwrap_err(),
            BsvHomeServiceError::Api {
                msg: "BsvHome subscription version is exhausted".to_string()
            }
        );
        assert_eq!(preferences.get_pushed_subscriptions(), None);
        assert_eq!(preferences.get_subscriptions_version(), 0);
    });
}
