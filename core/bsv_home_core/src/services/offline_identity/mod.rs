mod model;
mod rules;
#[cfg(test)]
mod tests;

pub use model::{BsvHomeOfflineIdentityBackup, BsvHomeOfflineIdentityChallenge, BsvHomeOfflineIdentityPurpose};

use crate::services::error::BsvHomeServiceError;

#[derive(Default, uniffi::Object)]
pub struct BsvHomeOfflineIdentityService;

#[uniffi::export]
impl BsvHomeOfflineIdentityService {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self
    }

    pub fn public_key_fingerprint(&self, public_key: String) -> Result<String, BsvHomeServiceError> {
        rules::fingerprint(&public_key)
    }

    pub fn review_challenge(&self, challenge_json: String, public_key: String) -> Result<BsvHomeOfflineIdentityChallenge, BsvHomeServiceError> {
        rules::challenge(&challenge_json, &public_key)
    }

    pub fn proof_document(&self, challenge_json: String, public_key: String, signature: String) -> Result<String, BsvHomeServiceError> {
        rules::proof_document(&challenge_json, &public_key, &signature)
    }

    pub fn read_backup(&self, document: String) -> Result<BsvHomeOfflineIdentityBackup, BsvHomeServiceError> {
        rules::backup(&document)
    }

    pub fn backup_document(&self, salt: String, nonce: String, ciphertext: String) -> Result<String, BsvHomeServiceError> {
        rules::backup_document(salt, nonce, ciphertext)
    }
}
