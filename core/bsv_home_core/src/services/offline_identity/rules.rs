use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use bsv_home_hash::sha2::sha256;
use chrono::{DateTime, NaiveDateTime};

use crate::services::error::BsvHomeServiceError;

use super::{BsvHomeOfflineIdentityBackup, BsvHomeOfflineIdentityChallenge, model::Proof};

fn invalid() -> BsvHomeServiceError {
    BsvHomeServiceError::InvalidInput {
        msg: "OFFLINE_IDENTITY_DOCUMENT_INVALID".into(),
    }
}

fn hexadecimal(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn decode(value: &str) -> Result<Vec<u8>, BsvHomeServiceError> {
    let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid())?;
    if URL_SAFE_NO_PAD.encode(&bytes) != value {
        return Err(invalid());
    }
    Ok(bytes)
}

pub(super) fn fingerprint(public_key: &str) -> Result<String, BsvHomeServiceError> {
    if public_key.len() != 122 {
        return Err(invalid());
    }
    let bytes = decode(public_key)?;
    let prefix = b"\x30\x59\x30\x13\x06\x07\x2a\x86\x48\xce\x3d\x02\x01\x06\x08\x2a\x86\x48\xce\x3d\x03\x01\x07\x03\x42\x00\x04";
    if bytes.len() != 91 || !bytes.starts_with(prefix) {
        return Err(invalid());
    }
    Ok(hex::encode(sha256(public_key.as_bytes())))
}

pub(super) fn challenge(document: &str, public_key: &str) -> Result<BsvHomeOfflineIdentityChallenge, BsvHomeServiceError> {
    if document.len() > 4096 {
        return Err(invalid());
    }
    let challenge: BsvHomeOfflineIdentityChallenge = serde_json::from_str(document).map_err(|_| invalid())?;
    if challenge.version != 1
        || challenge.kind != "wowpay-offline-identity-challenge"
        || !hexadecimal(&challenge.id, 32)
        || !hexadecimal(&challenge.action_hash, 64)
        || !text(&challenge.expires_at, 80)
        || (DateTime::parse_from_rfc3339(&challenge.expires_at).is_err() && NaiveDateTime::parse_from_str(&challenge.expires_at, "%Y-%m-%dT%H:%M:%S%.f").is_err())
        || challenge.user_id.is_empty()
        || challenge.user_id.len() > 32
        || !challenge.user_id.bytes().all(|byte| byte.is_ascii_digit())
        || !text(&challenge.label, 256)
        || challenge.fingerprint != fingerprint(public_key)?
    {
        return Err(invalid());
    }
    Ok(challenge)
}

pub(super) fn proof_document(document: &str, public_key: &str, signature: &str) -> Result<String, BsvHomeServiceError> {
    let challenge = challenge(document, public_key)?;
    let bytes = decode(signature)?;
    if bytes.len() < 8 || bytes.len() > 72 || bytes[0] != 0x30 || bytes[1] as usize != bytes.len() - 2 {
        return Err(invalid());
    }
    serde_json::to_string(&Proof {
        version: 1,
        kind: "wowpay-offline-identity-proof",
        id: &challenge.id,
        purpose: &challenge.purpose,
        action_hash: &challenge.action_hash,
        public_key,
        signature,
    })
    .map_err(|_| invalid())
}

pub(super) fn backup(document: &str) -> Result<BsvHomeOfflineIdentityBackup, BsvHomeServiceError> {
    if document.len() > 16384 {
        return Err(invalid());
    }
    let backup: BsvHomeOfflineIdentityBackup = serde_json::from_str(document).map_err(|_| invalid())?;
    if backup.version != 1
        || backup.kind != "wowpay-offline-identity-backup"
        || backup.kdf != "PBKDF2-HMAC-SHA256"
        || backup.iterations != 310000
        || decode(&backup.salt)?.len() != 16
        || decode(&backup.nonce)?.len() != 12
        || decode(&backup.ciphertext)?.len() < 16
    {
        return Err(invalid());
    }
    Ok(backup)
}

pub(super) fn backup_document(salt: String, nonce: String, ciphertext: String) -> Result<String, BsvHomeServiceError> {
    let document = serde_json::to_string(&BsvHomeOfflineIdentityBackup {
        version: 1,
        kind: "wowpay-offline-identity-backup".into(),
        kdf: "PBKDF2-HMAC-SHA256".into(),
        iterations: 310000,
        salt,
        nonce,
        ciphertext,
    })
    .map_err(|_| invalid())?;
    backup(&document)?;
    Ok(document)
}
