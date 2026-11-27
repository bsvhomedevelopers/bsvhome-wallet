use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use futures::{channel::oneshot, executor::block_on, join};
use serde_json::{Value, json};

use super::model::{BackupProof, BsvHomeMpcBalance, StoredState, StoredTransfer, Task};
use super::recovery::{recovery_information, safe_reason};
use super::rules::{self, PROTOCOL};
use super::testkit::{MemoryMpcDeviceProof, MemoryMpcParticipant};
use super::{BsvHomeMpcBackupMaterial, BsvHomeMpcError, BsvHomeMpcRecoveryAction, BsvHomeMpcTransfer, BsvHomeMpcTransferAction, BsvHomeMpcWalletAction, BsvHomeMpcWalletService};
use crate::services::portal::BsvHomePaymailDestinationService;
use crate::services::portal::platform::BsvHomePlatformAccountService;
use crate::services::portal::platform::testkit::{MemoryPlatformAuthenticator, MemoryPlatformProvider, MemoryPlatformSecureStore, PlatformReply};

const BASE: &str = "https://api-dev.hj0252.com/v2/api/";
const EMAIL_CHALLENGE: &str = r#"{"success":true,"code":"OK","data":{"challengeId":"11111111111111111111111111111111","expiresAt":"2099-01-01T00:00:00Z","retryAfterSeconds":60}}"#;
const EMAIL_SESSION: &str = r#"{"success":true,"code":"OK","data":{"token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","userId":"42","expiresAt":"2099-01-01T00:00:00Z","authMethod":"EMAIL_OTP","walletAuthority":false}}"#;
const PASSKEY_START: &str = r#"{"success":true,"code":"OK","data":{"challengeId":"22222222222222222222222222222222","expiresAt":"2099-01-01T00:00:00Z","options":{"publicKey":{"challenge":"eA","rpId":"test.example"}}}}"#;
const STEP_UP: &str = r#"{"success":true,"code":"OK","data":{"grant":"single-use-test-grant","actionHash":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}}"#;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../../testdata/phaseone/mpc.json")).unwrap()
}
fn envelope(value: Value) -> String {
    json!({"success":true,"code":"OK","data":value}).to_string()
}
fn public_wallet() -> Value {
    let fixture = fixture();
    json!({"walletId":"2".repeat(32),"publicKey":fixture["publicKey"],"address":fixture["address"],"epoch":0,"status":"ACTIVE","protocol":PROTOCOL})
}
fn local_status() -> Value {
    let fixture = fixture();
    json!({"context":{"wallet_id":"2".repeat(32),"public_key":fixture["publicKey"],"protocol":PROTOCOL,"epoch":0,"format":1},"status":"ACTIVE","address":fixture["address"]})
}
async fn service(
    provider: Arc<MemoryPlatformProvider>,
    store: Arc<MemoryPlatformSecureStore>,
    participant: Arc<MemoryMpcParticipant>,
    device: Arc<MemoryMpcDeviceProof>,
) -> BsvHomeMpcWalletService {
    let account = Arc::new(
        BsvHomePlatformAccountService::new(
            provider.clone(),
            store.clone(),
            Arc::new(MemoryPlatformAuthenticator::default()),
            BASE.into(),
            "zh-CN".into(),
            "ios-test-installation".into(),
        )
        .unwrap(),
    );
    provider.reply(200, EMAIL_CHALLENGE);
    provider.reply(200, EMAIL_SESSION);
    let challenge = account.email_start("person@example.com".into()).await.unwrap();
    account.email_finish(challenge.id, "001234".into()).await.unwrap();
    BsvHomeMpcWalletService::new(account, participant, device, store, Arc::new(BsvHomePaymailDestinationService::new(provider)))
}

#[test]
fn test_wallet_transfer_and_recovery_actions_fail_closed() {
    let mut wallet = rules::wallet(&public_wallet(), "test".into(), false).unwrap();
    assert_eq!(rules::wallet_actions(&wallet, None), vec![BsvHomeMpcWalletAction::ExportBackup]);
    wallet.balance = Some(BsvHomeMpcBalance {
        state: "CURRENT".into(),
        confirmed_satoshis: Some(100),
        unconfirmed_satoshis: Some(200),
        available_confirmed_satoshis: Some(0),
        reserved_confirmed_satoshis: Some(100),
        history_state: None,
        synced_at: None,
        source: None,
    });
    assert_eq!(rules::wallet_actions(&wallet, None), vec![BsvHomeMpcWalletAction::ExportBackup]);
    wallet.balance.as_mut().unwrap().available_confirmed_satoshis = Some(100);
    assert_eq!(
        rules::wallet_actions(&wallet, None),
        vec![BsvHomeMpcWalletAction::ExportBackup, BsvHomeMpcWalletAction::Send]
    );
    wallet.balance.as_mut().unwrap().state = "STALE".into();
    assert_eq!(rules::wallet_actions(&wallet, None), vec![BsvHomeMpcWalletAction::ExportBackup]);
    wallet.restored = true;
    assert_eq!(rules::wallet_actions(&wallet, Some("AWAIT_BACKUP")), vec![BsvHomeMpcWalletAction::ExportBackup]);
    assert_eq!(rules::wallet_actions(&wallet, None), vec![BsvHomeMpcWalletAction::StartRecovery]);
    assert_eq!(rules::transfer_actions("FUTURE_STATUS", true), vec![BsvHomeMpcTransferAction::Refresh]);
    assert_eq!(rules::transfer_actions("UNKNOWN", false), vec![BsvHomeMpcTransferAction::Refresh]);
    assert_eq!(
        rules::transfer_actions("UNKNOWN", true),
        vec![BsvHomeMpcTransferAction::RetryBroadcast, BsvHomeMpcTransferAction::Refresh]
    );
    assert_eq!(
        rules::recovery_actions("COMMITTING"),
        vec![BsvHomeMpcRecoveryAction::Status, BsvHomeMpcRecoveryAction::Reconcile]
    );
    assert_eq!(rules::recovery_actions("FUTURE_STATUS"), vec![BsvHomeMpcRecoveryAction::Status]);
}

