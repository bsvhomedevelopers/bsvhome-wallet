use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use primitives::{Transaction, Wallet, WalletId};
use serde_json::Value;

use super::{BsvHomeTransactionStore, BsvHomeTransactionsService};
use crate::alien::{AlienProvider, AlienResponse};
use crate::api::{BsvHomeApiClient, BsvHomeDeviceApiClient};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::services::{
    assets::{BsvHomeAssetsService, testkit::MemoryAssetStore},
    chain::BsvHomeRuntimeEnvironment,
    device::BsvHomeDeviceKeyService,
    error::BsvHomeServiceError,
    preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore},
    price::{BsvHomePriceService, testkit::MemoryPriceStore},
    transaction_state::BsvHomeTransactionStatusService,
    wallet::{
        addresses::testkit::pool,
        testkit::{MemoryAddressStore, MemoryWalletStore},
    },
    wallet_preferences::{BsvHomeWalletPreferencesService, testkit::MemoryWalletPreferencesStore},
    wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore},
};
use crate::testkit::TestAlienProvider;

pub const TEST_BSV_ADDRESS: &str = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
pub const TEST_BSV_NODE: &str = "https://api.whatsonchain.com/v1/bsv/main";
pub const TEST_BSV_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000001";
pub const TEST_BSV_PREVIOUS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000002";

pub struct TransactionsTestkit {
    pub service: Arc<BsvHomeTransactionsService>,
    pub store: Arc<MemoryTransactionStore>,
    pub gateway: Arc<BsvHomeGateway>,
    pub assets: Arc<BsvHomeAssetsService>,
    pub asset_store: Arc<MemoryAssetStore>,
    pub wallet_store: Arc<MemoryWalletStore>,
    pub wallet_id: WalletId,
    pub wallet_preferences: Arc<BsvHomeWalletPreferencesService>,
    pub status: Arc<RecordingTransactionStatus>,
}

impl TransactionsTestkit {
    pub fn new(wallet: Wallet, provider: Arc<TestAlienProvider>) -> Self {
        Self::new_with_environment(wallet, provider, BsvHomeRuntimeEnvironment::Production)
    }

    pub fn new_with_environment(wallet: Wallet, provider: Arc<dyn AlienProvider>, environment: BsvHomeRuntimeEnvironment) -> Self {
        let wallet_id = wallet.id.clone();
        let wallet_store = Arc::new(MemoryWalletStore {
            wallets: Mutex::new(vec![wallet]),
            ..Default::default()
        });
        let preferences_store = Arc::new(MemoryPreferencesStore::default());
        let preferences = Arc::new(BsvHomePreferencesService::new(preferences_store.clone()));
        let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
            Arc::new(MemoryWalletSessionStore {
                current: Mutex::new(Some(wallet_id.clone())),
            }),
            wallet_store.clone(),
            environment,
        ));
        let gateway = Arc::new(BsvHomeGateway::new_with_environment(
            provider.clone(),
            preferences_store,
            Arc::new(EmptyPreferences),
            environment,
        ));
        let asset_store = Arc::new(MemoryAssetStore::default());
        let assets = Arc::new(BsvHomeAssetsService::new(
            Arc::new(BsvHomeApiClient::new(provider.clone())),
            gateway.clone(),
            asset_store.clone(),
            Arc::new(BsvHomePriceService::new(Arc::new(MemoryPriceStore::default()), preferences.clone())),
            preferences.clone(),
            session.clone(),
        ));
        let store = Arc::new(MemoryTransactionStore::default());
        let status = Arc::new(RecordingTransactionStatus::default());
        let wallet_preferences = Arc::new(BsvHomeWalletPreferencesService::new(Arc::new(MemoryWalletPreferencesStore::default())));
        let service = Arc::new(BsvHomeTransactionsService::new(
            Arc::new(BsvHomeDeviceApiClient::new(provider, Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))))),
            gateway.clone(),
            assets.clone(),
            store.clone(),
            Arc::new(MemoryAddressStore::default()),
            wallet_preferences.clone(),
            preferences,
            session,
            status.clone(),
            pool(gateway.clone(), wallet_store.clone()),
        ));
        Self {
            service,
            store,
            gateway,
            assets,
            asset_store,
            wallet_store,
            wallet_id,
            wallet_preferences,
            status,
        }
    }
}

#[derive(Default)]
pub struct MemoryTransactionStore {
    pub transactions: Mutex<Vec<Transaction>>,
    pub saved: Mutex<Vec<(WalletId, Vec<Transaction>)>>,
    pub failure: Mutex<Option<BsvHomeServiceError>>,
}

#[async_trait]
impl BsvHomeTransactionStore for MemoryTransactionStore {
    async fn save_transactions(&self, wallet_id: WalletId, transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError> {
        if let Some(error) = self.failure.lock().unwrap().clone() {
            return Err(error);
        }
        let mut stored = self.transactions.lock().unwrap();
        for transaction in &transactions {
            stored.retain(|existing| existing.id != transaction.id);
            stored.push(transaction.clone());
        }
        self.saved.lock().unwrap().push((wallet_id, transactions));
        Ok(())
    }
}

#[derive(Default)]
pub struct RecordingTransactionStatus(pub Mutex<Vec<(WalletId, Vec<Transaction>)>>);

impl BsvHomeTransactionStatusService for RecordingTransactionStatus {
    fn track(&self, wallet_id: WalletId, transactions: Vec<Transaction>) {
        self.0.lock().unwrap().push((wallet_id, transactions));
    }
}

pub fn bsv_response(path: &str, body: Value) -> (String, AlienResponse) {
    (format!("{TEST_BSV_NODE}{path}"), AlienResponse::new(Some(200), serde_json::to_vec(&body).unwrap()))
}

pub fn bsv_history_responses(pending: bool) -> Vec<(String, AlienResponse)> {
    let mut history: Value = serde_json::from_str(include_str!("../../../testdata/bsv/address_history.json")).unwrap();
    let mut transaction: Value = serde_json::from_str(include_str!("../../../testdata/bsv/transaction.json")).unwrap();
    let previous_transaction: Value = serde_json::from_str(include_str!("../../../testdata/bsv/previous_transaction.json")).unwrap();
    if pending {
        history[0]["unconfirmed"]["result"] = history[0]["confirmed"]["result"].take();
        history[0]["unconfirmed"]["result"][0]["height"] = Value::Null;
        history[0]["confirmed"]["result"] = Value::Array(Vec::new());
        transaction["confirmations"] = Value::Number(0.into());
        transaction["time"] = transaction["blocktime"].take();
        transaction["blockheight"] = Value::Null;
    } else {
        history[0]["unconfirmed"]["result"] = history[0]["confirmed"]["result"].clone();
        history[0]["unconfirmed"]["result"][0]["height"] = Value::Number(0.into());
    }
    vec![
        bsv_response("/addresses/history/all", history),
        bsv_response(&format!("/tx/hash/{TEST_BSV_HASH}"), transaction),
        bsv_response(&format!("/tx/hash/{TEST_BSV_PREVIOUS_HASH}"), previous_transaction),
    ]
}
