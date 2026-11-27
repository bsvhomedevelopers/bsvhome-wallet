use aes_gcm::{
    aead::{Aead, Payload},
    Aes256Gcm, KeyInit, Nonce,
};
use dkls23_secp256k1::Party;
use elliptic_curve::sec1::ToSec1Point;
use hkdf::Hkdf;
use ripemd::Ripemd160;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub const PROTOCOL: &str = "DKLs23-secp256k1-v0.5.1-2of2";

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupContext {
    pub wallet_id: String,
    pub epoch: u64,
    pub public_key: String,
    pub protocol: String,
    pub format: u8,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedBackup {
    pub context: BackupContext,
    pub nonce: String,
    pub ciphertext: String,
}

pub fn random<const N: usize>() -> Result<[u8; N], String> {
    let mut value = [0u8; N];
    getrandom::fill(&mut value).map_err(|_| "RANDOM_UNAVAILABLE")?;
    Ok(value)
}

pub fn public_key(party: &Party) -> String {
    hex::encode(party.pk.to_sec1_point(true).as_bytes())
}

pub fn bsv_address(party: &Party) -> String {
    let bytes = party.pk.to_sec1_point(true);
    let hash = Ripemd160::digest(Sha256::digest(bytes.as_bytes()));
    let mut versioned = vec![0];
    versioned.extend_from_slice(&hash);
    let checksum = Sha256::digest(Sha256::digest(&versioned));
    versioned.extend_from_slice(&checksum[..4]);
    bs58::encode(versioned).into_string()
}

fn cipher(key: &[u8; 32], context: &BackupContext) -> Result<Aes256Gcm, String> {
    if context.protocol != PROTOCOL || context.format != 1 || context.wallet_id.is_empty() {
        return Err("BACKUP_CONTEXT_INVALID".into());
    }
    let hkdf = Hkdf::<Sha256>::new(Some(b"WowPay/RecoveryB/v1"), key);
    let mut derived = Zeroizing::new([0u8; 32]);
    hkdf.expand(b"client-share-a/aes256gcm", derived.as_mut())
        .map_err(|_| "BACKUP_KEY_DERIVATION")?;
    Aes256Gcm::new_from_slice(derived.as_ref()).map_err(|_| "BACKUP_KEY_INVALID".into())
}

pub fn encrypt_backup(
    party: &Party,
    key: &[u8; 32],
    context: BackupContext,
) -> Result<EncryptedBackup, String> {
    if party.party_index.as_u8() != 1 || public_key(party) != context.public_key {
        return Err("BACKUP_PARTICIPANT_INVALID".into());
    }
    let plaintext = Zeroizing::new(serde_json::to_vec(party).map_err(|_| "BACKUP_ENCODING")?);
    let nonce = random::<12>()?;
    let aad = serde_json::to_vec(&context).map_err(|_| "BACKUP_CONTEXT_INVALID")?;
    let ciphertext = cipher(key, &context)?
        .encrypt(
            &Nonce::try_from(nonce.as_slice()).map_err(|_| "NONCE_INVALID")?,
            Payload {
                msg: &plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| "BACKUP_ENCRYPTION_FAILED")?;
    Ok(EncryptedBackup {
        context,
        nonce: hex::encode(nonce),
        ciphertext: hex::encode(ciphertext),
    })
}

pub fn decrypt_backup(
    backup: &EncryptedBackup,
    key: &[u8; 32],
    expected: &BackupContext,
) -> Result<Party, String> {
    if &backup.context != expected || backup.ciphertext.len() > 8_000_000 {
        return Err("BACKUP_CONTEXT_MISMATCH".into());
    }
    let nonce = hex::decode(&backup.nonce).map_err(|_| "BACKUP_AUTHENTICATION_FAILED")?;
    if nonce.len() != 12 {
        return Err("BACKUP_AUTHENTICATION_FAILED".into());
    }
    let ciphertext = hex::decode(&backup.ciphertext).map_err(|_| "BACKUP_AUTHENTICATION_FAILED")?;
    let aad = serde_json::to_vec(expected).map_err(|_| "BACKUP_CONTEXT_INVALID")?;
    let plaintext = Zeroizing::new(
        cipher(key, expected)?
            .decrypt(
                &Nonce::try_from(nonce.as_slice()).map_err(|_| "NONCE_INVALID")?,
                Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| "BACKUP_AUTHENTICATION_FAILED")?,
    );
    let party: Party = serde_json::from_slice(&plaintext).map_err(|_| "BACKUP_STATE_INVALID")?;
    if party.party_index.as_u8() != 1 || public_key(&party) != expected.public_key {
        return Err("BACKUP_PARTICIPANT_INVALID".into());
    }
    Ok(party)
}
