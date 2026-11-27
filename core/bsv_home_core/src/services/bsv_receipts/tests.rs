use super::rules::{self, Event, Receipt, ReceiptQueue, Work};
use super::*;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;

use async_trait::async_trait;
use bsv_home_bsv::BsvAddress;
use bsv_home_derivation::{bsv_account_xpub, bsv_address, derive_accounts_from_mnemonic};
use futures::channel::oneshot;
use futures::executor::block_on;
use futures::future::{Either, pending, select};
use primitives::testkit::ABANDON_PHRASE;
use primitives::{Address, BsvAddressBranch, BsvAddressLocator, Transaction, TransactionId};
use serde_json::Value;
use tempfile::TempDir;

use crate::alien::{AlienError, AlienProvider, AlienResponse, AlienTarget};
use crate::api::{BsvHomeDeviceApiClient, BsvHomeStaticApiClient};
use crate::gateway::EmptyPreferences;
use crate::keystore::BsvHomeKeystore;
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::confirm::testkit::UnusedStakeStore;
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::nft::{BsvHomeNftService, testkit::MemoryNftStore};
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::stake::BsvHomeStakeService;
use crate::services::stream::testkit::{MemoryBalanceStore, SubscriptionTestkit};
use crate::services::transaction_state::{BsvHomePendingTransaction, BsvHomeTransactionStateStore, BsvHomeTransactionStateUpdate};
use crate::services::transactions::testkit::{
    MemoryTransactionStore, RecordingTransactionStatus, TEST_BSV_ADDRESS, TEST_BSV_HASH, TEST_BSV_PREVIOUS_HASH, TransactionsTestkit, bsv_history_responses,
};
use crate::services::wallet::addresses::testkit::{BsvAddressProvider, complete_record, pool_with_keystore};
use crate::services::wallet::testkit::{MemoryAddressStore, MemoryKeystorePassword, MemoryWalletStore};
use crate::services::wallet_session::testkit::MemoryWalletSessionStore;
use crate::testkit::TestAlienProvider;

const ADDRESS: &str = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5";

#[derive(Default)]
struct ReceiptStore {
    saved: Mutex<Vec<Transaction>>,
    state: Mutex<Option<TransactionState>>,
}

#[async_trait]
impl BsvHomeTransactionStateStore for ReceiptStore {
    async fn get_state(&self, _wallet: WalletId, _transaction: TransactionId) -> Result<Option<TransactionState>, BsvHomeServiceError> {
        Ok(*self.state.lock().unwrap())
    }
    async fn add_transactions(&self, _wallet: WalletId, transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError> {
        *self.state.lock().unwrap() = transactions.last().map(|transaction| transaction.state);
        self.saved.lock().unwrap().extend(transactions);
        Ok(())
    }
    async fn get_pending_transactions(&self) -> Result<Vec<BsvHomePendingTransaction>, BsvHomeServiceError> {
        panic!("receipt worker must schedule through the status port");
    }
    async fn get_transaction(&self, _wallet: WalletId, _transaction: TransactionId) -> Result<Option<BsvHomePendingTransaction>, BsvHomeServiceError> {
        panic!("receipt worker must not poll transactions itself");
    }
    async fn update_transaction_hash(&self, _wallet: WalletId, _transaction: TransactionId, _hash: String) -> Result<(), BsvHomeServiceError> {
        panic!("receipt must preserve transaction identity");
    }
    async fn delete_transaction(&self, _wallet: WalletId, _transaction: TransactionId) -> Result<(), BsvHomeServiceError> {
        panic!("receipt must preserve history");
    }
    async fn update_transaction(&self, _wallet: WalletId, _transaction: TransactionId, _update: BsvHomeTransactionStateUpdate) -> Result<bool, BsvHomeServiceError> {
        panic!("receipt must schedule through the status port");
    }
}

#[derive(Debug)]
struct ReceiptProvider {
    base: TestAlienProvider,
    hd: Mutex<Option<Arc<HdReceiptProvider>>>,
    indexed: AtomicBool,
    index_after_empty: AtomicBool,
    forged: AtomicBool,
    paths: Mutex<Vec<String>>,
    gate: Mutex<Option<oneshot::Receiver<()>>>,
    paused_suffix: Mutex<Option<String>>,
    entered: Mutex<Option<oneshot::Sender<()>>>,
}

impl ReceiptProvider {
    fn new() -> Self {
        Self {
            base: TestAlienProvider::with_responses(bsv_history_responses(true)),
            hd: Mutex::new(None),
            indexed: AtomicBool::new(false),
            index_after_empty: AtomicBool::new(false),
            forged: AtomicBool::new(false),
            paths: Mutex::new(Vec::new()),
            gate: Mutex::new(None),
            paused_suffix: Mutex::new(None),
            entered: Mutex::new(None),
        }
    }
    fn pause_transaction(&self) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        self.pause_request(&format!("/tx/hash/{TEST_BSV_HASH}"))
    }
    fn pause_request(&self, suffix: &str) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (entered, started) = oneshot::channel();
        let (release, gate) = oneshot::channel();
        *self.entered.lock().unwrap() = Some(entered);
        *self.gate.lock().unwrap() = Some(gate);
        *self.paused_suffix.lock().unwrap() = Some(suffix.into());
        (started, release)
    }
}

