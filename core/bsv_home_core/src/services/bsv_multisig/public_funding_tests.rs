use serde::Serialize;
use std::{
    env, fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use bsv_home_bsv::multisig::policy_descriptor;
use futures::executor::block_on;
use futures::{FutureExt, pin_mut, poll};

use super::*;
use crate::gateway::GatewayError;
use crate::services::BsvHomeServiceError;
use crate::services::bsv_cold_wallet::{
    BsvHomeBsvColdWalletStateStamp, BsvHomeBsvColdWalletStore, BsvHomeBsvColdWalletStoredState,
    testkit::{Context as ColdContext, MemoryColdWalletStore},
};
use crate::services::bsv_multisig::action::BsvHomeBsvMultisigActionSubmission;
use crate::services::bsv_multisig::funding_model_tests::checkpoint;
use crate::services::bsv_multisig::model::BsvHomeBsvMultisigFundingStatus;
use crate::services::bsv_multisig::testkit::{Device, fixture};
use crate::services::bsv_multisig::{BsvHomeBsvMultisigAuthorizationRequest, bsv_multisig_payment_request_from_script};
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use futures::channel::oneshot;
use primitives::{Asset, Chain, TransactionInputType, WalletId, WalletType};

type AuthorizationHook = Box<dyn Fn(Arc<ActionScope>) -> Result<(), BsvHomeServiceError> + Send + Sync>;

#[derive(Default)]
struct Authorization {
    calls: AtomicUsize,
    hook: Mutex<Option<AuthorizationHook>>,
}

#[async_trait]
impl BsvHomeBsvMultisigAuthorization for Authorization {
    async fn authorize(&self, wallet: Wallet, _request: BsvHomeBsvMultisigAuthorizationRequest, scope: Arc<ActionScope>) -> Result<(), BsvHomeServiceError> {
        scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        assert_eq!(wallet.id, Device::wallet(2).id);
        self.calls.fetch_add(1, Ordering::SeqCst);
        let hook = self.hook.lock().unwrap().take();
        if let Some(hook) = hook {
            hook(scope.clone())?;
        }
        scope.check().map_err(|_| BsvHomeServiceError::Cancelled)
    }
}

fn service(device: &Device, authorization: Arc<Authorization>) -> BsvHomeBsvMultisigPublicFundingService {
    BsvHomeBsvMultisigPublicFundingService::new(
        device.session.clone(),
        device.gateway.clone(),
        device.addresses.clone(),
        device.signer.clone(),
        authorization,
        device.auth.clone(),
        device.wallet.clone(),
        Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default()))),
        Arc::new(MemoryColdWalletStore::default()),
    )
    .unwrap()
}

fn source() -> BsvHomeBsvMultisigPublicFundingSource {
    let script = policy_descriptor(&fixture().proposal.policy).unwrap().locking_script;
    BsvHomeBsvMultisigPublicFundingSource::PaymentRequest {
        request: bsv_multisig_payment_request_from_script("Public group".into(), script, "0.001".into()).unwrap(),
    }
}

fn funded() -> Device {
    let device = Device::with_fixed_wallet(2);
    device.provider.seed_funding(2);
    device.session.set_current_wallet_id(Some(Device::wallet(1).id)).unwrap();
    device
}

#[test]
fn test_public_funding_rejects_current_auth_and_never_uses_a_fabricated_member_identity() {
    let device = Device::new(2);
    let result = BsvHomeBsvMultisigPublicFundingService::new(
        device.session.clone(),
        device.gateway.clone(),
        device.addresses.clone(),
        device.signer.clone(),
        Arc::new(Authorization::default()),
        device.auth.clone(),
        device.wallet.clone(),
        Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default()))),
        Arc::new(MemoryColdWalletStore::default()),
    );
    assert_eq!(result.err(), Some(Error::WalletChanged));
    assert_eq!(device.provider.requests.lock().unwrap().len(), 0);
}

