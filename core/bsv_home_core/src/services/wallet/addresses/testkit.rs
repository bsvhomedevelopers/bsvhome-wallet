use std::collections::HashMap;
use std::mem::take;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bsv_home_bsv::BsvAddress;
use futures::channel::oneshot;
use primitives::{Address, Transaction, Wallet, WalletId};
use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;

use super::state::PoolState;
use super::{BsvHomeBsvAddressPool, BsvHomeBsvAddressStore, BsvHomeBsvAddressStoredState, BsvHomeBsvDiscoveryState, BsvHomeBsvFundingStoredState};
use crate::alien::{AlienError, AlienProvider, AlienResponse, AlienTarget};
use crate::gateway::BsvHomeGateway;
use crate::keystore::BsvHomeKeystore;
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::testkit::{MemoryKeystorePassword, MemoryWalletStore};
use crate::services::wallet::{BsvHomeKeystorePassword, BsvHomeWalletStore};

#[derive(Default)]
pub struct MemoryBsvAddressStore {
    pub states: Mutex<HashMap<WalletId, BsvHomeBsvAddressStoredState>>,
    pub pending: Mutex<HashMap<WalletId, Vec<Transaction>>>,
    pub funding_states: Mutex<Vec<BsvHomeBsvFundingStoredState>>,
    pub funding_error: Mutex<Option<BsvHomeServiceError>>,
    pub read_error: Mutex<Option<BsvHomeServiceError>>,
    pub save_error: Mutex<Option<BsvHomeServiceError>>,
    pub conflict_on_save: Mutex<bool>,
    pub hold_next_save: Mutex<bool>,
    held_saves: Mutex<Vec<HeldAddressWrite>>,
    directories: Mutex<Vec<TempDir>>,
}

struct HeldAddressWrite {
    wallet_id: WalletId,
    expected_revision: Option<u64>,
    state: BsvHomeBsvAddressStoredState,
    result: oneshot::Sender<Result<bool, BsvHomeServiceError>>,
}

impl MemoryBsvAddressStore {
    pub fn held_save_count(&self) -> usize {
        self.held_saves.lock().unwrap().len()
    }
    pub fn complete_held_save(&self) -> Result<bool, BsvHomeServiceError> {
        let write = self.held_saves.lock().unwrap().remove(0);
        let result = self.commit(write.wallet_id, write.expected_revision, write.state);
        let _ = write.result.send(result.clone());
        result
    }
    fn commit(&self, wallet_id: WalletId, expected_revision: Option<u64>, state: BsvHomeBsvAddressStoredState) -> Result<bool, BsvHomeServiceError> {
        if let Some(error) = self.save_error.lock().unwrap().clone() {
            return Err(error);
        }
        let mut states = self.states.lock().unwrap();
        if take(&mut self.conflict_on_save.lock().unwrap()) {
            if let Some(saved) = states.get_mut(&wallet_id) {
                saved.revision += 1;
            }
        }
        if states.get(&wallet_id).map(|state| state.revision) != expected_revision {
            return Ok(false);
        }
        states.insert(wallet_id, state);
        Ok(true)
    }
}

