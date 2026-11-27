use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize};

use super::BsvHomePlatformAccountError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomePlatformPasskeyOperation {
    Register,
    Login,
    StepUp,
}

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomePlatformAuthenticator: Send + Sync {
    async fn authenticate(&self, operation: BsvHomePlatformPasskeyOperation, options_json: String) -> Result<String, BsvHomePlatformAccountError>;
    fn verify_offline_proof(&self, public_key: String, action_hash: String, signature: String) -> Result<bool, BsvHomePlatformAccountError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum BsvHomePlatformAuthMethod {
    #[serde(rename = "EMAIL_OTP")]
    EmailOtp,
    #[serde(rename = "PASSKEY_UV")]
    PasskeyUv,
    #[serde(rename = "ACCOUNT_REOPEN_PASSKEY_EMAIL")]
    AccountReopenPasskeyEmail,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformSession {
    #[serde(deserialize_with = "user_identifier")]
    pub user_id: String,
    pub expires_at: String,
    pub auth_method: BsvHomePlatformAuthMethod,
    pub wallet_authority: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformEmailChallenge {
    #[serde(rename = "challengeId")]
    pub id: String,
    pub expires_at: String,
    pub retry_after_seconds: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformPasskey {
    pub id: String,
    pub label: String,
    pub created_at: String,
    pub backup_eligible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformProfile {
    #[serde(deserialize_with = "user_identifier")]
    pub user_id: String,
    pub email: String,
    pub nickname: Option<String>,
    pub language: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformDevice {
    pub id: String,
    pub device_type: String,
    pub last_seen_at: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomePlatformAccountSnapshot {
    pub session: BsvHomePlatformSession,
    pub profile: BsvHomePlatformProfile,
    pub passkeys: Vec<BsvHomePlatformPasskey>,
    pub devices: Vec<BsvHomePlatformDevice>,
    pub can_revoke_passkey: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, uniffi::Enum)]
pub enum BsvHomePlatformClosureStatus {
    #[serde(rename = "WAITING")]
    Waiting,
    #[serde(rename = "CANCELLED")]
    Cancelled,
    #[serde(rename = "CLOSED")]
    Closed,
    #[serde(rename = "REOPENED")]
    Reopened,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformClosure {
    pub id: String,
    pub status: BsvHomePlatformClosureStatus,
    pub ready_at: Option<String>,
    pub policy_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformHdAccount {
    pub id: String,
    pub label: String,
    pub base_path: String,
    pub root_wallet_id: Option<String>,
    pub next_external: u32,
    pub next_internal: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformHdAddress {
    pub wallet_id: String,
    pub branch: u32,
    pub index: u32,
    pub address: String,
    pub public_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformHdBalance {
    pub state: String,
    pub history_state: String,
    pub confirmed_satoshis: Option<String>,
    pub unconfirmed_satoshis: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomePlatformHdSnapshot {
    pub account: BsvHomePlatformHdAccount,
    pub balance: BsvHomePlatformHdBalance,
    pub addresses: Vec<BsvHomePlatformHdAddress>,
    pub unverified_address_count: u64,
    pub can_allocate: bool,
    pub last_scan_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformHdScan {
    pub id: String,
    pub account_id: String,
    pub status: String,
    pub branch: u32,
    pub next_index: u32,
    pub gap_limit: u32,
    pub max_index: u32,
    pub error_code: Option<String>,
    #[serde(skip)]
    pub actions: Vec<BsvHomePlatformHdScanAction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomePlatformHdScanAction {
    Refresh,
    Cancel,
    Resume,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomePlatformHdEnrollment {
    pub id: String,
    pub message: String,
    pub action_hash: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomePlatformHdLocalWallet {
    pub id: String,
    pub name: String,
    pub first_address: String,
    pub base_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomePlatformHdOverview {
    pub local_wallets: Vec<BsvHomePlatformHdLocalWallet>,
    pub accounts: Vec<BsvHomePlatformHdAccount>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformAlternativeKey {
    pub id: String,
    pub label: String,
    pub status: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, uniffi::Record)]
#[serde(rename_all = "camelCase")]
pub struct BsvHomePlatformAlternativeRequest {
    pub id: String,
    pub status: String,
    #[serde(rename = "approvedReviewCount")]
    pub approved: u32,
    #[serde(rename = "requiredIndependentReviewers")]
    pub required: u32,
    #[serde(rename = "notificationAccepted")]
    pub notified: bool,
    pub ready_at: Option<String>,
    pub expires_at: String,
    #[serde(skip)]
    pub actions: Vec<BsvHomePlatformAlternativeAction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomePlatformAlternativeAction {
    Refresh,
    Cancel,
    Register,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomePlatformAlternativeSnapshot {
    pub keys: Vec<BsvHomePlatformAlternativeKey>,
    pub request: Option<BsvHomePlatformAlternativeRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomePlatformAlternativeChallenge {
    pub id: String,
    pub action_hash: String,
    pub expires_at: String,
    pub purpose: String,
    pub public_key: String,
    pub label: String,
    pub user_id: String,
    pub fingerprint: String,
}

fn user_identifier<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Identifier {
        Text(String),
        Number(u64),
    }
    Ok(match Identifier::deserialize(deserializer)? {
        Identifier::Text(value) => value,
        Identifier::Number(value) => value.to_string(),
    })
}
