use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use bsv_home_bsv::multisig::policy_descriptor;
use futures::executor::block_on;

use super::{BsvHomeBsvMultisigPaymentRequest, bsv_multisig_payment_request_from_script, bsv_multisig_payment_request_qr_payload, parse_bsv_multisig_payment_request};
use crate::services::bsv_multisig::testkit::{Device, fixture};

fn request() -> BsvHomeBsvMultisigPaymentRequest {
    let descriptor = policy_descriptor(&fixture().proposal.policy).unwrap();
    BsvHomeBsvMultisigPaymentRequest {
        name: "Team wallet".into(),
        threshold: descriptor.policy.threshold,
        public_keys: descriptor.policy.public_keys,
        fingerprint: descriptor.configuration_hash,
        amount: "0.00042".into(),
    }
}

#[test]
fn test_canonical_payment_request_preserves_the_android_array_and_unpadded_uri() {
    let value = request();
    let encoded = bsv_multisig_payment_request_qr_payload(value.clone()).unwrap();
    let data = URL_SAFE_NO_PAD.decode(encoded.strip_prefix("bsvhome://multisig/pay?v=1&request=").unwrap()).unwrap();
    let fields: (String, u32, Vec<String>, String, String) = serde_json::from_slice(&data).unwrap();
    assert_eq!(
        fields,
        (
            value.name.clone(),
            value.threshold,
            value.public_keys.clone(),
            value.fingerprint.clone(),
            value.amount.clone()
        )
    );
    assert_eq!(parse_bsv_multisig_payment_request(format!("  {encoded}\n")).unwrap(), value);
    let descriptor = policy_descriptor(&fixture().proposal.policy).unwrap();
    assert_eq!(
        bsv_multisig_payment_request_from_script("Team wallet".into(), descriptor.locking_script, "0.00042".into()).unwrap(),
        value,
    );
}

#[test]
fn test_payment_request_rejects_changed_policy_fingerprint_names_and_amounts() {
    let original = request();
    let mut changed = original.clone();
    changed.fingerprint = "0".repeat(64);
    assert!(bsv_multisig_payment_request_qr_payload(changed).is_err());
    let mut changed = original.clone();
    changed.public_keys.reverse();
    assert!(bsv_multisig_payment_request_qr_payload(changed).is_err());
    let mut changed = original.clone();
    changed.public_keys[1] = changed.public_keys[0].clone();
    assert!(bsv_multisig_payment_request_qr_payload(changed).is_err());
    for amount in ["0", "01", "0.000000001", "-1", "+1", "1e-8", "1.", ".1", "21000000.00000001", " 1"] {
        let mut changed = original.clone();
        changed.amount = amount.into();
        assert!(bsv_multisig_payment_request_qr_payload(changed).is_err(), "amount must be rejected");
    }
    for name in [" ", "Team\nwallet", "Team\u{200b}wallet", "Team\u{202e}wallet"] {
        let mut changed = original.clone();
        changed.name = name.into();
        assert!(bsv_multisig_payment_request_qr_payload(changed).is_err());
    }
    let mut changed = original.clone();
    changed.name = "😀".repeat(41);
    assert!(bsv_multisig_payment_request_qr_payload(changed).is_err());
    let mut optional = original;
    optional.amount.clear();
    assert_eq!(
        parse_bsv_multisig_payment_request(bsv_multisig_payment_request_qr_payload(optional.clone()).unwrap()).unwrap(),
        optional
    );
}

#[test]
fn test_payment_request_rejects_other_schemes_extra_parameters_and_invalid_encoding() {
    let encoded = bsv_multisig_payment_request_qr_payload(request()).unwrap();
    for code in [
        encoded.replace("bsvhome://", "https://"),
        encoded.replace("v=1", "v=2"),
        format!("{encoded}&extra=1"),
        format!("{encoded}="),
        format!("bsvhome://multisig/pay?v=1&request={}", "A".repeat(1601)),
        "bsvhome://multisig/pay?v=1&request=_w".into(),
        "bsvhome://multisig/pay?v=1&request=W10".into(),
    ] {
        assert!(parse_bsv_multisig_payment_request(code).is_err());
    }
    assert!(bsv_multisig_payment_request_from_script("Team".into(), "76a91400".into(), "1".into()).is_err());
}