#[async_trait]
impl AlienProvider for ReceiptProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        self.paths.lock().unwrap().push(target.url.clone());
        let hd = self.hd.lock().unwrap().clone();
        let captured = match hd {
            Some(provider) => Some(provider.request(target.clone()).await),
            None => None,
        };
        let gate = {
            let suffix = self.paused_suffix.lock().unwrap();
            if suffix.as_ref().is_some_and(|suffix| target.url.ends_with(suffix)) {
                self.gate.lock().unwrap().take()
            } else {
                None
            }
        };
        if let Some(gate) = gate {
            if let Some(entered) = self.entered.lock().unwrap().take() {
                let _ = entered.send(());
            }
            let _ = gate.await;
        }
        if let Some(response) = captured {
            return response;
        }
        if target.url.ends_with("/unspent/all") {
            let outputs = if self.indexed.load(Ordering::SeqCst) {
                format!(r#"[{{"tx_hash":"{TEST_BSV_HASH}","tx_pos":0,"value":5000,"height":null,"status":"unconfirmed","isSpentInMempoolTx":false}}]"#)
            } else {
                "[]".into()
            };
            if self.index_after_empty.swap(false, Ordering::SeqCst) {
                self.indexed.store(true, Ordering::SeqCst);
            }
            return Ok(Arc::new(AlienResponse::new(
                Some(200),
                format!(r#"{{"address":"{TEST_BSV_ADDRESS}","result":{outputs},"error":""}}"#).into_bytes(),
            )));
        }
        if target.url.ends_with(&format!("/tx/hash/{TEST_BSV_HASH}")) {
            if self.forged.load(Ordering::SeqCst) {
                let mut value: Value = serde_json::from_str(include_str!("../../../testdata/bsv/transaction.json")).unwrap();
                value["vout"][0]["scriptPubKey"]["hex"] = Value::String("76a914d0a69b53d9a1e229b51a60cc24814f8750b7446188ac".into());
                return Ok(Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&value).unwrap())));
            }
        }
        self.base.request(target).await
    }
}

#[derive(Debug)]
struct HdReceiptProvider {
    addresses: BsvAddressProvider,
    transaction: Arc<AlienResponse>,
    previous_transaction: Arc<AlienResponse>,
}

impl HdReceiptProvider {
    fn new(address: &str, outgoing: bool) -> Self {
        let script = hex::encode(BsvAddress::from_str(address).unwrap().script_pubkey().as_bytes());
        let mut transaction: Value = serde_json::from_str(include_str!("../../../testdata/bsv/transaction.json")).unwrap();
        transaction["vout"][0]["scriptPubKey"]["hex"] = script.clone().into();
        transaction["confirmations"] = 0.into();
        transaction["time"] = transaction["blocktime"].take();
        transaction["blockheight"] = Value::Null;
        let mut previous: Value = serde_json::from_str(include_str!("../../../testdata/bsv/previous_transaction.json")).unwrap();
        if outgoing {
            previous["vout"][0]["scriptPubKey"]["hex"] = script.into();
        }
        Self {
            addresses: BsvAddressProvider::default(),
            transaction: Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&transaction).unwrap())),
            previous_transaction: Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&previous).unwrap())),
        }
    }
}

