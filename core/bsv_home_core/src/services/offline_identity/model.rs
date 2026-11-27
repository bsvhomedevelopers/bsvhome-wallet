use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, uniffi::Record)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BsvHomeOfflineIdentityChallenge {
    pub version: u32,
    pub kind: String,
    pub id: String,
    pub action_hash: String,
    pub purpose: BsvHomeOfflineIdentityPurpose,
    pub expires_at: String,
    pub user_id: String,
    pub label: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct BsvHomeOfflineIdentityBackup {
    pub version: u32,
    pub kind: String,
    pub kdf: String,
    pub iterations: u32,
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Proof<'a> {
    pub version: u32,
    pub kind: &'a str,
    pub id: &'a str,
    pub purpose: &'a BsvHomeOfflineIdentityPurpose,
    pub action_hash: &'a str,
    pub public_key: &'a str,
    pub signature: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, uniffi::Enum)]
pub enum BsvHomeOfflineIdentityPurpose {
    Enroll,
    Recover,
    Register,
}
