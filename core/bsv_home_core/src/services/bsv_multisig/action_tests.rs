use std::sync::{Arc, Barrier, Mutex};
use std::thread;

use chrono::Utc;
use futures::{FutureExt, channel::oneshot, executor::block_on, join, pin_mut, poll};
use primitives::{Account, Chain, WalletId};
use serde_json::Value;
use std::sync::atomic::AtomicUsize;
use std::task::Poll;
use uuid::Uuid;

use super::*;
use crate::services::bsv_cold_wallet::{BsvHomeBsvColdWalletStoredState, testkit::MemoryColdWalletStore};
use crate::services::bsv_multisig::testkit::{Device, fixture, remote_proposal};
use crate::services::bsv_multisig::{BsvHomeBsvMultisigFundingStatus, BsvHomeBsvMultisigPaymailSettings, bsv_multisig_paymail_alias};
use bsv_home_bsv::rpc::BsvRequestScheduler;

fn intent() -> BsvHomeBsvMultisigActionIntent {
    BsvHomeBsvMultisigActionIntent::PrepareProposal {
        wallet_id: "collective-fixture".into(),
        destination: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5".into(),
        amount_bsv: "0.00042".into(),
        memo: "".into(),
    }
}

fn scope(device: &Device, owner: Uuid) -> Arc<BsvHomeBsvMultisigActionScope> {
    BsvHomeBsvMultisigActionScope::new(
        owner,
        device.auth.clone(),
        device.auth.multisig_version(),
        device.wallet.clone(),
        intent(),
        [1; 32],
        device.cold_store.clone(),
        None,
    )
}

#[test]
fn test_action_scope_binds_owner_identity_intent_and_review_without_consuming_other_scopes() {
    let device = Device::new(1);
    let owner = Uuid::new_v4();
    let action = scope(&device, owner);
    assert_eq!(action.claim(Uuid::new_v4(), &device.wallet, &intent(), &[1; 32]), Err(Error::WalletChanged));
    let mut changed = device.wallet.clone();
    changed.accounts = vec![Account::mock(Chain::BitcoinSv, "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5")];
    assert_eq!(action.claim(owner, &changed, &intent(), &[1; 32]), Err(Error::WalletChanged));
    let other = BsvHomeBsvMultisigActionIntent::WithdrawProposal {
        proposal_id: "proposal-fixture".into(),
        expected_version: 1,
        review_id: "review-fixture".into(),
    };
    assert_eq!(action.claim(owner, &device.wallet, &other, &[1; 32]), Err(Error::ConcurrentUpdate));
    let mut changed_input = intent();
    if let BsvHomeBsvMultisigActionIntent::PrepareProposal { amount_bsv, .. } = &mut changed_input {
        *amount_bsv = "0.00043".into();
    }
    assert_eq!(action.claim(owner, &device.wallet, &changed_input, &[1; 32]), Err(Error::ConcurrentUpdate));
    assert_eq!(action.claim(owner, &device.wallet, &intent(), &[2; 32]), Err(Error::ConcurrentUpdate));
    assert_eq!(action.claim(owner, &device.wallet, &intent(), &[1; 32]), Ok(()));
    assert_eq!(action.claim(owner, &device.wallet, &intent(), &[1; 32]), Err(Error::ConcurrentUpdate));
    action.finish().unwrap();
    assert_eq!(action.claim(owner, &device.wallet, &intent(), &[1; 32]), Err(Error::ConcurrentUpdate));
    assert_ne!(scope(&device, owner).id(), action.id());
    let reviewed = BsvHomeBsvMultisigActionIntent::SignProposal {
        proposal_id: "proposal-fixture".into(),
        expected_version: 1,
        review_id: "review-fixture".into(),
    };
    let proposal = BsvHomeBsvMultisigActionScope::new(
        owner,
        device.auth.clone(),
        0,
        device.wallet.clone(),
        reviewed.clone(),
        [1; 32],
        device.cold_store.clone(),
        None,
    );
    let stale = BsvHomeBsvMultisigActionIntent::SignProposal {
        proposal_id: "proposal-fixture".into(),
        expected_version: 2,
        review_id: "review-fixture".into(),
    };
    assert_eq!(proposal.claim(owner, &device.wallet, &stale, &[1; 32]), Err(Error::ConcurrentUpdate));
    assert_eq!(proposal.claim(owner, &device.wallet, &reviewed, &[1; 32]), Ok(()));
}

#[test]
fn test_action_cancellation_before_first_poll_and_before_send_does_not_cancel_the_feature() {
    let device = Device::new(1);
    let owner = Uuid::new_v4();
    let action = scope(&device, owner);
    action.cancel();
    assert_eq!(action.claim(owner, &device.wallet, &intent(), &[1; 32]), Err(Error::Cancelled));
    assert_eq!(action.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission);
    assert_eq!(device.auth.multisig_version(), 0);
    let next = scope(&device, owner);
    assert_eq!(next.claim(owner, &device.wallet, &intent(), &[1; 32]), Ok(()));
    next.cancel();
    assert_eq!(next.before_submission(None, None), Err(Error::Cancelled));
    assert_eq!(next.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission);
    assert_eq!(scope(&device, owner).check(), Ok(()));
}

