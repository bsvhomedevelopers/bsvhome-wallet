use bsv_home_hash::sha2::sha256;

use primitives::{Transaction, Wallet};

use super::BsvHomeBsvColdWalletRole;

use crate::services::{balance::BsvHomeAssetBalance, wallet::addresses::BsvHomeBsvColdAddressState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeOfflineSigningPasswordDomain {
    LegacyCold,
    UnifiedCold,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvColdWalletStateStamp {
    pub revision: u64,
    pub payload_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvColdWalletStoredState {
    pub revision: u64,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvColdWalletReview {
    pub commitment: String,
    pub wallet_id: String,
    pub recipient: String,
    pub amount: u64,
    pub fee: u64,
    pub total_debit: u64,
    pub change: u64,
    pub change_address: Option<String>,
    pub input_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvColdWalletPending {
    pub request_json: String,
    pub review_json: String,
    pub review: BsvHomeBsvColdWalletReview,
    pub signed_json: Option<String>,
    pub transaction_id: Option<String>,
    pub broadcasted: bool,
    pub persistence_error: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvColdWalletSnapshot {
    pub wallet: Wallet,
    pub pairing_json: String,
    pub addresses: BsvHomeBsvColdAddressState,
    pub balance: Option<BsvHomeAssetBalance>,
    pub transactions: Vec<Transaction>,
    pub pending: Option<BsvHomeBsvColdWalletPending>,
    pub role: BsvHomeBsvColdWalletRole,
}

impl BsvHomeBsvColdWalletStoredState {
    pub(crate) fn stamp(&self) -> BsvHomeBsvColdWalletStateStamp {
        BsvHomeBsvColdWalletStateStamp {
            revision: self.revision,
            payload_hash: hex::encode(sha256(self.value.as_bytes())),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvColdWalletCandidate {
    pub wallet: Wallet,
    pub role: BsvHomeBsvColdWalletRole,
}
