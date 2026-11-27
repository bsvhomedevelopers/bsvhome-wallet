use std::sync::Arc;

use futures::{channel::oneshot, executor::block_on, join};
use primitives::{Chain, Wallet, WalletId, testkit::ABANDON_PHRASE};

use super::testkit::Context;
use super::{BsvHomeBsvColdWalletActionIntent, BsvHomeBsvColdWalletActionScope, BsvHomeBsvColdWalletPending, BsvHomeBsvColdWalletSubmission};
use crate::services::error::BsvHomeServiceError;

async fn reviewed() -> (Context, Wallet, BsvHomeBsvColdWalletPending) {
    let context = Context::new();
    let wallet = context.create().await;
    let request = context.request(wallet.id.clone(), 75000).await;
    let pending = context.service.import_request(wallet.id.clone(), request).await.unwrap();
    *context.signer.authentication_count.lock().unwrap() = 0;
    *context.store.writes.lock().unwrap() = 0;
    (context, wallet, pending)
}

fn signing_scope(context: &Context, wallet: &Wallet, pending: &BsvHomeBsvColdWalletPending) -> Arc<BsvHomeBsvColdWalletActionScope> {
    context
        .service
        .begin_action(
            wallet.id.clone(),
            BsvHomeBsvColdWalletActionIntent::Sign {
                commitment: pending.review.commitment.clone(),
            },
        )
        .unwrap()
}

#[test]
fn test_cold_action_cancel_before_first_poll_blocks_authentication_and_key_then_fresh_scope_works() {
    block_on(async {
        let (context, wallet, pending) = reviewed().await;
        let scope = signing_scope(&context, &wallet, &pending);
        let signing = context.service.sign(wallet.id.clone(), pending.review.commitment.clone(), scope.clone());
        scope.cancel();
        assert_eq!(signing.await.err(), Some(BsvHomeServiceError::Cancelled));
        assert_eq!(*context.signer.authorization_count.lock().unwrap(), 0);
        assert_eq!(*context.signer.authentication_count.lock().unwrap(), 0);
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
        assert_eq!(*context.store.writes.lock().unwrap(), 0);
        assert_eq!(scope.outcome().unwrap().submission, BsvHomeBsvColdWalletSubmission::NotSubmitted);
        let next = signing_scope(&context, &wallet, &pending);
        assert!(
            context
                .service
                .sign(wallet.id.clone(), pending.review.commitment.clone(), next.clone())
                .await
                .unwrap()
                .signed_json
                .is_some()
        );
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 1);
        assert_eq!(
            context.service.sign(wallet.id, pending.review.commitment, next).await.err(),
            Some(BsvHomeServiceError::Cancelled)
        );
    });
}

#[test]
fn test_cold_action_rejects_other_owner_method_and_commitment_without_consuming_original_ready_scope() {
    block_on(async {
        let (context, wallet, pending) = reviewed().await;
        let scope = signing_scope(&context, &wallet, &pending);
        assert_eq!(
            context.restored().sign(wallet.id.clone(), pending.review.commitment.clone(), scope.clone()).await.err(),
            Some(BsvHomeServiceError::Cancelled)
        );
        assert_eq!(
            context.service.broadcast(wallet.id.clone(), pending.review.commitment.clone(), scope.clone()).await.err(),
            Some(BsvHomeServiceError::Cancelled)
        );
        assert_eq!(
            context.service.sign(wallet.id.clone(), "different".into(), scope.clone()).await.err(),
            Some(BsvHomeServiceError::Cancelled)
        );
        assert_eq!(*context.signer.authorization_count.lock().unwrap(), 0);
        assert!(context.service.sign(wallet.id, pending.review.commitment, scope).await.unwrap().signed_json.is_some());
    });
}