#[test]
fn test_feature_cancellation_invalidates_all_previous_action_tickets() {
    let device = Device::new(1);
    let owner = Uuid::new_v4();
    let first = scope(&device, owner);
    let second = scope(&device, owner);
    device.auth.cancel();
    assert_eq!(first.check(), Err(Error::Cancelled));
    assert_eq!(second.claim(owner, &device.wallet, &intent(), &[1; 32]), Err(Error::Cancelled));
    assert_eq!(first.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission);
}

#[test]
fn test_action_unknown_and_submitted_outcomes_survive_cancel_and_sync_failure_without_identity_replacement() {
    let device = Device::new(1);
    let owner = Uuid::new_v4();
    let action = scope(&device, owner);
    action.claim(owner, &device.wallet, &intent(), &[1; 32]).unwrap();
    action.before_submission(None, Some("transaction-fixture".into())).unwrap();
    assert_eq!(action.before_submission(None, None), Err(Error::ConcurrentUpdate));
    let unknown = action.outcome().unwrap();
    assert_eq!(
        action.submitted(Some("untrusted-proposal".into()), Some("other-transaction".into())),
        Err(Error::ConcurrentUpdate)
    );
    assert_eq!(action.outcome().unwrap(), unknown);
    action.cancel();
    assert_eq!(action.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::Unknown);
    action.submitted(Some("proposal-fixture".into()), Some("transaction-fixture".into())).unwrap();
    action.sync_failed().unwrap();
    let accepted = action.outcome().unwrap();
    assert_eq!(
        accepted,
        BsvHomeBsvMultisigActionOutcome {
            submission: BsvHomeBsvMultisigActionSubmission::Submitted,
            proposal_id: Some("proposal-fixture".into()),
            transaction_id: Some("transaction-fixture".into()),
            sync_failed: true,
        }
    );
    assert_eq!(
        action.submitted(Some("other-proposal".into()), Some("other-transaction".into())),
        Err(Error::ConcurrentUpdate)
    );
    assert_eq!(action.outcome().unwrap(), accepted);
    assert_eq!(action.check(), Err(Error::Cancelled));
    let next = scope(&device, owner);
    assert_eq!(next.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::NotSubmitted);
    assert_eq!(next.check(), Ok(()));
}

#[test]
fn test_action_scope_allows_only_one_concurrent_execution() {
    let device = Device::new(1);
    let owner = Uuid::new_v4();
    let action = scope(&device, owner);
    let barrier = Arc::new(Barrier::new(2));
    let workers = (0..2)
        .map(|_| {
            let action = action.clone();
            let wallet = device.wallet.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                action.claim(owner, &wallet, &intent(), &[1; 32])
            })
        })
        .collect::<Vec<_>>();
    let results = workers.into_iter().map(|worker| worker.join().unwrap()).collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| **result == Err(Error::ConcurrentUpdate)).count(), 1);
}

#[derive(Default)]
struct Authorization {
    requests: Mutex<Vec<(String, BsvHomeBsvMultisigAuthorizationRequest)>>,
    on_authorize: Mutex<Option<Box<dyn Fn() -> Result<(), BsvHomeServiceError> + Send + Sync>>>,
}

#[async_trait]
impl BsvHomeBsvMultisigAuthorization for Authorization {
    async fn authorize(&self, wallet: Wallet, request: BsvHomeBsvMultisigAuthorizationRequest, scope: Arc<BsvHomeBsvMultisigActionScope>) -> Result<(), BsvHomeServiceError> {
        scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        self.requests.lock().unwrap().push((wallet.id.id(), request));
        let callback = self.on_authorize.lock().unwrap().take();
        if let Some(callback) = callback {
            callback()?;
        }
        scope.check().map_err(|_| BsvHomeServiceError::Cancelled)
    }
}

fn authorized(device: &Device, authorization: Arc<Authorization>) -> BsvHomeBsvMultisigService {
    BsvHomeBsvMultisigService::new_with_authorization(
        device.store.clone(),
        device.session.clone(),
        device.gateway.clone(),
        device.signer.clone(),
        device.auth.clone(),
        device.addresses.clone(),
        device.wallet.id.id(),
        authorization,
        device.cold_store.clone(),
    )
}

async fn loaded(device: &Device, service: &BsvHomeBsvMultisigService) {
    device.trust().await;
    service.load().await.unwrap();
}

fn payment_intent() -> BsvHomeBsvMultisigActionIntent {
    BsvHomeBsvMultisigActionIntent::PrepareProposal {
        wallet_id: "team-wallet".into(),
        destination: fixture().proposal.recipient,
        amount_bsv: "0.00042".into(),
        memo: "Team payment".into(),
    }
}

