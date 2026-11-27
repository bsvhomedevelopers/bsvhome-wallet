use bsv_home_hash::sha2::sha256;
use primitives::SignerError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use serde_serializers::hex_bytes;
#[cfg(feature = "signer")]
use signer::{RECOVERY_ID_INDEX, SIGNATURE_LENGTH, Signer};

use super::{RawDataJson, TransactionApproval};
#[cfg(feature = "signer")]
use crate::address::TronAddress;

#[derive(Deserialize)]
pub(crate) struct WalletConnectRequest {
    pub(crate) transaction: WalletConnectTransaction,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct WalletConnectTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_data: Option<Value>,
    #[serde(with = "hex_bytes")]
    raw_data_hex: Vec<u8>,
    #[serde(rename = "txID", skip_serializing_if = "Option::is_none")]
    pub(crate) transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signature: Option<Vec<String>>,
    // Preserve non-wire dApp fields like `visible`; only raw_data_hex is signed.
    #[serde(flatten)]
    extra: Map<String, Value>,
}

impl WalletConnectTransaction {
    pub(crate) fn validate(&self) -> Result<([u8; 32], RawDataJson), SignerError> {
        let raw_data = self.raw_data_hex.as_slice();
        let transaction_hash = sha256(raw_data);
        let transaction_id = hex::encode(transaction_hash);

        let raw_data_json = self.raw_data.as_ref().ok_or_else(|| SignerError::invalid_input("Missing raw_data"))?;

        if let Some(provided_transaction_id) = &self.transaction_id
            && !provided_transaction_id.eq_ignore_ascii_case(&transaction_id)
        {
            return SignerError::invalid_input_err("transaction ID does not match hash of raw_data_hex");
        }

        let decoded = serde_json::from_value::<RawDataJson>(raw_data_json.clone())?;
        if decoded.encode()? != raw_data {
            return SignerError::invalid_input_err("raw_data does not match raw_data_hex");
        }

        Ok((transaction_hash, decoded))
    }
}

pub fn decode_wallet_connect_approval(data: &str) -> Result<Option<TransactionApproval>, SignerError> {
    let payload: WalletConnectRequest = serde_json::from_str(data)?;
    let (_, raw_data) = payload.transaction.validate()?;
    raw_data.approval()
}

pub fn validate_signed_token_transfer(data: &str, sender: &str, recipient: &str, token: &str, value: &str) -> Result<String, SignerError> {
    let transaction: WalletConnectTransaction = serde_json::from_str(data)?;
    let (hash, raw_data) = transaction.validate()?;
    raw_data.validate_token_transfer(sender, recipient, token, value)?;
    let signature = transaction
        .signature
        .as_ref()
        .filter(|signatures| signatures.len() == 1)
        .and_then(|signatures| signatures.first())
        .ok_or_else(|| SignerError::invalid_input("missing TRON payment signature"))?;
    if hex::decode(signature).map_err(SignerError::from_display)?.len() != 65 {
        return SignerError::invalid_input_err("invalid TRON payment signature");
    }
    Ok(hex::encode(hash))
}

