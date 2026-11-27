use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use async_trait::async_trait;
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness, absolute::LockTime, consensus::serialize, transaction::Version};
use bsv_home_bsv::{
    BsvAddress,
    cold_wallet::{Pairing, Request},
};
use futures::channel::oneshot;
use primitives::{Address, UTXO, Wallet, WalletId, testkit::ABANDON_PHRASE};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::password::{BsvHomeOfflineSigningPasswordService, testkit::MemoryOfflinePasswordStore};
use super::{
    BsvHomeBsvColdWalletActionIntent, BsvHomeBsvColdWalletActionScope, BsvHomeBsvColdWalletPending, BsvHomeBsvColdWalletService, BsvHomeBsvColdWalletSigner,
    BsvHomeBsvColdWalletStateStamp, BsvHomeBsvColdWalletStore, BsvHomeBsvColdWalletStoredState,
};
use crate::alien::{AlienError, AlienProvider, AlienResponse, AlienTarget};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::keystore::{BsvHomeKeystore, decode_password, keystore_id_for_wallet};
use crate::services::error::BsvHomeServiceError;
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::file::testkit::NoopFileStore;
use crate::services::localization::testkit::EnglishLocalizer;
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::wallet::{
    BsvHomeWalletService, BsvHomeWalletStore,
    addresses::{BsvHomeBsvAddressPool, testkit::pool_with_keystore},
    testkit::{MemoryAddressStore, MemoryKeystorePassword, MemoryWalletStore, TEST_PASSWORD},
};
use crate::services::wallet_preferences::{BsvHomeWalletPreferencesService, testkit::MemoryWalletPreferencesStore};
use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
use crate::testkit::TestAlienProvider;

#[derive(Debug, Default)]
pub(crate) struct MemoryColdWalletStore {
    pub records: Mutex<HashMap<WalletId, BsvHomeBsvColdWalletStoredState>>,
    pub fail: Mutex<bool>,
    pub read_fail: Mutex<bool>,
    pub writes: Mutex<u32>,
}

