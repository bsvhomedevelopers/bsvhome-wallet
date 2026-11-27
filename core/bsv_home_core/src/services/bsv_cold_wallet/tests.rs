use std::sync::{Arc, Mutex};

use bitcoin::{
    Transaction,
    consensus::{deserialize, serialize},
};
use bsv_home_bsv::cold_wallet::{Pairing, Request, Signed};
use futures::{channel::oneshot, executor::block_on, join};
use primitives::{Chain, WalletId, WalletType, testkit::ABANDON_PHRASE};

use super::testkit::ColdWalletProvider;
use super::{BsvHomeBsvColdWalletActionIntent, BsvHomeBsvColdWalletService, BsvHomeBsvColdWalletSubmission};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::services::preferences::testkit::MemoryPreferencesStore;
use crate::services::wallet::{addresses::testkit::pool_with_keystore, testkit::MemoryKeystorePassword};

use super::state::{Pending, State};
use super::testkit::{Context, broadcast_action, prepare_action, sign_action};
use crate::keystore::{decode_password, keystore_id_for_wallet};
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::testkit::TEST_PASSWORD;
use crate::testkit::TestAlienProvider;

#[test]
fn test_offline_cold_wallet_request_sign_restart_and_signed_state_cannot_be_replaced() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let request = context.request(wallet.id.clone(), 75000).await;
        let imported = context.service.import_request(wallet.id.clone(), request.clone()).await.unwrap();
        let signed = sign_action(&context.service, wallet.id.clone(), imported.review.commitment.clone()).await.unwrap();
        assert!(signed.signed_json.is_some());
        assert_eq!(signed.review.total_debit, signed.review.amount + signed.review.fee);
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 1);
        let restored = context.restored();
        assert_eq!(restored.snapshot(wallet.id.clone()).await.unwrap().pending, Some(signed.clone()));
        assert_eq!(restored.import_request(wallet.id.clone(), request).await.unwrap(), signed);
        assert!(restored.cancel_request(wallet.id.clone(), signed.review.commitment).await.is_err());
        assert!(restored.import_request(wallet.id.clone(), context.request(wallet.id, 75001).await).await.is_err());
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_cold_wallet_persistence_failure_and_corrupt_restoration_fail_closed() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        *context.store.fail.lock().unwrap() = true;
        assert_eq!(
            context.service.import_request(wallet.id.clone(), context.request(wallet.id.clone(), 75000).await).await,
            Err(BsvHomeServiceError::Store {
                msg: "injected write failure".into()
            })
        );
        assert_eq!(context.service.snapshot(wallet.id.clone()).await.unwrap().pending, None);
        *context.store.fail.lock().unwrap() = false;
        let imported = context
            .service
            .import_request(wallet.id.clone(), context.request(wallet.id.clone(), 75000).await)
            .await
            .unwrap();
        assert!(sign_action(&context.service, wallet.id.clone(), "different commitment".into()).await.is_err());
        {
            let mut records = context.store.records.lock().unwrap();
            let record = records.get_mut(&wallet.id).unwrap();
            let mut state: State = serde_json::from_str(&record.value).unwrap();
            state.pending.as_mut().unwrap().review = imported.review_json.replace("75000", "75001");
            record.value = serde_json::to_string(&state).unwrap();
        }
        assert!(context.restored().snapshot(wallet.id).await.is_err());
    });
}

#[test]
fn test_cold_wallet_authentication_cancellation_wallet_switch_and_offline_network_guards() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let imported = context
            .service
            .import_request(wallet.id.clone(), context.request(wallet.id.clone(), 75000).await)
            .await
            .unwrap();
        *context.signer.reject.lock().unwrap() = true;
        assert_eq!(
            sign_action(&context.service, wallet.id.clone(), imported.review.commitment.clone()).await,
            Err(BsvHomeServiceError::Cancelled)
        );
        *context.signer.reject.lock().unwrap() = false;
        *context.signer.switch_to.lock().unwrap() = Some(WalletId::Single(Chain::BitcoinSv, "unavailable".into()));
        assert!(sign_action(&context.service, wallet.id.clone(), imported.review.commitment.clone()).await.is_err());
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
        context.service.select_wallet(wallet.id.clone()).await.unwrap();
        assert!(prepare_action(&context.service, wallet.id.clone(), imported.review.recipient, 75000).await.is_err());
        assert!(broadcast_action(&context.service, wallet.id.clone(), imported.review.commitment).await.is_err());
        assert!(context.service.refresh(wallet.id.clone()).await.is_err());
        assert_eq!(context.service.snapshot(wallet.id).await.unwrap().pending.unwrap().signed_json, None);
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_cold_wallet_busy_rejects_duplicate_action_instead_of_queueing() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let imported = context
            .service
            .import_request(wallet.id.clone(), context.request(wallet.id.clone(), 75000).await)
            .await
            .unwrap();
        let (release, wait) = oneshot::channel();
        *context.signer.authentication_wait.lock().unwrap() = Some(wait);
        let signing = sign_action(&context.service, wallet.id.clone(), imported.review.commitment.clone());
        let duplicate = async {
            assert!(sign_action(&context.service, wallet.id.clone(), imported.review.commitment).await.is_err());
            assert!(context.service.select_wallet(wallet.id.clone()).await.is_err());
            release.send(()).unwrap();
        };
        let (result, ()) = join!(signing, duplicate);
        assert!(result.unwrap().signed_json.is_some());
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 1);
    });
}