#[test]
fn test_public_funding_uses_actual_fixed_payer_shared_ledger_and_preserves_personal_selection() {
    block_on(async {
        let device = funded();
        let authorization = Arc::new(Authorization::default());
        let service = service(&device, authorization.clone());
        let prepared = service.prepare_funding(source(), "0.001".into()).await.unwrap();
        assert_eq!(prepared.payer_wallet_id, device.wallet.id.id());
        let action = service
            .begin_action(ActionIntent::SubmitFunding {
                preparation_id: prepared.id.clone(),
            })
            .unwrap();
        let result = service.submit_funding(prepared.id, action.clone()).await.unwrap();
        assert_eq!(result.status, BsvHomeBsvMultisigFundingStatus::Broadcast);
        assert_eq!(authorization.calls.load(Ordering::SeqCst), 1);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
        assert_eq!(device.session.current_wallet_id().unwrap(), Device::wallet(1).id);
        assert_eq!(service.funding_results().await.unwrap().len(), 1);
        assert_eq!(action.outcome().unwrap().transaction_id, Some(result.transaction_id));
        assert_eq!(device.provider.paths().iter().filter(|path| path.starts_with("/api/portal/")).count(), 0);
        assert_eq!(device.store.read().funding.len(), 0);
    });
}

#[test]
fn test_public_funding_unknown_restart_uses_same_raw_without_source_resolution_or_signing_again() {
    block_on(async {
        let device = funded();
        let authorization = Arc::new(Authorization::default());
        let first = service(&device, authorization.clone());
        *device.provider.fail_broadcast.lock().unwrap() = true;
        let prepared = first.prepare_funding(source(), "0.001".into()).await.unwrap();
        let action = first
            .begin_action(ActionIntent::SubmitFunding {
                preparation_id: prepared.id.clone(),
            })
            .unwrap();
        let pending = first.submit_funding(prepared.id, action).await.unwrap();
        assert_eq!(pending.status, BsvHomeBsvMultisigFundingStatus::Pending);
        let original = device.provider.broadcasts.lock().unwrap()[0].clone();
        first.cancel();
        let reopened = service(&device, authorization.clone());
        let results = reopened.funding_results().await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].operation_id, pending.operation_id);
        *device.provider.fail_broadcast.lock().unwrap() = false;
        let action = reopened
            .begin_action(ActionIntent::RetryPublicFunding {
                operation_id: pending.operation_id.clone(),
            })
            .unwrap();
        let accepted = reopened.retry_funding(pending.operation_id, action).await.unwrap();
        assert_eq!(accepted.status, BsvHomeBsvMultisigFundingStatus::Broadcast);
        let broadcasts = device.provider.broadcasts.lock().unwrap();
        assert_eq!(broadcasts.len(), 2);
        assert!(broadcasts.iter().all(|raw| *raw == original));
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
        assert_eq!(*device.signer.authentications.lock().unwrap(), 1);
        assert_eq!(authorization.calls.load(Ordering::SeqCst), 2);
        assert_eq!(device.session.current_wallet_id().unwrap(), Device::wallet(1).id);
    });
}

#[test]
fn test_public_funding_first_poll_and_owned_queue_cancellation_never_authorize_or_sign() {
    block_on(async {
        for queued in [false, true] {
            let device = funded();
            let authorization = Arc::new(Authorization::default());
            let service = service(&device, authorization.clone());
            let prepared = service.prepare_funding(source(), "0.001".into()).await.unwrap();
            let action = service
                .begin_action(ActionIntent::SubmitFunding {
                    preparation_id: prepared.id.clone(),
                })
                .unwrap();
            if queued {
                let queue = service.mutation.lock().await;
                let submit = service.submit_funding(prepared.id, action.clone()).fuse();
                pin_mut!(submit);
                assert!(poll!(&mut submit).is_pending());
                action.cancel();
                drop(queue);
                assert_eq!(submit.await.err(), Some(Error::Cancelled));
            } else {
                action.cancel();
                assert_eq!(service.submit_funding(prepared.id, action).await.err(), Some(Error::Cancelled));
            }
            assert_eq!(authorization.calls.load(Ordering::SeqCst), 0);
            assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
            assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
            assert_eq!(service.unsubmitted_funding().await.unwrap().len(), 0);
        }
    });
}

#[test]
fn test_public_funding_password_cancellation_releases_unsigned_without_touching_the_shared_auth() {
    block_on(async {
        let device = funded();
        let authorization = Arc::new(Authorization::default());
        *authorization.hook.lock().unwrap() = Some(Box::new(|scope| {
            scope.cancel();
            Ok(())
        }));
        let service = service(&device, authorization);
        let prepared = service.prepare_funding(source(), "0.001".into()).await.unwrap();
        let action = service
            .begin_action(ActionIntent::SubmitFunding {
                preparation_id: prepared.id.clone(),
            })
            .unwrap();
        assert_eq!(service.submit_funding(prepared.id, action).await.err(), Some(Error::Cancelled));
        assert_eq!(service.unsubmitted_funding().await.unwrap().len(), 0);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
        assert_eq!(device.auth.multisig_version(), 0);
        let next = service.prepare_funding(source(), "0.001".into()).await.unwrap();
        assert!(!next.id.is_empty());
    });
}