#[test]
fn test_transfer_proposal_is_reviewed_and_every_intent_field_is_bound() {
    let fixture = fixture();
    let recipient = fixture["recipient"].as_str().unwrap();
    let public = fixture["publicKey"].as_str().unwrap();
    let review = rules::review(&fixture["proposal"], public, recipient, 75000, 500).unwrap();
    assert_eq!(
        review.digests,
        vec![
            "109300ae667a6f714f7f2b331c42a85eebbaedea1a0f36c46986a5b966b307f3",
            "a2086a7d299a8be00c1ffde8e6ec912211a1239dbd4e03d5c3682a6e569ec3ff"
        ]
    );
    let stored = StoredTransfer {
        transfer: BsvHomeMpcTransfer {
            id: "3".repeat(32),
            wallet_id: "2".repeat(32),
            recipient: recipient.into(),
            recipient_handle: None,
            amount: 75000,
            fee: 500,
            status: "PREPARED".into(),
            transaction_id: None,
            confirmations: None,
            review,
            actions: Vec::new(),
            error_code: None,
        },
        public_key: public.into(),
        proposal: fixture["proposal"].clone(),
        signed_transaction: None,
    };
    let response = json!({"id":"3".repeat(32),"walletId":"2".repeat(32),"recipient":recipient,"amountSatoshis":"75000","feeSatoshis":"500","proposal":fixture["proposal"]});
    assert_eq!(rules::transfer_unchanged(&response, &stored), Ok(()));
    for (key, value) in [
        ("id", json!("4".repeat(32))),
        ("walletId", json!("4".repeat(32))),
        ("recipient", json!(fixture["address"])),
        ("amountSatoshis", json!("75001")),
        ("feeSatoshis", json!("501")),
    ] {
        let mut changed = response.clone();
        changed[key] = value;
        assert_eq!(rules::transfer_unchanged(&changed, &stored), Err(BsvHomeMpcError::TransferChanged));
    }
}

#[test]
fn test_task_binding_rejects_changed_purpose_wallet_device_epoch_and_business_context() {
    let body = json!({"kind":"SPEND","walletId":"2".repeat(32),"transferId":"3".repeat(32)});
    let task = Task {
        id: "1".repeat(32),
        wallet_id: "2".repeat(32),
        kind: "SPEND".into(),
        epoch: 4,
        context: json!({"protocol":PROTOCOL,"devicePublicKey":"device","transferId":"3".repeat(32)}),
        action_hash: "c".repeat(64),
        status: "PREPARED".into(),
    };
    assert_eq!(rules::task_matches(&task, &body, "device", Some(4)), Ok(()));
    assert_eq!(rules::task_matches(&task, &body, "other", Some(4)), Err(BsvHomeMpcError::InvalidResponse));
    assert_eq!(rules::task_matches(&task, &body, "device", Some(5)), Err(BsvHomeMpcError::EpochMismatch));
    let mut changed = task.clone();
    changed.context["transferId"] = json!("4".repeat(32));
    assert_eq!(rules::task_matches(&changed, &body, "device", Some(4)), Err(BsvHomeMpcError::InvalidResponse));
    changed = task.clone();
    changed.wallet_id = "4".repeat(32);
    assert_eq!(rules::task_matches(&changed, &body, "device", Some(4)), Err(BsvHomeMpcError::IdentityMismatch));
    changed = task;
    changed.kind = "PAYMAIL".into();
    assert_eq!(rules::task_matches(&changed, &body, "device", Some(4)), Err(BsvHomeMpcError::InvalidResponse));
}

#[test]
fn test_dkg_routes_only_the_documented_public_fragments() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let participant = Arc::new(MemoryMpcParticipant::default());
        let device = Arc::new(MemoryMpcDeviceProof::default());
        let service = service(provider.clone(), Arc::new(MemoryPlatformSecureStore::default()), participant.clone(), device.clone()).await;
        let task = Task {
            id: "1".repeat(32),
            wallet_id: "2".repeat(32),
            kind: "CREATE".into(),
            epoch: 0,
            context: json!({}),
            action_hash: "c".repeat(64),
            status: "PREPARED".into(),
        };
        provider.reply(200, PASSKEY_START);
        provider.reply(200, STEP_UP);
        for value in [
            json!("b1"),
            json!(["b2-proof", "b2-z2", "b2-keep"]),
            json!(["b3-z3", "b3-mul", "b3-keep"]),
            json!({"public_key":"public"}),
        ] {
            provider.reply(200, &envelope(json!({"messageJson":value.to_string()})));
        }
        participant.replies.lock().unwrap().extend([
            json!(["LOCAL-ONLY-FRAGMENT", "a1-public"]),
            json!(["a2-proof", "a2-z2", "a2-keep"]),
            json!(["a3-z3", "a3-mul", "a3-keep"]),
            json!({"public_key":"public"}),
        ]);
        let lease = service.account.session_lease().unwrap();
        service.grouped(&lease, &task, false).await.unwrap();
        let requests = provider.requests.lock().unwrap();
        let exchange = requests
            .iter()
            .filter(|request| request.url.ends_with("/tasks/exchange"))
            .map(|request| serde_json::from_slice::<Value>(request.body.as_ref().unwrap()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            exchange.iter().map(|request| request["messageJson"].as_str().unwrap()).collect::<Vec<_>>(),
            vec!["\"a1-public\"", "[\"a2-proof\",\"a2-z2\",\"a2-keep\"]", "[\"a3-z3\",\"a3-mul\",\"a3-keep\"]"]
        );
        assert_eq!(
            participant.requests.lock().unwrap()[3],
            json!([["a2-proof","b2-proof"],"b2-z2","b3-z3","b3-mul",{"1":"a2-keep","2":"b2-keep"},{"1":"a3-keep","2":"b3-keep"}])
        );
        assert_eq!(
            device.messages.lock().unwrap()[1],
            format!("WowPay/MPC/exchange/v1\n{}\n0\n{}", task.id, rules::hash(b"\"a1-public\""))
        );
    });
}

