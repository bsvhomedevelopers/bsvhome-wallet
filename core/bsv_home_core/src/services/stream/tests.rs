use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::executor::block_on;
use num_bigint::BigUint;
use primitives::{Account, Chain, FiatTransactionData, StreamTransactionsUpdate, SupportMessage, SupportTyping, Wallet, WalletId};
use serde_json::Value;

use super::testkit::{MemoryBalanceStore, SubscriptionTestkit};
use super::{BsvHomeStreamService, sync_transaction_event};
use crate::alien::AlienResponse;
use crate::api::BsvHomeDeviceApiClient;
use crate::gateway::EmptyPreferences;
use crate::services::balance::{BsvHomeAssetBalance, BsvHomeBalanceRecord, BsvHomeBalanceService};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::device::{BsvHomeDeviceKeyService, BsvHomeDeviceService, testkit::MemoryDevicePlatform};
use crate::services::error::BsvHomeServiceError;
use crate::services::fiat::{BsvHomeFiatService, BsvHomeFiatStore};
use crate::services::nft::{BsvHomeNftService, testkit::MemoryNftStore};
use crate::services::notification::testkit::MemoryNotificationStore;
use crate::services::perpetual::{BsvHomePerpetualService, testkit::UnusedPerpetualStore};
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::price::{BsvHomePriceService, testkit::MemoryPriceStore};
use crate::services::price_alert::testkit as alert_testkit;
use crate::services::subscription::BsvHomeSubscriptionService;
use crate::services::support::BsvHomeSupportStore;
use crate::services::transactions::testkit::{TEST_BSV_ADDRESS, TEST_BSV_NODE, TransactionsTestkit, bsv_history_responses};
use crate::services::wallet::addresses::testkit::pool;
use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
use crate::testkit::TestAlienProvider;

struct UnusedStreamStore;

#[async_trait]
impl BsvHomeFiatStore for UnusedStreamStore {
    async fn set_transactions(&self, _wallet_id: WalletId, _transactions: Vec<FiatTransactionData>) -> Result<(), BsvHomeServiceError> {
        panic!("connection preparation must not write fiat transactions");
    }
}

#[async_trait]
impl BsvHomeSupportStore for UnusedStreamStore {
    async fn save_messages(&self, _messages: Vec<SupportMessage>) -> Result<(), BsvHomeServiceError> {
        panic!("connection preparation must not write support messages");
    }
    async fn save_message(&self, _id: String, _message: SupportMessage) -> Result<(), BsvHomeServiceError> {
        panic!("connection preparation must not write support messages");
    }
    fn update_typing(&self, _typing: SupportTyping) -> Result<(), BsvHomeServiceError> {
        panic!("connection preparation must not update support typing");
    }
    fn clear_typing(&self) -> Result<(), BsvHomeServiceError> {
        panic!("connection preparation must not clear support typing");
    }
}

fn connection_service(environment: BsvHomeRuntimeEnvironment, provider: Arc<TestAlienProvider>) -> BsvHomeStreamService {
    let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, TEST_BSV_ADDRESS)]);
    let kit = TransactionsTestkit::new_with_environment(wallet.clone(), provider.clone(), environment);
    let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
        Arc::new(MemoryWalletSessionStore {
            current: Mutex::new(Some(wallet.id)),
        }),
        kit.wallet_store.clone(),
        environment,
    ));
    let subscriptions = SubscriptionTestkit::new(&[], &[]);
    let balance = Arc::new(BsvHomeBalanceService::new(
        kit.gateway.clone(),
        kit.wallet_store.clone(),
        kit.asset_store.clone(),
        subscriptions.balances,
        kit.assets.clone(),
        Arc::new(subscriptions.service),
        pool(kit.gateway.clone(), kit.wallet_store.clone()),
    ));
    let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
    let price = Arc::new(BsvHomePriceService::new(Arc::new(MemoryPriceStore::default()), preferences.clone()));
    let api = Arc::new(BsvHomeDeviceApiClient::new(
        provider.clone(),
        Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
    ));
    let device = Arc::new(BsvHomeDeviceService::new(
        api.clone(),
        Arc::new(BsvHomeSubscriptionService::new(api.clone(), kit.wallet_store.clone())),
        kit.wallet_store,
        Arc::new(MemoryDevicePlatform),
        preferences.clone(),
    ));
    let perpetual = Arc::new(BsvHomePerpetualService::new(
        kit.gateway,
        price.clone(),
        Arc::new(UnusedPerpetualStore),
        kit.asset_store,
        preferences,
        balance.clone(),
        kit.wallet_preferences,
        session.clone(),
    ));
    BsvHomeStreamService::new(
        price,
        Arc::new(alert_testkit::service(provider, vec![]).0),
        balance,
        kit.service,
        Arc::new(BsvHomeNftService::new(api.clone(), Arc::new(MemoryNftStore::default()), session.clone())),
        perpetual,
        Arc::new(BsvHomeFiatService::new(api, kit.assets, Arc::new(UnusedStreamStore))),
        Arc::new(MemoryNotificationStore::default()),
        Arc::new(UnusedStreamStore),
        Arc::new(SubscriptionTestkit::new(&[], &[]).service),
        session,
        device,
    )
}