#[test]
fn test_public_unsigned_member_discard_remains_available_after_signer_deletion_but_cannot_broadcast_member_data() {
    block_on(async {
        let device = funded();
        let service = service(&device, Arc::new(Authorization::default()));
        let mut original = checkpoint(false);
        original.binding.scope = format!("{}|{}|team-wallet", device.auth.multisig_origin(), Device::wallet(1).id.id());
        let lease = device.addresses.reserve_funding(&device.wallet, original.binding.clone()).await.unwrap();
        drop(lease);
        device.wallets.wallets.lock().unwrap().retain(|wallet| wallet.id != Device::wallet(1).id);
        let unsigned = service.unsubmitted_funding().await.unwrap();
        assert_eq!(unsigned.len(), 1);
        assert_eq!(unsigned[0].operation_id, original.binding.operation_id);
        assert_eq!(service.funding_results().await.unwrap().len(), 0);
        assert_eq!(
            service
                .begin_action(ActionIntent::RetryPublicFunding {
                    operation_id: original.binding.operation_id.clone()
                })
                .err(),
            Some(Error::MissingRecord)
        );
        service.discard_unsubmitted_funding(original.binding.operation_id).await.unwrap();
        assert_eq!(service.unsubmitted_funding().await.unwrap().len(), 0);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_public_payer_eligibility_rechecks_dedicated_metadata_after_service_construction() {
    block_on(async {
        let device = funded();
        let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
        let service = BsvHomeBsvMultisigPublicFundingService::new(
            device.session.clone(),
            device.gateway.clone(),
            device.addresses.clone(),
            device.signer.clone(),
            Arc::new(Authorization::default()),
            device.auth.clone(),
            device.wallet.clone(),
            preferences.clone(),
            Arc::new(MemoryColdWalletStore::default()),
        )
        .unwrap();
        assert_eq!(
            super::super::funding::bsv_multisig_public_funding_payers(vec![device.wallet.clone()], preferences.clone(), Arc::new(MemoryColdWalletStore::default()), source())
                .await
                .unwrap()
                .len(),
            1
        );
        preferences.mark_multisig_identity(&device.wallet.id).unwrap();
        assert_eq!(
            super::super::funding::bsv_multisig_public_funding_payers(vec![device.wallet.clone()], preferences, Arc::new(MemoryColdWalletStore::default()), source())
                .await
                .unwrap()
                .len(),
            0
        );
        assert_eq!(service.prepare_funding(source(), "0.001".into()).await.err(), Some(Error::FundingPayerMissing));
        assert_eq!(device.provider.requests.lock().unwrap().len(), 0);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
    });
}

#[derive(Default)]
struct MetadataStore {
    inner: MemoryColdWalletStore,
    fail_reads: AtomicBool,
}

#[async_trait]
impl BsvHomeBsvColdWalletStore for MetadataStore {
    fn state_stamp(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvColdWalletStateStamp>, BsvHomeServiceError> {
        if self.fail_reads.load(Ordering::SeqCst) {
            return Err(BsvHomeServiceError::Store {
                msg: "fixture cold metadata read failed".into(),
            });
        }
        self.inner.state_stamp(wallet_id)
    }

    fn has_state(&self, wallet_id: WalletId) -> Result<bool, BsvHomeServiceError> {
        if self.fail_reads.load(Ordering::SeqCst) {
            return Err(BsvHomeServiceError::Store {
                msg: "fixture cold metadata read failed".into(),
            });
        }
        self.inner.has_state(wallet_id)
    }

    async fn get(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvColdWalletStoredState>, BsvHomeServiceError> {
        if self.fail_reads.load(Ordering::SeqCst) {
            return Err(BsvHomeServiceError::Store {
                msg: "fixture cold metadata read failed".into(),
            });
        }
        self.inner.get(wallet_id).await
    }

    async fn compare_and_swap(&self, wallet_id: WalletId, expected_revision: Option<u64>, state: BsvHomeBsvColdWalletStoredState) -> Result<bool, BsvHomeServiceError> {
        self.inner.compare_and_swap(wallet_id, expected_revision, state).await
    }
}

#[test]
fn test_public_payer_eligibility_excludes_a_real_saved_single_cold_pairing() {
    block_on(async {
        let cold = ColdContext::new();
        let wallet = cold.create().await;
        assert_eq!(wallet.wallet_type, WalletType::Single);
        assert_eq!(cold.store.records.lock().unwrap().len(), 1);
        let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
        let eligible = super::super::funding::bsv_multisig_public_funding_payers(vec![wallet], preferences, cold.store.clone(), source())
            .await
            .unwrap();
        assert_eq!(eligible.len(), 0);
    });
}

#[test]
fn test_checked_broadcast_rechecks_cold_metadata_after_scheduler_queue_and_retains_the_signed_checkpoint() {
    block_on(async {
        let cold = ColdContext::new();
        let cold_wallet = cold.create().await;
        let cold_record = cold.store.records.lock().unwrap()[&cold_wallet.id].clone();
        for fail_read in [false, true] {
            let device = funded();
            let metadata = Arc::new(MetadataStore::default());
            let service = BsvHomeBsvMultisigPublicFundingService::new(
                device.session.clone(),
                device.gateway.clone(),
                device.addresses.clone(),
                device.signer.clone(),
                Arc::new(Authorization::default()),
                device.auth.clone(),
                device.wallet.clone(),
                Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default()))),
                metadata.clone(),
            )
            .unwrap();
            let mut original = checkpoint(true);
            original.binding.scope = service.scope();
            let lease = device.addresses.reserve_funding(&device.wallet, original.binding.clone()).await.unwrap();
            device.addresses.save_funding_signed(&device.wallet, &lease.id, original.clone()).await.unwrap();
            drop(lease);
            service.funding_results().await.unwrap();
            let intent = ActionIntent::RetryPublicFunding {
                operation_id: original.binding.operation_id.clone(),
            };
            let action = service.begin_action(intent.clone()).unwrap();
            action.claim(service.owner, &service.payer, &intent, &funding_action_hash(&original).unwrap()).unwrap();
            device
                .gateway
                .get_fee_rates(
                    Chain::BitcoinSv,
                    TransactionInputType::Transfer {
                        asset: Asset::from_chain(Chain::BitcoinSv),
                    },
                )
                .await
                .unwrap();
            let (entered, wait) = oneshot::channel();
            let entered = Mutex::new(Some(entered));
            let (release, released) = oneshot::channel();
            let released = Mutex::new(Some(released));
            let checks = AtomicUsize::new(0);
            let check = || {
                action.check().map_err(|_| GatewayError::PlatformError { msg: "scope cancelled".into() })?;
                if checks.fetch_add(1, Ordering::SeqCst) == 1 {
                    entered.lock().unwrap().take().unwrap().send(()).unwrap();
                }
                Ok(())
            };
            let before_send = || async {
                let wait = released.lock().unwrap().take().unwrap();
                wait.await.unwrap();
                service.eligible_payer().await.map(|_| ()).map_err(|_| GatewayError::PlatformError {
                    msg: "payer metadata unavailable".into(),
                })
            };
            let before_submission = || {
                action
                    .before_submission(None, Some(original.transaction_id.clone()))
                    .map_err(|_| GatewayError::PlatformError { msg: "scope cancelled".into() })
            };
            let update = async {
                wait.await.unwrap();
                if fail_read {
                    metadata.fail_reads.store(true, Ordering::SeqCst);
                } else {
                    metadata.inner.records.lock().unwrap().insert(device.wallet.id.clone(), cold_record.clone());
                }
                release.send(()).unwrap();
            };
            let broadcast = device.gateway.bsv_multisig_funding_broadcast_checked(
                original.binding.funding_json.clone(),
                original.binding.review_json.clone(),
                original.raw.clone(),
                &check,
                before_send,
                &before_submission,
            );
            let (result, ()) = futures::join!(broadcast, update);
            assert!(result.is_err());
            assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
            assert_eq!(action.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission);
            let retained = device
                .addresses
                .funding_checkpoints(&device.wallet)
                .await
                .unwrap()
                .into_iter()
                .find(|checkpoint| checkpoint.binding.operation_id == original.binding.operation_id)
                .unwrap();
            assert!(retained.same_signed_payment(&original));
        }
    });
}

#[test]
fn test_public_recovery_keeps_signed_and_unsigned_journals_visible_after_payer_role_changes() {
    block_on(async {
        for signed in [false, true] {
            let device = funded();
            let metadata = Arc::new(MetadataStore::default());
            let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
            let service = BsvHomeBsvMultisigPublicFundingService::new(
                device.session.clone(),
                device.gateway.clone(),
                device.addresses.clone(),
                device.signer.clone(),
                Arc::new(Authorization::default()),
                device.auth.clone(),
                device.wallet.clone(),
                preferences.clone(),
                metadata.clone(),
            )
            .unwrap();
            let mut original = checkpoint(true);
            original.binding.scope = service.scope();
            let lease = device.addresses.reserve_funding(&device.wallet, original.binding.clone()).await.unwrap();
            if signed {
                device.addresses.save_funding_signed(&device.wallet, &lease.id, original.clone()).await.unwrap();
            }
            drop(lease);
            preferences.mark_multisig_identity(&device.wallet.id).unwrap();
            metadata.fail_reads.store(true, Ordering::SeqCst);
            assert_eq!(service.prepare_funding(source(), "0.001".into()).await.err(), Some(Error::FundingPayerMissing));
            if signed {
                assert_eq!(service.funding_results().await.unwrap().len(), 1);
                assert_eq!(
                    service
                        .begin_action(ActionIntent::RetryPublicFunding {
                            operation_id: original.binding.operation_id.clone()
                        })
                        .err(),
                    Some(Error::FundingPayerMissing)
                );
                assert_eq!(service.retry_funding_sync(original.binding.operation_id).await.err(), Some(Error::FundingPending));
            } else {
                let unsigned = service.unsubmitted_funding().await.unwrap();
                assert_eq!(unsigned.len(), 1);
                service.discard_unsubmitted_funding(unsigned[0].operation_id.clone()).await.unwrap();
                assert_eq!(service.unsubmitted_funding().await.unwrap().len(), 0);
            }
            assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
        }
    });
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeScopeFixture {
    format_version: u32,
    origin: String,
    payer: Wallet,
    operation_id: String,
    stored_revision: u64,
    stored_value: String,
}

#[test]
fn test_public_retry_scope_fixture_restores_the_actual_sdk_validated_payer_journal_without_authentication_or_network() {
    block_on(async {
        let device = funded();
        let authorization = Arc::new(Authorization::default());
        let first = service(&device, authorization.clone());
        *device.provider.fail_broadcast.lock().unwrap() = true;
        let preparation = first.prepare_funding(source(), "0.001".into()).await.unwrap();
        let action = first
            .begin_action(ActionIntent::SubmitFunding {
                preparation_id: preparation.id.clone(),
            })
            .unwrap();
        let pending = first.submit_funding(preparation.id, action).await.unwrap();
        assert_eq!(pending.status, BsvHomeBsvMultisigFundingStatus::Pending);
        let stored = device.wallets.bsv_addresses.states.lock().unwrap()[&device.wallet.id].clone();
        first.cancel();
        let counts = (
            device.provider.requests.lock().unwrap().len(),
            authorization.calls.load(Ordering::SeqCst),
            *device.signer.funding_signatures.lock().unwrap(),
        );
        let restored = service(&device, authorization.clone());
        let results = restored.funding_results().await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].operation_id, pending.operation_id);
        let action = restored
            .begin_action(ActionIntent::RetryPublicFunding {
                operation_id: pending.operation_id.clone(),
            })
            .unwrap();
        assert_eq!(action.check(), Ok(()));
        assert_eq!(
            (
                device.provider.requests.lock().unwrap().len(),
                authorization.calls.load(Ordering::SeqCst),
                *device.signer.funding_signatures.lock().unwrap()
            ),
            counts
        );
        if env::var("WOWPAY_EXPORT_PUBLIC_SCOPE_FIXTURE").ok().as_deref() == Some("1") {
            let fixture = NativeScopeFixture {
                format_version: 1,
                origin: device.auth.multisig_origin().into(),
                payer: device.wallet.clone(),
                operation_id: pending.operation_id,
                stored_revision: stored.revision,
                stored_value: stored.value,
            };
            let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ios/build/phase6-core/public-retry-scope-fixture.json");
            fs::create_dir_all(output.parent().unwrap()).unwrap();
            fs::write(output, serde_json::to_vec(&fixture).unwrap()).unwrap();
        }
    });
}

#[test]
fn test_public_funding_preparation_preserves_plain_decimal_amounts_at_small_satoshi_boundaries() {
    block_on(async {
        let device = funded();
        let service = service(&device, Arc::new(Authorization::default()));
        for amount in ["0.00000001", "0.0000001", "0.000001", "0.00001"] {
            let prepared = service.prepare_funding(source(), amount.into()).await.unwrap();
            let expected = rules::amount_bsv(amount).unwrap();
            assert_eq!(prepared.amount_satoshis, expected.to_string());
            match prepared.target {
                BsvHomeBsvMultisigFundingPreparationTarget::Public { request } => {
                    assert_eq!(request.amount, amount);
                    assert_eq!(request.validate(), Ok(()));
                }
                BsvHomeBsvMultisigFundingPreparationTarget::Member { .. } => panic!("public funding must retain the canonical public request"),
            }
        }
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_public_financial_scope_rechecks_the_real_synchronous_cold_store_and_rejects_read_errors() {
    block_on(async {
        let cold = ColdContext::new();
        let cold_wallet = cold.create().await;
        let record = cold.store.records.lock().unwrap()[&cold_wallet.id].clone();
        for fail_read in [false, true] {
            let device = funded();
            let metadata = Arc::new(MetadataStore::default());
            let service = BsvHomeBsvMultisigPublicFundingService::new(
                device.session.clone(),
                device.gateway.clone(),
                device.addresses.clone(),
                device.signer.clone(),
                Arc::new(Authorization::default()),
                device.auth.clone(),
                device.wallet.clone(),
                Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default()))),
                metadata.clone(),
            )
            .unwrap();
            let prepared = service.prepare_funding(source(), "0.001".into()).await.unwrap();
            let action = service.begin_action(ActionIntent::SubmitFunding { preparation_id: prepared.id }).unwrap();
            assert_eq!(action.check(), Ok(()));
            if fail_read {
                metadata.fail_reads.store(true, Ordering::SeqCst);
            } else {
                metadata.inner.records.lock().unwrap().insert(device.wallet.id.clone(), record.clone());
            }
            assert!(action.check().is_err());
            assert_eq!(action.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission);
            assert_eq!(service.funding_results().await.unwrap().len(), 0);
            assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
            assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
        }
    });
}

#[test]
fn test_dropped_funding_future_retains_unsigned_for_explicit_recovery_instead_of_claiming_async_drop_cleanup() {
    block_on(async {
        let device = funded();
        let service = service(&device, Arc::new(Authorization::default()));
        let preparation = service.prepare_funding(source(), "0.001".into()).await.unwrap();
        let action = service
            .begin_action(ActionIntent::SubmitFunding {
                preparation_id: preparation.id.clone(),
            })
            .unwrap();
        let (entered, started) = oneshot::channel();
        let (release, released) = oneshot::channel();
        *device.signer.sign_pause.lock().unwrap() = Some((entered, released));
        let submit = Box::pin(service.submit_funding(preparation.id, action));
        match futures::future::select(submit, started).await {
            futures::future::Either::Right((started, submit)) => {
                started.unwrap();
                drop(submit);
            }
            futures::future::Either::Left(_) => panic!("funding signing must remain gated"),
        }
        assert!(release.send(()).is_err());
        let unsigned = service.unsubmitted_funding().await.unwrap();
        assert_eq!(unsigned.len(), 1);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
        service.discard_unsubmitted_funding(unsigned[0].operation_id.clone()).await.unwrap();
        assert_eq!(service.unsubmitted_funding().await.unwrap().len(), 0);
    });
}

#[test]
fn test_funding_reservation_failure_prevents_password_authorization_and_key_access() {
    block_on(async {
        let device = funded();
        let authorization = Arc::new(Authorization::default());
        let service = service(&device, authorization.clone());
        let prepared = service.prepare_funding(source(), "0.001".into()).await.unwrap();
        *device.wallets.bsv_addresses.save_error.lock().unwrap() = Some(BsvHomeServiceError::Store {
            msg: "fixture reservation persistence failed".into(),
        });
        let action = service
            .begin_action(ActionIntent::SubmitFunding {
                preparation_id: prepared.id.clone(),
            })
            .unwrap();
        assert_eq!(
            service.submit_funding(prepared.id, action.clone()).await.err(),
            Some(Error::Storage {
                reason: "FUNDING_RESERVATION_RELEASE_FAILED".into()
            })
        );
        assert_eq!(authorization.calls.load(Ordering::SeqCst), 0);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
        assert_eq!(action.outcome().unwrap().submission, BsvHomeBsvMultisigActionSubmission::NotSubmitted);
        assert_eq!(device.wallets.bsv_addresses.states.lock().unwrap().len(), 0);
    });
}