#[test]
fn test_online_prepare_reservation_offline_sign_and_same_raw_broadcast_retry() {
    block_on(async {
        let context = Context::new();
        let daily = Context::new();
        let offline_wallet = context.create().await;
        let known_request: Request = serde_json::from_str(&context.request(offline_wallet.id.clone(), 75000).await).unwrap();
        let provider = Arc::new(ColdWalletProvider {
            pairing: known_request.pairing.clone(),
            previous_raw: known_request.inputs[0].previous_transaction.clone(),
            previous_id: known_request.previous_transaction_ids().unwrap()[0].clone(),
            accepted_raw: Mutex::new(None),
            broadcasted: Mutex::new(Vec::new()),
            fail_store_after_accepted: Mutex::new(None),
            cancel_after_accepted: Mutex::new(None),
        });
        let gateway = Arc::new(BsvHomeGateway::new(
            provider.clone(),
            Arc::new(MemoryPreferencesStore::default()),
            Arc::new(EmptyPreferences),
        ));
        let addresses = pool_with_keystore(
            gateway.clone(),
            daily.wallet_store.clone(),
            daily.keystore.clone(),
            Arc::new(MemoryKeystorePassword::default()),
            daily.wallet_store.bsv_addresses.clone(),
        );
        let online = BsvHomeBsvColdWalletService::online(
            daily.wallets.clone(),
            addresses.clone(),
            gateway.clone(),
            daily.keystore.clone(),
            daily.store.clone(),
            daily.signer.clone(),
        );
        let pairing = context.service.pairing(offline_wallet.id.clone()).await.unwrap();
        let pairing_frames = context.service.qr_frames(pairing).unwrap();
        let pairing = online.qr_assemble(pairing_frames).unwrap().unwrap();
        let observer = online.import_observer("Observer".into(), pairing, online.capture_generation()).await.unwrap().wallet;
        assert_eq!(observer.wallet_type, WalletType::View);
        assert_eq!(
            daily.wallets.wallets().await.unwrap().into_iter().map(|wallet| wallet.id).collect::<Vec<_>>(),
            vec![observer.id.clone()]
        );
        assert!(!daily.keystore.has_stored_wallets().unwrap());
        let prepared = prepare_action(&online, observer.id.clone(), known_request.recipient, 75000).await.unwrap();
        assert_eq!(prepared.review.input_count, 2);
        assert!(prepare_action(&online, observer.id.clone(), prepared.review.recipient.clone(), 75001).await.is_err());
        let mut request_frames = online.qr_frames(prepared.request_json.clone()).unwrap();
        request_frames.reverse();
        let request = context.service.qr_assemble(request_frames).unwrap().unwrap();
        context.service.select_wallet(offline_wallet.id.clone()).await.unwrap();
        let imported = context.service.import_request(offline_wallet.id.clone(), request).await.unwrap();
        assert_eq!(imported.review, prepared.review);
        let signed = sign_action(&context.service, offline_wallet.id.clone(), imported.review.commitment).await.unwrap();
        let signed_frames = context.service.qr_frames(signed.signed_json.clone().unwrap()).unwrap();
        let signed_returned = online.qr_assemble(signed_frames).unwrap().unwrap();
        let mut tampered: Signed = serde_json::from_str(&signed_returned).unwrap();
        let mut tampered_transaction: Transaction = deserialize(&hex::decode(&tampered.raw_transaction).unwrap()).unwrap();
        tampered_transaction.input[0].script_sig = tampered_transaction.input[1].script_sig.clone();
        tampered.transaction_id = tampered_transaction.compute_txid().to_string();
        tampered.raw_transaction = hex::encode(serialize(&tampered_transaction));
        let before_invalid = daily.store.records.lock().unwrap().get(&observer.id).unwrap().clone();
        assert!(online.import_signed(observer.id.clone(), serde_json::to_string(&tampered).unwrap()).await.is_err());
        assert_eq!(daily.store.records.lock().unwrap().get(&observer.id), Some(&before_invalid));
        assert_eq!(*provider.broadcasted.lock().unwrap(), Vec::<String>::new());
        let saved = online.import_signed(observer.id.clone(), signed_returned).await.unwrap();
        let online = BsvHomeBsvColdWalletService::online(daily.wallets.clone(), addresses, gateway, daily.keystore.clone(), daily.store.clone(), daily.signer.clone());
        assert_eq!(online.snapshot(observer.id.clone()).await.unwrap().pending, Some(saved.clone()));
        assert!(broadcast_action(&online, observer.id.clone(), saved.review.commitment.clone()).await.is_err());
        assert_eq!(online.snapshot(observer.id.clone()).await.unwrap().pending, Some(saved.clone()));
        let signed_payload: Signed = serde_json::from_str(saved.signed_json.as_ref().unwrap()).unwrap();
        *provider.accepted_raw.lock().unwrap() = Some(signed_payload.raw_transaction.clone());
        *provider.fail_store_after_accepted.lock().unwrap() = Some(daily.store.clone());
        let scope = online
            .begin_action(
                observer.id.clone(),
                BsvHomeBsvColdWalletActionIntent::Broadcast {
                    commitment: saved.review.commitment.clone(),
                },
            )
            .unwrap();
        *provider.cancel_after_accepted.lock().unwrap() = Some(Arc::downgrade(&scope));
        let broadcast = online.broadcast(observer.id.clone(), saved.review.commitment.clone(), scope.clone()).await.unwrap();
        assert_eq!(scope.outcome().unwrap().submission, BsvHomeBsvColdWalletSubmission::Submitted);
        assert!(scope.outcome().unwrap().requires_reload);
        assert!(broadcast.broadcasted);
        assert!(broadcast.persistence_error.is_some());
        assert_eq!(broadcast.transaction_id, Some(signed_payload.transaction_id));
        assert_eq!(online.snapshot(observer.id.clone()).await.unwrap().pending, Some(broadcast.clone()));
        assert_eq!(broadcast_action(&online, observer.id.clone(), saved.review.commitment.clone()).await.unwrap(), broadcast);
        assert!(!context.service.snapshot(offline_wallet.id.clone()).await.unwrap().pending.unwrap().broadcasted);
        let original = daily.store.records.lock().unwrap().get(&observer.id).unwrap().clone();
        let other_request = context.request(offline_wallet.id.clone(), 75002).await;
        let request: Request = serde_json::from_str(&other_request).unwrap();
        let other_review = serde_json::to_string(&request.review(&request.pairing).unwrap()).unwrap();
        let other_signed = context
            .keystore
            .bsv_cold_sign(
                keystore_id_for_wallet(offline_wallet.id.id()),
                other_request.clone(),
                other_review.clone(),
                decode_password(TEST_PASSWORD),
            )
            .unwrap();
        let mut replaced: State = serde_json::from_str(&original.value).unwrap();
        replaced.pending = Some(Pending {
            request: other_request,
            review: other_review,
            signed: Some(other_signed),
            broadcasted: false,
            persistence_error: None,
            reservation: None,
        });
        daily.store.records.lock().unwrap().get_mut(&observer.id).unwrap().value = serde_json::to_string(&replaced).unwrap();
        assert!(!online.snapshot(observer.id.clone()).await.unwrap().pending.unwrap().broadcasted);
        daily.store.records.lock().unwrap().insert(observer.id.clone(), original);
        *daily.store.fail.lock().unwrap() = false;
        let restored = online.snapshot(observer.id.clone()).await.unwrap().pending.unwrap();
        assert!(restored.broadcasted);
        assert_eq!(restored.persistence_error, None);
        assert_eq!(
            *provider.broadcasted.lock().unwrap(),
            vec![signed_payload.raw_transaction.clone(), signed_payload.raw_transaction]
        );
        assert_eq!(broadcast_action(&online, observer.id, saved.review.commitment).await.unwrap(), restored);
        assert!(!daily.keystore.has_stored_wallets().unwrap());
        assert_eq!(*daily.signer.signing_count.lock().unwrap(), 0);
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 1);
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_offline_next_request_retains_previous_signed_until_replacement_is_durable() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let first = context
            .service
            .import_request(wallet.id.clone(), context.request(wallet.id.clone(), 75000).await)
            .await
            .unwrap();
        let previous = sign_action(&context.service, wallet.id.clone(), first.review.commitment).await.unwrap();
        let next_json = context.request(wallet.id.clone(), 75001).await;
        assert!(context.service.review_request(wallet.id.clone(), next_json.clone(), Some("stale".into())).await.is_err());
        let next = context
            .service
            .review_request(wallet.id.clone(), next_json.clone(), Some(previous.review.commitment.clone()))
            .await
            .unwrap();
        context.service.cancel_request(wallet.id.clone(), next.review.commitment).await.unwrap();
        assert_eq!(context.service.snapshot(wallet.id.clone()).await.unwrap().pending, Some(previous.clone()));
        let next = context
            .service
            .review_request(wallet.id.clone(), next_json, Some(previous.review.commitment.clone()))
            .await
            .unwrap();
        *context.store.fail.lock().unwrap() = true;
        assert!(sign_action(&context.service, wallet.id.clone(), next.review.commitment.clone()).await.is_err());
        *context.store.fail.lock().unwrap() = false;
        let state: State = serde_json::from_str(&context.store.records.lock().unwrap().get(&wallet.id).unwrap().value).unwrap();
        assert_eq!(state.pending.unwrap().signed, previous.signed_json);
        assert_eq!(state.next.unwrap().signed, None);
        let replaced = sign_action(&context.service, wallet.id.clone(), next.review.commitment).await.unwrap();
        assert!(replaced.signed_json.is_some());
        assert_eq!(replaced.review.amount, 75001);
        assert_eq!(context.restored().snapshot(wallet.id).await.unwrap().pending, Some(replaced));
    });
}

