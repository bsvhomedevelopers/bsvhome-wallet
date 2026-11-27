use std::{
    env,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::{backup::random, Participant};

struct Peer {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Peer {
    fn open(root: &Path) -> Self {
        let binary = env::var("WOWPAY_MPC_PEER_BINARY").unwrap();
        let mut child = Command::new(binary)
            .arg("2")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut peer = Self {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
        };
        assert_eq!(peer.read()["ready"], true);
        peer
    }
    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    fn ask(&mut self, command: Value) -> Value {
        serde_json::to_writer(&mut self.input, &command).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
        self.read()
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn ask(actor: &mut Participant, command: Value) -> Value {
    serde_json::from_str(
        &actor
            .exchange(&serde_json::to_vec(&command).unwrap())
            .unwrap(),
    )
    .unwrap()
}
fn sid() -> String {
    hex::encode(random::<32>().unwrap())
}
fn received(a: &Value, b: &Value, role: u8) -> Value {
    Value::Array(
        a.as_array()
            .unwrap()
            .iter()
            .chain(b.as_array().unwrap())
            .filter(|m| m["parties"]["receiver"] == role)
            .cloned()
            .collect(),
    )
}

fn grouped(a: &mut Participant, b: &mut Peer, refresh: bool) -> Value {
    let command = if refresh {
        json!({"op":"refresh","epoch":0,"session_id":sid()})
    } else {
        json!({"op":"dkg","wallet_id":"test-wallet","session_id":sid()})
    };
    let x = ask(a, command.clone());
    let y = b.ask(command);
    let x2 = ask(a, json!([x[0], y[0]]));
    let y2 = b.ask(json!([x[1], y[1]]));
    let x3 = ask(a, Value::Null);
    let y3 = b.ask(Value::Null);
    let final_frame = |role| {
        let mut frame = json!([
            [x2[0], y2[0]],
            received(&x2[1], &y2[1], role),
            received(&x3[0], &y3[0], role),
            received(&x3[1], &y3[1], role)
        ]);
        if !refresh {
            frame
                .as_array_mut()
                .unwrap()
                .extend([json!({"1":x2[2],"2":y2[2]}), json!({"1":x3[2],"2":y3[2]})]);
        }
        frame
    };
    let result = ask(a, final_frame(1));
    let other = b.ask(final_frame(2));
    assert_eq!(result["public_key"], other["public_key"]);
    result
}

fn sign(a: &mut Participant, b: &mut Peer, epoch: u64, proof: bool, id: &str) {
    let digest = random::<32>().unwrap();
    let command = json!({"op":if proof {"proof"} else {"sign"},"epoch":epoch,"digest":hex::encode(digest),"session_id":id});
    let x = ask(a, command.clone());
    let y = b.ask(command);
    let x2 = ask(a, received(&x, &y, 1));
    let y2 = b.ask(received(&x, &y, 2));
    let x3 = ask(a, received(&x2, &y2, 1));
    let y3 = b.ask(received(&x2, &y2, 2));
    let result = ask(a, json!([x3, y3]));
    let other = b.ask(json!([x3, y3]));
    assert_eq!(result["der"], other["der"]);
    let signature = secp256k1::ecdsa::Signature::from_der(
        &hex::decode(result["der"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    let key = secp256k1::PublicKey::from_slice(
        &hex::decode(result["public_key"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    secp256k1::Secp256k1::verification_only()
        .verify_ecdsa(secp256k1::Message::from_digest(digest), &signature, &key)
        .unwrap();
}

#[test]
fn test_exclusive_wallet_state_and_transport_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let key = random::<32>().unwrap();
    let mut participant = Participant::open(
        directory.path().into(),
        "test-wallet".into(),
        Zeroizing::new(key),
    )
    .unwrap();
    assert_eq!(
        Participant::open(
            directory.path().into(),
            "test-wallet".into(),
            Zeroizing::new(key)
        )
        .err(),
        Some("MPC_PARTICIPANT_ALREADY_OPEN".into())
    );
    assert_eq!(
        participant.exchange(b"{}\n{}").err(),
        Some("MESSAGE_INVALID".into())
    );
    assert_eq!(
        participant.exchange(b"invalid").err(),
        Some("INVALID_MESSAGE".into())
    );
    drop(participant);
    assert_eq!(
        Participant::open(
            directory.path().into(),
            "test-wallet".into(),
            Zeroizing::new(key)
        )
        .err(),
        None
    );
}

#[test]
#[ignore = "requires independently built server peer via WOWPAY_MPC_PEER_BINARY"]
fn test_real_server_peer_dkg_sign_backup_restore_refresh_and_replay_rejection() {
    let root = tempfile::tempdir().unwrap();
    let key = random::<32>().unwrap();
    let mut a = Participant::open(
        root.path().join("a"),
        "test-wallet".into(),
        Zeroizing::new(key),
    )
    .unwrap();
    let mut b = Peer::open(&root.path().join("b"));
    let identity = grouped(&mut a, &mut b, false);
    sign(&mut a, &mut b, 0, false, &sid());
    let backup = ask(&mut a, json!({"op":"backup"}));
    assert_eq!(backup["backup_verified"], true);
    let before_verification = ask(&mut a, json!({"op":"status"}));
    assert_eq!(
        ask(
            &mut a,
            json!({"op":"verify_backup","backup":backup["backup"],"recovery_key":backup["recovery_key"],"expected":backup["context"]})
        )["verified"],
        true
    );
    assert_eq!(ask(&mut a, json!({"op":"status"})), before_verification);
    assert!(!root.path().join("a/storage.key").exists());
    drop(a);
    let mut a = Participant::open(
        root.path().join("a"),
        "test-wallet".into(),
        Zeroizing::new(key),
    )
    .unwrap();
    assert_eq!(
        ask(&mut a, json!({"op":"status"}))["context"]["public_key"],
        identity["public_key"]
    );
    let mut restored = Participant::open(
        root.path().join("restored"),
        "test-wallet".into(),
        Zeroizing::new(random::<32>().unwrap()),
    )
    .unwrap();
    let restoration = ask(
        &mut restored,
        json!({"op":"restore","backup":backup["backup"],"recovery_key":backup["recovery_key"],"expected":backup["context"]}),
    );
    assert_eq!(restoration["status"], "RESTORED_REFRESH_REQUIRED");
    assert_eq!(
        grouped(&mut restored, &mut b, true)["public_key"],
        identity["public_key"]
    );
    assert_eq!(
        ask(&mut restored, json!({"op":"abort_refresh","epoch":0}))["status"],
        "RESTORED_REFRESH_REQUIRED"
    );
    assert_eq!(
        b.ask(json!({"op":"abort_refresh","epoch":0}))["status"],
        "ACTIVE"
    );
    assert_eq!(
        grouped(&mut restored, &mut b, true)["public_key"],
        identity["public_key"]
    );
    let fresh = ask(&mut restored, json!({"op":"backup"}));
    assert_eq!(fresh["context"]["epoch"], 1);
    sign(&mut restored, &mut b, 1, true, &sid());
    assert_eq!(
        ask(
            &mut restored,
            json!({"op":"commit","epoch":1,"backup_confirmed":true})
        )["epoch"],
        1
    );
    assert_eq!(
        b.ask(json!({"op":"commit","epoch":1,"backup_confirmed":true}))["epoch"],
        1
    );
    let used = sid();
    sign(&mut restored, &mut b, 1, false, &used);
    let rejected = ask(
        &mut restored,
        json!({"op":"sign","epoch":1,"session_id":used,"digest":hex::encode(random::<32>().unwrap())}),
    );
    assert_eq!(rejected["error"], "SESSION_REPLAY");
    let mut tampered = backup["backup"].clone();
    tampered["ciphertext"] = json!("00");
    let mut invalid = Participant::open(
        root.path().join("invalid"),
        "test-wallet".into(),
        Zeroizing::new(random::<32>().unwrap()),
    )
    .unwrap();
    let rejected = ask(
        &mut invalid,
        json!({"op":"restore","backup":tampered,"recovery_key":backup["recovery_key"],"expected":backup["context"]}),
    );
    assert_eq!(rejected["error"], "BACKUP_AUTHENTICATION_FAILED");
}
