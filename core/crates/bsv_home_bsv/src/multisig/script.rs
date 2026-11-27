use std::collections::HashSet;

use bitcoin::{
    ScriptBuf,
    hashes::{Hash, sha256},
    opcodes::all::OP_CHECKMULTISIG,
    script::Builder,
    secp256k1::PublicKey,
};
use primitives::{Address, SignerError};

use super::MultisigConfig;
use crate::BsvAddress;

pub const SCRIPT_TARGET_PREFIX: &str = "bsv-script:";

pub struct BsvScriptTarget;

impl BsvScriptTarget {
    pub fn parse(target: &str) -> Result<ScriptBuf, SignerError> {
        match target.strip_prefix(SCRIPT_TARGET_PREFIX) {
            Some(script) => parse_locking_script(script)?.locking_script(),
            None => Ok(BsvAddress::from_str(target).map_err(SignerError::from_display)?.script_pubkey()),
        }
    }
}

impl MultisigConfig {
    pub fn validate(&self) -> Result<(), SignerError> {
        let count = self.public_keys.len();
        if !(2..=5).contains(&count) || !(2..=count).contains(&(self.threshold as usize)) {
            return SignerError::invalid_input_err("BSV multisig requires 2 to 5 members and a threshold from 2 to the member count");
        }
        let mut keys = HashSet::new();
        for key in &self.public_keys {
            let bytes = canonical_hex(key)?;
            let public_key = PublicKey::from_slice(&bytes).map_err(SignerError::from_display)?;
            if bytes.len() != 33 || public_key.serialize().as_slice() != bytes || !keys.insert(bytes) {
                return SignerError::invalid_input_err("BSV multisig requires distinct compressed public keys");
            }
        }
        Ok(())
    }

    pub fn locking_script(&self) -> Result<ScriptBuf, SignerError> {
        self.validate()?;
        let mut keys = self.public_keys.iter().map(|key| canonical_hex(key)).collect::<Result<Vec<_>, _>>()?;
        keys.sort();
        let mut builder = Builder::new().push_int(self.threshold as i64);
        for key in keys {
            let key: [u8; 33] = key.try_into().map_err(|_| SignerError::invalid_input("invalid BSV member public key length"))?;
            builder = builder.push_slice(key);
        }
        Ok(builder.push_int(self.public_keys.len() as i64).push_opcode(OP_CHECKMULTISIG).into_script())
    }

    pub fn script_hash(&self) -> Result<String, SignerError> {
        let mut hash = sha256::Hash::hash(self.locking_script()?.as_bytes()).to_byte_array();
        hash.reverse();
        Ok(hex::encode(hash))
    }

    pub fn script_id(&self) -> Result<String, SignerError> {
        Ok(hex::encode(sha256::Hash::hash(self.locking_script()?.as_bytes()).to_byte_array()))
    }

    pub(crate) fn sorted_public_keys(&self) -> Result<Vec<PublicKey>, SignerError> {
        self.validate()?;
        let mut keys = self
            .public_keys
            .iter()
            .map(|key| PublicKey::from_slice(&canonical_hex(key)?).map_err(SignerError::from_display))
            .collect::<Result<Vec<_>, _>>()?;
        keys.sort_by_key(PublicKey::serialize);
        Ok(keys)
    }
}

pub fn parse_locking_script(script: &str) -> Result<MultisigConfig, SignerError> {
    let bytes = canonical_hex(script)?;
    if ![71, 105, 139, 173].contains(&bytes.len()) {
        return SignerError::invalid_input_err("invalid BSV multisig locking script length");
    }
    let count = (bytes.len() - 3) / 34;
    if bytes[bytes.len() - 2] != 0x50 + count as u8 || bytes[bytes.len() - 1] != 0xae || !(0x52..=0x55).contains(&bytes[0]) {
        return SignerError::invalid_input_err("invalid BSV multisig locking script opcodes");
    }
    let mut public_keys = Vec::with_capacity(count);
    for index in 0..count {
        let offset = 1 + index * 34;
        if bytes[offset] != 33 {
            return SignerError::invalid_input_err("noncanonical BSV public key push");
        }
        public_keys.push(hex::encode(&bytes[offset + 1..offset + 34]));
    }
    let configuration = MultisigConfig {
        threshold: bytes[0] - 0x50,
        public_keys,
    };
    if configuration.locking_script()?.as_bytes() != bytes {
        return SignerError::invalid_input_err("BSV multisig public keys must be canonically sorted");
    }
    Ok(configuration)
}

pub(crate) fn canonical_hex(value: &str) -> Result<Vec<u8>, SignerError> {
    if value.is_empty() || !value.len().is_multiple_of(2) || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return SignerError::invalid_input_err("expected canonical lowercase hexadecimal");
    }
    hex::decode(value).map_err(SignerError::from_display)
}

pub(crate) fn destination_script(script: &str) -> Result<ScriptBuf, SignerError> {
    let bytes = canonical_hex(script)?;
    let output = ScriptBuf::from_bytes(bytes);
    if !output.is_p2pkh() {
        parse_locking_script(script)?;
    }
    Ok(output)
}
