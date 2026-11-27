use std::fs;

use serde_json::{json, Value};
use tempfile::TempDir;

use super::*;
use crate::transport::Participant;

fn fixture(mode: &str) -> (TempDir, [u8; 32], Value) {
    let first = PartyIndex::new(1).unwrap();
    let second = PartyIndex::new(2).unwrap();
    let session_id = random::<32>().unwrap().to_vec();
    let mut a = DkgSession::<k256::Secp256k1>::new(
        Parameters::new(2, 2).unwrap(),
        first,
        session_id.clone(),
    );
    let mut b =
        DkgSession::<k256::Secp256k1>::new(Parameters::new(2, 2).unwrap(), second, session_id);
    let a1 = a.phase1();
    let b1 = b.phase1();
    let (a_proof, _, a_derivation2) = a.phase2(&[a1[0], b1[0]]).unwrap();
    let (b_proof, b_zero2, b_derivation2) = b.phase2(&[a1[1], b1[1]]).unwrap();
    let (_, _, a_derivation3) = a.phase3().unwrap();
    let (b_zero3, b_multiplication, b_derivation3) = b.phase3().unwrap();
    let (mut party, _) = a
        .phase4(
            &[a_proof, b_proof],
            &b_zero2,
            &b_zero3,
            &b_multiplication,
            &BTreeMap::from([(first, a_derivation2), (second, b_derivation2)]),
            &BTreeMap::from([(first, a_derivation3), (second, b_derivation3)]),
            |_| String::new(),
        )
        .unwrap();
    party.address = bsv_address(&party);
    let state = State {
        party,
        wallet_id: "test-wallet".into(),
        epoch: 0,
        mode: mode.into(),
        candidate: None,
        used: BTreeSet::new(),
        backup_confirmed: false,
        restored: mode == "RESTORED_REFRESH_REQUIRED",
        identity_secret: identity::new_secret().unwrap(),
    };
    let expected = context(&state, false).unwrap();
    let recovery_key = random::<32>().unwrap();
    let backup = encrypt_backup(&state.party, &recovery_key, expected.clone()).unwrap();
    let command = json!({"op":"verify_backup","backup":backup,"recovery_key":hex::encode(recovery_key),"expected":expected});
    let directory = tempfile::tempdir().unwrap();
    let key = random::<32>().unwrap();
    persist(directory.path(), &key, &state).unwrap();
    (directory, key, command)
}

fn ask(actor: &mut Participant, command: &Value) -> Value {
    serde_json::from_str(
        &actor
            .exchange(&serde_json::to_vec(command).unwrap())
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn test_verify_backup_validates_actual_ciphertext_without_changing_active_state() {
    let (directory, key, command) = fixture("ACTIVE");
    let original = fs::read(directory.path().join("participant.share")).unwrap();
    let mut actor = Participant::open(
        directory.path().into(),
        "test-wallet".into(),
        Zeroizing::new(key),
    )
    .unwrap();
    assert_eq!(
        ask(&mut actor, &command),
        json!({"verified":true,"context":command["expected"]})
    );
    assert_eq!(ask(&mut actor, &json!({"op":"status"}))["status"], "ACTIVE");
    assert_eq!(
        fs::read(directory.path().join("participant.share")).unwrap(),
        original
    );
}

#[test]
fn test_verify_backup_rejects_wrong_recovery_key_and_ciphertext_tampering() {
    let (directory, key, command) = fixture("ACTIVE");
    let original = fs::read(directory.path().join("participant.share")).unwrap();
    for changed_ciphertext in [false, true] {
        let mut changed = command.clone();
        if changed_ciphertext {
            let mut bytes = hex::decode(changed["backup"]["ciphertext"].as_str().unwrap()).unwrap();
            bytes[0] ^= 1;
            changed["backup"]["ciphertext"] = json!(hex::encode(bytes));
        } else {
            let mut bytes = hex::decode(changed["recovery_key"].as_str().unwrap()).unwrap();
            bytes[0] ^= 1;
            changed["recovery_key"] = json!(hex::encode(bytes));
        }
        let mut actor = Participant::open(
            directory.path().into(),
            "test-wallet".into(),
            Zeroizing::new(key),
        )
        .unwrap();
        assert_eq!(
            ask(&mut actor, &changed)["error"],
            "BACKUP_AUTHENTICATION_FAILED"
        );
        assert_eq!(
            fs::read(directory.path().join("participant.share")).unwrap(),
            original
        );
    }
}

#[test]
fn test_verify_backup_rejects_cross_context_and_never_activates_restored_state() {
    let (directory, key, command) = fixture("RESTORED_REFRESH_REQUIRED");
    let original = fs::read(directory.path().join("participant.share")).unwrap();
    let backup: EncryptedBackup = serde_json::from_value(command["backup"].clone()).unwrap();
    let expected: BackupContext = serde_json::from_value(command["expected"].clone()).unwrap();
    let recovery_key: [u8; 32] = hex::decode(command["recovery_key"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let recovered = decrypt_backup(&backup, &recovery_key, &expected).unwrap();
    let changes: [fn(&mut Value); 3] = [
        |value| value["expected"]["wallet_id"] = json!("other-wallet"),
        |value| value["expected"]["epoch"] = json!(1),
        |value| value["expected"]["protocol"] = json!("unsupported-protocol"),
    ];
    for change in changes {
        let mut changed = command.clone();
        change(&mut changed);
        let changed_context: BackupContext =
            serde_json::from_value(changed["expected"].clone()).unwrap();
        if changed_context.protocol == PROTOCOL {
            changed["backup"] = serde_json::to_value(
                encrypt_backup(&recovered, &recovery_key, changed_context).unwrap(),
            )
            .unwrap();
        }
        let mut actor = Participant::open(
            directory.path().into(),
            "test-wallet".into(),
            Zeroizing::new(key),
        )
        .unwrap();
        assert_eq!(
            ask(&mut actor, &changed)["error"],
            "BACKUP_CONTEXT_MISMATCH"
        );
    }
    let mut actor = Participant::open(
        directory.path().into(),
        "test-wallet".into(),
        Zeroizing::new(key),
    )
    .unwrap();
    assert_eq!(ask(&mut actor, &command)["verified"], true);
    assert_eq!(
        ask(&mut actor, &json!({"op":"status"}))["status"],
        "RESTORED_REFRESH_REQUIRED"
    );
    assert_eq!(
        fs::read(directory.path().join("participant.share")).unwrap(),
        original
    );
}