#[test]
fn test_preparation_persists_before_http_and_reuses_original_key_after_uncertain_response() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let participant = Arc::new(MemoryMpcParticipant::default());
        let service = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            participant.clone(),
            Arc::new(MemoryMpcDeviceProof::default()),
        )
        .await;
        let lease = service.account.session_lease().unwrap();
        let known = rules::wallet(&public_wallet(), "test".into(), false).unwrap();
        service
            .save(
                &lease,
                &StoredState {
                    wallets: vec![known],
                    ..StoredState::default()
                },
            )
            .unwrap();
        participant.replies.lock().unwrap().extend([local_status(), local_status(), local_status()]);
        let old_key = "11111111-1111-4111-8111-111111111111";
        let new_key = "22222222-2222-4222-8222-222222222222";
        let fixture = fixture();
        let recipient = fixture["recipient"].as_str().unwrap();
        provider.reply(200, &envelope(public_wallet()));
        let (started, waiting) = oneshot::channel();
        let (release, resumed) = oneshot::channel();
        provider
            .replies
            .lock()
            .unwrap()
            .push_back(PlatformReply::Paused(started, resumed, 503, b"not JSON".to_vec()));
        let first = service.prepare_transfer("2".repeat(32), recipient.into(), 75000, old_key.into());
        let check = async {
            waiting.await.unwrap();
            let saved = service.state(&lease).unwrap().pending_preparation.unwrap();
            assert_eq!((saved.key, saved.recipient, saved.amount), (old_key.into(), recipient.into(), 75000));
            release.send(()).unwrap();
        };
        let (first, ()) = join!(first, check);
        assert!(first.is_err());
        provider.reply(200, &envelope(public_wallet()));
        provider.reply(200, r#"{"code":"INSUFFICIENT_AVAILABLE_CONFIRMED_FUNDS"}"#);
        assert!(service.prepare_transfer("2".repeat(32), recipient.into(), 75000, new_key.into()).await.is_err());
        assert_eq!(service.pending_preparation().unwrap().unwrap().key, old_key);
        provider.reply(200, &envelope(public_wallet()));
        provider.reply(200,&envelope(json!({"id":"3".repeat(32),"walletId":"2".repeat(32),"recipient":recipient,"amountSatoshis":"75000","feeSatoshis":"500","proposal":fixture["proposal"],"status":"PREPARED"})));
        assert_eq!(
            service.prepare_transfer("2".repeat(32), recipient.into(), 75000, new_key.into()).await.unwrap().amount,
            75000
        );
        let requests = provider.requests.lock().unwrap();
        let keys = requests
            .iter()
            .filter(|request| request.url.ends_with("/transfers/prepare"))
            .map(|request| {
                serde_json::from_slice::<Value>(request.body.as_ref().unwrap()).unwrap()["idempotencyKey"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(keys, vec![old_key, old_key, old_key]);
        assert!(service.state(&lease).unwrap().pending_preparation.is_none());
    });
}

#[test]
fn test_broadcast_lost_response_persists_actions_and_retries_the_original_transfer() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let device = Arc::new(MemoryMpcDeviceProof::default());
        let service = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            Arc::new(MemoryMpcParticipant::default()),
            device.clone(),
        )
        .await;
        let lease = service.account.session_lease().unwrap();
        let fixture = fixture();
        let recipient = fixture["recipient"].as_str().unwrap();
        let public_key = fixture["publicKey"].as_str().unwrap();
        let transaction_id = fixture["txid"].as_str().unwrap();
        let signed_transaction = fixture["signedTransaction"].as_str().unwrap();
        let id = "3".repeat(32);
        let wallet_id = "2".repeat(32);
        let action = "c".repeat(64);
        let review = rules::review(&fixture["proposal"], public_key, recipient, 75000, 500).unwrap();
        let commitment = review.commitment.clone();
        let transfer = BsvHomeMpcTransfer {
            id: id.clone(),
            wallet_id: wallet_id.clone(),
            recipient: recipient.into(),
            recipient_handle: None,
            amount: 75000,
            fee: 500,
            status: "SIGNED".into(),
            transaction_id: Some(transaction_id.into()),
            confirmations: None,
            review,
            actions: vec![BsvHomeMpcTransferAction::Broadcast, BsvHomeMpcTransferAction::Refresh],
            error_code: None,
        };
        service
            .save(
                &lease,
                &StoredState {
                    wallets: vec![rules::wallet(&public_wallet(), "test".into(), false).unwrap()],
                    transfers: vec![StoredTransfer {
                        transfer,
                        public_key: public_key.into(),
                        proposal: fixture["proposal"].clone(),
                        signed_transaction: Some(signed_transaction.into()),
                    }],
                    ..StoredState::default()
                },
            )
            .unwrap();
        let mut response = json!({
            "id":id,"walletId":wallet_id,"recipient":recipient,"amountSatoshis":"75000","feeSatoshis":"500",
            "proposal":fixture["proposal"],"status":"SIGNED","txid":transaction_id,"broadcastActionHash":action
        });
        provider.reply(200, &envelope(response.clone()));
        provider.reply(200, PASSKEY_START);
        provider.reply(200, STEP_UP);
        let (started, waiting) = oneshot::channel();
        let (release, resumed) = oneshot::channel();
        provider
            .replies
            .lock()
            .unwrap()
            .push_back(PlatformReply::Paused(started, resumed, 503, b"not JSON".to_vec()));
        let check = async {
            waiting.await.unwrap();
            let stored = service.state(&lease).unwrap().transfers.remove(0);
            release.send(()).unwrap();
            assert_eq!(stored.transfer.status, "SUBMITTING");
            assert_eq!(stored.transfer.actions, vec![BsvHomeMpcTransferAction::Refresh]);
            assert_eq!(stored.signed_transaction.as_deref(), Some(signed_transaction));
        };
        let (first, ()) = join!(service.broadcast_transfer(id.clone(), commitment.clone()), check);
        let unknown = first.unwrap();
        assert_eq!(unknown.status, "UNKNOWN");
        assert_eq!(unknown.transaction_id.as_deref(), Some(transaction_id));
        assert_eq!(unknown.actions, vec![BsvHomeMpcTransferAction::RetryBroadcast, BsvHomeMpcTransferAction::Refresh]);
        let stored = service.state(&lease).unwrap().transfers.remove(0);
        assert!(stored.transfer == unknown);
        assert_eq!(stored.signed_transaction.as_deref(), Some(signed_transaction));

        let retry_action = "d".repeat(64);
        response["status"] = json!("UNKNOWN");
        response["retryBroadcastActionHash"] = json!(retry_action);
        provider.reply(200, &envelope(response.clone()));
        provider.reply(200, PASSKEY_START);
        provider.reply(200, &envelope(json!({"actionHash":retry_action,"grant":"fresh-test-grant"})));
        response["status"] = json!("SUBMITTED");
        provider.reply(200, &envelope(response));
        let submitted = service.broadcast_transfer(id.clone(), commitment).await.unwrap();
        assert_eq!(submitted.id, id);
        assert_eq!(submitted.transaction_id.as_deref(), Some(transaction_id));
        assert_eq!(submitted.status, "SUBMITTED");
        assert_eq!(submitted.actions, vec![BsvHomeMpcTransferAction::Refresh]);
        assert_eq!(*device.messages.lock().unwrap(), vec![action, retry_action]);
        let requests = provider.requests.lock().unwrap();
        let broadcasts = requests
            .iter()
            .filter(|request| request.url.ends_with("/broadcast") || request.url.ends_with("/retry-broadcast"))
            .collect::<Vec<_>>();
        assert_eq!(broadcasts.len(), 2);
        assert!(broadcasts[0].url.ends_with("/transfers/broadcast"));
        assert!(broadcasts[1].url.ends_with("/transfers/retry-broadcast"));
        for (request, grant) in broadcasts.into_iter().zip(["single-use-test-grant", "fresh-test-grant"]) {
            let body = serde_json::from_slice::<Value>(request.body.as_ref().unwrap()).unwrap();
            assert_eq!(body["id"], id);
            assert_eq!(body["grant"], grant);
        }
    });
}