#[async_trait]
impl AlienProvider for HdReceiptProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        if target.url.ends_with(&format!("/tx/hash/{TEST_BSV_HASH}")) {
            return Ok(self.transaction.clone());
        }
        if target.url.ends_with(&format!("/tx/hash/{TEST_BSV_PREVIOUS_HASH}")) {
            return Ok(self.previous_transaction.clone());
        }
        self.addresses.request(target).await
    }
}

struct Testkit {
    service: Arc<BsvHomeBsvReceiptService>,
    provider: Arc<ReceiptProvider>,
    wallets: Arc<MemoryWalletStore>,
    session: Arc<BsvHomeWalletSessionService>,
    balances: Arc<MemoryBalanceStore>,
    state: Arc<ReceiptStore>,
    status: Arc<RecordingTransactionStatus>,
    history: Arc<MemoryTransactionStore>,
    password: Arc<MemoryKeystorePassword>,
    _directory: TempDir,
}

impl Testkit {
    async fn new(wallet_type: WalletType) -> Self {
        Self::with_environment(wallet_type, BsvHomeRuntimeEnvironment::WowPay).await
    }
    async fn with_environment(wallet_type: WalletType, environment: BsvHomeRuntimeEnvironment) -> Self {
        let provider = Arc::new(ReceiptProvider::new());
        let wallet = Wallet {
            wallet_type,
            ..Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, TEST_BSV_ADDRESS)])
        };
        let kit = TransactionsTestkit::new_with_environment(wallet.clone(), provider.clone(), environment);
        kit.assets.ensure_default_assets().await.unwrap();
        let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
            Arc::new(MemoryWalletSessionStore {
                current: Mutex::new(Some(wallet.id)),
            }),
            kit.wallet_store.clone(),
            environment,
        ));
        let directory = TempDir::new().unwrap();
        let password = Arc::new(MemoryKeystorePassword::default());
        let addresses = pool_with_keystore(
            kit.gateway.clone(),
            kit.wallet_store.clone(),
            BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap(),
            password.clone(),
            kit.wallet_store.bsv_addresses.clone(),
        );
        let subscriptions = SubscriptionTestkit::new(&[], &[]);
        let balances = subscriptions.balances;
        let balance = Arc::new(BsvHomeBalanceService::new(
            kit.gateway.clone(),
            kit.wallet_store.clone(),
            kit.asset_store,
            balances.clone(),
            kit.assets.clone(),
            Arc::new(subscriptions.service),
            addresses.clone(),
        ));
        let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
        let api = Arc::new(BsvHomeDeviceApiClient::new(
            provider.clone(),
            Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
        ));
        let stake = Arc::new(BsvHomeStakeService::new(
            kit.gateway.clone(),
            Arc::new(BsvHomeStaticApiClient::new(provider.clone())),
            Arc::new(UnusedStakeStore),
            Arc::new(MemoryAddressStore::default()),
            Arc::new(BsvHomeExplorerService::new(preferences.clone())),
            preferences,
            session.clone(),
        ));
        let state = Arc::new(ReceiptStore::default());
        let state_service = Arc::new(BsvHomeTransactionStateService::new(
            kit.gateway.clone(),
            state.clone(),
            kit.assets,
            balance.clone(),
            stake,
            Arc::new(BsvHomeNftService::new(api, Arc::new(MemoryNftStore::default()), session.clone())),
        ));
        let service = Arc::new(
            BsvHomeBsvReceiptService::new(
                kit.gateway,
                addresses,
                session.clone(),
                balance,
                kit.service,
                state_service,
                kit.status.clone(),
                "https://paymail.bsvhome.com/api/portal".into(),
            )
            .unwrap(),
        );
        Self {
            service,
            provider,
            wallets: kit.wallet_store,
            session,
            balances,
            state,
            status: kit.status,
            history: kit.store,
            password,
            _directory: directory,
        }
    }
    fn receipt(&self) -> Receipt {
        Receipt {
            hash: TEST_BSV_HASH.into(),
            address: TEST_BSV_ADDRESS.into(),
        }
    }
    fn generation(&self) -> u64 {
        self.service.generation.load(Ordering::SeqCst)
    }
    fn prepare_hd(&self, outgoing: bool) -> (Wallet, Arc<HdReceiptProvider>) {
        let xpub = bsv_account_xpub(ABANDON_PHRASE).unwrap();
        let mut account = derive_accounts_from_mnemonic(ABANDON_PHRASE, vec![Chain::BitcoinSv]).unwrap().remove(0);
        account.extended_public_key = Some(xpub);
        let wallet = {
            let mut wallets = self.wallets.wallets.lock().unwrap();
            wallets[0].accounts[0] = account;
            wallets[0].clone()
        };
        self.wallets
            .bsv_addresses
            .states
            .lock()
            .unwrap()
            .insert(wallet.id.clone(), complete_record(&wallet, ABANDON_PHRASE));
        let provider = Arc::new(HdReceiptProvider::new(&wallet.accounts[0].address, outgoing));
        *self.provider.hd.lock().unwrap() = Some(provider.clone());
        (wallet, provider)
    }
}