#[test]
fn test_cancelled_action_first_poll_and_mutation_queue_never_authorize_unlock_or_request() {
    block_on(async {
        for queued in [false, true] {
            let device = Device::new(1);
            let authorization = Arc::new(Authorization::default());
            let service = authorized(&device, authorization.clone());
            loaded(&device, &service).await;
            let scope = service.begin_action(payment_intent()).unwrap();
            let paths = device.provider.paths();
            let lock = service.mutation.lock().await;
            let operation = service.prepare_proposal("team-wallet".into(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into(), scope.clone());
            let result = if queued {
                let (result, ()) = join!(operation, async {
                    scope.cancel();
                    drop(lock);
                });
                result
            } else {
                scope.cancel();
                drop(lock);
                operation.await
            };
            assert_eq!(result.unwrap_err(), Error::Cancelled);
            assert_eq!(authorization.requests.lock().unwrap().len(), 0);
            assert_eq!(*device.signer.authentications.lock().unwrap(), 0);
            assert_eq!(device.provider.paths(), paths);
            assert_eq!(scope.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission);
            assert_eq!(service.begin_action(payment_intent()).unwrap().check(), Ok(()));
        }
    });
}

#[test]
fn test_online_password_rejection_and_cancellation_block_system_authentication_and_mutation() {
    block_on(async {
        for cancel in [false, true] {
            let device = Device::new(1);
            let authorization = Arc::new(Authorization::default());
            let service = authorized(&device, authorization.clone());
            loaded(&device, &service).await;
            let scope = service.begin_action(payment_intent()).unwrap();
            let cancelled = scope.clone();
            *authorization.on_authorize.lock().unwrap() = Some(Box::new(move || {
                if cancel {
                    cancelled.cancel();
                    Ok(())
                } else {
                    Err(BsvHomeServiceError::Cancelled)
                }
            }));
            assert_eq!(
                service
                    .prepare_proposal("team-wallet".into(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into(), scope.clone())
                    .await
                    .unwrap_err(),
                Error::Cancelled
            );
            assert_eq!(authorization.requests.lock().unwrap().len(), 1);
            assert_eq!(*device.signer.authentications.lock().unwrap(), 0);
            assert_eq!(device.provider.posts().len(), 0);
            assert_eq!(
                scope.outcome().unwrap().submission,
                if cancel {
                    BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission
                } else {
                    BsvHomeBsvMultisigActionSubmission::NotSubmitted
                }
            );
        }
    });
}

#[test]
fn test_system_unlock_and_queued_sign_cancellation_never_reach_keys_or_post() {
    block_on(async {
        for signing in [false, true] {
            let device = Device::new(1);
            *device.provider.proposals.lock().unwrap() = vec![remote_proposal("collecting", "COLLECTING")];
            let authorization = Arc::new(Authorization::default());
            let service = authorized(&device, authorization.clone());
            loaded(&device, &service).await;
            let scope = service
                .begin_action(reviewed_intent(
                    &service,
                    if signing {
                        BsvHomeBsvMultisigActionIntent::SignProposal {
                            proposal_id: "collecting".into(),
                            expected_version: 3,
                            review_id: "review-fixture".into(),
                        }
                    } else {
                        payment_intent()
                    },
                ))
                .unwrap();
            let cancelled = scope.clone();
            let callback: Box<dyn Fn() + Send + Sync> = Box::new(move || cancelled.cancel());
            if signing {
                *device.signer.on_sign.lock().unwrap() = Some(callback);
            } else {
                *device.signer.on_authenticate.lock().unwrap() = Some(callback);
            }
            let result = if signing {
                service.sign_proposal("collecting".into(), 3, scope.clone()).await
            } else {
                service
                    .prepare_proposal("team-wallet".into(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into(), scope.clone())
                    .await
            };
            assert_eq!(result.unwrap_err(), Error::Cancelled);
            assert_eq!(*device.signer.proposal_signatures.lock().unwrap(), 0);
            assert_eq!(device.provider.posts().len(), 0);
            assert_eq!(authorization.requests.lock().unwrap().len(), 1);
        }
    });
}

#[test]
fn test_successful_submission_survives_local_cas_failure_and_cancel_without_replaying() {
    block_on(async {
        let device = Device::new(1);
        let authorization = Arc::new(Authorization::default());
        let service = authorized(&device, authorization.clone());
        loaded(&device, &service).await;
        let scope = service.begin_action(payment_intent()).unwrap();
        let store = device.store.clone();
        *device.store.on_commit.lock().unwrap() = Some(Box::new(move || *store.fail_next_cas.lock().unwrap() = true));
        assert_eq!(
            service
                .prepare_proposal("team-wallet".into(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into(), scope.clone())
                .await
                .unwrap_err(),
            Error::ConcurrentUpdate
        );
        let outcome = scope.outcome().unwrap();
        assert_eq!(outcome.submission, BsvHomeBsvMultisigActionSubmission::Submitted);
        assert!(outcome.sync_failed);
        assert!(outcome.proposal_id.is_some());
        scope.cancel();
        assert_eq!(scope.outcome().unwrap(), outcome);
        assert_eq!(device.provider.posts().len(), 1);
        assert_eq!(
            service
                .prepare_proposal("team-wallet".into(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into(), scope.clone())
                .await
                .unwrap_err(),
            Error::Cancelled
        );
        assert_eq!(device.provider.posts().len(), 1);
    });
}

#[test]
fn test_funding_authorizes_actual_payer_and_cancel_before_broadcast_preserves_original_checkpoint() {
    block_on(async {
        let device = Device::new(1);
        device.provider.seed_funding(2);
        let authorization = Arc::new(Authorization::default());
        let service = authorized(&device, authorization.clone());
        loaded(&device, &service).await;
        let preparation = service.prepare_funding("team-wallet".into(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        let cancelled_before_poll = service
            .begin_action(BsvHomeBsvMultisigActionIntent::SubmitFunding {
                preparation_id: preparation.id.clone(),
            })
            .unwrap();
        cancelled_before_poll.cancel();
        assert_eq!(service.submit_funding(preparation.id.clone(), cancelled_before_poll).await.unwrap_err(), Error::Cancelled);
        assert!(service.cache().unwrap().preparations.contains_key(&preparation.id));
        let scope = service
            .begin_action(BsvHomeBsvMultisigActionIntent::SubmitFunding {
                preparation_id: preparation.id.clone(),
            })
            .unwrap();
        let cancelled = scope.clone();
        *device.store.on_commit.lock().unwrap() = Some(Box::new(move || cancelled.cancel()));
        assert_eq!(service.submit_funding(preparation.id, scope.clone()).await.unwrap_err(), Error::Cancelled);
        let requests = authorization.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, Device::wallet(2).id.id());
        drop(requests);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
        assert_eq!(device.store.read().funding.len(), 1);
        assert_eq!(scope.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission);
        let retry = service
            .begin_action(BsvHomeBsvMultisigActionIntent::RetryFunding { wallet_id: "team-wallet".into() })
            .unwrap();
        retry.cancel();
        assert_eq!(service.retry_funding("team-wallet".into(), retry).await.unwrap_err(), Error::Cancelled);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
        assert_eq!(authorization.requests.lock().unwrap().len(), 1);
    });
}

#[test]
fn test_failed_broadcast_retries_only_the_reviewed_fully_signed_unexpired_transaction() {
    block_on(async {
        for expired in [false, true] {
            let device = Device::new(1);
            let mut proposal = remote_proposal("failed", "READY");
            proposal.status = "BROADCAST_FAILED".into();
            if expired {
                proposal.expires_at = Utc::now().timestamp() - 1;
            }
            *device.provider.proposals.lock().unwrap() = vec![proposal];
            let authorization = Arc::new(Authorization::default());
            let service = authorized(&device, authorization.clone());
            loaded(&device, &service).await;
            assert_eq!(service.get_proposal("failed".into()).unwrap().can_broadcast, !expired);
            let scope = service
                .begin_action(reviewed_intent(
                    &service,
                    BsvHomeBsvMultisigActionIntent::BroadcastProposal {
                        proposal_id: "failed".into(),
                        expected_version: 3,
                        review_id: "review-fixture".into(),
                    },
                ))
                .unwrap();
            let result = service.broadcast_proposal("failed".into(), 3, scope.clone()).await;
            if expired {
                assert_eq!(result.unwrap_err(), Error::invalid("PROPOSAL_NOT_READY"));
                assert_eq!(authorization.requests.lock().unwrap().len(), 0);
                assert_eq!(device.provider.posts().len(), 0);
            } else {
                assert_eq!(result.unwrap().transaction_id, Some(fixture().signed_transaction_id));
                let posts = device.provider.posts();
                assert_eq!(posts.len(), 1);
                let body: Value = serde_json::from_slice(posts[0].body.as_ref().unwrap()).unwrap();
                assert_eq!(body["rawTransaction"], fixture().signed_transaction);
                assert_eq!(body["expectedVersion"], 3);
                assert_eq!(authorization.requests.lock().unwrap().len(), 1);
                assert_eq!(scope.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::Submitted);
            }
        }
    });
}

#[test]
fn test_collective_paymail_alias_canonicalization_rejects_unicode_and_non_alias_input() {
    assert_eq!(bsv_multisig_paymail_alias("  TEAM-7  ".into()).unwrap(), "team-7");
    for value in ["", "équipe", "İdentity", "Key", "team@mail.example", "team-", "a b"] {
        assert_eq!(bsv_multisig_paymail_alias(value.into()), Err(Error::Api { code: "INVALID_ALIAS".into() }));
    }
}

#[test]
fn test_proposal_and_paymail_actions_cancel_before_first_poll_without_touching_their_ports() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.proposals.lock().unwrap() = vec![remote_proposal("collecting", "COLLECTING")];
        let authorization = Arc::new(Authorization::default());
        let service = authorized(&device, authorization.clone());
        loaded(&device, &service).await;
        let settings = BsvHomeBsvMultisigPaymailSettings {
            wallet_id: "team-wallet".into(),
            handle: "team@paymail.example".into(),
            alias: "team".into(),
            domain: "paymail.example".into(),
            active: true,
            can_manage: true,
            version: 4,
            configuration_hash: service.get_wallet("team-wallet".into()).unwrap().configuration_id.unwrap(),
        };
        service.cache().unwrap().paymail_settings.insert("team-wallet".into(), settings.clone());
        let paths = device.provider.paths();
        for intent in [
            BsvHomeBsvMultisigActionIntent::SignProposal {
                proposal_id: "collecting".into(),
                expected_version: 3,
                review_id: "review-fixture".into(),
            },
            BsvHomeBsvMultisigActionIntent::BroadcastProposal {
                proposal_id: "collecting".into(),
                expected_version: 3,
                review_id: "review-fixture".into(),
            },
            BsvHomeBsvMultisigActionIntent::WithdrawProposal {
                proposal_id: "collecting".into(),
                expected_version: 3,
                review_id: "review-fixture".into(),
            },
            BsvHomeBsvMultisigActionIntent::UpdatePaymailSettings {
                wallet_id: "team-wallet".into(),
                alias: "changed".into(),
                active: false,
                expected_version: 4,
                configuration_hash: settings.configuration_hash.clone(),
            },
        ] {
            let scope = service.begin_action(reviewed_intent(&service, intent.clone())).unwrap();
            scope.cancel();
            let result = match intent {
                BsvHomeBsvMultisigActionIntent::SignProposal {
                    proposal_id, expected_version, ..
                } => service.sign_proposal(proposal_id, expected_version, scope.clone()).await.map(|_| ()),
                BsvHomeBsvMultisigActionIntent::BroadcastProposal {
                    proposal_id, expected_version, ..
                } => service.broadcast_proposal(proposal_id, expected_version, scope.clone()).await.map(|_| ()),
                BsvHomeBsvMultisigActionIntent::WithdrawProposal {
                    proposal_id, expected_version, ..
                } => service.withdraw_proposal(proposal_id, expected_version, scope.clone()).await.map(|_| ()),
                BsvHomeBsvMultisigActionIntent::UpdatePaymailSettings { alias, active, .. } => {
                    service.update_paymail_settings(settings.clone(), alias, active, scope.clone()).await.map(|_| ())
                }
                BsvHomeBsvMultisigActionIntent::PrepareProposal { .. }
                | BsvHomeBsvMultisigActionIntent::SubmitFunding { .. }
                | BsvHomeBsvMultisigActionIntent::RetryFunding { .. }
                | BsvHomeBsvMultisigActionIntent::RetryPublicFunding { .. } => {
                    unreachable!()
                }
            };
            assert_eq!(result, Err(Error::Cancelled));
            assert_eq!(scope.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission);
        }
        assert_eq!(device.provider.paths(), paths);
        assert_eq!(authorization.requests.lock().unwrap().len(), 0);
        assert_eq!(*device.signer.proposal_signatures.lock().unwrap(), 0);
        assert_eq!(*device.signer.authentications.lock().unwrap(), 0);
    });
}

#[test]
fn test_passive_identity_configuration_and_withdraw_never_request_online_payment_authorization() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.proposals.lock().unwrap() = vec![remote_proposal("collecting", "COLLECTING")];
        let authorization = Arc::new(Authorization::default());
        *authorization.on_authorize.lock().unwrap() = Some(Box::new(|| Err(BsvHomeServiceError::Cancelled)));
        let service = authorized(&device, authorization.clone());
        loaded(&device, &service).await;
        service.current_signing_identity().await.unwrap();
        service.approve_configuration("team-wallet".into(), 7).await.unwrap();
        let action = service
            .begin_action(reviewed_intent(
                &service,
                BsvHomeBsvMultisigActionIntent::WithdrawProposal {
                    proposal_id: "collecting".into(),
                    expected_version: 3,
                    review_id: "review-fixture".into(),
                },
            ))
            .unwrap();
        let result = service.withdraw_proposal("collecting".into(), 3, action.clone()).await.unwrap();
        assert_eq!(result.remote_status, "WITHDRAWN");
        assert_eq!(authorization.requests.lock().unwrap().len(), 0);
        assert_eq!(action.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::Submitted);
    });
}