#[test]
fn test_transfer_error_codes_only_expose_platform_public_codes() {
    block_on(async {
        for (code, expected) in [("BROADCAST_RESULT_UNKNOWN", "BROADCAST_RESULT_UNKNOWN"), ("xprvSyntheticServerValue", "SERVICE_ERROR")] {
            let provider = Arc::new(MemoryPlatformProvider::default());
            let participant = Arc::new(MemoryMpcParticipant::default());
            let service = service(
                provider.clone(),
                Arc::new(MemoryPlatformSecureStore::default()),
                participant.clone(),
                Arc::new(MemoryMpcDeviceProof::default()),
            )
            .await;
            let lease = service.account.session_lease().unwrap();
            service
                .save(
                    &lease,
                    &StoredState {
                        wallets: vec![rules::wallet(&public_wallet(), "test".into(), false).unwrap()],
                        ..StoredState::default()
                    },
                )
                .unwrap();
            participant.replies.lock().unwrap().push_back(local_status());
            let fixture = fixture();
            let recipient = fixture["recipient"].as_str().unwrap();
            let mut response = json!({"id":"3".repeat(32),"walletId":"2".repeat(32),"recipient":recipient,"amountSatoshis":"75000","feeSatoshis":"500","proposal":fixture["proposal"],"status":"PREPARED","errorCode":code});
            provider.reply(200, &envelope(public_wallet()));
            provider.reply(200, &envelope(response.clone()));
            let transfer = service
                .prepare_transfer("2".repeat(32), recipient.into(), 75000, "11111111-1111-4111-8111-111111111111".into())
                .await
                .unwrap();
            assert_eq!(transfer.error_code.as_deref(), Some(expected));
            let mut stored = service.state(&lease).unwrap().transfers.remove(0);
            assert_eq!(stored.transfer.error_code.as_deref(), Some(expected));
            for (value, expected) in [
                (json!("CHAIN_PROVIDER_UNAVAILABLE"), Some("CHAIN_PROVIDER_UNAVAILABLE")),
                (json!("synthetic raw upstream detail"), Some("SERVICE_ERROR")),
                (Value::Null, None),
            ] {
                response["errorCode"] = value;
                service.update_transfer(&mut stored, &response).unwrap();
                assert_eq!(stored.transfer.error_code.as_deref(), expected);
            }
        }
    });
}