async fn await_captured_request<F: Future>(request: Pin<&mut F>, entered: oneshot::Receiver<()>) {
    match select(entered, request).await {
        Either::Left((entered, _)) => entered.unwrap(),
        Either::Right(_) => panic!("Receipt work finished before reaching its request gate"),
    }
}

#[test]
fn test_wallet_events_bind_protocol_request_and_owned_scripts_without_trusting_amounts() {
    let scripts = rules::receive_scripts(vec![ADDRESS.into()]).unwrap();
    let script = scripts.keys().next().unwrap();
    let hash = "a".repeat(64);
    let event = format!(
        r#"{{"version":1,"type":"transaction.changed","network":"bsv-mainnet","requestId":"current","transactionId":"{hash}","scriptHashes":["{script}"],"amount":"untrusted"}}"#
    );
    let expected = Event::Changed(Receipt {
        hash: hash.clone(),
        address: ADDRESS.into(),
    });
    assert_eq!(rules::parse_event(&event, "current", &scripts), Some(expected));
    for invalid in [
        event.replace("bsv-mainnet", "bsv-testnet"),
        event.replace("\"version\":1", "\"version\":2"),
        event.replace("current", "previous"),
        event.replace(&hash, &hash.to_uppercase()),
        event.replace(script, &"0".repeat(64)),
        event.replace(&hash, "invalid"),
        event[..event.len() - 1].into(),
        " ".repeat(150_001),
    ] {
        assert_eq!(rules::parse_event(&invalid, "current", &scripts), None);
    }
    let resync = r#"{"version":1,"type":"resync.required","network":"bsv-mainnet","requestId":"current"}"#;
    assert_eq!(rules::parse_event(resync, "current", &scripts), Some(Event::Resync));
    assert_eq!(rules::parse_event(resync, "previous", &scripts), None);
    let request: Value = serde_json::from_str(&rules::subscription("current", &scripts).unwrap()).unwrap();
    assert_eq!(request["requestId"], "current");
    assert_eq!(request["scriptHashes"][0], *script);
    assert_eq!(
        rules::socket_url("https://paymail.bsvhome.com/api/portal").unwrap(),
        "wss://paymail.bsvhome.com/api/v1/wallet-events"
    );
    for origin in [
        "http://paymail.bsvhome.com/api/portal",
        "https://paymail.bsvhome.com/v2/api/portal",
        "https://user@paymail.bsvhome.com/api/portal",
        "https://paymail.bsvhome.com/api/portal?secret=x",
    ] {
        assert_eq!(rules::socket_url(origin).is_err(), true);
    }
}