fn reviewed_intent(service: &BsvHomeBsvMultisigService, intent: BsvHomeBsvMultisigActionIntent) -> BsvHomeBsvMultisigActionIntent {
    match intent {
        BsvHomeBsvMultisigActionIntent::SignProposal {
            proposal_id, expected_version, ..
        } => BsvHomeBsvMultisigActionIntent::SignProposal {
            review_id: service.proposal_review_snapshot(proposal_id.clone()).unwrap().review_id,
            proposal_id,
            expected_version,
        },
        BsvHomeBsvMultisigActionIntent::BroadcastProposal {
            proposal_id, expected_version, ..
        } => BsvHomeBsvMultisigActionIntent::BroadcastProposal {
            review_id: service.proposal_review_snapshot(proposal_id.clone()).unwrap().review_id,
            proposal_id,
            expected_version,
        },
        BsvHomeBsvMultisigActionIntent::WithdrawProposal {
            proposal_id, expected_version, ..
        } => BsvHomeBsvMultisigActionIntent::WithdrawProposal {
            review_id: service.proposal_review_snapshot(proposal_id.clone()).unwrap().review_id,
            proposal_id,
            expected_version,
        },
        other @ (BsvHomeBsvMultisigActionIntent::PrepareProposal { .. }
        | BsvHomeBsvMultisigActionIntent::SubmitFunding { .. }
        | BsvHomeBsvMultisigActionIntent::RetryFunding { .. }
        | BsvHomeBsvMultisigActionIntent::RetryPublicFunding { .. }
        | BsvHomeBsvMultisigActionIntent::UpdatePaymailSettings { .. }) => other,
    }
}

