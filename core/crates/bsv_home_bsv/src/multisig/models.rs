use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MultisigConfig {
    pub threshold: u8,
    pub public_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviousOutput {
    pub transaction_id: String,
    pub vout: u32,
    #[serde(serialize_with = "serialize_satoshis", deserialize_with = "deserialize_satoshis")]
    pub value: u64,
    pub raw_transaction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransactionPlan {
    pub unsigned_transaction: String,
    pub previous_outputs: Vec<PreviousOutput>,
    pub fee_rate: u64,
    #[serde(serialize_with = "serialize_satoshis", deserialize_with = "deserialize_satoshis")]
    pub fee: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputSignature {
    pub input_index: u32,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartialSignature {
    pub public_key: String,
    pub signatures: Vec<InputSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FinalizedTransaction {
    pub raw_transaction: String,
    pub transaction_id: String,
    #[serde(serialize_with = "serialize_satoshis", deserialize_with = "deserialize_satoshis")]
    pub fee: u64,
    pub serialized_size: u64,
}

fn serialize_satoshis<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

fn deserialize_satoshis<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let value = String::deserialize(deserializer)?;
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(D::Error::custom("expected canonical decimal satoshi string"));
    }
    value.parse().map_err(D::Error::custom)
}
