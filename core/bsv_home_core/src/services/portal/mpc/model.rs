use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::BsvHomeMpcError;
use crate::services::portal::BsvHomePaymailBindingAvailability;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeMpcParticipant: Send + Sync {
    async fn exchange(&self, wallet_id: String, message_json: String) -> Result<String, BsvHomeMpcError>;
    async fn close(&self, wallet_id: String) -> Result<(), BsvHomeMpcError>;
}

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeMpcDeviceProof: Send + Sync {
    async fn public_key(&self) -> Result<String, BsvHomeMpcError>;
    async fn sign_message(&self, message: String) -> Result<String, BsvHomeMpcError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum BsvHomeMpcWalletAction {
    ExportBackup,
    Send,
    StartRecovery,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum BsvHomeMpcTransferAction {
    Sign,
    ResumeSigning,
    Broadcast,
    RetryBroadcast,
    Refresh,
    Cancel,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum BsvHomeMpcRecoveryAction {
    Refresh,
    Cancel,
    ExportBackup,
    Reconcile,
    SupplyInformation,
    Status,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct BsvHomeMpcWallet {
    pub wallet_id: String,
    pub name: String,
    pub public_key: String,
    pub address: String,
    pub epoch: u64,
    pub status: String,
    pub restored: bool,
    pub balance: Option<BsvHomeMpcBalance>,
    pub actions: Vec<BsvHomeMpcWalletAction>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomeMpcBalance {
    pub state: String,
    pub confirmed_satoshis: Option<u64>,
    pub unconfirmed_satoshis: Option<i64>,
    pub available_confirmed_satoshis: Option<u64>,
    pub reserved_confirmed_satoshis: Option<u64>,
    pub history_state: Option<String>,
    pub synced_at: Option<String>,
    pub source: Option<String>,
}

#[derive(uniffi::Record)]
pub struct BsvHomeMpcBackupMaterial {
    pub wallet_id: String,
    pub owner_id: String,
    pub environment: String,
    pub epoch: u64,
    pub document: String,
    pub recovery_key: String,
    pub commitment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomeMpcTransferReview {
    pub recipient: String,
    pub recipient_handle: Option<String>,
    pub amount: u64,
    pub fee: u64,
    pub change: u64,
    pub digests: Vec<String>,
    pub unsigned_transaction: String,
    pub commitment: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct BsvHomeMpcTransfer {
    pub id: String,
    pub wallet_id: String,
    pub recipient: String,
    pub recipient_handle: Option<String>,
    pub amount: u64,
    pub fee: u64,
    pub status: String,
    pub transaction_id: Option<String>,
    pub confirmations: Option<u32>,
    pub review: BsvHomeMpcTransferReview,
    pub actions: Vec<BsvHomeMpcTransferAction>,
    pub error_code: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomeMpcRecovery {
    pub id: String,
    pub wallet_id: String,
    pub status: String,
    pub source_epoch: u64,
    pub not_before_at: Option<String>,
    pub review_note: Option<String>,
    pub actions: Vec<BsvHomeMpcRecoveryAction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomeMpcRecoveryInformation {
    pub id: String,
    pub wallet_id: String,
    pub status: String,
    pub review_note: Option<String>,
    pub not_before_at: Option<String>,
    pub notification_status: String,
    #[serde(rename = "timeline")]
    pub events: Vec<BsvHomeMpcRecoveryEvent>,
    #[serde(skip)]
    pub can_supply_information: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomeMpcRecoveryEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub time: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub actor_type: Option<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct BsvHomeMpcHistoryItem {
    pub id: String,
    pub wallet_id: String,
    pub transaction_id: String,
    pub received: u64,
    pub spent: u64,
    pub amount: u64,
    pub incoming: bool,
    pub fee: Option<u64>,
    pub direction: String,
    pub status: String,
    pub confirmations: u32,
    pub observed_at: Option<String>,
    pub source: Option<String>,
}

#[derive(uniffi::Record)]
pub struct BsvHomeMpcHistoryPage {
    pub rows: Vec<BsvHomeMpcHistoryItem>,
    pub total: u64,
    pub page: u32,
    pub pending_transfers: Vec<BsvHomeMpcTransfer>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct StoredTransfer {
    pub transfer: BsvHomeMpcTransfer,
    pub public_key: String,
    pub proposal: Value,
    pub signed_transaction: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, uniffi::Record)]
pub struct BsvHomeMpcPendingPreparation {
    pub wallet_id: String,
    pub recipient: String,
    pub target: String,
    pub recipient_handle: Option<String>,
    pub amount: u64,
    pub key: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct BackupProof {
    pub wallet_id: String,
    pub epoch: u64,
    pub commitment: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Task {
    pub id: String,
    pub wallet_id: String,
    pub kind: String,
    pub epoch: u64,
    pub context: Value,
    pub action_hash: String,
    pub status: String,
}

#[derive(Default, Serialize, Deserialize)]
pub(super) struct StoredState {
    pub wallets: Vec<BsvHomeMpcWallet>,
    pub transfers: Vec<StoredTransfer>,
    pub pending_preparation: Option<BsvHomeMpcPendingPreparation>,
    pub backup: Option<BackupProof>,
    pub active_task: Option<Task>,
    pub pending_task_request: Option<Value>,
    pub recovery: Option<BsvHomeMpcRecovery>,
}

#[derive(Clone, uniffi::Record)]
pub struct BsvHomeMpcPaymailBinding {
    pub availability: BsvHomePaymailBindingAvailability,
    pub handle: String,
    pub address: String,
    pub identity_public_key: String,
    pub active: bool,
}
#[derive(uniffi::Record)]
pub struct BsvHomeMpcPaymailSnapshot {
    pub wallet_id: String,
    pub domains: Vec<String>,
    pub public_base_url: String,
    pub bindings: Vec<BsvHomeMpcPaymailBinding>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeMpcPendingTaskState {
    KnownTask,
    PrepareReceiptUnknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeMpcPendingTaskAction {
    Reconcile,
    Cancel,
}
#[derive(uniffi::Record)]
pub struct BsvHomeMpcPendingTask {
    pub kind: String,
    pub id: Option<String>,
    pub wallet_id: Option<String>,
    pub status: Option<String>,
    pub state: BsvHomeMpcPendingTaskState,
    pub actions: Vec<BsvHomeMpcPendingTaskAction>,
}