#[test]
fn test_receipt_queue_bounds_work_deduplicates_verified_hashes_and_recovers_overflow() {
    let receipt = |index| Receipt {
        hash: format!("{index:064x}"),
        address: ADDRESS.into(),
    };
    let mut queue = ReceiptQueue::default();
    for index in 0..33 {
        assert!(queue.push(Event::Changed(receipt(index))));
    }
    assert_eq!(queue.next(), Some(Work::Resync));
    for index in 0..32 {
        assert_eq!(queue.next(), Some(Work::Receipt(receipt(index))));
        assert!(!queue.push(Event::Changed(receipt(index))));
        queue.finish(&receipt(index), true);
        assert!(!queue.push(Event::Changed(receipt(index))));
    }
    assert_eq!(queue.next(), None);
    queue.push(Event::Changed(receipt(40)));
    assert_eq!(queue.next(), Some(Work::Receipt(receipt(40))));
    queue.finish(&receipt(40), false);
    assert_eq!(queue.next(), Some(Work::Resync));
    queue.push(Event::Changed(receipt(40)));
    assert_eq!(queue.next(), Some(Work::Receipt(receipt(40))));
    queue.finish(&receipt(40), true);
    for index in 50..180 {
        queue.push(Event::Changed(receipt(index)));
        assert_eq!(queue.next(), Some(Work::Receipt(receipt(index))));
        queue.finish(&receipt(index), true);
    }
    queue.push(Event::Changed(receipt(0)));
    assert_eq!(queue.next(), Some(Work::Receipt(receipt(0))));
}