#[test]
fn test_cold_action_requires_real_password_proof_despite_successful_authorization_callback() {
    block_on(async {
        let (context, wallet, pending) = reviewed().await;
        *context.signer.skip_authorization.lock().unwrap() = true;
        let scope = signing_scope(&context, &wallet, &pending);
        assert_eq!(
            context.service.sign(wallet.id, pending.review.commitment, scope).await.err(),
            Some(BsvHomeServiceError::InvalidInput {
                msg: "Offline signing password authorization is required".into(),
            })
        );
        assert_eq!(*context.signer.authorization_count.lock().unwrap(), 1);
        assert_eq!(*context.signer.authentication_count.lock().unwrap(), 0);
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
        assert_eq!(*context.store.writes.lock().unwrap(), 0);
    });
}

#[test]
fn test_cold_action_cancellation_during_system_authentication_preserves_unsigned_review() {
    block_on(async {
        let (context, wallet, pending) = reviewed().await;
        let scope = signing_scope(&context, &wallet, &pending);
        let (entered, wait) = oneshot::channel();
        let (release, gate) = oneshot::channel();
        *context.signer.authentication_entered.lock().unwrap() = Some(entered);
        *context.signer.authentication_wait.lock().unwrap() = Some(gate);
        let signing = context.service.sign(wallet.id.clone(), pending.review.commitment.clone(), scope.clone());
        let cancel = async {
            wait.await.unwrap();
            scope.cancel();
            release.send(()).unwrap();
        };
        let (result, ()) = join!(signing, cancel);
        assert_eq!(result.err(), Some(BsvHomeServiceError::Cancelled));
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
        assert_eq!(*context.store.writes.lock().unwrap(), 0);
        assert!(context.service.snapshot(wallet.id).await.unwrap().pending.unwrap().signed_json.is_none());
    });
}

#[derive(Clone, Copy)]
enum RecordChange {
    Revision,
    Payload,
    Missing,
    ReadError,
}

#[test]
fn test_cold_action_actual_key_gate_rechecks_real_reviewed_store_truth_without_wallet_or_epoch_change() {
    block_on(async {
        for change in [RecordChange::Revision, RecordChange::Payload, RecordChange::Missing, RecordChange::ReadError] {
            let (context, wallet, pending) = reviewed().await;
            let scope = signing_scope(&context, &wallet, &pending);
            let generation = context.service.capture_generation();
            let (entered, wait) = oneshot::channel();
            let (release, gate) = oneshot::channel();
            *context.signer.signing_entered.lock().unwrap() = Some(entered);
            *context.signer.signing_wait.lock().unwrap() = Some(gate);
            let signing = context.service.sign(wallet.id.clone(), pending.review.commitment.clone(), scope.clone());
            let mutate = async {
                wait.await.unwrap();
                match change {
                    RecordChange::Revision => context.store.records.lock().unwrap().get_mut(&wallet.id).unwrap().revision += 1,
                    RecordChange::Payload => context.store.records.lock().unwrap().get_mut(&wallet.id).unwrap().value.push(' '),
                    RecordChange::Missing => {
                        context.store.records.lock().unwrap().remove(&wallet.id);
                    }
                    RecordChange::ReadError => *context.store.read_fail.lock().unwrap() = true,
                }
                assert_eq!(context.service.current_wallet_id().unwrap(), Some(wallet.id.clone()));
                assert_eq!(context.service.capture_generation(), generation);
                release.send(()).unwrap();
            };
            let (result, ()) = join!(signing, mutate);
            assert!(result.is_err());
            assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
            assert_eq!(*context.store.writes.lock().unwrap(), 0);
            assert_eq!(scope.outcome().unwrap().submission, BsvHomeBsvColdWalletSubmission::NotSubmitted);
        }
    });
}