#[test]
fn test_review_snapshot_rejects_changed_cache_before_scope_or_claim_even_when_version_is_unchanged() {
    block_on(async {
        for change_version in [false, true] {
            for claimed_before_change in [false, true] {
                let device = Device::new(1);
                *device.provider.proposals.lock().unwrap() = vec![remote_proposal("collecting", "COLLECTING")];
                let authorization = Arc::new(Authorization::default());
                let service = authorized(&device, authorization.clone());
                loaded(&device, &service).await;
                let reviewed = service.proposal_review_snapshot("collecting".into()).unwrap();
                assert_eq!(reviewed.proposal.version, 3);
                assert_eq!(reviewed.fields[2].value, reviewed.proposal.value_satoshis);
                let intent = BsvHomeBsvMultisigActionIntent::SignProposal {
                    proposal_id: reviewed.proposal.id.clone(),
                    expected_version: reviewed.proposal.version,
                    review_id: reviewed.review_id.clone(),
                };
                let action = claimed_before_change.then(|| service.begin_action(intent.clone()).unwrap());
                {
                    let mut cache = service.cache().unwrap();
                    let current = cache.proposals.get_mut("collecting").unwrap();
                    if change_version {
                        current.version += 1;
                    } else {
                        current.proposal.amount += 1;
                    }
                }
                assert_eq!(service.begin_action(intent).err(), Some(Error::ConcurrentUpdate));
                if let Some(action) = action {
                    assert_eq!(
                        service.sign_proposal(reviewed.proposal.id, reviewed.proposal.version, action).await.unwrap_err(),
                        Error::ConcurrentUpdate
                    );
                }
                assert_eq!(authorization.requests.lock().unwrap().len(), 0);
                assert_eq!(*device.signer.proposal_signatures.lock().unwrap(), 0);
                assert_eq!(device.provider.posts().len(), 0);
            }
        }
    });
}