#[async_trait]
impl BsvHomeBsvAddressStore for MemoryBsvAddressStore {
    async fn get(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvAddressStoredState>, BsvHomeServiceError> {
        if let Some(error) = self.read_error.lock().unwrap().clone() {
            return Err(error);
        }
        Ok(self.states.lock().unwrap().get(&wallet_id).cloned())
    }

    async fn save(&self, wallet_id: WalletId, expected_revision: Option<u64>, state: BsvHomeBsvAddressStoredState) -> Result<bool, BsvHomeServiceError> {
        if let Some(error) = self.save_error.lock().unwrap().clone() {
            return Err(error);
        }
        if take(&mut self.hold_next_save.lock().unwrap()) {
            let (result, response) = oneshot::channel();
            self.held_saves.lock().unwrap().push(HeldAddressWrite {
                wallet_id,
                expected_revision,
                state,
                result,
            });
            return response.await.map_err(|_| BsvHomeServiceError::Store {
                msg: "held address write lost its response".into(),
            })?;
        }
        self.commit(wallet_id, expected_revision, state)
    }

    async fn unresolved_transactions(&self, wallet_id: WalletId) -> Result<Vec<Transaction>, BsvHomeServiceError> {
        Ok(self.pending.lock().unwrap().get(&wallet_id).cloned().unwrap_or_default())
    }

    async fn funding_states(&self) -> Result<Vec<BsvHomeBsvFundingStoredState>, BsvHomeServiceError> {
        if let Some(error) = self.funding_error.lock().unwrap().clone() {
            return Err(error);
        }
        Ok(self.funding_states.lock().unwrap().clone())
    }
}

pub fn pool(gateway: Arc<BsvHomeGateway>, wallets: Arc<MemoryWalletStore>) -> Arc<BsvHomeBsvAddressPool> {
    let directory = TempDir::new().unwrap();
    let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
    let store = wallets.bsv_addresses.clone();
    store.directories.lock().unwrap().push(directory);
    pool_with_keystore(gateway, wallets, keystore, Arc::new(MemoryKeystorePassword::default()), store)
}

pub fn pool_with_keystore(
    gateway: Arc<BsvHomeGateway>,
    wallets: Arc<dyn BsvHomeWalletStore>,
    keystore: Arc<BsvHomeKeystore>,
    password: Arc<dyn BsvHomeKeystorePassword>,
    store: Arc<dyn BsvHomeBsvAddressStore>,
) -> Arc<BsvHomeBsvAddressPool> {
    Arc::new(BsvHomeBsvAddressPool::new(gateway, wallets, store, keystore, password))
}

pub fn complete_record(wallet: &Wallet, words: &str) -> BsvHomeBsvAddressStoredState {
    let mut record = BsvHomeBsvAddressPool::initial_record(wallet, words).unwrap().unwrap();
    let mut state: PoolState = serde_json::from_str(&record.value).unwrap();
    for cursor in [&mut state.receive, &mut state.change] {
        cursor.next_index = 20;
        cursor.empty_count = 20;
        cursor.stopped = true;
    }
    state.discovery = BsvHomeBsvDiscoveryState::Complete;
    record.value = serde_json::to_string(&state).unwrap();
    record
}

#[derive(Clone, Serialize)]
struct TestOutput {
    tx_hash: String,
    tx_pos: u32,
    value: u64,
    height: Option<u64>,
    status: &'static str,
    #[serde(rename = "isSpentInMempoolTx")]
    spent_in_mempool: bool,
}

#[derive(Serialize)]
struct OutputResponse<'a> {
    address: &'a str,
    result: Vec<TestOutput>,
    error: &'static str,
}

#[derive(Serialize)]
struct HistoryResponse<'a> {
    address: &'a str,
    confirmed: HistorySection,
    unconfirmed: HistorySection,
}

#[derive(Serialize, Default)]
struct HistorySection {
    result: Vec<HistoryEntry>,
    error: &'static str,
}

#[derive(Serialize)]
struct HistoryEntry {
    tx_hash: String,
    height: Option<u64>,
}

#[derive(Default)]
pub struct BsvAddressProvider {
    outputs: Mutex<HashMap<String, Vec<TestOutput>>>,
    histories: Mutex<HashMap<String, Vec<TestOutput>>>,
    transactions: Mutex<HashMap<String, Vec<u8>>>,
    pub requested: Mutex<Vec<String>>,
    pub history_error: Mutex<Option<&'static str>>,
    pub revision_change: Mutex<Option<(Arc<MemoryBsvAddressStore>, WalletId)>>,
}

impl std::fmt::Debug for BsvAddressProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("BsvAddressProvider").finish()
    }
}

impl BsvAddressProvider {
    pub fn add_output(&self, address: &str, transaction_id: &str, value: u64, confirmed: bool) {
        let output = TestOutput {
            tx_hash: transaction_id.into(),
            tx_pos: 0,
            value,
            height: if confirmed { Some(100) } else { None },
            status: if confirmed { "confirmed" } else { "unconfirmed" },
            spent_in_mempool: false,
        };
        self.outputs.lock().unwrap().entry(address.into()).or_default().push(output.clone());
        self.histories.lock().unwrap().entry(address.into()).or_default().push(output);
        let mut transaction: Value = serde_json::from_str(include_str!("../../../../../crates/bsv_home_bsv/testdata/wallet_spent_activity.json")).unwrap();
        transaction["txid"] = transaction_id.into();
        transaction["vin"][0]["txid"] = "f".repeat(64).into();
        transaction["vin"][0]["vout"] = self.transactions.lock().unwrap().len().into();
        transaction["confirmations"] = if confirmed { 1.into() } else { 0.into() };
        transaction["blockheight"] = if confirmed { 100.into() } else { Value::Null };
        let amount = format!("{}.{:08}", value / 100_000_000, value % 100_000_000);
        transaction["vout"][0]["value"] = Value::Number(amount.parse().unwrap());
        transaction["vout"][0]["scriptPubKey"]["hex"] = hex::encode(BsvAddress::from_str(address).unwrap().script_pubkey().as_bytes()).into();
        self.transactions.lock().unwrap().insert(transaction_id.into(), serde_json::to_vec(&transaction).unwrap());
    }