#[cfg(feature = "signer")]
pub fn validate_signed_native_transfer(data: &str, sender: &str, recipient: &str, value: &str) -> Result<String, SignerError> {
    let transaction: WalletConnectTransaction = serde_json::from_str(data)?;
    let (hash, raw_data) = transaction.validate()?;
    raw_data.validate_native_transfer(sender, recipient, value)?;
    let signature = transaction
        .signature
        .as_ref()
        .filter(|signatures| signatures.len() == 1)
        .and_then(|signatures| signatures.first())
        .ok_or_else(|| SignerError::invalid_input("missing TRON payment signature"))?;
    let signature = hex::decode(signature).map_err(SignerError::from_display)?;
    if signature.len() != SIGNATURE_LENGTH || signature[RECOVERY_ID_INDEX] > 1 {
        return SignerError::invalid_input_err("invalid TRON payment signature");
    }
    let address = Signer::recover_ethereum_address(&hash, &signature)?;
    if TronAddress::from_hex(&address) != Some(TronAddress::parse(sender)?) {
        return SignerError::invalid_input_err("TRON payment signature owner mismatch");
    }
    Ok(hex::encode(hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_sol_types::SolCall;
    use bsv_home_evm::permit2::IAllowanceTransfer;
    use num_bigint::BigUint;
    use primitives::ApprovalData;

    const PAYLOAD: &str = include_str!("../../../bsv_home_wallet_connect/testdata/tron_send_transaction.json");
    const PERMIT2_PAYLOAD: &str = include_str!("../../testdata/wallet_connect_permit2_approval.json");
    const ABI_WORD_BYTES: usize = 32;
    #[cfg(feature = "signer")]
    const NATIVE_PAYMENT: &str = include_str!("../../testdata/transaction_coin_transfer.json");
    #[cfg(feature = "signer")]
    const NATIVE_SENDER: &str = "TEB39Rt69QkgD1BKhqaRNqGxfQzCarkRCb";
    #[cfg(feature = "signer")]
    const NATIVE_RECIPIENT: &str = "TFanVGmnjKqFRd8dVQAccZE2t42Vu4rv51";

    #[cfg(feature = "signer")]
    #[test]
    fn test_validate_signed_native_transfer() {
        assert_eq!(
            validate_signed_native_transfer(NATIVE_PAYMENT, NATIVE_SENDER, NATIVE_RECIPIENT, "25000000").unwrap(),
            "5a9935a1b7be0150a511111582bbfed62ddb873333b3986bd712e6105fe90ad5"
        );
    }

    #[cfg(feature = "signer")]
    fn encode_payment(mut payment: Value) -> String {
        let raw_data = serde_json::from_value::<RawDataJson>(payment["raw_data"].clone()).unwrap().encode().unwrap();
        payment["raw_data_hex"] = Value::String(hex::encode(&raw_data));
        payment["txID"] = Value::String(hex::encode(sha256(&raw_data)));
        payment.to_string()
    }

    #[cfg(feature = "signer")]
    #[test]
    fn test_validate_signed_native_transfer_rejects_unconfirmed_fields() {
        for (sender, recipient, value) in [
            (NATIVE_RECIPIENT, NATIVE_RECIPIENT, "25000000"),
            (NATIVE_SENDER, NATIVE_SENDER, "25000000"),
            (NATIVE_SENDER, NATIVE_RECIPIENT, "25000001"),
            (NATIVE_SENDER, NATIVE_RECIPIENT, "0"),
        ] {
            assert_eq!(
                validate_signed_native_transfer(NATIVE_PAYMENT, sender, recipient, value).unwrap_err().to_string(),
                "Invalid input: TRON payment does not match confirmed sender, recipient, or value"
            );
        }
        for value in ["", "-1", "+25000000", "0.25", "18446744073709551616"] {
            assert!(validate_signed_native_transfer(NATIVE_PAYMENT, NATIVE_SENDER, NATIVE_RECIPIENT, value).is_err(), "{value}");
        }
        for (sender, recipient) in [("invalid", NATIVE_RECIPIENT), (NATIVE_SENDER, "invalid")] {
            assert!(validate_signed_native_transfer(NATIVE_PAYMENT, sender, recipient, "25000000").is_err());
        }
    }

    #[cfg(feature = "signer")]
    #[test]
    fn test_validate_signed_native_transfer_rejects_mismatched_hash_and_raw_data() {
        let payment: Value = serde_json::from_str(NATIVE_PAYMENT).unwrap();
        let mut invalid_hash = payment.clone();
        invalid_hash["txID"] = Value::String("00".repeat(32));
        assert_eq!(
            validate_signed_native_transfer(&invalid_hash.to_string(), NATIVE_SENDER, NATIVE_RECIPIENT, "25000000")
                .unwrap_err()
                .to_string(),
            "Invalid input: transaction ID does not match hash of raw_data_hex"
        );
        for (field, value) in [
            ("owner_address", Value::String(NATIVE_RECIPIENT.into())),
            ("to_address", Value::String(NATIVE_SENDER.into())),
            ("amount", Value::from(25000001)),
        ] {
            let mut invalid_data = payment.clone();
            invalid_data["raw_data"]["contract"][0]["parameter"]["value"][field] = value;
            assert_eq!(
                validate_signed_native_transfer(&invalid_data.to_string(), NATIVE_SENDER, NATIVE_RECIPIENT, "25000000")
                    .unwrap_err()
                    .to_string(),
                "Invalid input: raw_data does not match raw_data_hex",
                "{field}"
            );
        }
        let mut invalid_data = payment;
        invalid_data.as_object_mut().unwrap().remove("txID");
        invalid_data["raw_data_hex"] = Value::String("00".into());
        assert_eq!(
            validate_signed_native_transfer(&invalid_data.to_string(), NATIVE_SENDER, NATIVE_RECIPIENT, "25000000")
                .unwrap_err()
                .to_string(),
            "Invalid input: raw_data does not match raw_data_hex"
        );
    }

    #[cfg(feature = "signer")]
    #[test]
    fn test_validate_signed_native_transfer_rejects_additional_or_different_contracts() {
        let payment: Value = serde_json::from_str(NATIVE_PAYMENT).unwrap();
        let mut duplicate = payment.clone();
        let contract = duplicate["raw_data"]["contract"][0].clone();
        duplicate["raw_data"]["contract"].as_array_mut().unwrap().push(contract);
        assert_eq!(
            validate_signed_native_transfer(&encode_payment(duplicate), NATIVE_SENDER, NATIVE_RECIPIENT, "25000000")
                .unwrap_err()
                .to_string(),
            "Invalid input: TRON payment must have exactly one contract"
        );
        let mut memo = payment.clone();
        memo["raw_data"]["data"] = Value::String("01".into());
        assert_eq!(
            validate_signed_native_transfer(&encode_payment(memo), NATIVE_SENDER, NATIVE_RECIPIENT, "25000000")
                .unwrap_err()
                .to_string(),
            "Invalid input: TRON payment does not match confirmed sender, recipient, or value"
        );
        let token_payment = include_str!("../../testdata/transaction_token_transfer.json");
        assert_eq!(
            validate_signed_native_transfer(token_payment, NATIVE_SENDER, NATIVE_RECIPIENT, "25000000")
                .unwrap_err()
                .to_string(),
            "Invalid input: TRON payment is not a native transfer"
        );
        for type_url in [Value::Null, Value::String("type.googleapis.com/protocol.TriggerSmartContract".into())] {
            let mut invalid_type = payment.clone();
            invalid_type["raw_data"]["contract"][0]["parameter"]["type_url"] = type_url;
            assert_eq!(
                validate_signed_native_transfer(&invalid_type.to_string(), NATIVE_SENDER, NATIVE_RECIPIENT, "25000000")
                    .unwrap_err()
                    .to_string(),
                "Invalid input: TRON payment is not a native transfer"
            );
        }
    }

    #[cfg(feature = "signer")]
    #[test]
    fn test_validate_signed_native_transfer_rejects_invalid_signature_identity() {
        let payment: Value = serde_json::from_str(NATIVE_PAYMENT).unwrap();
        for signature in [Value::Null, Value::Array(vec![]), Value::Array(vec![payment["signature"][0].clone(); 2])] {
            let mut invalid_signature = payment.clone();
            invalid_signature["signature"] = signature;
            assert_eq!(
                validate_signed_native_transfer(&invalid_signature.to_string(), NATIVE_SENDER, NATIVE_RECIPIENT, "25000000")
                    .unwrap_err()
                    .to_string(),
                "Invalid input: missing TRON payment signature"
            );
        }
        for signature in ["00".repeat(64), "00".repeat(65), format!("{}1b", "00".repeat(64)), "invalid".into()] {
            let mut invalid_signature = payment.clone();
            invalid_signature["signature"][0] = Value::String(signature);
            assert!(validate_signed_native_transfer(&invalid_signature.to_string(), NATIVE_SENDER, NATIVE_RECIPIENT, "25000000").is_err());
        }
        let mut changed = payment;
        changed["raw_data"]["contract"][0]["parameter"]["value"]["amount"] = Value::from(25000001);
        assert_eq!(
            validate_signed_native_transfer(&encode_payment(changed), NATIVE_SENDER, NATIVE_RECIPIENT, "25000001")
                .unwrap_err()
                .to_string(),
            "Invalid input: TRON payment signature owner mismatch"
        );
    }

    fn payload_with_calldata(payload: &str, data: &str) -> String {
        let mut payload: Value = serde_json::from_str(payload).unwrap();
        payload["transaction"]["raw_data"]["contract"][0]["parameter"]["value"]["data"] = Value::String(data.to_string());
        encode_payload(payload)
    }

    fn encode_payload(mut payload: Value) -> String {
        let raw_data = serde_json::from_value::<RawDataJson>(payload["transaction"]["raw_data"].clone()).unwrap().encode().unwrap();
        payload["transaction"]["raw_data_hex"] = Value::String(hex::encode(&raw_data));
        payload["transaction"]["txID"] = Value::String(hex::encode(sha256(&raw_data)));
        payload.to_string()
    }

    #[test]
    fn test_decode_wallet_connect_approval() {
        let selector_and_spender = "095ea7b300000000000000000000000060e00625a95cbc180f290e2611c826f90eeba56f";
        for (amount, is_unlimited) in [(BigUint::from(0u32), false), (BigUint::from(100u32), false), (BigUint::from_bytes_be(&[0xff; 32]), true)] {
            let payload = payload_with_calldata(PAYLOAD, &format!("{selector_and_spender}{amount:064x}"));
            assert_eq!(
                decode_wallet_connect_approval(&payload).unwrap().map(|decoded| decoded.approval),
                Some(ApprovalData {
                    token: "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".to_string(),
                    spender: "TJoSEwEqt7cT3TUwmEoUYnYs5cZR3xSukM".to_string(),
                    value: amount,
                    is_unlimited,
                })
            );
        }
        assert!(decode_wallet_connect_approval(PAYLOAD).unwrap().is_some());
        assert_eq!(decode_wallet_connect_approval(&payload_with_calldata(PAYLOAD, "deadbeef")).unwrap(), None);
        assert!(decode_wallet_connect_approval(&payload_with_calldata(PAYLOAD, "095ea7b3abcd")).is_err());
        assert!(decode_wallet_connect_approval(&payload_with_calldata(PAYLOAD, &format!("095ea7b3{}{}", "ff".repeat(32), "00".repeat(32)))).is_err());
    }

    #[test]
    fn test_decode_wallet_connect_permit2_approval() {
        let expected = TransactionApproval {
            approval: ApprovalData {
                token: "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".to_string(),
                spender: "TQqgNg13s2DjvXhW1ky4v6TsR8wZGvb7Y4".to_string(),
                value: BigUint::from_bytes_be(&[0xff; 20]),
                is_unlimited: true,
            },
            contract: "TTJxU3P8rHycAyFY4kVtGNfmnMH4ezcuM9".to_string(),
            expiration: Some(1791689320),
        };
        assert_eq!(decode_wallet_connect_approval(PERMIT2_PAYLOAD).unwrap(), Some(expected.clone()));
        let payload: Value = serde_json::from_str(PERMIT2_PAYLOAD).unwrap();
        let data = payload["transaction"]["raw_data"]["contract"][0]["parameter"]["value"]["data"].as_str().unwrap();
        let mut approval = IAllowanceTransfer::approveCall::abi_decode_validate(&hex::decode(data).unwrap()).unwrap();
        for amount in [0u64, 100] {
            approval.amount = amount.try_into().unwrap();
            let data = hex::encode(approval.abi_encode());
            assert_eq!(
                decode_wallet_connect_approval(&payload_with_calldata(PERMIT2_PAYLOAD, &data)).unwrap(),
                Some(TransactionApproval {
                    approval: ApprovalData {
                        value: BigUint::from(amount),
                        is_unlimited: false,
                        ..expected.approval.clone()
                    },
                    ..expected.clone()
                })
            );
        }
    }

    #[test]
    fn test_decode_wallet_connect_permit2_approval_validation() {
        let payload: Value = serde_json::from_str(PERMIT2_PAYLOAD).unwrap();
        let data = payload["transaction"]["raw_data"]["contract"][0]["parameter"]["value"]["data"].as_str().unwrap();
        let malformed = [data[..data.len() - 2].to_string(), format!("{data}00")];
        for (argument_index, argument) in ["token", "spender", "amount", "expiration"].into_iter().enumerate() {
            let mut data = hex::decode(data).unwrap();
            let word_start = IAllowanceTransfer::approveCall::SELECTOR.len() + argument_index * ABI_WORD_BYTES;
            data[word_start] = 1;
            assert!(
                decode_wallet_connect_approval(&payload_with_calldata(PERMIT2_PAYLOAD, &hex::encode(data))).is_err(),
                "{argument}"
            );
        }
        for data in malformed {
            assert!(decode_wallet_connect_approval(&payload_with_calldata(PERMIT2_PAYLOAD, &data)).is_err());
        }
        let mut unknown_contract = payload.clone();
        unknown_contract["transaction"]["raw_data"]["contract"][0]["parameter"]["value"]["contract_address"] =
            Value::String("41a614f803b6fd780986a42c78ec9c7f77e6ded13c".to_string());
        assert_eq!(decode_wallet_connect_approval(&encode_payload(unknown_contract)).unwrap(), None);
        for field in ["call_value", "call_token_value"] {
            let mut payload = payload.clone();
            payload["transaction"]["raw_data"]["contract"][0]["parameter"]["value"][field] = Value::from(1);
            assert!(decode_wallet_connect_approval(&encode_payload(payload)).is_err());
        }
        assert!(decode_wallet_connect_approval(&PERMIT2_PAYLOAD.replacen("87517c45", "87517c44", 1)).is_err());
    }

    #[test]
    fn test_decode_wallet_connect_approval_with_additional_effects() {
        for field in ["call_value", "call_token_value"] {
            let mut payload: Value = serde_json::from_str(PAYLOAD).unwrap();
            payload["transaction"]["raw_data"]["contract"][0]["parameter"]["value"][field] = Value::from(1);
            assert!(decode_wallet_connect_approval(&encode_payload(payload)).is_err());
        }
        let mut payload: Value = serde_json::from_str(PAYLOAD).unwrap();
        let contract = payload["transaction"]["raw_data"]["contract"][0].clone();
        payload["transaction"]["raw_data"]["contract"].as_array_mut().unwrap().push(contract);
        assert_eq!(decode_wallet_connect_approval(&encode_payload(payload)).unwrap(), None);
    }

    #[test]
    fn test_decode_wallet_connect_approval_rejects_mismatched_signed_data() {
        let payload = PAYLOAD.replace("095ea7b3", "a9059cbb").replacen("a9059cbb", "095ea7b3", 1);
        assert!(decode_wallet_connect_approval(&payload).is_err());
        let mut payload: Value = serde_json::from_str(PAYLOAD).unwrap();
        payload["transaction"].as_object_mut().unwrap().remove("txID");
        payload["transaction"]["raw_data"]["contract"][0]["parameter"]["value"]["data"] = Value::String("deadbeef".to_string());
        assert!(decode_wallet_connect_approval(&payload.to_string()).is_err());
    }
}
