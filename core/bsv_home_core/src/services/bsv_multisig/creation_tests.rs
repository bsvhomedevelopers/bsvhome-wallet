use futures::{executor::block_on, join};
use primitives::WalletType;

use crate::services::bsv_multisig::model::State;
use crate::services::bsv_multisig::testkit::{Device, member};
use crate::services::bsv_multisig::{BsvHomeBsvMultisigCreationDraft, BsvHomeBsvMultisigCreationStatus as Status, BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigStoredState};

fn names() -> Vec<String> {
    ["Creator", "Second", "Third"].map(String::from).to_vec()
}

async fn begin(device: &Device) -> BsvHomeBsvMultisigCreationDraft {
    device.service.begin_creation("Team wallet".into(), 2, names(), device.wallet.id.id()).await.unwrap()
}

async fn unknown(device: &Device) -> BsvHomeBsvMultisigCreationDraft {
    let draft = begin(device).await;
    *device.provider.fail_create_after_commit.lock().unwrap() = true;
    assert_eq!(
        device.service.submit_creation(draft.id.clone()).await.unwrap_err(),
        Error::CreationUncertain { draft_id: draft.id.clone() }
    );
    draft
}

#[test]
fn test_ready_creation_is_bounded_and_does_not_persist_or_submit() {
    block_on(async {
        let device = Device::new(1);
        let first = begin(&device).await;
        assert_eq!(first.status, Status::Ready);
        assert_eq!(begin(&device).await, first);
        let other = device.service.begin_creation("Different".into(), 2, names(), device.wallet.id.id()).await.unwrap();
        assert_ne!(first.id, other.id);
        assert_eq!(device.service.submit_creation(first.id.clone()).await.unwrap_err(), Error::MissingRecord);
        let replacement = begin(&device).await;
        assert_ne!(replacement.id, first.id);
        assert_ne!(replacement.id, other.id);
        assert_eq!(device.store.read().creations.len(), 0);
        assert_eq!(device.provider.posts().len(), 0);
    });
}

#[test]
fn test_explicit_new_creation_allows_equal_payload_after_resolution() {
    block_on(async {
        let device = Device::new(1);
        let first = begin(&device).await;
        let original = device.service.submit_creation(first.id.clone()).await.unwrap();
        let repeated = device.service.submit_creation(first.id.clone()).await.unwrap();
        assert_eq!(repeated.id, original.id);
        assert_eq!(device.provider.posts().len(), 1);
        let second = begin(&device).await;
        assert_eq!(second.status, Status::Ready);
        assert_ne!(second.id, first.id);
        let created = device.service.submit_creation(second.id).await.unwrap();
        assert_ne!(created.id, original.id);
        assert_eq!(created.name, original.name);
        assert_eq!(device.provider.posts().len(), 2);
        assert_eq!(device.store.read().creations.len(), 2);
        assert_eq!(device.store.read().trusted.len(), 0);
    });
}

#[test]
fn test_unknown_creation_restarts_as_submitted_and_requires_explicit_acceptance() {
    block_on(async {
        let device = Device::new(1);
        let draft = unknown(&device).await;
        let service = device.reopened_service();
        let resumed = service.begin_creation("Team wallet".into(), 2, names(), device.wallet.id.id()).await.unwrap();
        assert_eq!(resumed.id, draft.id);
        assert_eq!(resumed.status, Status::Submitted);
        assert_eq!(
            service.submit_creation(draft.id.clone()).await.unwrap_err(),
            Error::CreationUncertain { draft_id: draft.id.clone() }
        );
        let candidates = service.reconcile_creation(draft.id.clone()).await.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(device.store.read().creations[&service.scope(&draft.id)].wallet_id, None);
        assert_eq!(device.store.read().member_names.len(), 0);
        let accepted = service.accept_creation(draft.id.clone(), candidates[0].id.clone()).await.unwrap();
        assert_eq!(accepted.members.iter().map(|member| member.name.clone()).collect::<Vec<_>>(), names());
        assert_eq!(device.store.read().creations[&service.scope(&draft.id)].wallet_id, Some(accepted.id));
        assert_eq!(device.store.read().trusted.len(), 0);
        assert_eq!(device.provider.posts().len(), 1);
        assert_ne!(service.begin_creation("Team wallet".into(), 2, names(), device.wallet.id.id()).await.unwrap().id, draft.id);
    });
}

#[test]
fn test_reconciliation_excludes_baseline_and_does_not_choose_between_candidates() {
    block_on(async {
        let device = Device::new(1);
        let draft = unknown(&device).await;
        let mut parallel = device.provider.wallets.lock().unwrap().last().unwrap().clone();
        parallel.id = "parallel-created-wallet".into();
        device.provider.wallets.lock().unwrap().push(parallel);
        let candidates = device.service.reconcile_creation(draft.id.clone()).await.unwrap();
        assert_eq!(candidates.len(), 2);
        assert_eq!(
            device.service.accept_creation(draft.id.clone(), "team-wallet".into()).await.unwrap_err(),
            Error::MissingSigner
        );
        assert_eq!(device.store.read().creations[&device.service.scope(&draft.id)].wallet_id, None);
        assert_eq!(device.store.read().member_names.len(), 0);
        let accepted = device.service.accept_creation(draft.id, "parallel-created-wallet".into()).await.unwrap();
        assert_eq!(accepted.id, "parallel-created-wallet");
        assert_eq!(device.store.read().trusted.len(), 0);
        assert_eq!(device.provider.posts().len(), 1);
    });
}

