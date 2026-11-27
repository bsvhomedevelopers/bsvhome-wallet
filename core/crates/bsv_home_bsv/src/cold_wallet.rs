use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use bitcoin::{
    Amount, OutPoint, Transaction, Txid,
    bip32::ChildNumber,
    consensus::encode::{deserialize, serialize},
    hashes::{Hash, sha256},
    secp256k1::Secp256k1,
};
use bsv_home_bitcoin::signer::{
    UnlockingScript,
    account_path::AccountPath,
    build_unsigned_transaction,
    planner::{PlanInput, PlanOutput, SpendPlan, SpendTarget, UtxoPlanner},
    sign_fork_id_plan_with_keys, verify_fork_id_plan,
};
use primitives::{Account, Address, Chain, SignerError, UTXO};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{BsvAddress, signer::BsvSpendPolicy};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColdAddress {
    pub address: String,
    pub branch: u32,
    pub index: u32,
}

pub const MAX_PAYLOAD: usize = 1_048_576;
const MAX_INPUTS: usize = 100;
const MAX_MONEY: u64 = 2_100_000_000_000_000;
const MAX_FEE: u64 = 10_000_000;
const FRAME_BYTES: usize = 800;
const MAX_FRAMES: usize = 1800;
#[cfg(test)]
const ACCOUNT_PATH: &str = "m/44'/236'/0'";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pairing {
    pub version: u32,
    pub kind: String,
    pub network: String,
    pub account_path: String,
    pub xpub: String,
    pub wallet_id: String,
    pub first_address: String,
}

impl Pairing {
    pub fn from_account(account: &Account) -> Result<Self, SignerError> {
        let account_path = account
            .derivation_path
            .strip_suffix("/0/0")
            .ok_or_else(|| SignerError::invalid_input("Unsupported cold wallet account"))?;
        let account_path = AccountPath::parse(account_path)?.canonical();
        if account.chain != Chain::BitcoinSv {
            return SignerError::invalid_input_err("Unsupported cold wallet account");
        }
        let xpub = account
            .extended_public_key
            .clone()
            .ok_or_else(|| SignerError::invalid_input("Cold wallet requires an account xPub"))?;
        let identity = format!("WowPay/BSV/Cold/v1:mainnet:{account_path}:{xpub}");
        let pairing = Self {
            version: 1,
            kind: "wowpay-bsv-pairing".into(),
            network: "mainnet".into(),
            account_path,
            xpub,
            wallet_id: sha256::Hash::hash(identity.as_bytes()).to_string(),
            first_address: account.address.clone(),
        };
        pairing.validate()?;
        Ok(pairing)
    }

    pub fn validate(&self) -> Result<(), SignerError> {
        let identity = format!("WowPay/BSV/Cold/v1:mainnet:{}:{}", self.account_path, self.xpub);
        if self.version != 1
            || self.kind != "wowpay-bsv-pairing"
            || self.network != "mainnet"
            || AccountPath::parse(&self.account_path)?.canonical() != self.account_path
            || self.wallet_id != sha256::Hash::hash(identity.as_bytes()).to_string()
            || self.address(0, 0)?.address != self.first_address
        {
            return SignerError::invalid_input_err("Invalid cold wallet pairing");
        }
        Ok(())
    }

    pub fn account(&self) -> Result<Account, SignerError> {
        self.validate()?;
        Ok(Account {
            chain: Chain::BitcoinSv,
            address: self.first_address.clone(),
            derivation_path: format!("{}/0/0", self.account_path),
            extended_public_key: Some(self.xpub.clone()),
        })
    }