#[test]
fn test_wallet_payment_request_requires_local_configuration_trust_and_cannot_outlive_cancel() {
    block_on(async {
        let device = Device::new(1);
        device.service.load().await.unwrap();
        assert!(device.service.payment_request("team-wallet".into(), "1".into()).is_err());
        device.service.approve_configuration("team-wallet".into(), 7).await.unwrap();
        let value = device.service.payment_request("team-wallet".into(), "0.00042".into()).unwrap();
        assert_eq!(value, request());
        device.service.cancel();
        assert!(device.service.payment_request("team-wallet".into(), "0.00042".into()).is_err());
    });
}

#[test]
fn test_member_recipient_scan_prefills_mainnet_p2pkh_and_exact_bsv_amount_only() {
    use super::bsv_multisig_scan_recipient;
    const ADDRESS: &str = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5";
    for value in [ADDRESS.to_string(), format!("  {ADDRESS}\n"), format!("bitcoinsv:{ADDRESS}?amount=&memo=")] {
        let payment = bsv_multisig_scan_recipient(value).unwrap();
        assert_eq!(payment.recipient.address, ADDRESS);
        assert_eq!(payment.amount, None);
        assert_eq!(payment.recipient.memo, None);
        assert!(payment.recipient.references.is_empty());
        assert_eq!(payment.recipient.name, None);
    }
    for (value, expected) in [
        (format!("{ADDRESS}?amount=0.00000001"), "0.00000001"),
        (format!("bitcoinsv:{ADDRESS}?amount=0.12345678"), "0.12345678"),
        (format!("bitcoinsv:{ADDRESS}?amount=21000000"), "21000000"),
        (format!("bitcoinsv:{ADDRESS}?amount=1.000000000"), "1"),
    ] {
        let payment = bsv_multisig_scan_recipient(value).unwrap();
        assert_eq!(payment.recipient.address, ADDRESS);
        assert_eq!(payment.amount.as_deref(), Some(expected));
    }
}

#[test]
fn test_member_recipient_scan_rejects_invalid_explicit_amounts_instead_of_erasing_them() {
    use super::bsv_multisig_scan_recipient;
    const ADDRESS: &str = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5";
    for prefix in ["", "bitcoinsv:"] {
        for amount in ["-1", "0", "0.000000001", "0.123456789", "21000000.00000001", "1e-8", "NaN", "Infinity", "not-a-number"] {
            assert!(bsv_multisig_scan_recipient(format!("{prefix}{ADDRESS}?amount={amount}")).is_err());
        }
    }
}

#[test]
fn test_member_recipient_scan_rejects_other_chains_paymail_p2ms_links_and_non_p2pkh_addresses() {
    use super::bsv_multisig_scan_recipient;
    for value in [
        "bitcoin:1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5?amount=1",
        "alice@example.com",
        "https://example.com/pay",
        "solana:https://example.com/pay",
        "bsvhome://multisig/join?v=1&token=abc",
        "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
        "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
        "mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn",
        "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE6",
    ] {
        assert!(bsv_multisig_scan_recipient(value.into()).is_err());
    }
    assert!(bsv_multisig_scan_recipient(bsv_multisig_payment_request_qr_payload(request()).unwrap()).is_err());
}

#[test]
fn test_member_recipient_scan_does_not_silently_drop_memo_or_references() {
    use super::{bsv_multisig_scan_recipient, scan_recipient_request};
    use crate::models::payment::BsvHomePaymentRequest;
    use primitives::{AssetId, Chain};
    const ADDRESS: &str = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5";
    assert!(bsv_multisig_scan_recipient(format!("bitcoinsv:{ADDRESS}?amount=1&memo=note")).is_err());
    assert!(bsv_multisig_scan_recipient(format!("bitcoinsv:{ADDRESS}?memo=%20")).is_err());
    let original = BsvHomePaymentRequest {
        address: ADDRESS.into(),
        amount: None,
        memo: None,
        label: None,
        references: None,
        asset_id: Some(AssetId::from_chain(Chain::BitcoinSv)),
    };
    let mut referenced = original.clone();
    referenced.references = Some(vec!["reference".into()]);
    assert!(scan_recipient_request(referenced).is_err());
    let mut empty = original;
    empty.references = Some(Vec::new());
    empty.memo = Some(String::new());
    let payment = scan_recipient_request(empty).unwrap();
    assert_eq!(payment.recipient.memo, None);
    assert!(payment.recipient.references.is_empty());
}