#[test]
fn test_acceptance_rechecks_the_selected_remote_record_before_local_resolution() {
    block_on(async {
        for change in ["id", "name", "threshold", "creator"] {
            let device = Device::new(1);
            let draft = unknown(&device).await;
            let candidates = device.service.reconcile_creation(draft.id.clone()).await.unwrap();
            let id = candidates[0].id.clone();
            let mut forged = device.provider.wallets.lock().unwrap().last().unwrap().clone();
            match change {
                "id" => forged.id = "different-wallet".into(),
                "name" => forged.name = "Changed".into(),
                "threshold" => forged.threshold = 3,
                "creator" => forged.members[0] = member(2, false),
                _ => unreachable!(),
            }
            device.provider.override_data(&format!("/auth/multisig/wallets/{id}"), forged);
            assert_eq!(device.service.accept_creation(draft.id.clone(), id).await.unwrap_err(), Error::MissingSigner);
            assert_eq!(device.store.read().creations[&device.service.scope(&draft.id)].wallet_id, None);
            assert_eq!(device.store.read().member_names.len(), 0);
            assert_eq!(device.store.read().trusted.len(), 0);
            assert_eq!(device.provider.posts().len(), 1);
        }
    });
}

#[test]
fn test_creation_storage_failure_prevents_post_and_preserves_ready_review() {
    block_on(async {
        let device = Device::new(1);
        let draft = begin(&device).await;
        *device.store.fail_next_cas.lock().unwrap() = true;
        let result = device.service.submit_creation(draft.id.clone()).await;
        assert_eq!(device.provider.posts().len(), 0);
        assert_eq!(result.unwrap_err(), Error::ConcurrentUpdate);
        assert_eq!(device.store.read().creations.len(), 0);
        assert_eq!(begin(&device).await, draft);
        device.service.submit_creation(draft.id).await.unwrap();
        assert_eq!(device.provider.posts().len(), 1);
    });
}

#[test]
fn test_cancellation_after_submitted_cas_prevents_any_create_request() {
    block_on(async {
        let device = Device::new(1);
        let draft = begin(&device).await;
        let auth = device.auth.clone();
        *device.store.on_commit.lock().unwrap() = Some(Box::new(move || auth.cancel()));
        assert_eq!(device.service.submit_creation(draft.id.clone()).await.unwrap_err(), Error::Cancelled);
        assert_eq!(device.store.read().creations[&device.service.scope(&draft.id)].wallet_id, None);
        assert_eq!(device.provider.posts().len(), 0);
        assert_eq!(device.service.submit_creation(draft.id).await.unwrap_err(), Error::Cancelled);
    });
}

#[test]
fn test_cancellation_after_remote_creation_does_not_publish_or_resolve_the_draft() {
    block_on(async {
        let device = Device::new(1);
        let draft = begin(&device).await;
        let auth = device.auth.clone();
        *device.provider.on_create.lock().unwrap() = Some(Box::new(move || auth.cancel()));
        assert_eq!(device.service.submit_creation(draft.id.clone()).await.unwrap_err(), Error::Cancelled);
        assert_eq!(device.provider.posts().len(), 1);
        assert_eq!(device.store.read().creations[&device.service.scope(&draft.id)].wallet_id, None);
        assert_eq!(device.store.read().member_names.len(), 0);
        assert_eq!(device.store.read().trusted.len(), 0);
        assert_eq!(device.service.submit_creation(draft.id).await.unwrap_err(), Error::Cancelled);
        assert_eq!(device.provider.posts().len(), 1);
    });
}

#[test]
fn test_queued_creation_cannot_submit_after_cancellation() {
    block_on(async {
        let device = Device::new(1);
        let draft = begin(&device).await;
        let lock = device.service.mutation.lock().await;
        let (result, ()) = join!(device.service.submit_creation(draft.id), async {
            device.service.cancel();
            drop(lock);
        });
        assert_eq!(result.unwrap_err(), Error::Cancelled);
        assert_eq!(device.store.read().creations.len(), 0);
        assert_eq!(device.provider.posts().len(), 0);
    });
}

#[test]
fn test_creation_ticket_does_not_change_existing_current_scope_refresh_semantics() {
    block_on(async {
        let device = Device::new(1);
        device.service.cancel();
        device.service.load().await.unwrap();
        assert_eq!(device.service.get_wallets().unwrap().len(), 1);
        assert_eq!(
            device.service.begin_creation("Team wallet".into(), 2, names(), device.wallet.id.id()).await.unwrap_err(),
            Error::Cancelled
        );
        assert_eq!(device.provider.posts().len(), 0);
    });
}