    pub fn address(&self, branch: u32, index: u32) -> Result<ColdAddress, SignerError> {
        if branch > 1 || index >= 10_000 || self.xpub.len() > 120 {
            return SignerError::invalid_input_err("Invalid cold wallet derivation index");
        }
        let xpub = AccountPath::parse(&self.account_path)?.validate_xpub(&self.xpub)?;
        let child = xpub
            .derive_pub(&Secp256k1::verification_only(), &[ChildNumber::Normal { index: branch }, ChildNumber::Normal { index }])
            .map_err(SignerError::from_display)?;
        let address = BsvAddress::from_public_key(&child.public_key.serialize()).map_err(SignerError::from_display)?.encode();
        Ok(ColdAddress { address, branch, index })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub previous_transaction: String,
    pub branch: u32,
    pub index: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub kind: String,
    pub pairing: Pairing,
    pub unsigned_transaction: String,
    pub inputs: Vec<Input>,
    pub recipient: String,
    pub amount: u64,
    pub change_index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub commitment: String,
    pub wallet_id: String,
    pub recipient: String,
    pub amount: u64,
    pub fee: u64,
    pub change: u64,
    pub change_address: Option<String>,
    pub input_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Signed {
    pub version: u32,
    pub kind: String,
    pub commitment: String,
    pub transaction_id: String,
    pub raw_transaction: String,
}

impl Request {
    pub fn prepare(
        account: &Account,
        recipient: String,
        amount: u64,
        fee_rate: u64,
        utxos: Vec<UTXO>,
        addresses: Vec<ColdAddress>,
        change_index: u32,
    ) -> Result<Self, SignerError> {
        let pairing = Pairing::from_account(account)?;
        if amount == 0 || amount > MAX_MONEY || fee_rate == 0 || fee_rate > 10_000 || utxos.len() > 1000 {
            return SignerError::invalid_input_err("Invalid cold wallet transfer");
        }
        for child in &addresses {
            if pairing.address(child.branch, child.index)? != *child {
                return SignerError::invalid_input_err("Cold wallet address metadata mismatch");
            }
        }
        let change = pairing.address(1, change_index)?;
        let mut outpoints = HashSet::new();
        let inputs = utxos
            .into_iter()
            .filter(|utxo| utxo.value > 1u64.into())
            .map(|utxo| {
                let previous_output = OutPoint::new(
                    Txid::from_str(&utxo.transaction_id).map_err(SignerError::from_display)?,
                    u32::try_from(utxo.vout).map_err(SignerError::from_display)?,
                );
                if !outpoints.insert(previous_output) || !addresses.iter().any(|child| child.address == utxo.address) {
                    return SignerError::invalid_input_err("Invalid cold wallet UTXO owner or duplicate outpoint");
                }
                let value = u64::try_from(&utxo.value).map_err(SignerError::from_display)?;
                if value > MAX_MONEY {
                    return SignerError::invalid_input_err("Cold wallet UTXO exceeds monetary limit");
                }
                Ok(PlanInput {
                    previous_output,
                    value: Amount::from_sat(value),
                    script_pubkey: BsvAddress::from_str(&utxo.address).map_err(SignerError::from_display)?.script_pubkey(),
                    unlocking_script: UnlockingScript::P2pkh,
                    sequence: 0xffff_fffd,
                })
            })
            .collect::<Result<Vec<_>, SignerError>>()?;
        let plan = UtxoPlanner::plan_spend(
            &BsvSpendPolicy { satoshis_per_kilobyte: fee_rate },
            SpendTarget::Exact(amount),
            BsvAddress::from_str(&recipient).map_err(SignerError::from_display)?.script_pubkey(),
            BsvAddress::from_str(&change.address).map_err(SignerError::from_display)?.script_pubkey(),
            None,
            inputs,
        )?;
        if plan.inputs.len() > MAX_INPUTS || plan.fee > MAX_FEE {
            return SignerError::invalid_input_err("Cold wallet transfer exceeds limits");
        }
        let owned = addresses
            .iter()
            .map(|child| Ok((child, BsvAddress::from_str(&child.address).map_err(SignerError::from_display)?.script_pubkey())))
            .collect::<Result<Vec<_>, SignerError>>()?;
        let inputs = plan
            .inputs
            .iter()
            .map(|input| {
                let (child, _) = owned
                    .iter()
                    .find(|(_, script)| *script == input.script_pubkey)
                    .ok_or_else(|| SignerError::invalid_input("Cold wallet input not found"))?;
                Ok(Input {
                    previous_transaction: String::new(),
                    branch: child.branch,
                    index: child.index,
                })
            })
            .collect::<Result<Vec<_>, SignerError>>()?;
        Ok(Self {
            version: 1,
            kind: "wowpay-bsv-request".into(),
            pairing,
            unsigned_transaction: hex::encode(serialize(&build_unsigned_transaction(&plan))),
            inputs,
            recipient,
            amount,
            change_index: (plan.outputs.len() == 2).then_some(change_index),
        })
    }

    pub fn outpoints(&self) -> Result<Vec<(String, u32)>, SignerError> {
        Ok(decode_transaction(&self.unsigned_transaction)?
            .input
            .into_iter()
            .map(|input| (input.previous_output.txid.to_string(), input.previous_output.vout))
            .collect())
    }

    pub fn previous_transaction_ids(&self) -> Result<Vec<String>, SignerError> {
        Ok(decode_transaction(&self.unsigned_transaction)?
            .input
            .iter()
            .map(|input| input.previous_output.txid.to_string())
            .collect())
    }

    fn plan(&self, pairing: &Pairing) -> Result<SpendPlan, SignerError> {
        pairing.validate()?;
        if &self.pairing != pairing
            || self.version != 1
            || self.kind != "wowpay-bsv-request"
            || self.amount == 0
            || self.amount > MAX_MONEY
            || self.inputs.is_empty()
            || self.inputs.len() > MAX_INPUTS
            || serde_json::to_vec(self)?.len() > MAX_PAYLOAD
        {
            return SignerError::invalid_input_err("Invalid cold wallet request");
        }
        let transaction = decode_transaction(&self.unsigned_transaction)?;
        if transaction.version.0 != 2
            || transaction.lock_time.to_consensus_u32() != 0
            || transaction.input.len() != self.inputs.len()
            || transaction.output.len() != 1 + usize::from(self.change_index.is_some())
        {
            return SignerError::invalid_input_err("Unsupported cold wallet transaction");
        }
        let recipient = BsvAddress::from_str(&self.recipient).map_err(SignerError::from_display)?.script_pubkey();
        if !recipient.is_p2pkh() || transaction.output[0].script_pubkey != recipient || transaction.output[0].value.to_sat() != self.amount {
            return SignerError::invalid_input_err("Cold wallet recipient mismatch");
        }
        if let Some(index) = self.change_index {
            let child = pairing.address(1, index)?;
            if transaction.output[1].script_pubkey != BsvAddress::from_str(&child.address).map_err(SignerError::from_display)?.script_pubkey()
                || transaction.output[1].value.to_sat() == 0
            {
                return SignerError::invalid_input_err("Cold wallet change does not belong to wallet");
            }
        }
        let mut seen = HashSet::new();
        let inputs = transaction
            .input
            .iter()
            .zip(&self.inputs)
            .map(|(input, metadata)| {
                if !seen.insert(input.previous_output) || input.sequence.0 != 0xffff_fffd || !input.script_sig.is_empty() || !input.witness.is_empty() {
                    return SignerError::invalid_input_err("Invalid cold wallet transaction input");
                }
                let previous = decode_transaction(&metadata.previous_transaction)?;
                if previous.compute_txid() != input.previous_output.txid {
                    return SignerError::invalid_input_err("Cold wallet previous transaction hash mismatch");
                }
                let output = previous
                    .output
                    .get(input.previous_output.vout as usize)
                    .ok_or_else(|| SignerError::invalid_input("Cold wallet previous output missing"))?;
                let child = pairing.address(metadata.branch, metadata.index)?;
                if output.script_pubkey != BsvAddress::from_str(&child.address).map_err(SignerError::from_display)?.script_pubkey()
                    || output.value.to_sat() <= 1
                    || output.value.to_sat() > MAX_MONEY
                {
                    return SignerError::invalid_input_err("Cold wallet input does not belong to wallet");
                }
                Ok(PlanInput {
                    previous_output: input.previous_output,
                    value: output.value,
                    script_pubkey: output.script_pubkey.clone(),
                    unlocking_script: UnlockingScript::P2pkh,
                    sequence: input.sequence.0,
                })
            })
            .collect::<Result<Vec<_>, SignerError>>()?;
        let input_total: u128 = inputs.iter().map(|input| input.value.to_sat() as u128).sum();
        let output_total: u128 = transaction.output.iter().map(|output| output.value.to_sat() as u128).sum();
        let fee = input_total
            .checked_sub(output_total)
            .filter(|fee| *fee <= MAX_FEE as u128)
            .ok_or_else(|| SignerError::invalid_input("Invalid cold wallet fee"))? as u64;
        if input_total > MAX_MONEY as u128 || fee == 0 {
            return SignerError::invalid_input_err("Invalid cold wallet amounts");
        }
        Ok(SpendPlan {
            inputs,
            outputs: transaction
                .output
                .into_iter()
                .map(|output| PlanOutput {
                    value: output.value,
                    script_pubkey: output.script_pubkey,
                })
                .collect(),
            fee,
        })
    }

    pub fn review(&self, pairing: &Pairing) -> Result<Review, SignerError> {
        let plan = self.plan(pairing)?;
        let mut commitment = b"WowPay/BSV/Cold/Request/v1:".to_vec();
        commitment.extend(serde_json::to_vec(self)?);
        Ok(Review {
            commitment: sha256::Hash::hash(&commitment).to_string(),
            wallet_id: pairing.wallet_id.clone(),
            recipient: self.recipient.clone(),
            amount: self.amount,
            fee: plan.fee,
            change: plan.outputs.get(1).map(|output| output.value.to_sat()).unwrap_or_default(),
            change_address: self.change_index.map(|index| pairing.address(1, index).map(|child| child.address)).transpose()?,
            input_count: plan.inputs.len() as u32,
        })
    }

    pub fn addresses(&self) -> Result<Vec<ColdAddress>, SignerError> {
        self.inputs.iter().map(|input| self.pairing.address(input.branch, input.index)).collect()
    }

    pub fn sign(&self, expected: &Review, keys: &[(&str, &[u8])]) -> Result<Signed, SignerError> {
        if self.review(&self.pairing)? != *expected {
            return SignerError::invalid_input_err("Cold wallet request changed after review");
        }
        let raw = sign_fork_id_plan_with_keys(Chain::BitcoinSv, &self.plan(&self.pairing)?, |input| {
            let key = keys
                .iter()
                .find(|(address, _)| BsvAddress::from_str(address).is_ok_and(|address| address.script_pubkey() == input.script_pubkey))
                .ok_or_else(|| SignerError::invalid_input("Cold wallet private key missing"))?;
            Ok(Zeroizing::new(key.1.to_vec()))
        })?;
        let signed = Signed {
            version: 1,
            kind: "wowpay-bsv-signed".into(),
            commitment: expected.commitment.clone(),
            transaction_id: decode_transaction(&raw)?.compute_txid().to_string(),
            raw_transaction: raw,
        };
        self.transaction_id(expected, &signed)?;
        Ok(signed)
    }

    pub fn transaction_id(&self, expected: &Review, signed: &Signed) -> Result<String, SignerError> {
        if signed.version != 1 || signed.kind != "wowpay-bsv-signed" || self.review(&self.pairing)? != *expected || signed.commitment != expected.commitment {
            return SignerError::invalid_input_err("Cold wallet signed response mismatch");
        }
        let transaction_id = verify_fork_id_plan(Chain::BitcoinSv, &self.plan(&self.pairing)?, &signed.raw_transaction)?;
        if transaction_id != signed.transaction_id {
            return SignerError::invalid_input_err("Cold wallet signed transaction changed");
        }
        Ok(signed.transaction_id.clone())
    }
}

pub fn decode_json<T: for<'de> Deserialize<'de>>(json: &str) -> Result<T, SignerError> {
    if json.len() > MAX_PAYLOAD {
        return SignerError::invalid_input_err("Cold wallet payload exceeds limit");
    }
    serde_json::from_str(json).map_err(SignerError::from_display)
}

fn decode_transaction(raw: &str) -> Result<Transaction, SignerError> {
    if raw.len() > MAX_PAYLOAD {
        return SignerError::invalid_input_err("Cold wallet transaction exceeds limit");
    }
    deserialize(&hex::decode(raw).map_err(SignerError::from_display)?).map_err(SignerError::from_display)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Frame {
    version: u32,
    kind: String,
    checksum: String,
    total: usize,
    index: usize,
    data: String,
}

pub fn qr_frames(payload: &str) -> Result<Vec<String>, SignerError> {
    let _: serde_json::Value = decode_json(payload)?;
    if payload.is_empty() {
        return SignerError::invalid_input_err("Empty cold wallet QR payload");
    }
    let checksum = sha256::Hash::hash(payload.as_bytes()).to_string();
    let encoded = STANDARD.encode(payload.as_bytes());
    let total = encoded.len().div_ceil(FRAME_BYTES);
    encoded
        .as_bytes()
        .chunks(FRAME_BYTES)
        .enumerate()
        .map(|(index, data)| {
            serde_json::to_string(&Frame {
                version: 1,
                kind: "wowpay-bsv-qr".into(),
                checksum: checksum.clone(),
                total,
                index,
                data: String::from_utf8(data.to_vec()).map_err(SignerError::from_display)?,
            })
            .map_err(SignerError::from_display)
        })
        .collect()
}

pub fn qr_assemble(frames: &[String]) -> Result<Option<String>, SignerError> {
    if frames.is_empty() {
        return Ok(None);
    }
    if frames.len() > MAX_FRAMES * 2 {
        return SignerError::invalid_input_err("Too many cold wallet QR frames");
    }
    let mut chunks = HashMap::new();
    let first: Frame = decode_json(&frames[0])?;
    for json in frames {
        if json.len() > FRAME_BYTES + 300 {
            return SignerError::invalid_input_err("Cold wallet QR frame exceeds limit");
        }
        let frame: Frame = decode_json(json)?;
        if frame.version != 1
            || frame.kind != "wowpay-bsv-qr"
            || frame.checksum.len() != 64
            || hex::decode(&frame.checksum).map_err(SignerError::from_display)?.len() != 32
            || frame.total == 0
            || frame.total > MAX_FRAMES
            || frame.index >= frame.total
            || frame.data.is_empty()
            || frame.data.len() > FRAME_BYTES
            || (frame.index + 1 < frame.total && (frame.data.len() != FRAME_BYTES || frame.data.ends_with('=')))
            || !frame.data.len().is_multiple_of(4)
            || STANDARD.decode(&frame.data).is_err()
            || frame.total != first.total
            || frame.checksum != first.checksum
        {
            return SignerError::invalid_input_err("Invalid or mixed cold wallet QR frames");
        }
        if let Some(previous) = chunks.insert(frame.index, frame.data.clone())
            && previous != frame.data
        {
            return SignerError::invalid_input_err("Conflicting cold wallet QR frame");
        }
    }
    if chunks.len() < first.total {
        return Ok(None);
    }
    let encoded = (0..first.total)
        .map(|index| chunks.get(&index).cloned().ok_or_else(|| SignerError::invalid_input("Missing cold wallet QR frame")))
        .collect::<Result<Vec<_>, SignerError>>()?
        .concat();
    let bytes = STANDARD.decode(encoded).map_err(SignerError::from_display)?;
    if bytes.len() > MAX_PAYLOAD || sha256::Hash::hash(&bytes).to_string() != first.checksum {
        return SignerError::invalid_input_err("Cold wallet QR checksum mismatch");
    }
    let payload = String::from_utf8(bytes).map_err(SignerError::from_display)?;
    let _: serde_json::Value = decode_json(&payload)?;
    Ok(Some(payload))
}

#[cfg(test)]
#[path = "cold_wallet_tests.rs"]
mod tests;