#[async_trait]
impl BsvHomeBsvColdWalletStore for MemoryColdWalletStore {
    fn state_stamp(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvColdWalletStateStamp>, BsvHomeServiceError> {
        self.check_read()?;
        Ok(self.records.lock().unwrap().get(&wallet_id).map(BsvHomeBsvColdWalletStoredState::stamp))
    }
    fn has_state(&self, wallet_id: WalletId) -> Result<bool, BsvHomeServiceError> {
        self.check_read()?;
        Ok(self.records.lock().unwrap().contains_key(&wallet_id))
    }
    async fn get(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvColdWalletStoredState>, BsvHomeServiceError> {
        self.check_read()?;
        Ok(self.records.lock().unwrap().get(&wallet_id).cloned())
    }
    async fn compare_and_swap(&self, wallet_id: WalletId, revision: Option<u64>, state: BsvHomeBsvColdWalletStoredState) -> Result<bool, BsvHomeServiceError> {
        if *self.fail.lock().unwrap() {
            return Err(BsvHomeServiceError::Store {
                msg: "injected write failure".into(),
            });
        }
        let mut records = self.records.lock().unwrap();
        if records.get(&wallet_id).map(|record| record.revision) != revision {
            return Ok(false);
        }
        records.insert(wallet_id, state);
        *self.writes.lock().unwrap() += 1;
        Ok(true)
    }
}

impl MemoryColdWalletStore {
    fn check_read(&self) -> Result<(), BsvHomeServiceError> {
        if *self.read_fail.lock().unwrap() {
            return Err(BsvHomeServiceError::Store {
                msg: "fixture cold state read failed".into(),
            });
        }
        Ok(())
    }
}

pub(crate) struct KeystoreColdWalletSigner {
    pub keystore: Arc<BsvHomeKeystore>,
    pub session: Arc<BsvHomeWalletSessionService>,
    pub switch_to: Mutex<Option<WalletId>>,
    pub password_store: Arc<MemoryOfflinePasswordStore>,
    pub authorization_count: Mutex<u32>,
    pub skip_authorization: Mutex<bool>,
    pub authentication_count: Mutex<u32>,
    pub signing_count: Mutex<u32>,
    pub reject: Mutex<bool>,
    pub authentication_wait: Mutex<Option<oneshot::Receiver<()>>>,
    pub authentication_entered: Mutex<Option<oneshot::Sender<()>>>,
    pub signing_wait: Mutex<Option<oneshot::Receiver<()>>>,
    pub signing_entered: Mutex<Option<oneshot::Sender<()>>>,
}

#[async_trait]
impl BsvHomeBsvColdWalletSigner for KeystoreColdWalletSigner {
    async fn authenticate_import(&self) -> Result<(), BsvHomeServiceError> {
        *self.authentication_count.lock().unwrap() += 1;
        if *self.reject.lock().unwrap() {
            return Err(BsvHomeServiceError::Cancelled);
        }
        if let Some(entered) = self.authentication_entered.lock().unwrap().take() {
            entered.send(()).unwrap();
        }
        let wait = self.authentication_wait.lock().unwrap().take();
        if let Some(wait) = wait {
            wait.await.map_err(|_| BsvHomeServiceError::Cancelled)?;
        }
        Ok(())
    }
    async fn authorize(&self, _wallet: Wallet, scope: Arc<BsvHomeBsvColdWalletActionScope>) -> Result<(), BsvHomeServiceError> {
        *self.authorization_count.lock().unwrap() += 1;
        if *self.skip_authorization.lock().unwrap() {
            return Ok(());
        }
        let password = BsvHomeOfflineSigningPasswordService::for_scope(self.password_store.clone(), scope).map_err(|_| BsvHomeServiceError::InvalidInput {
            msg: "fixture offline authorization unavailable".into(),
        })?;
        let snapshot = password.status().await.map_err(|_| BsvHomeServiceError::InvalidInput {
            msg: "fixture offline authorization unavailable".into(),
        })?;
        if snapshot.configured {
            password.verify(snapshot, TEST_PASSWORD.into()).await.map_err(|_| BsvHomeServiceError::InvalidInput {
                msg: "fixture offline authorization rejected".into(),
            })?;
        } else {
            password
                .set(snapshot, None, TEST_PASSWORD.into(), TEST_PASSWORD.into())
                .await
                .map_err(|_| BsvHomeServiceError::InvalidInput {
                    msg: "fixture offline authorization rejected".into(),
                })?;
        }
        Ok(())
    }
    async fn authenticate(&self, _wallet: Wallet, scope: Arc<BsvHomeBsvColdWalletActionScope>) -> Result<(), BsvHomeServiceError> {
        scope.check()?;
        self.authenticate_import().await?;
        scope.check()?;
        if let Some(wallet_id) = self.switch_to.lock().unwrap().take() {
            self.session.set_current_wallet_id(Some(wallet_id))?;
        }
        Ok(())
    }
    async fn sign_request(&self, wallet: Wallet, request_json: String, review_json: String, scope: Arc<BsvHomeBsvColdWalletActionScope>) -> Result<String, BsvHomeServiceError> {
        if let Some(entered) = self.signing_entered.lock().unwrap().take() {
            entered.send(()).unwrap();
        }
        let wait = self.signing_wait.lock().unwrap().take();
        if let Some(wait) = wait {
            wait.await.map_err(|_| BsvHomeServiceError::Cancelled)?;
        }
        scope.check_signing(wallet.clone(), request_json.clone(), review_json.clone())?;
        *self.signing_count.lock().unwrap() += 1;
        Ok(self
            .keystore
            .bsv_cold_sign(keystore_id_for_wallet(wallet.id.id()), request_json, review_json, decode_password(TEST_PASSWORD))?)
    }
}

pub(crate) struct Context {
    pub service: BsvHomeBsvColdWalletService,
    pub wallets: Arc<BsvHomeWalletService>,
    pub wallet_store: Arc<MemoryWalletStore>,
    pub addresses: Arc<BsvHomeBsvAddressPool>,
    pub keystore: Arc<BsvHomeKeystore>,
    pub store: Arc<MemoryColdWalletStore>,
    pub signer: Arc<KeystoreColdWalletSigner>,
    pub provider: Arc<TestAlienProvider>,
    pub _directory: TempDir,
}

impl Context {
    pub fn new() -> Self {
        Self::with_wallet_store(|wallets| wallets)
    }

    pub fn with_wallet_store(make_store: impl FnOnce(Arc<MemoryWalletStore>) -> Arc<dyn BsvHomeWalletStore>) -> Self {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let wallets = Arc::new(MemoryWalletStore::default());
        let wallet_store = make_store(wallets.clone());
        let password = Arc::new(MemoryKeystorePassword::default());
        let preferences = Arc::new(MemoryPreferencesStore::default());
        let session = Arc::new(BsvHomeWalletSessionService::new(Arc::new(MemoryWalletSessionStore::default()), wallet_store.clone()));
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let gateway = Arc::new(BsvHomeGateway::new(provider.clone(), preferences.clone(), Arc::new(EmptyPreferences)));
        let addresses = pool_with_keystore(gateway, wallet_store.clone(), keystore.clone(), password.clone(), wallets.bsv_addresses.clone());
        let app_preferences = Arc::new(BsvHomePreferencesService::new(preferences));
        let wallet_service = Arc::new(BsvHomeWalletService::new(
            keystore.clone(),
            password,
            wallet_store,
            session.clone(),
            app_preferences.clone(),
            Arc::new(NoopFileStore),
            Arc::new(BsvHomeWalletPreferencesService::new(Arc::new(MemoryWalletPreferencesStore::default()))),
            Arc::new(BsvHomeExplorerService::new(app_preferences)),
            Arc::new(MemoryAddressStore::default()),
            Arc::new(EnglishLocalizer),
            addresses.clone(),
        ));
        let store = Arc::new(MemoryColdWalletStore::default());
        let signer = Arc::new(KeystoreColdWalletSigner {
            keystore: keystore.clone(),
            session,
            switch_to: Mutex::new(None),
            password_store: Arc::new(MemoryOfflinePasswordStore::default()),
            authorization_count: Mutex::new(0),
            skip_authorization: Mutex::new(false),
            authentication_count: Mutex::new(0),
            signing_count: Mutex::new(0),
            reject: Mutex::new(false),
            authentication_wait: Mutex::new(None),
            authentication_entered: Mutex::new(None),
            signing_wait: Mutex::new(None),
            signing_entered: Mutex::new(None),
        });
        let service = BsvHomeBsvColdWalletService::offline(wallet_service.clone(), addresses.clone(), keystore.clone(), store.clone(), signer.clone());
        Self {
            service,
            wallets: wallet_service,
            wallet_store: wallets,
            addresses,
            keystore,
            store,
            signer,
            provider,
            _directory: directory,
        }
    }
    pub fn restored(&self) -> BsvHomeBsvColdWalletService {
        BsvHomeBsvColdWalletService::offline(self.wallets.clone(), self.addresses.clone(), self.keystore.clone(), self.store.clone(), self.signer.clone())
    }
    pub async fn create(&self) -> Wallet {
        self.service
            .create(
                "Offline".into(),
                ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                self.service.capture_generation(),
            )
            .await
            .unwrap()
            .wallet
    }
    pub async fn request(&self, wallet_id: WalletId, amount: u64) -> String {
        let pairing: Pairing = serde_json::from_str(&self.service.pairing(wallet_id).await.unwrap()).unwrap();
        let account = pairing.account().unwrap();
        let addresses = (0..2).map(|index| pairing.address(0, index).unwrap()).collect::<Vec<_>>();
        let previous = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::from_bytes(vec![1, 7]),
                sequence: Sequence::MAX,
                witness: Witness::default(),
            }],
            output: addresses
                .iter()
                .map(|address| TxOut {
                    value: Amount::from_sat(50000),
                    script_pubkey: BsvAddress::from_str(&address.address).unwrap().script_pubkey(),
                })
                .collect(),
        };
        let utxos = addresses
            .iter()
            .enumerate()
            .map(|(index, address)| UTXO {
                transaction_id: previous.compute_txid().to_string(),
                vout: index as i32,
                value: 50000u64.into(),
                address: address.address.clone(),
            })
            .collect();
        let mut request = Request::prepare(&account, pairing.address(0, 9).unwrap().address, amount, 1000, utxos, addresses, 0).unwrap();
        for input in &mut request.inputs {
            input.previous_transaction = hex::encode(serialize(&previous));
        }
        serde_json::to_string(&request).unwrap()
    }
}