#[test]
fn test_review_ids_are_service_scoped_bounded_and_allow_a_fresh_review_after_action_cancellation() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.proposals.lock().unwrap() = vec![remote_proposal("collecting", "COLLECTING")];
        let authorization = Arc::new(Authorization::default());
        let service = authorized(&device, authorization);
        loaded(&device, &service).await;
        let first = service.proposal_review_snapshot("collecting".into()).unwrap();
        for _ in 0..32 {
            service.proposal_review_snapshot("collecting".into()).unwrap();
        }
        assert_eq!(service.cache().unwrap().reviews.len(), 32);
        assert_eq!(
            service
                .begin_action(BsvHomeBsvMultisigActionIntent::SignProposal {
                    proposal_id: first.proposal.id,
                    expected_version: first.proposal.version,
                    review_id: first.review_id
                })
                .err(),
            Some(Error::ConcurrentUpdate)
        );
        let reviewed = service.proposal_review_snapshot("collecting".into()).unwrap();
        let intent = BsvHomeBsvMultisigActionIntent::SignProposal {
            proposal_id: reviewed.proposal.id.clone(),
            expected_version: reviewed.proposal.version,
            review_id: reviewed.review_id.clone(),
        };
        let action = service.begin_action(intent.clone()).unwrap();
        action.cancel();
        assert_eq!(service.begin_action(intent).unwrap().check(), Ok(()));
        let other = device.reopened_service();
        other.load().await.unwrap();
        assert_eq!(
            other
                .begin_action(BsvHomeBsvMultisigActionIntent::SignProposal {
                    proposal_id: reviewed.proposal.id,
                    expected_version: reviewed.proposal.version,
                    review_id: reviewed.review_id
                })
                .err(),
            Some(Error::ConcurrentUpdate)
        );
        service.cancel();
        assert_eq!(service.cache().unwrap().reviews.len(), 0);
    });
}

