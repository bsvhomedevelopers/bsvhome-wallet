use super::client::ProviderError;
use crate::BsvAddress;
use bitcoin::{
    ScriptBuf,
    hashes::{Hash, sha256},
    opcodes::all::{OP_CHECKSIG, OP_CHECKSIGVERIFY, OP_CODESEPARATOR, OP_ENDIF, OP_IF, OP_PUSHNUM_1},
    script::Instruction,
    secp256k1::PublicKey,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ChainInfo {
    pub chain: String,
    pub blocks: u64,
    pub headers: u64,
}
#[derive(Debug, Deserialize)]
pub struct Block {
    pub hash: String,
}
#[derive(Debug, Deserialize)]
pub struct FeeRecommendation {
    pub fee_unit: String,
    pub fee: u64,
    pub mempool_min_fee: u64,
}
#[derive(Debug, Deserialize)]
pub struct AddressResult<T> {
    pub address: String,
    pub result: Vec<T>,
    pub error: String,
    #[serde(default, rename = "nextPageToken", alias = "next-page", alias = "next_page")]
    pub next_page: Option<String>,
}
impl<T> AddressResult<T> {
    pub fn validate(&self, address: &str) -> Result<(), ProviderError> {
        if self.address != address || !self.error.is_empty() {
            return Err("invalid WhatsOnChain address response".into());
        }
        Ok(())
    }
}
#[derive(Debug, Deserialize)]
pub struct AddressHistory {
    pub address: String,
    pub confirmed: HistoryResult,
    pub unconfirmed: HistoryResult,
}
impl AddressHistory {
    pub fn validate(&self, address: &str) -> Result<(), ProviderError> {
        if self.address != address || !self.confirmed.error.is_empty() || !self.unconfirmed.error.is_empty() {
            return Err("invalid WhatsOnChain address history response".into());
        }
        Ok(())
    }
}
#[derive(Debug, Deserialize)]
pub struct HistoryResult {
    pub result: Vec<History>,
    pub error: String,
    #[serde(default, rename = "nextPageToken", alias = "next-page", alias = "next_page")]
    pub next_page: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct Unspent {
    pub tx_hash: String,
    pub tx_pos: u32,
    pub value: u64,
    #[serde(default)]
    pub height: Option<u64>,
    #[serde(rename = "isSpentInMempoolTx")]
    pub spent_in_mempool: bool,
    pub status: String,
}
#[derive(Debug, Deserialize)]
pub struct History {
    pub tx_hash: String,
    pub height: Option<u64>,
}
#[derive(Debug, Deserialize)]
pub struct TransactionInfo {
    pub txid: String,
    pub confirmations: Option<u64>,
    pub blockheight: Option<u64>,
    pub blocktime: Option<i64>,
    pub time: Option<i64>,
    pub vin: Vec<TransactionInput>,
    pub vout: Vec<TransactionOutput>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionInput {
    pub txid: Option<String>,
    pub vout: Option<u32>,
    pub coinbase: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct TransactionOutput {
    pub n: u32,
    pub value: serde_json::Number,
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: ScriptPublicKey,
}
#[derive(Debug, Deserialize)]
pub struct ScriptPublicKey {
    pub hex: String,
    #[serde(default, rename = "isTruncated")]
    pub truncated: bool,
}
impl TransactionOutput {
    pub(crate) fn belongs_to(&self, owner: &BsvAddress) -> Result<bool, super::client::ProviderError> {
        let script = self.script()?;
        let owner_script = owner.script_pubkey();
        Ok(script == owner_script || protected_owner_script(&script, &owner_script)?)
    }

    pub fn native_value(&self, owner: &BsvAddress) -> Result<Option<u64>, super::client::ProviderError> {
        let value = self.satoshis()?;
        if value == 0 {
            return Err("zero BSV source value".into());
        }
        if value == 1 {
            return Ok(None);
        }
        let script = self.script()?;
        let owner_script = owner.script_pubkey();
        if script == owner_script {
            return Ok(Some(value));
        }
        if protected_owner_script(&script, &owner_script)? {
            return Ok(None);
        }
        Err("unsupported BSV source script or owner mismatch".into())
    }
    pub fn satoshis(&self) -> Result<u64, super::client::ProviderError> {
        use num_traits::ToPrimitive;
        number_formatter::BigNumberFormatter::value_from_amount_exact(&self.value.to_string(), 8)?
            .to_u64()
            .ok_or_else(|| "BSV output amount overflow".into())
    }
    pub fn address(&self) -> Result<Option<String>, super::client::ProviderError> {
        if self.script_pubkey.truncated {
            bsv_home_tracing::info_with_fields!("BSV large output omitted from address projection; value counted in totals", output_index = self.n);
            return Ok(None);
        }
        let script = self.script()?;
        if script.is_multisig() {
            let mut hash = sha256::Hash::hash(script.as_bytes()).to_byte_array();
            hash.reverse();
            return Ok(Some(format!("P2MS:{}", hex::encode(hash))));
        }
        Ok(bitcoin::Address::from_script(&script, bitcoin::Network::Bitcoin).ok().map(|address| address.to_string()))
    }
    pub fn script(&self) -> Result<bitcoin::ScriptBuf, super::client::ProviderError> {
        if self.script_pubkey.truncated {
            return Err("BSV output script was truncated".into());
        }
        Ok(bitcoin::ScriptBuf::from_bytes(hex::decode(&self.script_pubkey.hex)?))
    }
}

fn protected_owner_script(script: &ScriptBuf, owner: &ScriptBuf) -> Result<bool, super::client::ProviderError> {
    let instructions = script.instructions().collect::<Result<Vec<_>, _>>()?;
    let ord_prefix = instructions.len() >= 8
        && instructions[0] == Instruction::PushBytes((&[][..]).try_into()?)
        && instructions[1] == Instruction::Op(OP_IF)
        && instructions[2] == Instruction::PushBytes((&b"ord"[..]).try_into()?)
        && instructions[3] == Instruction::Op(OP_PUSHNUM_1)
        && instructions[5] == Instruction::PushBytes((&[][..]).try_into()?)
        && instructions[7] == Instruction::Op(OP_ENDIF);
    let prefix = if ord_prefix { 8 } else { 0 };
    let prefix = prefix + usize::from(instructions.get(prefix) == Some(&Instruction::Op(OP_CODESEPARATOR)));
    let locking = &instructions[prefix..];
    let owner = owner.instructions().collect::<Result<Vec<_>, _>>()?;
    if ord_prefix && locking == owner {
        return Ok(true);
    }
    if locking.len() != 7 || locking[..4] != owner[..4] || locking[4] != Instruction::Op(OP_CHECKSIGVERIFY) || locking[6] != Instruction::Op(OP_CHECKSIG) {
        return Ok(false);
    }
    match locking[5] {
        Instruction::PushBytes(public_key) => Ok(public_key.len() == 33 && PublicKey::from_slice(public_key.as_bytes()).is_ok()),
        Instruction::Op(_) => Ok(false),
    }
}
impl TransactionInfo {
    pub fn is_coinbase(&self) -> bool {
        self.vin.len() == 1 && self.vin[0].coinbase.as_ref().is_some_and(|value| !value.is_empty())
    }
    pub fn output(&self, index: u32) -> Result<&TransactionOutput, super::client::ProviderError> {
        let mut matches = self.vout.iter().filter(|output| output.n == index);
        let output = matches.next().ok_or("BSV previous output missing")?;
        if matches.next().is_some() {
            return Err("duplicate BSV output index".into());
        }
        Ok(output)
    }
}