#[derive(Debug)]
pub(crate) struct ColdWalletProvider {
    pub pairing: Pairing,
    pub previous_raw: String,
    pub previous_id: String,
    pub accepted_raw: Mutex<Option<String>>,
    pub broadcasted: Mutex<Vec<String>>,
    pub fail_store_after_accepted: Mutex<Option<Arc<MemoryColdWalletStore>>>,
    pub cancel_after_accepted: Mutex<Option<Weak<BsvHomeBsvColdWalletActionScope>>>,
}

#[async_trait]
impl AlienProvider for ColdWalletProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        let path = target.url.strip_prefix("https://api.whatsonchain.com/v1/bsv/main").unwrap();
        let (status, body) = if path == "/chain/info" {
            (200, "{\"chain\":\"main\",\"blocks\":100,\"headers\":100}".as_bytes().to_vec())
        } else if path == "/feerecommendation" {
            (200, "{\"fee_unit\":\"sat/KB\",\"fee\":1000,\"mempool_min_fee\":1000}".as_bytes().to_vec())
        } else if path == "/addresses/history/all" {
            let request: Value = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
            let responses = request["addresses"]
                .as_array()
                .unwrap()
                .iter()
                .map(|address| {
                    let address = address.as_str().unwrap();
                    let history = if (0..2).any(|index| self.pairing.address(0, index).unwrap().address == address) {
                        vec![json!({"tx_hash": self.previous_id, "height": 100})]
                    } else {
                        Vec::new()
                    };
                    json!({"address": address, "confirmed": {"result": history, "error": ""}, "unconfirmed": {"result": [], "error": ""}})
                })
                .collect::<Vec<_>>();
            (200, serde_json::to_vec(&responses).unwrap())
        } else if let Some(address) = path.strip_prefix("/address/").and_then(|path| path.strip_suffix("/unspent/all")) {
            let index = (0..2).find(|index| self.pairing.address(0, *index).unwrap().address == address);
            let result = index
                .map(|index| {
                    format!(
                        "[{{\"tx_hash\":\"{}\",\"tx_pos\":{index},\"value\":50000,\"height\":100,\"status\":\"confirmed\",\"isSpentInMempoolTx\":false}}]",
                        self.previous_id
                    )
                })
                .unwrap_or_else(|| "[]".into());
            (200, format!("{{\"address\":\"{address}\",\"result\":{result},\"error\":\"\"}}").into_bytes())
        } else if path == format!("/tx/hash/{}", self.previous_id) {
            let mut info: Value = serde_json::from_str(include_str!("../../../../crates/bsv_home_bsv/testdata/wallet_spent_activity.json")).unwrap();
            info["txid"] = self.previous_id.clone().into();
            info["vout"] = Value::Array(
                (0..2)
                    .map(|index| {
                        let script = hex::encode(BsvAddress::from_str(&self.pairing.address(0, index).unwrap().address).unwrap().script_pubkey().as_bytes());
                        json!({"n":index,"value":0.0005,"scriptPubKey":{"hex":script}})
                    })
                    .collect(),
            );
            (200, serde_json::to_vec(&info).unwrap())
        } else if path == "/tx/raw" {
            let request: Value = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
            self.broadcasted.lock().unwrap().push(request["txhex"].as_str().unwrap().to_string());
            (503, Vec::new())
        } else if path == format!("/tx/{}/hex", self.previous_id) {
            (200, self.previous_raw.as_bytes().to_vec())
        } else if path.starts_with("/tx/") && path.ends_with("/hex") {
            match self.accepted_raw.lock().unwrap().clone() {
                Some(raw) => {
                    if let Some(store) = self.fail_store_after_accepted.lock().unwrap().take() {
                        *store.fail.lock().unwrap() = true;
                    }
                    if let Some(scope) = self.cancel_after_accepted.lock().unwrap().take().and_then(|scope| scope.upgrade()) {
                        scope.cancel();
                    }
                    (200, raw.into_bytes())
                }
                None => (404, Vec::new()),
            }
        } else {
            panic!("unexpected fixture request: {path}")
        };
        Ok(Arc::new(AlienResponse::new(Some(status), body)))
    }
}

pub(crate) async fn sign_action(service: &BsvHomeBsvColdWalletService, wallet_id: WalletId, commitment: String) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
    let scope = service.begin_action(wallet_id.clone(), BsvHomeBsvColdWalletActionIntent::Sign { commitment: commitment.clone() })?;
    service.sign(wallet_id, commitment, scope).await
}

pub(crate) async fn broadcast_action(service: &BsvHomeBsvColdWalletService, wallet_id: WalletId, commitment: String) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
    let scope = service.begin_action(wallet_id.clone(), BsvHomeBsvColdWalletActionIntent::Broadcast { commitment: commitment.clone() })?;
    service.broadcast(wallet_id, commitment, scope).await
}

pub(crate) async fn prepare_action(
    service: &BsvHomeBsvColdWalletService,
    wallet_id: WalletId,
    recipient: String,
    amount: u64,
) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
    let scope = service.begin_action(
        wallet_id.clone(),
        BsvHomeBsvColdWalletActionIntent::Prepare {
            recipient: recipient.clone(),
            amount,
        },
    )?;
    service.prepare(wallet_id, recipient, amount, scope).await
}