#[test]
fn test_funding_payer_deleted_during_online_authorization_is_rejected_before_key_access() {
    block_on(async {
        let device = Device::new(1);
        device.provider.seed_funding(2);
        let authorization = Arc::new(Authorization::default());
        let service = authorized(&device, authorization.clone());
        loaded(&device, &service).await;
        let prepared = service.prepare_funding("team-wallet".into(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        let action = service
            .begin_action(BsvHomeBsvMultisigActionIntent::SubmitFunding {
                preparation_id: prepared.id.clone(),
            })
            .unwrap();
        let wallets = device.wallets.clone();
        *authorization.on_authorize.lock().unwrap() = Some(Box::new(move || {
            wallets.wallets.lock().unwrap().retain(|wallet| wallet.id != Device::wallet(2).id);
            Ok(())
        }));
        assert_eq!(service.submit_funding(prepared.id, action).await.unwrap_err(), Error::FundingPayerMissing);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_unknown_network_results_keep_scope_fact_and_retry_only_original_funding_after_fresh_authorization() {
    block_on(async {
        let device = Device::new(1);
        device.provider.seed_funding(2);
        let authorization = Arc::new(Authorization::default());
        let service = authorized(&device, authorization.clone());
        loaded(&device, &service).await;
        *device.provider.fail_prepare_after_commit.lock().unwrap() = true;
        let scope = service.begin_action(payment_intent()).unwrap();
        assert!(
            service
                .prepare_proposal("team-wallet".into(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into(), scope.clone())
                .await
                .is_err()
        );
        assert_eq!(scope.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::Unknown);
        scope.cancel();
        assert_eq!(scope.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::Unknown);
        assert_eq!(device.provider.posts().len(), 1);
        *device.provider.fail_broadcast.lock().unwrap() = true;
        let prepared = service.prepare_funding("team-wallet".into(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        let scope = service
            .begin_action(BsvHomeBsvMultisigActionIntent::SubmitFunding {
                preparation_id: prepared.id.clone(),
            })
            .unwrap();
        let pending = service.submit_funding(prepared.id, scope.clone()).await.unwrap();
        assert_eq!(pending.status, BsvHomeBsvMultisigFundingStatus::Pending);
        assert_eq!(scope.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::Unknown);
        let original = device.provider.broadcasts.lock().unwrap()[0].clone();
        *device.provider.fail_broadcast.lock().unwrap() = false;
        let retry = service
            .begin_action(BsvHomeBsvMultisigActionIntent::RetryFunding { wallet_id: "team-wallet".into() })
            .unwrap();
        let accepted = service.retry_funding("team-wallet".into(), retry.clone()).await.unwrap();
        assert_eq!(accepted.transaction_id, pending.transaction_id);
        assert_eq!(accepted.status, BsvHomeBsvMultisigFundingStatus::Broadcast);
        assert_eq!(retry.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::Submitted);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
        assert_eq!(*device.provider.broadcasts.lock().unwrap(), [original.clone(), original]);
        let requests = authorization.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[1].0, Device::wallet(2).id.id());
        assert_eq!(requests[2].0, Device::wallet(2).id.id());
    });
}

#[test]
fn test_action_cancel_aborts_a_queued_future_without_releasing_its_gate_or_touching_keys() {
    block_on(async {
        for cancel_feature in [false, true] {
            let device = Device::new(1);
            let authorization = Arc::new(Authorization::default());
            let service = authorized(&device, authorization);
            loaded(&device, &service).await;
            let scope = service.begin_action(payment_intent()).unwrap();
            service.claim_action(&scope, payment_intent()).unwrap();
            let (release, waiting) = oneshot::channel::<()>();
            let keys = AtomicUsize::new(0);
            let operation = scope.run(async {
                waiting.await.unwrap();
                keys.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });
            pin_mut!(operation);
            assert_eq!(poll!(operation.as_mut()), Poll::Pending);
            if cancel_feature {
                service.cancel();
            } else {
                scope.cancel();
            }
            assert_eq!(operation.now_or_never(), Some(Err(Error::Cancelled)));
            assert_eq!(keys.load(Ordering::SeqCst), 0);
            drop(release);
        }
    });
}

fn mark_cold(store: &MemoryColdWalletStore, wallet_id: WalletId) {
    store
        .records
        .lock()
        .unwrap()
        .insert(wallet_id, BsvHomeBsvColdWalletStoredState { revision: 0, value: "{}".into() });
}

#[test]
fn test_member_financial_admission_rejects_existing_cold_metadata_even_without_online_authorization() {
    block_on(async {
        let device = Device::new(1);
        device.trust().await;
        device.service.load().await.unwrap();
        let before = device.provider.paths();
        mark_cold(&device.cold_store, device.wallet.id.clone());
        assert!(device.service.begin_action(payment_intent()).is_err());
        assert_eq!(device.provider.paths(), before);
        assert_eq!(*device.signer.authentications.lock().unwrap(), 0);
        assert_eq!(*device.signer.proposal_signatures.lock().unwrap(), 0);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
    });
}

#[test]
fn test_member_cold_marker_or_read_error_after_real_mutation_queue_blocks_authentication_and_business_post() {
    block_on(async {
        for read_error in [false, true] {
            let device = Device::new(1);
            let authorization = Arc::new(Authorization::default());
            let service = authorized(&device, authorization.clone());
            loaded(&device, &service).await;
            let scope = service.begin_action(payment_intent()).unwrap();
            let before = device.provider.paths();
            let lock = service.mutation.lock().await;
            let operation = service.prepare_proposal("team-wallet".into(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into(), scope);
            pin_mut!(operation);
            assert!(poll!(operation.as_mut()).is_pending());
            if read_error {
                *device.cold_store.read_fail.lock().unwrap() = true;
            } else {
                mark_cold(&device.cold_store, device.wallet.id.clone());
            }
            drop(lock);
            assert!(operation.await.is_err());
            assert_eq!(device.provider.paths(), before);
            assert_eq!(authorization.requests.lock().unwrap().len(), 0);
            assert_eq!(*device.signer.authentications.lock().unwrap(), 0);
            assert_eq!(*device.signer.proposal_signatures.lock().unwrap(), 0);
            assert_eq!(device.provider.posts().len(), 0);
        }
    });
}

#[test]
fn test_member_real_key_callback_queue_rechecks_cold_truth_before_signatures_or_signature_post() {
    block_on(async {
        for read_error in [false, true] {
            let device = Device::new(1);
            *device.provider.proposals.lock().unwrap() = vec![remote_proposal("collecting", "COLLECTING")];
            let authorization = Arc::new(Authorization::default());
            let service = authorized(&device, authorization.clone());
            loaded(&device, &service).await;
            let intent = reviewed_intent(
                &service,
                BsvHomeBsvMultisigActionIntent::SignProposal {
                    proposal_id: "collecting".into(),
                    expected_version: 3,
                    review_id: "review-fixture".into(),
                },
            );
            let scope = service.begin_action(intent).unwrap();
            let (entered, wait) = oneshot::channel();
            let (release, gate) = oneshot::channel();
            *device.signer.sign_pause.lock().unwrap() = Some((entered, gate));
            let signing = service.sign_proposal("collecting".into(), 3, scope);
            let change = async {
                wait.await.unwrap();
                if read_error {
                    *device.cold_store.read_fail.lock().unwrap() = true;
                } else {
                    mark_cold(&device.cold_store, device.wallet.id.clone());
                }
                release.send(()).unwrap();
            };
            let (result, ()) = join!(signing, change);
            assert!(result.is_err());
            assert_eq!(*device.signer.proposal_signatures.lock().unwrap(), 0);
            assert_eq!(device.provider.posts().len(), 0);
            assert_eq!(authorization.requests.lock().unwrap().len(), 1);
        }
    });
}

#[test]
fn test_member_funding_scope_checks_the_actual_nonmember_payer_before_authorization() {
    block_on(async {
        let device = Device::new(1);
        device.provider.seed_funding(2);
        let authorization = Arc::new(Authorization::default());
        let service = authorized(&device, authorization.clone());
        loaded(&device, &service).await;
        let preparation = service.prepare_funding("team-wallet".into(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        mark_cold(&device.cold_store, Device::wallet(2).id);
        assert!(
            service
                .begin_action(BsvHomeBsvMultisigActionIntent::SubmitFunding { preparation_id: preparation.id })
                .is_err()
        );
        assert_eq!(authorization.requests.lock().unwrap().len(), 0);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_member_funding_cold_change_after_actual_broadcast_scheduler_preserves_signed_checkpoint_and_same_raw_retry() {
    block_on(async {
        for read_error in [false, true] {
            let device = Device::new(1);
            device.provider.seed_funding(2);
            let authorization = Arc::new(Authorization::default());
            let service = authorized(&device, authorization);
            loaded(&device, &service).await;
            let preparation = service.prepare_funding("team-wallet".into(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
            let scope = service
                .begin_action(BsvHomeBsvMultisigActionIntent::SubmitFunding {
                    preparation_id: preparation.id.clone(),
                })
                .unwrap();
            let scheduler = BsvRequestScheduler::shared();
            let queue = scheduler.hold_request_queue().await;
            let operation = service.submit_funding(preparation.id, scope);
            pin_mut!(operation);
            assert!(poll!(operation.as_mut()).is_pending());
            assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
            let signed = service.funding_snapshot("team-wallet").unwrap();
            assert!(!signed.raw.is_empty());
            if read_error {
                *device.cold_store.read_fail.lock().unwrap() = true;
            } else {
                mark_cold(&device.cold_store, Device::wallet(2).id);
            }
            drop(queue);
            assert!(operation.await.is_err());
            assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
            assert_eq!(service.funding_snapshot("team-wallet").unwrap(), signed);
            *device.cold_store.read_fail.lock().unwrap() = false;
            device.cold_store.records.lock().unwrap().remove(&Device::wallet(2).id);
            let retry = service
                .begin_action(BsvHomeBsvMultisigActionIntent::RetryFunding { wallet_id: "team-wallet".into() })
                .unwrap();
            assert!(service.retry_funding("team-wallet".into(), retry).await.is_ok());
            assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
            assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 1);
            assert!(device.provider.broadcasts.lock().unwrap()[0] == signed.raw);
        }
    });
}
