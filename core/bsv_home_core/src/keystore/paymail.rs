use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use bsv_home_bsv::{BsvAddress, signer::BsvChainSigner};
use bsv_home_hash::sha2::hmac_sha256;
use primitives::{Address, Chain};
use signer::secp256k1_public_key;
use zeroize::Zeroizing;

use super::BsvHomeKeystore;
use crate::BsvHomeCoreError;

const IDENTITY_LABEL: &[u8] = b"WowPay/Paymail/Identity/v1";
const IDENTITY_DERIVATION_ATTEMPTS: u32 = 1024;
const SECP256K1_ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0,
    0x36, 0x41, 0x41,
];

pub(crate) struct BsvHomePaymailSignatures {
    pub address_signature: String,
    pub identity_signature: String,
}

impl BsvHomeKeystore {
    pub(crate) fn paymail_identity_public_key(&self, keystore_id: String, main_address: &str, password: Vec<u8>) -> Result<String, BsvHomeCoreError> {
        let source_key = self.paymail_source_key(&keystore_id, main_address, password)?;
        let identity_key = derive_identity_key(&source_key)?;
        Ok(hex::encode(secp256k1_public_key(identity_key.as_slice())?))
    }

    pub(crate) fn sign_paymail_binding(
        &self,
        keystore_id: String,
        main_address: &str,
        identity_public_key: &str,
        message: &str,
        password: Vec<u8>,
    ) -> Result<BsvHomePaymailSignatures, BsvHomeCoreError> {
        let source_key = self.paymail_source_key(&keystore_id, main_address, password)?;
        let identity_key = derive_identity_key(&source_key)?;
        if hex::encode(secp256k1_public_key(identity_key.as_slice())?) != identity_public_key {
            return Err(BsvHomeCoreError::from("Paymail identity does not match signing wallet"));
        }
        let address_signature = BsvChainSigner::sign_message_compact(message, &source_key)?;
        let identity_signature = BsvChainSigner::sign_message_compact(message, identity_key.as_slice())?;
        if BsvChainSigner::recover_message_address(message, &address_signature)? != main_address
            || hex::encode(BsvChainSigner::recover_message_public_key(message, &identity_signature)?) != identity_public_key
        {
            return Err(BsvHomeCoreError::from("Paymail signatures do not match binding identity"));
        }
        Ok(BsvHomePaymailSignatures {
            address_signature: BASE64.encode(address_signature),
            identity_signature: BASE64.encode(identity_signature),
        })
    }

    fn paymail_source_key(&self, keystore_id: &str, main_address: &str, password: Vec<u8>) -> Result<Zeroizing<Vec<u8>>, BsvHomeCoreError> {
        let source_key = self.signing_key(keystore_id, Chain::BitcoinSv, password)?;
        if BsvAddress::from_public_key(&secp256k1_public_key(&source_key)?)
            .map_err(|error| BsvHomeCoreError::from(error.to_string()))?
            .encode()
            != main_address
        {
            return Err(BsvHomeCoreError::from("Paymail default address does not match signing wallet"));
        }
        Ok(source_key)
    }
}

fn derive_identity_key(source_key: &[u8]) -> Result<Zeroizing<[u8; 32]>, BsvHomeCoreError> {
    derive_identity_key_from_candidates(source_key, |counter| derive_identity_candidate(source_key, counter))
}

fn derive_identity_key_from_candidates(
    source_key: &[u8],
    mut candidate_for_counter: impl FnMut(u32) -> Result<Zeroizing<[u8; 32]>, BsvHomeCoreError>,
) -> Result<Zeroizing<[u8; 32]>, BsvHomeCoreError> {
    for counter in 0..IDENTITY_DERIVATION_ATTEMPTS {
        let candidate = candidate_for_counter(counter)?;
        if identity_candidate_is_valid(&candidate, source_key) {
            return Ok(candidate);
        }
    }
    Err(BsvHomeCoreError::from("Paymail identity derivation exhausted"))
}

fn derive_identity_candidate(source_key: &[u8], counter: u32) -> Result<Zeroizing<[u8; 32]>, BsvHomeCoreError> {
    let input = [IDENTITY_LABEL, &counter.to_be_bytes()].concat();
    hmac_sha256(source_key, &input)
        .map(Zeroizing::new)
        .map_err(|_| BsvHomeCoreError::from("Paymail identity derivation failed"))
}

fn identity_candidate_is_valid(candidate: &[u8; 32], source_key: &[u8]) -> bool {
    *candidate != [0; 32] && *candidate < SECP256K1_ORDER && candidate.as_slice() != source_key
}

#[cfg(test)]
#[path = "paymail_tests.rs"]
mod tests;
