use serde::{Deserialize, Serialize};
use typeshare::typeshare;

use crate::{BsvAddressLocator, UInt64};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[typeshare(swift = "Sendable")]
pub struct BsvSpendInput {
    pub transaction_id: String,
    pub vout: u32,
    pub value: UInt64,
    pub address: String,
    pub script_pubkey: String,
    pub locator: BsvAddressLocator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[typeshare(swift = "Sendable")]
pub struct BsvSpendChange {
    pub address: String,
    pub script_pubkey: String,
    pub locator: BsvAddressLocator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[typeshare(swift = "Sendable")]
pub struct BsvSpendMetadata {
    pub inputs: Vec<BsvSpendInput>,
    pub change: Option<BsvSpendChange>,
}