#[test]
fn test_cancelled_file_save_or_stale_material_cannot_start_backup_proof() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let service = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            Arc::new(MemoryMpcParticipant::default()),
            Arc::new(MemoryMpcDeviceProof::default()),
        )
        .await;
        let lease = service.account.session_lease().unwrap();
        let document = "encrypted-test-file";
        let commitment = rules::hash(document.as_bytes());
        service
            .save(
                &lease,
                &StoredState {
                    wallets: vec![rules::wallet(&public_wallet(), "test".into(), false).unwrap()],
                    backup: Some(BackupProof {
                        wallet_id: "2".repeat(32),
                        epoch: 0,
                        commitment: commitment.clone(),
                    }),
                    ..StoredState::default()
                },
            )
            .unwrap();
        for (saved, owner, epoch, key) in [
            (false, "42", 0, "a".repeat(64)),
            (true, "43", 0, "a".repeat(64)),
            (true, "42", 1, "a".repeat(64)),
            (true, "42", 0, "b".repeat(64)),
        ] {
            let material = BsvHomeMpcBackupMaterial {
                wallet_id: "2".repeat(32),
                owner_id: owner.into(),
                environment: BASE.into(),
                epoch,
                document: document.into(),
                recovery_key: "a".repeat(64),
                commitment: commitment.clone(),
            };
            assert!(service.confirm_backup(material, key, saved).await.is_err());
        }
        assert_eq!(provider.requests.lock().unwrap().len(), 2);
    });
}

#[test]
fn test_history_preserves_net_direction_unknown_fee_and_source() {
    let value = json!({"id":"1".repeat(64),"walletId":"2".repeat(32),"txid":"3".repeat(64),"receivedSatoshis":"99500","spentSatoshis":"100000","feeSatoshis":null,"direction":"SELF","status":"MEMPOOL","confirmations":0,"source":"provider","observedAt":"2026-10-31T01:02:03"});
    let item = rules::history(&value, &"2".repeat(32)).unwrap();
    assert_eq!(
        (item.amount, item.incoming, item.fee, item.source, item.status),
        (500, false, None, Some("provider".into()), "MEMPOOL".into())
    );
    let mut invalid = value;
    invalid["direction"] = json!("IN");
    assert!(rules::history(&invalid, &"2".repeat(32)).is_err());
}

#[test]
fn test_paymail_message_rejects_duplicates_changed_fields_and_expiry() {
    let expected = BTreeMap::from([("User", "42".into()), ("MPC Epoch", "3".into())]);
    let now = Utc.with_ymd_and_hms(2026, 10, 31, 0, 0, 0).unwrap();
    let valid = "WowPay Paymail Binding\nUser: 42\nMPC Epoch: 3\nExpires: 2026-10-31T00:01:00Z";
    assert_eq!(rules::paymail_fields(valid, &expected, now), Ok(()));
    for message in [
        valid.replace("User: 42", "User: 43"),
        format!("{valid}\nUser: 42"),
        valid.replace("00:01:00Z", "00:00:00Z"),
        valid.replace("00:01:00Z", "01:00:00Z"),
    ] {
        assert_eq!(rules::paymail_fields(&message, &expected, now), Err(BsvHomeMpcError::InvalidResponse));
    }
}

#[test]
fn test_bsv_amount_is_exact_to_the_satoshi_and_bounded() {
    for (text, expected) in [("0.00000001", 1), ("1.12345678", 112345678), ("1.000000000", 100000000), ("21000000", 2100000000000000)] {
        assert_eq!(rules::bsv_amount(text), Ok(expected));
    }
    for text in ["0", "-1", "0.000000001", "1.123456789", "21000000.00000001", "18446744073709551616", "1e100000000", "1.2.3"] {
        assert_eq!(rules::bsv_amount(text), Err(BsvHomeMpcError::InvalidInput));
    }
}

