use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};

use super::BsvHomeOfflineIdentityService;

const PUBLIC_KEY: &str = "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEaxfR8uEsQkf4vOblY6RA8ncDfYEt6zOg9KE5RdiYwpZP40Li_hp_m47n60p8D54WK84zV2sxXs7LtkBoN79R9Q";
const FINGERPRINT: &str = "8ed1a0a6c5e6365b5b0c3bb7af87f6cc093cd53feed8a5073409602f6ec4a12a";

fn challenge() -> Value {
    json!({"version":1,"kind":"wowpay-offline-identity-challenge","id":"11111111111111111111111111111111","actionHash":"2222222222222222222222222222222222222222222222222222222222222222","purpose":"Enroll","expiresAt":"2026-11-03T00:00:00Z","userId":"42","label":"Offline factor","fingerprint":FINGERPRINT})
}

#[test]
fn test_independent_public_key_fingerprint_and_challenge_binding() {
    let service = BsvHomeOfflineIdentityService::new();
    assert_eq!(service.public_key_fingerprint(PUBLIC_KEY.into()).unwrap(), FINGERPRINT);
    let document = challenge();
    assert_eq!(service.review_challenge(document.to_string(), PUBLIC_KEY.into()).unwrap().user_id, "42");
    let changes: [fn(&mut Value); 5] = [
        |v| v["fingerprint"] = json!("0".repeat(64)),
        |v| v["actionHash"] = json!("A".repeat(64)),
        |v| v["purpose"] = json!("Unknown"),
        |v| v["userId"] = json!("not-an-account"),
        |v| v["kind"] = json!("wowpay-offline-identity-proof"),
    ];
    for change in changes {
        let mut changed = document.clone();
        change(&mut changed);
        assert!(service.review_challenge(changed.to_string(), PUBLIC_KEY.into()).is_err());
    }
    assert!(service.public_key_fingerprint(format!("{PUBLIC_KEY}=")).is_err());
}

#[test]
fn test_backup_rejects_changed_kdf_parameters_and_nonce() {
    let service = BsvHomeOfflineIdentityService::new();
    let document = service
        .backup_document(URL_SAFE_NO_PAD.encode([1u8; 16]), URL_SAFE_NO_PAD.encode([2u8; 12]), URL_SAFE_NO_PAD.encode([3u8; 32]))
        .unwrap();
    assert_eq!(service.read_backup(document.clone()).unwrap().iterations, 310000);
    let mut changed: Value = serde_json::from_str(&document).unwrap();
    changed["iterations"] = json!(1);
    assert!(service.read_backup(changed.to_string()).is_err());
    changed = serde_json::from_str(&document).unwrap();
    changed["nonce"] = json!(URL_SAFE_NO_PAD.encode([0u8; 11]));
    assert!(service.read_backup(changed.to_string()).is_err());
    assert_eq!(
        service
            .read_backup(format!("{{\"version\":1,\"version\":1,{}}}", &document[1..document.len() - 1]))
            .is_err(),
        true
    );
}