#[test]
fn test_cold_action_actual_key_parameters_must_match_the_reviewed_wallet_request_and_review() {
    block_on(async {
        let (context, wallet, pending) = reviewed().await;
        let scope = signing_scope(&context, &wallet, &pending);
        let (entered, wait) = oneshot::channel();
        let (release, gate) = oneshot::channel();
        *context.signer.signing_entered.lock().unwrap() = Some(entered);
        *context.signer.signing_wait.lock().unwrap() = Some(gate);
        let signing = context.service.sign(wallet.id.clone(), pending.review.commitment.clone(), scope.clone());
        let review = async {
            wait.await.unwrap();
            let mut other = wallet.clone();
            other.name.push('x');
            assert_eq!(
                scope.check_signing(other, pending.request_json.clone(), pending.review_json.clone()),
                Err(BsvHomeServiceError::Cancelled)
            );
            assert_eq!(
                scope.check_signing(wallet.clone(), "{}".into(), pending.review_json.clone()),
                Err(BsvHomeServiceError::Cancelled)
            );
            assert_eq!(
                scope.check_signing(wallet.clone(), pending.request_json.clone(), "{}".into()),
                Err(BsvHomeServiceError::Cancelled)
            );
            assert_eq!(scope.check_signing(wallet.clone(), pending.request_json.clone(), pending.review_json.clone()), Ok(()));
            release.send(()).unwrap();
        };
        let (result, ()) = join!(signing, review);
        assert!(result.unwrap().signed_json.is_some());
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 1);
    });
}

#[test]
fn test_cold_secret_generation_blocks_late_first_poll_before_auth_or_import_writes() {
    block_on(async {
        let context = Context::new();
        let generation = context.service.capture_generation();
        let create = context
            .service
            .create("Late".into(), ABANDON_PHRASE.split_whitespace().map(String::from).collect(), generation);
        context.service.invalidate_actions().unwrap();
        assert_eq!(create.await.err(), Some(BsvHomeServiceError::Cancelled));
        assert_eq!(context.service.create_words(generation).err(), Some(BsvHomeServiceError::Cancelled));
        assert_eq!(
            context.service.preview_recovery("not-recovery-material".into(), None, None, None, None, generation).err(),
            Some(BsvHomeServiceError::Cancelled)
        );
        assert_eq!(
            context
                .service
                .import_recovery("Late".into(), ABANDON_PHRASE.into(), None, None, "{}".into(), None, generation)
                .await
                .err(),
            Some(BsvHomeServiceError::Cancelled)
        );
        assert_eq!(*context.signer.authentication_count.lock().unwrap(), 0);
        assert_eq!(context.wallets.wallets().await.unwrap().len(), 0);
        assert!(!context.keystore.has_stored_wallets().unwrap());
    });
}

#[test]
fn test_cold_wallet_selection_ticket_is_actual_epoch_current_and_single_use() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let other = WalletId::View(Chain::BitcoinSv, "public-other-wallet".into());
        assert!(!context.service.consume_wallet_selection(other).unwrap());
        assert!(context.service.consume_wallet_selection(wallet.id.clone()).unwrap());
        assert!(!context.service.consume_wallet_selection(wallet.id.clone()).unwrap());
        assert!(!context.restored().consume_wallet_selection(wallet.id.clone()).unwrap());
        let generation = context.service.capture_generation();
        context.service.select_wallet(wallet.id.clone()).await.unwrap();
        assert!(context.service.capture_generation() > generation);
        assert!(context.service.consume_wallet_selection(wallet.id.clone()).unwrap());
        context.service.select_wallet(wallet.id.clone()).await.unwrap();
        context.service.invalidate_actions().unwrap();
        assert!(!context.service.consume_wallet_selection(wallet.id).unwrap());
    });
}

#[test]
fn test_cold_secret_generation_cancellation_at_real_auth_gate_prevents_starting_import() {
    block_on(async {
        let context = Context::new();
        let generation = context.service.capture_generation();
        let (entered, wait) = oneshot::channel();
        let (release, gate) = oneshot::channel();
        *context.signer.authentication_entered.lock().unwrap() = Some(entered);
        *context.signer.authentication_wait.lock().unwrap() = Some(gate);
        let create = context
            .service
            .create("Cancelled".into(), ABANDON_PHRASE.split_whitespace().map(String::from).collect(), generation);
        let cancel = async {
            wait.await.unwrap();
            context.service.invalidate_actions().unwrap();
            release.send(()).unwrap();
        };
        let (result, ()) = join!(create, cancel);
        assert_eq!(result.err(), Some(BsvHomeServiceError::Cancelled));
        assert_eq!(context.wallets.wallets().await.unwrap().len(), 0);
        assert!(!context.keystore.has_stored_wallets().unwrap());
        assert_eq!(*context.store.writes.lock().unwrap(), 0);
    });
}