#[test]
fn test_preparing_receipts_never_unlocks_an_unprepared_hd_wallet() {
    block_on(async {
        let hd = Testkit::new(WalletType::Multicoin).await;
        assert_eq!(hd.service.prepare_connection().await.unwrap(), false);
        assert_eq!(*hd.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        assert_eq!(*hd.provider.paths.lock().unwrap(), Vec::<String>::new());
        let observer = Testkit::new(WalletType::View).await;
        assert_eq!(observer.service.prepare_connection().await.unwrap(), true);
        assert_eq!(observer.service.url(), "wss://paymail.bsvhome.com/api/v1/wallet-events");
        assert_eq!(*observer.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        observer.session.set_current_wallet_id(None).unwrap();
        assert_eq!(observer.service.prepare_connection().await.unwrap(), false);
        assert_eq!(observer.service.connected().await.unwrap_err(), BsvHomeServiceError::Cancelled);
    });
}

#[test]
fn test_generic_environments_never_open_the_bsv_product_receipt_service() {
    block_on(async {
        for environment in [BsvHomeRuntimeEnvironment::Production, BsvHomeRuntimeEnvironment::LocalDevelopment] {
            let kit = Testkit::with_environment(WalletType::View, environment).await;
            assert_eq!(kit.service.prepare_connection().await.unwrap(), false);
            assert_eq!(*kit.provider.paths.lock().unwrap(), Vec::<String>::new());
            assert_eq!(*kit.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        }
    });
}

#[test]
fn test_public_address_cas_creation_and_addition_rebuild_subscription_without_unlocking() {
    block_on(async {
        let kit = Testkit::new(WalletType::Multicoin).await;
        let xpub = bsv_account_xpub(ABANDON_PHRASE).unwrap();
        let main = bsv_address(
            &xpub,
            BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 0,
            },
        )
        .unwrap();
        let wallet = {
            let mut wallets = kit.wallets.wallets.lock().unwrap();
            wallets[0].accounts[0].address = main;
            wallets[0].clone()
        };
        assert_eq!(kit.service.prepare_connection().await.unwrap(), false);
        kit.wallets
            .bsv_addresses
            .states
            .lock()
            .unwrap()
            .insert(wallet.id.clone(), complete_record(&wallet, ABANDON_PHRASE));
        assert_eq!(kit.service.prepare_connection().await.unwrap(), true);
        let initial: Value = serde_json::from_str(&kit.service.connected().await.unwrap()).unwrap();
        assert_eq!(initial["scriptHashes"].as_array().unwrap().len(), 10);
        kit.service.addresses.add_address(wallet.id.clone()).await.unwrap();
        let updated: Value = serde_json::from_str(&kit.service.refresh_subscription().await.unwrap().unwrap()).unwrap();
        assert_eq!(updated["scriptHashes"].as_array().unwrap().len(), 11);
        assert_ne!(initial["requestId"], updated["requestId"]);
        kit.service.addresses.select_address(wallet.id, 0).await.unwrap();
        assert_eq!(kit.service.refresh_subscription().await.unwrap(), None);
        assert_eq!(*kit.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        assert_eq!(*kit.provider.paths.lock().unwrap(), Vec::<String>::new());
    });
}

#[test]
fn test_reconnect_rotates_request_identity_and_catches_up_without_accepting_stale_events() {
    block_on(async {
        let kit = Testkit::new(WalletType::View).await;
        kit.service.prepare_connection().await.unwrap();
        let first: Value = serde_json::from_str(&kit.service.connected().await.unwrap()).unwrap();
        assert_eq!(kit.service.state().unwrap().queue.next(), Some(Work::Resync));
        assert_eq!(kit.service.refresh_subscription().await.unwrap(), None);
        let second: Value = serde_json::from_str(&kit.service.connected().await.unwrap()).unwrap();
        assert_ne!(first["requestId"], second["requestId"]);
        assert_eq!(kit.service.state().unwrap().queue.next(), Some(Work::Resync));
        let frame = |request: &Value| {
            format!(
                r#"{{"version":1,"type":"transaction.changed","network":"bsv-mainnet","requestId":{},"transactionId":"{TEST_BSV_HASH}","scriptHashes":{}}}"#,
                request["requestId"], request["scriptHashes"]
            )
        };
        kit.service.receive_event(frame(&first)).unwrap();
        assert_eq!(kit.service.state().unwrap().queue.next(), None);
        kit.service.receive_event(frame(&second)).unwrap();
        assert_eq!(kit.service.state().unwrap().queue.next(), Some(Work::Receipt(kit.receipt())));
        kit.service.cancel();
        kit.service.receive_event(frame(&second)).unwrap();
        assert_eq!(kit.service.state().unwrap().queue.next(), None);
    });
}

#[test]
fn test_verified_receipt_waits_for_utxo_index_preserves_pending_balance_and_tracks_only_once() {
    block_on(async {
        let kit = Testkit::new(WalletType::View).await;
        kit.service.prepare_connection().await.unwrap();
        assert_eq!(kit.service.receipt(kit.generation(), &kit.receipt()).await.unwrap(), false);
        assert_eq!(kit.balances.saved.lock().unwrap().len(), 0);
        assert_eq!(*kit.state.saved.lock().unwrap(), Vec::<Transaction>::new());
        kit.provider.indexed.store(true, Ordering::SeqCst);
        assert_eq!(kit.service.receipt(kit.generation(), &kit.receipt()).await.unwrap(), true);
        let saved = kit.state.saved.lock().unwrap().clone();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].direction, TransactionDirection::Incoming);
        assert_eq!(saved[0].state, TransactionState::Pending);
        assert_eq!(saved[0].value, 5_000u32.into());
        let balances = kit.balances.saved.lock().unwrap().clone();
        assert_eq!(balances[0].1[0].available.value, 0u32.into());
        assert_eq!(balances[0].1[0].pending_unconfirmed.value, 5_000u32.into());
        assert_eq!(kit.status.0.lock().unwrap().len(), 1);
        assert_eq!(kit.service.receipt(kit.generation(), &kit.receipt()).await.unwrap(), true);
        assert_eq!(kit.state.saved.lock().unwrap().len(), 1);
        assert_eq!(kit.status.0.lock().unwrap().len(), 1);
        *kit.state.state.lock().unwrap() = Some(TransactionState::Confirmed);
        assert_eq!(kit.service.receipt(kit.generation(), &kit.receipt()).await.unwrap(), true);
        assert_eq!(*kit.state.state.lock().unwrap(), Some(TransactionState::Confirmed));
        assert_eq!(kit.state.saved.lock().unwrap().len(), 1);
    });
}

#[test]
fn test_forged_rpc_output_cannot_publish_receipt_values_or_history() {
    block_on(async {
        let kit = Testkit::new(WalletType::View).await;
        kit.service.prepare_connection().await.unwrap();
        kit.provider.forged.store(true, Ordering::SeqCst);
        assert_eq!(kit.service.receipt(kit.generation(), &kit.receipt()).await.is_err(), true);
        assert_eq!(kit.balances.saved.lock().unwrap().len(), 0);
        assert_eq!(kit.state.saved.lock().unwrap().len(), 0);
        assert_eq!(kit.status.0.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_wallet_deletion_address_changes_selection_changes_and_cancellation_discard_late_receipts() {
    block_on(async {
        for change in ["deleted", "address", "selection", "cancelled"] {
            let kit = Testkit::new(WalletType::View).await;
            kit.service.prepare_connection().await.unwrap();
            let (started, release) = kit.provider.pause_transaction();
            let receipt = kit.receipt();
            let pending = kit.service.receipt(kit.generation(), &receipt);
            let invalidation = async {
                started.await.unwrap();
                match change {
                    "deleted" => kit.wallets.wallets.lock().unwrap().clear(),
                    "address" => kit.wallets.wallets.lock().unwrap()[0].accounts[0].address = ADDRESS.into(),
                    "selection" => kit.session.set_current_wallet_id(None).unwrap(),
                    "cancelled" => kit.service.cancel(),
                    _ => unreachable!(),
                }
                release.send(()).unwrap();
            };
            let (result, ()) = futures::join!(pending, invalidation);
            assert_eq!(result.unwrap_err(), BsvHomeServiceError::Cancelled);
            assert_eq!(kit.balances.saved.lock().unwrap().len(), 0);
            assert_eq!(kit.state.saved.lock().unwrap().len(), 0);
            assert_eq!(kit.status.0.lock().unwrap().len(), 0);
        }
    });
}

#[test]
fn test_catch_up_preserves_independent_balance_and_history_results_and_reports_failures() {
    block_on(async {
        for balance_fails in [false, true] {
            let kit = Testkit::new(WalletType::View).await;
            kit.service.prepare_connection().await.unwrap();
            kit.provider.indexed.store(true, Ordering::SeqCst);
            let error = BsvHomeServiceError::Store {
                msg: "receipt persistence unavailable".into(),
            };
            if balance_fails {
                *kit.balances.failure.lock().unwrap() = Some(error.clone());
            } else {
                *kit.history.failure.lock().unwrap() = Some(error);
            }
            assert_eq!(kit.service.catch_up(kit.generation()).await.is_err(), true);
            assert_eq!(kit.history.saved.lock().unwrap().is_empty(), !balance_fails);
            assert_eq!(kit.balances.saved.lock().unwrap().is_empty(), balance_fails);
        }
    });
}

#[test]
fn test_worker_cancellation_aborts_an_in_flight_reconnect_catch_up() {
    block_on(async {
        let kit = Testkit::new(WalletType::View).await;
        kit.provider.indexed.store(true, Ordering::SeqCst);
        kit.service.prepare_connection().await.unwrap();
        kit.service.connected().await.unwrap();
        let (started, release) = kit.provider.pause_transaction();
        let worker = kit.service.run();
        let cancellation = async {
            started.await.unwrap();
            kit.service.cancel();
            release.send(()).unwrap();
        };
        let (result, ()) = futures::join!(worker, cancellation);
        assert_eq!(result.unwrap_err(), BsvHomeServiceError::Cancelled);
        assert_eq!(kit.balances.saved.lock().unwrap().len(), 0);
        assert_eq!(kit.history.saved.lock().unwrap().len(), 0);
        assert_eq!(kit.state.saved.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_receipt_retry_waits_for_indexing_and_deadline_drops_blocked_rpc() {
    block_on(async {
        let retry = Testkit::new(WalletType::View).await;
        retry.service.prepare_connection().await.unwrap();
        retry.provider.index_after_empty.store(true, Ordering::SeqCst);
        assert_eq!(retry.service.retry_receipt(retry.generation(), &retry.receipt(), &[0, 0], pending()).await.unwrap(), true);
        assert_eq!(retry.state.saved.lock().unwrap().len(), 1);
        assert_eq!(retry.status.0.lock().unwrap().len(), 1);

        let timeout = Testkit::new(WalletType::View).await;
        timeout.service.prepare_connection().await.unwrap();
        let (started, _release) = timeout.provider.pause_transaction();
        let (expire, deadline) = oneshot::channel();
        let receipt = timeout.receipt();
        let attempt = timeout.service.retry_receipt(timeout.generation(), &receipt, &[0], deadline.map(|_| ()));
        let expiry = async {
            started.await.unwrap();
            expire.send(()).unwrap();
        };
        let (result, ()) = futures::join!(attempt, expiry);
        assert_eq!(result.unwrap(), false);
        assert_eq!(timeout.balances.saved.lock().unwrap().len(), 0);
        assert_eq!(timeout.state.saved.lock().unwrap().len(), 0);
        assert_eq!(timeout.status.0.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_periodic_worker_catches_up_without_socket_events_and_respects_cancellation() {
    block_on(async {
        let kit = Testkit::new(WalletType::View).await;
        kit.service.prepare_connection().await.unwrap();
        kit.provider.indexed.store(true, Ordering::SeqCst);
        let (started, release) = kit.provider.pause_transaction();
        let worker = kit.service.run_worker(kit.generation(), Duration::ZERO);
        let cancellation = async {
            started.await.unwrap();
            kit.service.cancel();
            release.send(()).unwrap();
        };
        let (result, ()) = futures::join!(worker, cancellation);
        assert_eq!(result.unwrap_err(), BsvHomeServiceError::Cancelled);
        assert_eq!(kit.balances.saved.lock().unwrap().len(), 0);
        assert_eq!(kit.history.saved.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_late_catch_up_history_cannot_publish_after_the_current_wallet_changes() {
    block_on(async {
        let kit = Testkit::new(WalletType::View).await;
        kit.service.prepare_connection().await.unwrap();
        kit.provider.indexed.store(true, Ordering::SeqCst);
        let (started, release) = kit.provider.pause_request("/addresses/history/all");
        let catch_up = kit.service.catch_up(kit.generation());
        let switch = async {
            started.await.unwrap();
            kit.session.set_current_wallet_id(None).unwrap();
            release.send(()).unwrap();
        };
        let (result, ()) = futures::join!(catch_up, switch);
        assert_eq!(result.unwrap_err(), BsvHomeServiceError::Cancelled);
        assert_eq!(kit.balances.saved.lock().unwrap().len(), 1);
        assert_eq!(kit.history.saved.lock().unwrap().len(), 0);
        assert_eq!(kit.status.0.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_worker_resync_after_failed_or_nonincoming_receipt_rejects_an_older_display_balance() {
    block_on(async {
        for retry_exhausted in [false, true] {
            let kit = Testkit::new(WalletType::Multicoin).await;
            let (wallet, provider) = kit.prepare_hd(!retry_exhausted);
            let root = wallet.accounts[0].address.clone();
            kit.service.prepare_connection().await.unwrap();
            let subscription: Value = serde_json::from_str(&kit.service.connected().await.unwrap()).unwrap();
            assert_eq!(kit.service.state().unwrap().queue.next(), Some(Work::Resync));
            let script = kit.service.state().unwrap().scripts.iter().find(|(_, address)| address.as_str() == root).unwrap().0.clone();
            let changed = format!(
                r#"{{"version":1,"type":"transaction.changed","network":"bsv-mainnet","requestId":{},"transactionId":"{TEST_BSV_HASH}","scriptHashes":["{script}"]}}"#,
                subscription["requestId"]
            );
            kit.service.receive_event(changed).unwrap();

            let (entered, display_release) = kit.provider.pause_request(&format!("/address/{root}/unspent/all"));
            let mut display = Box::pin(kit.service.addresses.refresh_display_balance(wallet.id.clone()));
            await_captured_request(display.as_mut(), entered).await;
            provider.addresses.add_output(&root, &"3".repeat(64), 5_000, false);
            let (entered, history_release) = kit.provider.pause_request("/addresses/history/all");
            display_release.send(()).unwrap();
            let mut worker = Box::pin(kit.service.run());
            await_captured_request(worker.as_mut(), entered).await;

            let saved = kit.balances.saved.lock().unwrap().clone();
            assert_eq!(saved.len(), 1);
            assert_eq!(saved[0].1[0].pending_unconfirmed.value, 5_000u32.into());
            assert!(kit.state.saved.lock().unwrap().is_empty());
            assert_eq!(
                kit.service.state().unwrap().queue.push(Event::Changed(Receipt {
                    hash: TEST_BSV_HASH.into(),
                    address: root,
                })),
                retry_exhausted
            );
            assert!(kit.password.create_requests.lock().unwrap().is_empty());
            drop(display);
            kit.service.cancel();
            assert_eq!(worker.await.unwrap_err(), BsvHomeServiceError::Cancelled);
            assert!(history_release.send(()).is_err());
        }
    });
}