#[test]
fn test_creation_cancelled_before_first_poll_cannot_begin_or_submit() {
    block_on(async {
        let device = Device::new(1);
        let beginning = device.service.begin_creation("Team wallet".into(), 2, names(), device.wallet.id.id());
        device.service.cancel();
        assert_eq!(beginning.await.unwrap_err(), Error::Cancelled);
        assert_eq!(device.provider.paths(), Vec::<String>::new());

        let device = Device::new(1);
        let draft = begin(&device).await;
        let paths = device.provider.paths();
        let submitting = device.service.submit_creation(draft.id);
        device.service.cancel();
        assert_eq!(submitting.await.unwrap_err(), Error::Cancelled);
        assert_eq!(device.provider.paths(), paths);
        assert_eq!(device.store.read().creations.len(), 0);
    });
}

#[test]
fn test_fixed_creation_rejects_deleted_changed_watch_and_cancelled_identity() {
    block_on(async {
        for change in ["deleted", "address", "watch", "cancelled"] {
            let device = Device::with_fixed_wallet(1);
            device.session.set_current_wallet_id(Some(Device::wallet(2).id)).unwrap();
            let draft = begin(&device).await;
            let paths = device.provider.paths();
            match change {
                "deleted" => device.wallets.wallets.lock().unwrap().retain(|wallet| wallet.id != device.wallet.id),
                "address" => device.wallets.wallets.lock().unwrap()[0].accounts[0].address = member(2, true).address,
                "watch" => device.wallets.wallets.lock().unwrap()[0].wallet_type = WalletType::View,
                "cancelled" => device.service.cancel(),
                _ => unreachable!(),
            }
            let expected = match change {
                "watch" => Error::WatchOnly,
                "cancelled" => Error::Cancelled,
                _ => Error::WalletChanged,
            };
            assert_eq!(device.service.submit_creation(draft.id).await.unwrap_err(), expected);
            assert_eq!(device.provider.paths(), paths);
            assert_eq!(device.store.read().creations.len(), 0);
            assert_eq!(device.session.current_wallet_id().unwrap(), Device::wallet(2).id);
        }
    });
}

#[test]
fn test_concurrent_ready_drafts_share_the_first_persisted_submission_without_replay() {
    block_on(async {
        let device = Device::new(1);
        let first = begin(&device).await;
        let other = device.reopened_service();
        let second = other.begin_creation("Team wallet".into(), 2, names(), device.wallet.id.id()).await.unwrap();
        assert_ne!(first.id, second.id);
        *device.provider.fail_create_after_commit.lock().unwrap() = true;
        assert_eq!(
            device.service.submit_creation(first.id.clone()).await.unwrap_err(),
            Error::CreationUncertain { draft_id: first.id.clone() }
        );
        assert_eq!(other.submit_creation(second.id).await.unwrap_err(), Error::CreationUncertain { draft_id: first.id.clone() });
        let resumed = other.begin_creation("Team wallet".into(), 2, names(), device.wallet.id.id()).await.unwrap();
        assert_eq!(resumed.id, first.id);
        assert_eq!(resumed.status, Status::Submitted);
        assert_eq!(device.provider.posts().len(), 1);
        assert_eq!(device.store.read().creations.len(), 1);
    });
}

#[test]
fn test_legacy_multisig_state_remains_readable_without_creation_metadata() {
    block_on(async {
        let device = Device::new(1);
        let mut legacy = State::default();
        legacy.member_names.insert("legacy-notes".into(), vec!["Original".into()]);
        let mut json = serde_json::to_value(legacy).unwrap();
        json.as_object_mut().unwrap().remove("creations");
        *device.store.state.lock().unwrap() = Some(BsvHomeBsvMultisigStoredState {
            revision: 1,
            json: serde_json::to_string(&json).unwrap(),
        });
        let draft = begin(&device).await;
        device.service.submit_creation(draft.id).await.unwrap();
        assert_eq!(device.store.read().member_names["legacy-notes"], ["Original"]);
        assert_eq!(device.store.read().creations.len(), 1);
    });
}

#[test]
fn test_pending_creation_identity_cannot_be_rewritten_by_stored_metadata() {
    block_on(async {
        let device = Device::new(1);
        let draft = unknown(&device).await;
        let mut state = device.store.read();
        state.creations.get_mut(&device.service.scope(&draft.id)).unwrap().payload.signing_identity.public_key = member(2, false).public_key;
        device.store.state.lock().unwrap().as_mut().unwrap().json = serde_json::to_string(&state).unwrap();
        let paths = device.provider.paths();
        assert_eq!(device.service.submit_creation(draft.id.clone()).await.unwrap_err(), Error::WalletChanged);
        assert_eq!(device.service.reconcile_creation(draft.id.clone()).await.unwrap_err(), Error::WalletChanged);
        assert_eq!(device.service.accept_creation(draft.id, "created-wallet-1".into()).await.unwrap_err(), Error::WalletChanged);
        assert_eq!(device.provider.paths(), paths);
        assert_eq!(device.store.read().trusted.len(), 0);
    });
}