    pub fn add_raw_transaction(&self, raw: &str, confirmed: bool) -> String {
        let transaction: bitcoin::Transaction = bitcoin::consensus::deserialize(&hex::decode(raw).unwrap()).unwrap();
        let transaction_id = transaction.compute_txid().to_string();
        let inputs = if transaction.is_coinbase() {
            vec![serde_json::json!({"coinbase": hex::encode(transaction.input[0].script_sig.as_bytes())})]
        } else {
            transaction
                .input
                .iter()
                .map(|input| serde_json::json!({"txid": input.previous_output.txid.to_string(), "vout": input.previous_output.vout}))
                .collect()
        };
        let outputs = transaction
            .output
            .iter()
            .enumerate()
            .map(|(index, output)| {
                let value = output.value.to_sat();
                let value: serde_json::Number = format!("{}.{:08}", value / 100_000_000, value % 100_000_000).parse().unwrap();
                serde_json::json!({"n": index, "value": value, "scriptPubKey": {"hex": hex::encode(output.script_pubkey.as_bytes()), "isTruncated": false}})
            })
            .collect::<Vec<_>>();
        let info = serde_json::json!({
            "txid": transaction_id,
            "confirmations": if confirmed { 1 } else { 0 },
            "blockheight": if confirmed { Some(100) } else { None },
            "blocktime": if confirmed { Some(1_800_000_000) } else { None },
            "time": 1_800_000_000,
            "vin": inputs,
            "vout": outputs,
        });
        self.transactions.lock().unwrap().insert(transaction_id.clone(), serde_json::to_vec(&info).unwrap());
        transaction_id
    }

    pub fn remove_output(&self, address: &str, transaction_id: &str) {
        self.outputs
            .lock()
            .unwrap()
            .entry(address.into())
            .or_default()
            .retain(|output| output.tx_hash != transaction_id);
    }

    pub fn remove_history(&self, address: &str, transaction_id: &str) {
        self.histories
            .lock()
            .unwrap()
            .entry(address.into())
            .or_default()
            .retain(|output| output.tx_hash != transaction_id);
    }
}

#[async_trait]
impl AlienProvider for BsvAddressProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        let path = target.url.strip_prefix("https://api.whatsonchain.com/v1/bsv/main").unwrap();
        self.requested.lock().unwrap().push(path.to_string());
        if let Some((store, wallet_id)) = self.revision_change.lock().unwrap().take() {
            store.states.lock().unwrap().get_mut(&wallet_id).unwrap().revision += 1;
        }
        let body = if path == "/addresses/history/all" {
            let request: Value = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
            let responses = request["addresses"]
                .as_array()
                .unwrap()
                .iter()
                .map(|address| {
                    let address = address.as_str().unwrap();
                    let mut confirmed = HistorySection::default();
                    let mut unconfirmed = HistorySection {
                        error: self.history_error.lock().unwrap().unwrap_or(""),
                        ..Default::default()
                    };
                    for output in self.histories.lock().unwrap().get(address).cloned().unwrap_or_default() {
                        let entry = HistoryEntry {
                            tx_hash: output.tx_hash,
                            height: output.height,
                        };
                        if output.height.is_some() {
                            confirmed.result.push(entry);
                        } else {
                            unconfirmed.result.push(entry);
                        }
                    }
                    HistoryResponse { address, confirmed, unconfirmed }
                })
                .collect::<Vec<_>>();
            serde_json::to_vec(&responses).unwrap()
        } else if let Some(transaction_id) = path.strip_prefix("/tx/hash/") {
            self.transactions.lock().unwrap().get(transaction_id).unwrap().clone()
        } else {
            let address = path.strip_prefix("/address/").unwrap().strip_suffix("/unspent/all").unwrap();
            let result = self.outputs.lock().unwrap().get(address).cloned().unwrap_or_default();
            serde_json::to_vec(&OutputResponse { address, result, error: "" }).unwrap()
        };
        Ok(Arc::new(AlienResponse::new(Some(200), body)))
    }
}