#[test]
fn test_online_cold_wallet_rejects_secret_creation_preview_import_and_signing_before_authentication() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let pairing = context.service.pairing(wallet.id.clone()).await.unwrap();
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let gateway = Arc::new(BsvHomeGateway::new(
            provider.clone(),
            Arc::new(MemoryPreferencesStore::default()),
            Arc::new(EmptyPreferences),
        ));
        let online = BsvHomeBsvColdWalletService::online(
            context.wallets.clone(),
            context.addresses.clone(),
            gateway,
            context.keystore.clone(),
            context.store.clone(),
            context.signer.clone(),
        );
        assert!(online.create_words(online.capture_generation()).is_err());
        assert!(online.preview_recovery(ABANDON_PHRASE.into(), None, None, None, None, online.capture_generation()).is_err());
        assert!(
            online
                .create(
                    "forbidden".into(),
                    ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                    online.capture_generation()
                )
                .await
                .is_err()
        );
        assert!(
            online
                .import_recovery("forbidden".into(), ABANDON_PHRASE.into(), None, None, pairing, None, online.capture_generation())
                .await
                .is_err()
        );
        assert!(sign_action(&online, wallet.id, "unused".into()).await.is_err());
        assert_eq!(*context.signer.authentication_count.lock().unwrap(), 1);
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
        assert_eq!(provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_cold_wallet_observer_preview_uses_canonical_path_and_expected_address_rules() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let descriptor = context.service.pairing(wallet.id).await.unwrap();
        let pairing: Pairing = serde_json::from_str(&descriptor).unwrap();
        let expected = format!(" {} ", pairing.first_address);
        assert_eq!(
            context
                .service
                .preview_observer(descriptor.clone(), Some(" m/044h/236H/00' ".into()), Some(expected.clone()))
                .unwrap(),
            descriptor
        );
        assert_eq!(
            context.service.preview_observer(pairing.xpub, Some("m/044h/236H/00'".into()), Some(expected)).unwrap(),
            descriptor
        );
        assert!(context.service.preview_observer(descriptor.clone(), Some("m/44'/145'/0'".into()), None).is_err());
        assert!(
            context
                .service
                .preview_observer(descriptor, None, Some("1BoatSLRHtKNngkdXEeobR76b53LETtpyT".into()))
                .is_err()
        );
    });
}