#[test]
fn test_recovery_never_commits_before_remote_complete_and_reconciles_after_restart() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let participant = Arc::new(MemoryMpcParticipant::default());
        let service = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            participant.clone(),
            Arc::new(MemoryMpcDeviceProof::default()),
        )
        .await;
        let lease = service.account.session_lease().unwrap();
        let task = Task {
            id: "1".repeat(32),
            wallet_id: "2".repeat(32),
            kind: "RECOVERY_BACKUP".into(),
            epoch: 1,
            context: json!({"recoveryId":"4".repeat(32)}),
            action_hash: "c".repeat(64),
            status: "PREPARED".into(),
        };
        service
            .save(
                &lease,
                &StoredState {
                    wallets: vec![rules::wallet(&public_wallet(), "test".into(), true).unwrap()],
                    active_task: Some(task.clone()),
                    ..StoredState::default()
                },
            )
            .unwrap();
        provider.reply(200, &envelope(json!({"id":task.id,"walletId":task.wallet_id,"kind":task.kind,"status":"COMMITTING"})));
        assert_eq!(service.reconcile().await, Err(BsvHomeMpcError::OperationPending));
        assert_eq!(participant.requests.lock().unwrap().len(), 0);
        assert!(service.state(&lease).unwrap().active_task.is_some());
        provider.reply(200, &envelope(json!({"id":task.id,"walletId":task.wallet_id,"kind":task.kind,"status":"COMMITTING"})));
        assert_eq!(service.cancel_task().await, Err(BsvHomeMpcError::UnsupportedState { state: "COMMITTING".into() }));
        assert!(participant.closed.lock().unwrap().is_empty());
        provider.reply(200, &envelope(json!({"id":task.id,"walletId":task.wallet_id,"kind":task.kind,"status":"COMPLETE"})));
        let mut remote = public_wallet();
        remote["epoch"] = json!(1);
        provider.reply(200, &envelope(remote));
        provider.reply(200, &envelope(json!({"id":"4".repeat(32),"walletId":"2".repeat(32),"sourceEpoch":0,"status":"COMPLETE"})));
        let mut pending = local_status();
        pending["status"] = json!("REFRESH_PENDING");
        let mut active = local_status();
        active["context"]["epoch"] = json!(1);
        participant.replies.lock().unwrap().extend([pending, json!({"epoch":1,"status":"ACTIVE"}), active]);
        service.reconcile().await.unwrap();
        let state = service.state(&lease).unwrap();
        assert_eq!((state.wallets[0].epoch, state.wallets[0].restored, state.active_task.is_none()), (1, false, true));
        assert_eq!(
            participant.requests.lock().unwrap().clone(),
            vec![json!({"op":"status"}), json!({"op":"commit","epoch":1,"backup_confirmed":true}), json!({"op":"status"})]
        );
    });
}

#[test]
fn test_paymail_sender_binding_is_required_without_starting_payment() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let service = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            Arc::new(MemoryMpcParticipant::default()),
            Arc::new(MemoryMpcDeviceProof::default()),
        )
        .await;
        let lease = service.account.session_lease().unwrap();
        service
            .save(
                &lease,
                &StoredState {
                    wallets: vec![rules::wallet(&public_wallet(), "test".into(), false).unwrap()],
                    ..StoredState::default()
                },
            )
            .unwrap();
        provider.reply(
            200,
            &envelope(json!({"domains":["sender.test"],"publicBaseUrl":"https://sender.test","network":"BSV_MAINNET","addressType":"P2PKH"})),
        );
        provider.reply(200, &envelope(json!([])));
        assert_eq!(
            service.resolve_recipient("2".repeat(32), "alice@receiver.test".into(), "0.00075000".into()).await,
            Err(BsvHomeMpcError::PaymailSenderRequired)
        );
        assert!(service.state(&lease).unwrap().pending_preparation.is_none());
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests[2].headers.as_ref().unwrap().get("Authorization").unwrap(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(requests[2].headers, requests[3].headers);
    });
}

#[test]
fn test_paymail_preparation_freezes_resolved_address_and_does_not_resolve_again_on_retry() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let participant = Arc::new(MemoryMpcParticipant::default());
        let service = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            participant.clone(),
            Arc::new(MemoryMpcDeviceProof::default()),
        )
        .await;
        let lease = service.account.session_lease().unwrap();
        service
            .save(
                &lease,
                &StoredState {
                    wallets: vec![rules::wallet(&public_wallet(), "test".into(), false).unwrap()],
                    ..StoredState::default()
                },
            )
            .unwrap();
        participant.replies.lock().unwrap().extend([local_status(), local_status()]);
        provider.reply(200, &envelope(public_wallet()));
        provider.reply(
            200,
            &envelope(json!({"domains":["sender.test"],"publicBaseUrl":"https://sender.test","network":"BSV_MAINNET","addressType":"P2PKH"})),
        );
        provider.reply(200,&envelope(json!([{"handle":"me@sender.test","address":fixture()["address"],"identityPublicKey":"0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","active":true}])));
        provider.reply(200, r#"{"bsvalias":"1.0","capabilities":{"paymentDestination":"https://receiver.test/pay/{alias}"}}"#);
        provider.reply(200, r#"{"output":"76a914020202020202020202020202020202020202020288ac"}"#);
        provider.reply(503, "not JSON");
        let old_key = "11111111-1111-4111-8111-111111111111";
        assert!(
            service
                .prepare_transfer_bsv("2".repeat(32), "alice@receiver.test".into(), "0.00075000".into(), old_key.into())
                .await
                .is_err()
        );
        let pending = service.pending_preparation().unwrap().unwrap();
        assert_eq!(
            (pending.recipient, pending.target, pending.recipient_handle),
            (
                "alice@receiver.test".into(),
                fixture()["recipient"].as_str().unwrap().into(),
                Some("alice@receiver.test".into())
            )
        );
        provider.reply(200, &envelope(public_wallet()));
        provider.reply(200,&envelope(json!({"id":"3".repeat(32),"walletId":"2".repeat(32),"recipient":fixture()["recipient"],"amountSatoshis":"75000","feeSatoshis":"500","proposal":fixture()["proposal"],"status":"PREPARED"})));
        let prepared = service
            .prepare_transfer_bsv(
                "2".repeat(32),
                "alice@receiver.test".into(),
                "0.00075000".into(),
                "22222222-2222-4222-8222-222222222222".into(),
            )
            .await
            .unwrap();
        assert_eq!(prepared.recipient_handle, Some("alice@receiver.test".into()));
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.iter().filter(|request| request.url.ends_with("/.well-known/bsvalias")).count(), 1);
        let preparations = requests
            .iter()
            .filter(|request| request.url.ends_with("/transfers/prepare"))
            .map(|request| serde_json::from_slice::<Value>(request.body.as_ref().unwrap()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(preparations[0], preparations[1]);
        assert_eq!(requests[5].headers.as_ref().unwrap().get("Authorization"), None);
        assert_eq!(requests[6].headers.as_ref().unwrap().get("Authorization"), None);
    });
}

