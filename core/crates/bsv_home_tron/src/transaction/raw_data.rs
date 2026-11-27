use bsv_home_encoding::protobuf::MessageEncode;
use primitives::SignerError;
use serde::Deserialize;
use serde_json::Value;
use serde_serializers::hex_bytes;

use super::{TransactionApproval, TronContract, protobuf};
use crate::models::TronContractType;
use crate::{address::TronAddress, trc20::encode_transfer};

#[derive(Deserialize)]
pub(crate) struct RawDataJson {
    contract: Vec<RawContractJson>,
    expiration: u64,
    #[serde(with = "hex_bytes")]
    ref_block_bytes: Vec<u8>,
    #[serde(with = "hex_bytes")]
    ref_block_hash: Vec<u8>,
    timestamp: u64,
    fee_limit: Option<u64>,
    #[serde(default, with = "hex_bytes::option")]
    data: Option<Vec<u8>>,
}

impl RawDataJson {
    #[cfg(feature = "signer")]
    pub(crate) fn validate_native_transfer(&self, sender: &str, recipient: &str, value: &str) -> Result<(), SignerError> {
        let [contract] = self.contract.as_slice() else {
            return SignerError::invalid_input_err("TRON payment must have exactly one contract");
        };
        if contract.contract_type != TronContractType::Transfer || contract.parameter.type_url.as_deref() != Some(contract.contract_type.type_url().as_str()) {
            return SignerError::invalid_input_err("TRON payment is not a native transfer");
        }
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return SignerError::invalid_input_err("invalid TRON native payment value");
        }
        let value = value.parse::<u64>().map_err(SignerError::from_display)?;
        let sender = TronAddress::parse(sender)?;
        let recipient = TronAddress::parse(recipient)?;
        let TronContract::Transfer { owner, to, amount } = TronContract::from_json_value(contract.contract_type, contract.parameter.value.clone())? else {
            return SignerError::invalid_input_err("TRON payment is not a native transfer");
        };
        if value == 0 || owner != sender || to != recipient || amount != value || self.data.as_ref().is_some_and(|data| !data.is_empty()) {
            return SignerError::invalid_input_err("TRON payment does not match confirmed sender, recipient, or value");
        }
        Ok(())
    }

    pub(crate) fn validate_token_transfer(&self, sender: &str, recipient: &str, token: &str, value: &str) -> Result<(), SignerError> {
        let [contract] = self.contract.as_slice() else {
            return SignerError::invalid_input_err("TRON payment must have exactly one contract");
        };
        let sender = TronAddress::parse(sender)?;
        let recipient = TronAddress::parse(recipient)?;
        let token = TronAddress::parse(token)?;
        let expected_data = encode_transfer(&recipient, value).map_err(SignerError::invalid_input)?;
        match TronContract::from_json_value(contract.contract_type, contract.parameter.value.clone())? {
            TronContract::TriggerSmart {
                owner,
                contract,
                data,
                call_value,
                call_token_value,
                token_id,
            } => {
                if owner != sender
                    || contract != token
                    || data != expected_data
                    || call_value.is_some_and(|value| value != 0)
                    || call_token_value.is_some_and(|value| value != 0)
                    || token_id.is_some_and(|value| value != 0)
                    || self.data.as_ref().is_some_and(|data| !data.is_empty())
                {
                    return SignerError::invalid_input_err("TRON payment does not match confirmed token, recipient, or value");
                }
                Ok(())
            }
            TronContract::Transfer { .. }
            | TronContract::VoteWitness { .. }
            | TronContract::FreezeBalanceV2 { .. }
            | TronContract::UnfreezeBalanceV2 { .. }
            | TronContract::DelegateResource { .. }
            | TronContract::UnDelegateResource { .. }
            | TronContract::WithdrawBalance { .. }
            | TronContract::WithdrawExpireUnfreeze { .. } => SignerError::invalid_input_err("TRON payment is not a token transfer"),
        }
    }

    pub(crate) fn approval(&self) -> Result<Option<TransactionApproval>, SignerError> {
        let [contract] = self.contract.as_slice() else {
            return Ok(None);
        };
        match TronContract::from_json_value(contract.contract_type, contract.parameter.value.clone())? {
            TronContract::TriggerSmart {
                contract,
                data,
                call_value,
                call_token_value,
                ..
            } => {
                let Some(approval) = TransactionApproval::decode(contract, &data)? else {
                    return Ok(None);
                };
                if call_value.is_some_and(|value| value != 0) || call_token_value.is_some_and(|value| value != 0) {
                    return SignerError::invalid_input_err("Approval must not transfer native tokens");
                }
                Ok(Some(approval))
            }
            TronContract::Transfer { .. }
            | TronContract::VoteWitness { .. }
            | TronContract::FreezeBalanceV2 { .. }
            | TronContract::UnfreezeBalanceV2 { .. }
            | TronContract::DelegateResource { .. }
            | TronContract::UnDelegateResource { .. }
            | TronContract::WithdrawBalance { .. }
            | TronContract::WithdrawExpireUnfreeze { .. } => Ok(None),
        }
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, SignerError> {
        let contracts = self
            .contract
            .iter()
            .map(|contract| TronContract::from_json_value(contract.contract_type, contract.parameter.value.clone()).map(|contract| protobuf::ContractEnvelope::from(&contract)))
            .collect::<Result<Vec<_>, SignerError>>()?;

        Ok(protobuf::RawData {
            ref_block_bytes: Some(self.ref_block_bytes.clone()),
            ref_block_hash: Some(self.ref_block_hash.clone()),
            expiration: (self.expiration > 0).then_some(self.expiration),
            data: self.data.clone(),
            contracts,
            timestamp: (self.timestamp > 0).then_some(self.timestamp),
            fee_limit: self.fee_limit.filter(|value| *value > 0),
        }
        .encode())
    }
}

#[derive(Deserialize)]
struct RawContractJson {
    #[serde(rename = "type")]
    contract_type: TronContractType,
    parameter: RawParameterJson,
}

#[derive(Deserialize)]
struct RawParameterJson {
    value: Value,
    #[cfg(feature = "signer")]
    type_url: Option<String>,
}