#[test]
fn test_wowpay_stream_preparation_declines_before_hosted_device_synchronization() {
    block_on(async {
        for (environment, connect, paths) in [
            (BsvHomeRuntimeEnvironment::WowPay, false, vec![]),
            (BsvHomeRuntimeEnvironment::Production, true, vec!["/v2/devices/is_registered".to_string()]),
        ] {
            let provider = Arc::new(TestAlienProvider::with_status(503));
            let service = connection_service(environment, provider.clone());
            assert_eq!(service.prepare_connection().await.unwrap(), connect);
            assert_eq!(provider.requested_paths(), paths);
        }
    });
}

async fn transaction_services(
    history_error: Option<BsvHomeServiceError>,
    balance_error: Option<BsvHomeServiceError>,
) -> (TransactionsTestkit, BsvHomeBalanceService, Arc<MemoryBalanceStore>) {
    let mut unspent: Value = serde_json::from_str(include_str!("../../../../crates/bsv_home_bsv/testdata/confirmed_unspent.json")).unwrap();
    unspent["address"] = Value::String(TEST_BSV_ADDRESS.to_string());
    unspent["result"] = Value::Array(Vec::new());
    unspent["nextPageToken"] = Value::Null;
    let mut responses = bsv_history_responses(false);
    responses.push((
        format!("{TEST_BSV_NODE}/address/{TEST_BSV_ADDRESS}/unspent/all"),
        AlienResponse::new(Some(200), serde_json::to_vec(&unspent).unwrap()),
    ));
    let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, TEST_BSV_ADDRESS)]);
    let kit = TransactionsTestkit::new(wallet, Arc::new(TestAlienProvider::with_responses(responses)));
    kit.assets.ensure_default_assets().await.unwrap();
    *kit.store.failure.lock().unwrap() = history_error;
    let balance_store = Arc::new(MemoryBalanceStore::default());
    balance_store.rows.lock().unwrap().push(BsvHomeAssetBalance {
        available: BigUint::from(1u8),
        ..BsvHomeAssetBalance::zero(Chain::BitcoinSv.as_asset_id())
    });
    *balance_store.failure.lock().unwrap() = balance_error;
    let balance = BsvHomeBalanceService::new(
        kit.gateway.clone(),
        kit.wallet_store.clone(),
        kit.asset_store.clone(),
        balance_store.clone(),
        kit.assets.clone(),
        Arc::new(SubscriptionTestkit::new(&[], &[]).service),
        pool(kit.gateway.clone(), kit.wallet_store.clone()),
    );
    (kit, balance, balance_store)
}

fn transaction_event(kit: &TransactionsTestkit) -> StreamTransactionsUpdate {
    StreamTransactionsUpdate {
        wallet_id: kit.wallet_id.clone(),
        transactions: Vec::new(),
        asset_ids: vec![Chain::BitcoinSv.as_asset_id()],
    }
}

#[test]
fn test_sync_transaction_event_history_error_preserves_balance_save() {
    block_on(async {
        let error = BsvHomeServiceError::Platform {
            msg: "history persistence unavailable".to_string(),
        };
        let (kit, balance, balance_store) = transaction_services(Some(error.clone()), None).await;

        assert_eq!(
            sync_transaction_event(&kit.service, &balance, transaction_event(&kit)).await,
            Err(BsvHomeServiceError::Core { msg: format!("BSV: {error}") })
        );

        assert_eq!(*kit.store.saved.lock().unwrap(), Vec::new());
        assert_eq!(
            *balance_store.saved.lock().unwrap(),
            vec![(kit.wallet_id, vec![BsvHomeBalanceRecord::new(BsvHomeAssetBalance::zero(Chain::BitcoinSv.as_asset_id()), 8)])]
        );
    });
}

#[test]
fn test_sync_transaction_event_balance_error_preserves_history_save() {
    block_on(async {
        let error = BsvHomeServiceError::Store {
            msg: "balance persistence unavailable".to_string(),
        };
        let (kit, balance, balance_store) = transaction_services(None, Some(error.clone())).await;

        assert_eq!(sync_transaction_event(&kit.service, &balance, transaction_event(&kit)).await, Err(error));

        let saved = kit.store.saved.lock().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].0, kit.wallet_id);
        assert_eq!(
            saved[0].1.iter().map(|transaction| transaction.value.clone()).collect::<Vec<_>>(),
            vec![BigUint::from(5000u16)]
        );
        assert_eq!(*balance_store.saved.lock().unwrap(), Vec::new());
    });
}

#[test]
fn test_sync_transaction_event_reports_both_errors() {
    block_on(async {
        let (kit, balance, balance_store) = transaction_services(
            Some(BsvHomeServiceError::Platform {
                msg: "history persistence unavailable".to_string(),
            }),
            Some(BsvHomeServiceError::Store {
                msg: "balance persistence unavailable".to_string(),
            }),
        )
        .await;

        assert_eq!(
            sync_transaction_event(&kit.service, &balance, transaction_event(&kit)).await,
            Err(BsvHomeServiceError::Core {
                msg: "transaction history: BSV: history persistence unavailable; balance update: balance persistence unavailable".to_string()
            })
        );

        assert_eq!(*kit.store.saved.lock().unwrap(), Vec::new());
        assert_eq!(*balance_store.saved.lock().unwrap(), Vec::new());
    });
}

#[test]
fn test_sync_transaction_event_saves_history_and_balances() {
    block_on(async {
        let (kit, balance, balance_store) = transaction_services(None, None).await;

        sync_transaction_event(&kit.service, &balance, transaction_event(&kit)).await.unwrap();

        assert_eq!(kit.store.saved.lock().unwrap().len(), 1);
        assert_eq!(balance_store.saved.lock().unwrap().len(), 1);
    });
}
