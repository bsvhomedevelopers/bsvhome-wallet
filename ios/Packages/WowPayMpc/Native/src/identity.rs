use base64::{engine::general_purpose::STANDARD, Engine};
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::backup::random;

type Result<T> = std::result::Result<T, String>;

pub fn new_secret() -> Result<[u8; 32]> {
    let secret = random::<32>()?;
    SecretKey::from_byte_array(secret).map_err(|_| "IDENTITY_KEY_GENERATION_FAILED")?;
    Ok(secret)
}

pub fn public_key(secret: &[u8; 32]) -> Result<String> {
    let key = SecretKey::from_byte_array(*secret).map_err(|_| "IDENTITY_KEY_INVALID")?;
    Ok(hex::encode(
        PublicKey::from_secret_key(&Secp256k1::new(), &key).serialize(),
    ))
}

fn field<'a>(message: &'a str, name: &str) -> Result<&'a str> {
    let mut fields = message
        .lines()
        .filter_map(|line| line.split_once(": "))
        .filter(|(key, _)| *key == name);
    let value = fields.next().ok_or("PAYMAIL_MESSAGE_INVALID")?.1;
    if fields.next().is_some() {
        return Err("PAYMAIL_MESSAGE_INVALID".into());
    }
    Ok(value)
}

#[derive(Serialize)]
pub struct IdentityProof {
    pub signature: String,
    pub digest: String,
}

pub fn sign_binding(
    secret: &[u8; 32],
    message: &str,
    wallet_id: &str,
    epoch: u64,
    address: &str,
) -> Result<IdentityProof> {
    if message.len() > 8192
        || !message.starts_with("WowPay Paymail Binding\nOrigin: https://")
        || field(message, "Identity Key")? != public_key(secret)?
        || field(message, "MPC Wallet")? != wallet_id
        || field(message, "MPC Epoch")? != epoch.to_string()
        || field(message, "Address")? != address
        || field(message, "Network")? != "BSV_MAINNET"
    {
        return Err("PAYMAIL_MESSAGE_INVALID".into());
    }
    let digest = bsm_digest(message);
    let key = SecretKey::from_byte_array(*secret).map_err(|_| "IDENTITY_KEY_INVALID")?;
    let (recovery, compact) = Secp256k1::new()
        .sign_ecdsa_recoverable(Message::from_digest(digest), &key)
        .serialize_compact();
    let mut signature = Vec::with_capacity(65);
    signature.push(31 + i32::from(recovery) as u8);
    signature.extend_from_slice(&compact);
    Ok(IdentityProof {
        signature: STANDARD.encode(signature),
        digest: hex::encode(digest),
    })
}

fn bsm_digest(message: &str) -> [u8; 32] {
    let mut encoded = b"\x18Bitcoin Signed Message:\n".to_vec();
    let length = message.len() as u64;
    if length < 253 {
        encoded.push(length as u8);
    } else if length <= 0xffff {
        encoded.push(253);
        encoded.extend_from_slice(&(length as u16).to_le_bytes());
    } else if length <= 0xffff_ffff {
        encoded.push(254);
        encoded.extend_from_slice(&(length as u32).to_le_bytes());
    } else {
        encoded.push(255);
        encoded.extend_from_slice(&length.to_le_bytes());
    }
    encoded.extend_from_slice(message.as_bytes());
    Sha256::digest(Sha256::digest(encoded)).into()
}

#[cfg(test)]
mod tests {
    use super::{bsm_digest, new_secret, public_key, sign_binding};
    use base64::{engine::general_purpose::STANDARD, Engine};
    use secp256k1::{
        ecdsa::{RecoverableSignature, RecoveryId},
        Message, Secp256k1,
    };

    #[test]
    fn test_identity_binding_is_separate_and_bound_to_wallet_epoch_and_message() {
        let key = new_secret().unwrap();
        let public = public_key(&key).unwrap();
        let message=format!("WowPay Paymail Binding\nOrigin: https://wallet.example.invalid/v2/api\nUser: 1\nHandle: test@pay.example.invalid\nNetwork: BSV_MAINNET\nAddress: public-wallet-address\nIdentity Key: {public}\nActive: true\nVersion: 0\nMPC Wallet: test-wallet\nMPC Epoch: 0\nNonce: test-nonce\nExpires: 2026-01-01T00:00:00Z");
        let signature = STANDARD
            .decode(
                sign_binding(&key, &message, "test-wallet", 0, "public-wallet-address")
                    .unwrap()
                    .signature,
            )
            .unwrap();
        let parsed = RecoverableSignature::from_compact(
            &signature[1..],
            RecoveryId::try_from(i32::from(signature[0] - 31)).unwrap(),
        )
        .unwrap();
        let recovered = Secp256k1::new()
            .recover_ecdsa(Message::from_digest(bsm_digest(&message)), &parsed)
            .unwrap();
        assert_eq!(hex::encode(recovered.serialize()), public);
        assert_eq!(
            sign_binding(&key, &message, "other-wallet", 0, "public-wallet-address").err(),
            Some("PAYMAIL_MESSAGE_INVALID".into())
        );
        assert_eq!(
            sign_binding(&key, &message, "test-wallet", 1, "public-wallet-address").err(),
            Some("PAYMAIL_MESSAGE_INVALID".into())
        );
        assert_eq!(
            sign_binding(
                &key,
                "arbitrary digest",
                "test-wallet",
                0,
                "public-wallet-address"
            )
            .err(),
            Some("PAYMAIL_MESSAGE_INVALID".into())
        );
    }
}
