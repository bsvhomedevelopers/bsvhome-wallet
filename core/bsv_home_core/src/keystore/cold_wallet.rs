use bsv_home_bsv::cold_wallet::{Pairing, Request, Review, Signed, decode_json, qr_assemble, qr_frames};
use bsv_home_derivation::{derive_bsv_account_from_recovery, derive_bsv_private_keys_from_recovery};
use primitives::Account;
use zeroize::Zeroizing;

use super::BsvHomeKeystore;
use crate::BsvHomeCoreError;

#[uniffi::export]
impl BsvHomeKeystore {
    pub fn bsv_cold_pairing(&self, account: Account) -> Result<String, BsvHomeCoreError> {
        serde_json::to_string(&Pairing::from_account(&account)?).map_err(BsvHomeCoreError::from)
    }

    pub fn bsv_cold_pairing_validate(&self, descriptor_json: String) -> Result<String, BsvHomeCoreError> {
        let pairing: Pairing = decode_json(&descriptor_json)?;
        serde_json::to_string(&pairing.account()?).map_err(BsvHomeCoreError::from)
    }

    pub fn bsv_cold_review(&self, request_json: String, descriptor_json: String) -> Result<String, BsvHomeCoreError> {
        let request: Request = decode_json(&request_json)?;
        let pairing: Pairing = decode_json(&descriptor_json)?;
        serde_json::to_string(&request.review(&pairing)?).map_err(BsvHomeCoreError::from)
    }

    pub fn bsv_cold_sign(&self, keystore_id: String, request_json: String, review_json: String, password: Vec<u8>) -> Result<String, BsvHomeCoreError> {
        let password = Zeroizing::new(password);
        let request: Request = decode_json(&request_json)?;
        let review: Review = decode_json(&review_json)?;
        let recovery = self.bsv_recovery(&keystore_id, &password)?;
        let account = derive_bsv_account_from_recovery(&recovery)?;
        if Pairing::from_account(&account)? != request.pairing {
            return Err(BsvHomeCoreError::from("Cold signing request does not belong to this wallet"));
        }
        if request.review(&request.pairing)? != review {
            return Err(BsvHomeCoreError::from("Cold signing request changed after review"));
        }
        let keys = derive_bsv_private_keys_from_recovery(&recovery, &request.addresses()?)?;
        let references = keys.iter().map(|(address, key)| (address.as_str(), key.as_slice())).collect::<Vec<_>>();
        serde_json::to_string(&request.sign(&review, &references)?).map_err(BsvHomeCoreError::from)
    }

    pub fn bsv_cold_transaction_id(&self, request_json: String, review_json: String, signed_json: String) -> Result<String, BsvHomeCoreError> {
        let request: Request = decode_json(&request_json)?;
        let review: Review = decode_json(&review_json)?;
        let signed: Signed = decode_json(&signed_json)?;
        request.transaction_id(&review, &signed).map_err(BsvHomeCoreError::from)
    }

    pub fn bsv_cold_qr_frames(&self, payload_json: String) -> Result<Vec<String>, BsvHomeCoreError> {
        qr_frames(&payload_json).map_err(BsvHomeCoreError::from)
    }

    pub fn bsv_cold_qr_assemble(&self, frames: Vec<String>) -> Result<Option<String>, BsvHomeCoreError> {
        qr_assemble(&frames).map_err(BsvHomeCoreError::from)
    }
}