#[test]
fn test_restart_exposes_pending_operations_without_trusting_cached_current_balance() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let participant = Arc::new(MemoryMpcParticipant::default());
        let service = service(
            provider,
            Arc::new(MemoryPlatformSecureStore::default()),
            participant.clone(),
            Arc::new(MemoryMpcDeviceProof::default()),
        )
        .await;
        let lease = service.account.session_lease().unwrap();
        let mut wallet = rules::wallet(&public_wallet(), "test".into(), false).unwrap();
        wallet.balance = Some(BsvHomeMpcBalance {
            state: "CURRENT".into(),
            confirmed_satoshis: Some(100),
            unconfirmed_satoshis: Some(0),
            available_confirmed_satoshis: Some(100),
            reserved_confirmed_satoshis: Some(0),
            history_state: None,
            synced_at: None,
            source: None,
        });
        let mut state = StoredState {
            wallets: vec![wallet],
            pending_task_request: Some(json!({"kind":"CREATE","devicePublicKey":"device"})),
            ..StoredState::default()
        };
        service.save(&lease, &state).unwrap();
        let local = service.local_wallets().unwrap();
        assert_eq!(local[0].balance.as_ref().unwrap().state, "STALE");
        assert_eq!(local[0].actions, vec![BsvHomeMpcWalletAction::ExportBackup]);
        assert_eq!(service.state(&lease).unwrap().wallets[0].balance.as_ref().unwrap().state, "CURRENT");
        let pending = service.pending_task().unwrap().unwrap();
        assert_eq!(
            (pending.state, pending.id, pending.actions),
            (
                super::BsvHomeMpcPendingTaskState::PrepareReceiptUnknown,
                None,
                vec![super::BsvHomeMpcPendingTaskAction::Reconcile]
            )
        );
        state.pending_task_request = None;
        state.active_task = Some(Task {
            id: "1".repeat(32),
            wallet_id: "2".repeat(32),
            kind: "RECOVERY_BACKUP".into(),
            epoch: 1,
            context: json!({}),
            action_hash: "c".repeat(64),
            status: "COMMITTING".into(),
        });
        service.save(&lease, &state).unwrap();
        assert_eq!(service.pending_task().unwrap().unwrap().actions, vec![super::BsvHomeMpcPendingTaskAction::Reconcile]);
        assert_eq!(service.local_wallets().unwrap()[0].actions, Vec::new());
        participant.replies.lock().unwrap().push_back(local_status());
        service.native(&lease, &"2".repeat(32), json!({"op":"status"})).await.unwrap();
        service.account.cancel().unwrap();
        participant.close_failures.lock().unwrap().push_back(BsvHomeMpcError::NativeUnavailable);
        assert_eq!(service.suspend().await, Err(BsvHomeMpcError::NativeUnavailable));
        service.suspend().await.unwrap();
        assert_eq!(participant.closed.lock().unwrap().clone(), vec!["2".repeat(32), "2".repeat(32)]);
        assert!(service.account.require_lease(&lease).is_err());
        assert!(service.pending_task().unwrap().is_some());
    });
}

#[test]
fn test_cancelled_recovery_reconciles_lost_receipt_by_discarding_only_uncommitted_candidate() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let participant = Arc::new(MemoryMpcParticipant::default());
        let service = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            participant.clone(),
            Arc::new(MemoryMpcDeviceProof::default()),
        )
        .await;
        let lease = service.account.session_lease().unwrap();
        let recovery = super::BsvHomeMpcRecovery {
            id: "4".repeat(32),
            wallet_id: "2".repeat(32),
            source_epoch: 0,
            status: "AWAIT_BACKUP".into(),
            not_before_at: None,
            review_note: None,
            actions: rules::recovery_actions("AWAIT_BACKUP"),
        };
        service
            .save(
                &lease,
                &StoredState {
                    wallets: vec![rules::wallet(&public_wallet(), "test".into(), true).unwrap()],
                    recovery: Some(recovery),
                    backup: Some(BackupProof {
                        wallet_id: "2".repeat(32),
                        epoch: 1,
                        commitment: "b".repeat(64),
                    }),
                    ..StoredState::default()
                },
            )
            .unwrap();
        provider.reply(200, &envelope(json!({"id":"4".repeat(32),"walletId":"2".repeat(32),"sourceEpoch":0,"status":"CANCELLED"})));
        let mut pending = local_status();
        pending["status"] = json!("REFRESH_PENDING");
        participant
            .replies
            .lock()
            .unwrap()
            .extend([pending, json!({"epoch":0,"status":"RESTORED_REFRESH_REQUIRED"})]);
        assert_eq!(service.recovery_status("4".repeat(32)).await.unwrap().status, "CANCELLED");
        assert!(service.state(&lease).unwrap().backup.is_none());
        assert!(service.state(&lease).unwrap().wallets[0].restored);
        assert_eq!(service.local_wallets().unwrap()[0].actions, vec![BsvHomeMpcWalletAction::StartRecovery]);
        assert_eq!(
            participant.requests.lock().unwrap().clone(),
            vec![json!({"op":"status"}), json!({"op":"abort_refresh","epoch":0})]
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 3);
    });
}

