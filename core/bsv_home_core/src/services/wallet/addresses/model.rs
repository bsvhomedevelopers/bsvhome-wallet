use async_trait::async_trait;
use primitives::{BsvAddressBranch, Transaction, WalletId};
use serde::{Deserialize, Serialize};

use crate::services::error::BsvHomeServiceError;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvAddressStoredState {
    pub revision: u64,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvFundingStoredState {
    pub identifier: String,
    pub revision: u64,
    pub json: String,
}

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeBsvAddressStore: Send + Sync {
    async fn get(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvAddressStoredState>, BsvHomeServiceError>;
    async fn save(&self, wallet_id: WalletId, expected_revision: Option<u64>, state: BsvHomeBsvAddressStoredState) -> Result<bool, BsvHomeServiceError>;
    async fn unresolved_transactions(&self, wallet_id: WalletId) -> Result<Vec<Transaction>, BsvHomeServiceError>;
    async fn funding_states(&self) -> Result<Vec<BsvHomeBsvFundingStoredState>, BsvHomeServiceError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum BsvHomeBsvAddressStatus {
    Unused,
    Used,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum BsvHomeBsvDiscoveryState {
    NotStarted,
    Incomplete,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvAddressRecord {
    pub branch: BsvAddressBranch,
    pub index: u32,
    pub address: String,
    pub status: BsvHomeBsvAddressStatus,
    pub is_current: bool,
    pub is_main: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvAddressSnapshot {
    pub records: Vec<BsvHomeBsvAddressRecord>,
    pub current_address: Option<String>,
    pub receive_scanned: u32,
    pub change_scanned: u32,
    pub discovery: BsvHomeBsvDiscoveryState,
    pub allocation_paused: bool,
    pub receive_count: u32,
    pub selected_index: Option<u32>,
    pub can_add: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvColdAddressState {
    pub addresses: Vec<BsvHomeBsvAddressRecord>,
    pub selected_index: Option<u32>,
    pub can_add: bool,
}

impl From<BsvHomeBsvAddressSnapshot> for BsvHomeBsvColdAddressState {
    fn from(snapshot: BsvHomeBsvAddressSnapshot) -> Self {
        Self {
            addresses: snapshot
                .records
                .into_iter()
                .filter(|record| record.branch == BsvAddressBranch::Receive && record.index < snapshot.receive_count)
                .collect(),
            selected_index: snapshot.selected_index,
            can_add: snapshot.can_add,
        }
    }
}
