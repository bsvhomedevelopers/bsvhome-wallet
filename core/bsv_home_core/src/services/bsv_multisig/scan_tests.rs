use bsv_home_bsv::multisig::policy_descriptor;

use super::{BsvHomeBsvMultisigScanDestination, bsv_multisig_scan_destination};
use crate::services::bsv_multisig::{BsvHomeBsvMultisigPaymentRequest, bsv_multisig_invitation_qr_payload, bsv_multisig_payment_request_qr_payload, testkit::fixture};

#[test]
fn test_global_scan_validates_public_payment_before_routing() {
    let descriptor = policy_descriptor(&fixture().proposal.policy).unwrap();
    let request = BsvHomeBsvMultisigPaymentRequest {
        name: "Team wallet".into(),
        threshold: descriptor.policy.threshold,
        public_keys: descriptor.policy.public_keys,
        fingerprint: descriptor.configuration_hash,
        amount: "0.00042".into(),
    };
    let code = bsv_multisig_payment_request_qr_payload(request.clone()).unwrap();
    assert_eq!(
        bsv_multisig_scan_destination(format!("  {code}\n")).unwrap(),
        Some(BsvHomeBsvMultisigScanDestination::Payment { request })
    );
    for invalid in [code.replace("v=1", "v=2"), format!("{code}&extra=1"), "bsvhome://multisig/pay?request=invalid".into()] {
        assert!(bsv_multisig_scan_destination(invalid).is_err());
    }
}

#[test]
fn test_global_scan_accepts_canonical_or_raw_invitation_but_rejects_malformed_invitation() {
    let token = "a".repeat(43);
    let code = bsv_multisig_invitation_qr_payload(token.clone()).unwrap();
    for value in [format!("  {token}\n"), code.clone()] {
        assert_eq!(
            bsv_multisig_scan_destination(value).unwrap(),
            Some(BsvHomeBsvMultisigScanDestination::Invitation { token: token.clone() })
        );
    }
    for value in [
        format!("{code}&extra=1"),
        "bsvhome://multisig/join?token=short".into(),
        "bsvhome://multisig/join?other=value".into(),
    ] {
        assert!(bsv_multisig_scan_destination(value).is_err());
    }
}

#[test]
fn test_global_scan_leaves_ordinary_payment_and_other_payloads_to_existing_routes() {
    for value in [
        "1G9ZpqHUWJsT7LtvUfm4qKi8jKGkrFC5st",
        "bitcoin:1G9ZpqHUWJsT7LtvUfm4qKi8jKGkrFC5st?amount=0.001",
        "alice@example.com",
        "wc:session@2?relay-protocol=irn",
        "https://example.com",
        "{\"policy\":{\"threshold\":2}}",
        "bsvhome://multisig/config?value=invalid",
    ] {
        assert_eq!(bsv_multisig_scan_destination(value.into()).unwrap(), None);
    }
}