fn recovery_timeline_fixture(status: &str) -> Value {
    let mut value: Value = serde_json::from_str(r#"{"id":"44444444444444444444444444444444","userId":"42","walletId":"22222222222222222222222222222222","status":"NEEDS_INFO","reviewNote":"Explain device loss","notBeforeAt":"2026-11-03T11:12:13.123456","notificationStatus":"PENDING","timeline":[{"type":"RECOVERY_INFO_REQUESTED","time":"2026-11-02T10:11:12.123456","description":"Provide a non-secret explanation","actorType":"SYSTEM"}]}"#).unwrap();
    value["status"] = status.into();
    value
}

#[test]
fn test_recovery_timeline_binds_original_wallet_owner_and_keeps_public_event_fields() {
    let value = recovery_timeline_fixture("NEEDS_INFO");
    let information = recovery_information(&value, &"4".repeat(32), &"2".repeat(32), "42").unwrap();
    assert_eq!(information.not_before_at.as_deref(), Some("2026-11-03T11:12:13.123456"));
    assert_eq!(information.events[0].time, "2026-11-02T10:11:12.123456");
    assert_eq!(information.events[0].actor_type.as_deref(), Some("SYSTEM"));
    assert_eq!(information.events[0].status, None);
    assert!(information.can_supply_information);
    for (id, wallet, owner) in [
        ("3".repeat(32), "2".repeat(32), "42"),
        ("4".repeat(32), "3".repeat(32), "42"),
        ("4".repeat(32), "2".repeat(32), "43"),
    ] {
        assert_eq!(recovery_information(&value, &id, &wallet, owner), Err(BsvHomeMpcError::IdentityMismatch));
    }
    let unknown = recovery_information(&recovery_timeline_fixture("FUTURE_STATE"), &"4".repeat(32), &"2".repeat(32), "42").unwrap();
    assert!(!unknown.can_supply_information);
    assert_eq!(unknown.status, "FUTURE_STATE");
}

#[test]
fn test_recovery_business_note_rejects_secret_patterns_and_all_mnemonic_lengths() {
    assert!(safe_reason("更换设备后需要恢复钱包，请核实这份非秘密业务说明。"));
    assert!(safe_reason(&"说".repeat(500)));
    for reason in [
        "".into(),
        "   ".into(),
        "a".repeat(501),
        format!("note {}", "a".repeat(64)),
        "xprvSyntheticPrivateKey".into(),
        "-----BEGIN EC PRIVATE KEY-----".into(),
        r#"{"recovery_key": "synthetic"}"#.into(),
        r#"{"seed" : "synthetic"}"#.into(),
    ] {
        assert!(!safe_reason(&reason));
    }
    for words in [12, 15, 18, 21, 24] {
        assert!(!safe_reason(&vec!["abandon"; words].join(" ")));
    }
}

#[test]
fn test_recovery_information_submits_exact_non_secret_note_only_after_current_needs_info() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let service = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            Arc::new(MemoryMpcParticipant::default()),
            Arc::new(MemoryMpcDeviceProof::default()),
        )
        .await;
        let lease = service.account.session_lease().unwrap();
        service
            .save(
                &lease,
                &StoredState {
                    recovery: Some(super::BsvHomeMpcRecovery {
                        id: "4".repeat(32),
                        wallet_id: "2".repeat(32),
                        status: "NEEDS_INFO".into(),
                        source_epoch: 0,
                        not_before_at: None,
                        review_note: None,
                        actions: rules::recovery_actions("NEEDS_INFO"),
                    }),
                    ..StoredState::default()
                },
            )
            .unwrap();
        assert!(matches!(
            service.submit_recovery_information("4".repeat(32), "a".repeat(64)).await,
            Err(BsvHomeMpcError::InvalidInput)
        ));
        assert_eq!(provider.requests.lock().unwrap().len(), 2);
        provider.reply(200, &envelope(recovery_timeline_fixture("WAITING")));
        assert!(matches!(
            service.submit_recovery_information("4".repeat(32), "已更换设备，需要核验身份。".into()).await,
            Err(BsvHomeMpcError::OperationPending)
        ));
        assert_eq!(provider.requests.lock().unwrap().len(), 3);
        provider.reply(200, &envelope(recovery_timeline_fixture("NEEDS_INFO")));
        provider.reply(
            200,
            r#"{"success":true,"code":"OK","data":{"actionHash":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}}"#,
        );
        provider.reply(200, PASSKEY_START);
        provider.reply(200, STEP_UP);
        provider.reply(
            200,
            r#"{"success":true,"code":"OK","data":{"id":"44444444444444444444444444444444","walletId":"22222222222222222222222222222222","status":"WAIT_REVIEW"}}"#,
        );
        provider.reply(200, &envelope(recovery_timeline_fixture("WAIT_REVIEW")));
        let submitted = service.submit_recovery_information("4".repeat(32), "  已更换设备，需要核验身份。  ".into()).await.unwrap();
        assert_eq!(submitted.status, "WAIT_REVIEW");
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 9);
        let challenge: Value = serde_json::from_slice(requests[4].body.as_ref().unwrap()).unwrap();
        let body: Value = serde_json::from_slice(requests[7].body.as_ref().unwrap()).unwrap();
        assert_eq!(challenge["reason"].as_str(), Some("已更换设备，需要核验身份。"));
        assert_eq!(body["reason"], challenge["reason"]);
        assert_eq!(body.as_object().unwrap().len(), 3);
        assert_eq!(body["grant"].as_str(), Some("single-use-test-grant"));
    });
}
