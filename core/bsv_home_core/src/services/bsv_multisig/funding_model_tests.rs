use bsv_home_bsv::multisig::{Funding, FundingAddress, funding_transaction_id, policy_descriptor, review_funding, sign_funding_with_keys};
use primitives::{BsvAddressBranch, BsvAddressLocator, Chain, UTXO};
use serde_json::Value;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::funding_model::{BsvHomeBsvMultisigFundingTarget, FundingCheckpoint, LegacyFundingCheckpoint, PayerFundingBinding, PayerFundingCheckpoint};
use super::model::{BsvHomeBsvMultisigFundingStatus, State};
use super::payment::bsv_multisig_payment_request_from_script;
use super::testkit::{Device, TEST_WALLET_ID, fixture, member};

pub(super) fn checkpoint(public: bool) -> PayerFundingCheckpoint {
    let payer = Device::wallet(2);
    let address = member(2, true).address;
    let funding = Funding {
        chain: Chain::BitcoinSv,
        policy: fixture().proposal.policy,
        sender_address: address.clone(),
        amount: 100000,
        fee_rate: 100,
        utxos: vec![UTXO {
            transaction_id: "11".repeat(32),
            vout: 0,
            value: 200000u64.into(),
            address: address.clone(),
        }],
        addresses: vec![FundingAddress {
            address,
            locator: BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 0,
            },
        }],
        change: None,
    };
    let review = review_funding(&funding).unwrap();
    let target = if public {
        BsvHomeBsvMultisigFundingTarget::Public {
            request: bsv_multisig_payment_request_from_script("Public group".into(), policy_descriptor(&funding.policy).unwrap().locking_script, "0.001".into()).unwrap(),
        }
    } else {
        BsvHomeBsvMultisigFundingTarget::Member { wallet_id: TEST_WALLET_ID.into() }
    };
    let scope = if public {
        format!("https://bsvhome.com|{}", payer.id.id())
    } else {
        format!("https://bsvhome.com|{}|{TEST_WALLET_ID}", Device::wallet(1).id.id())
    };
    let raw = sign_funding_with_keys(&funding, &review, |_| Ok(Zeroizing::new(vec![2; 32]))).unwrap();
    let transaction_id = funding_transaction_id(&funding, &review, &raw).unwrap();
    PayerFundingCheckpoint {
        binding: PayerFundingBinding {
            scope,
            operation_id: Uuid::new_v4().to_string(),
            payer_wallet_id: payer.id,
            target,
            funding_json: serde_json::to_string(&funding).unwrap(),
            review_json: serde_json::to_string(&review).unwrap(),
        },
        raw,
        transaction_id,
        status: BsvHomeBsvMultisigFundingStatus::Pending,
        error_code: None,
    }
}

#[test]
fn test_checkpoint_roundtrip_preserves_signed_payload_and_public_target() {
    for public in [false, true] {
        let original = checkpoint(public);
        let json = serde_json::to_string(&FundingCheckpoint::Current(original.clone())).unwrap();
        let decoded = serde_json::from_str::<FundingCheckpoint>(&json).unwrap().validated_checkpoint().unwrap();
        assert!(decoded == original);
    }
}

#[test]
fn test_legacy_checkpoint_retains_wire_and_derives_stable_identifier_only_after_sdk_validation() {
    let original = checkpoint(false);
    let legacy = LegacyFundingCheckpoint {
        scope: original.binding.scope.clone(),
        wallet_id: TEST_WALLET_ID.into(),
        payer_wallet_id: original.binding.payer_wallet_id.id(),
        funding_json: original.binding.funding_json.clone(),
        review_json: original.binding.review_json.clone(),
        raw: original.raw.clone(),
        transaction_id: original.transaction_id.clone(),
        status: original.status.clone(),
        error_code: None,
    };
    let json = serde_json::to_string(&legacy).unwrap();
    let decoded = serde_json::from_str::<FundingCheckpoint>(&json).unwrap();
    assert!(serde_json::to_string(&decoded).unwrap() == json);
    let first = decoded.validated_checkpoint().unwrap();
    let second = serde_json::from_str::<FundingCheckpoint>(&json).unwrap().validated_checkpoint().unwrap();
    assert!(first == second);
    assert!(first.raw == original.raw);
    assert!(first.binding.funding_json == original.binding.funding_json);
    assert!(first.binding.review_json == original.binding.review_json);
    let mut invalid = legacy;
    invalid.transaction_id = "ff".repeat(32);
    assert!(invalid.validated_checkpoint().is_err());
}

#[test]
fn test_checkpoint_rejects_ambiguous_null_unknown_and_duplicate_representation_fields() {
    let original = checkpoint(true);
    let json = serde_json::to_string(&FundingCheckpoint::Current(original)).unwrap();
    for extra in [
        r#""walletId":null"#,
        r#""walletId":"group""#,
        r#""formatVersion":1"#,
        r#""target":{"kind":"member","walletId":"group"}"#,
    ] {
        let invalid = format!("{{{extra},{}", &json[1..]);
        assert!(serde_json::from_str::<FundingCheckpoint>(&invalid).is_err());
    }
    let mut value: Value = serde_json::from_str(&json).unwrap();
    for field in ["target", "formatVersion", "operationId"] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<FundingCheckpoint>(missing).is_err());
        let mut null = value.clone();
        null[field] = Value::Null;
        assert!(serde_json::from_value::<FundingCheckpoint>(null).is_err());
    }
    value["formatVersion"] = Value::from(2);
    assert!(serde_json::from_value::<FundingCheckpoint>(value).is_err());
    let member = serde_json::to_string(&FundingCheckpoint::Current(checkpoint(false))).unwrap();
    let duplicate_nested = member.replace(r#""walletId":"team-wallet""#, r#""walletId":"team-wallet","walletId":"other""#);
    assert!(serde_json::from_str::<FundingCheckpoint>(&duplicate_nested).is_err());
}

#[test]
fn test_state_rejects_duplicate_scope_even_when_each_signed_checkpoint_is_individually_valid() {
    let checkpoint = checkpoint(false);
    let row = serde_json::to_string(&FundingCheckpoint::Current(checkpoint.clone())).unwrap();
    let scope = serde_json::to_string(&checkpoint.binding.scope).unwrap();
    let state = format!(r#"{{"trusted":{{}},"memberNames":{{}},"intentions":{{}},"funding":{{{scope}:{row},{scope}:{row}}}}}"#);
    assert!(serde_json::from_str::<State>(&state).is_err());
    let legacy = r#"{"walletId":"one","walletId":"two","scope":"x","payerWalletId":"x","fundingJson":"x","reviewJson":"x","raw":"x","transactionId":"x","status":"Pending","errorCode":null}"#;
    assert!(serde_json::from_str::<FundingCheckpoint>(&legacy).is_err());
}

#[test]
fn test_public_binding_rejects_changed_amount_policy_and_payer_scope() {
    let original = checkpoint(true);
    let mut changed = original.clone();
    if let BsvHomeBsvMultisigFundingTarget::Public { request } = &mut changed.binding.target {
        request.amount = "0.002".into();
    }
    assert!(changed.validate().is_err());
    let mut changed = original.clone();
    if let BsvHomeBsvMultisigFundingTarget::Public { request } = &mut changed.binding.target {
        request.threshold = 1;
    }
    assert!(changed.validate().is_err());
    let mut changed = original;
    changed.binding.scope = format!("https://bsvhome.com|{}", Device::wallet(1).id.id());
    assert!(changed.validate().is_err());
}
