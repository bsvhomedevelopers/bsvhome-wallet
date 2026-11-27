use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::future::Future;
use std::sync::{Arc, Mutex, atomic::Ordering};
use std::task::{Context, Poll};

use async_trait::async_trait;
use bitcoin::{Transaction as BitcoinTransaction, consensus::deserialize};
use bsv_home_bsv::rpc::BsvRequestScheduler;
use futures::channel::oneshot;
use futures::executor::block_on;
use futures::task::noop_waker;
use hex::decode as decode_hex;
use primitives::testkit::ABANDON_PHRASE;
use primitives::{Asset, Chain, Wallet};

use serde_json::{Value, from_slice};

use super::BsvHomeConfirmSession;
use crate::BsvHomeCoreError;
use crate::alien::{AlienError, AlienProvider, AlienResponse, AlienTarget};
use crate::models::transaction::{BsvHomeSignedTransaction, BsvHomeSignerInput};
use crate::services::balance::BsvHomeAssetBalance;
use crate::services::bsv_cold_wallet::testkit::MemoryColdWalletStore;
use crate::services::bsv_cold_wallet::{BsvHomeBsvColdWalletStoredState, bsv_cold_wallet_require_hot_signing};
use crate::services::confirm::BsvHomeTransactionSigner;
use crate::services::confirm::testkit::{
    BsvTransactionSigner, ConfirmTestkit, DerivedBsvTransactionSigner, DerivedSendContext, GatedTransactionSigner, MemoryConfirmAuthorization, MemoryTransactionCheckpoint,
    RecordingBroadcastProvider, bsv_send_input,
};
use crate::services::confirm::{
    BsvHomeConfirmError, BsvHomeConfirmFeeSelection, BsvHomeConfirmLoad, BsvHomeConfirmLoadOptions, BsvHomeConfirmMetadata, BsvHomeConfirmPreload, BsvHomeConfirmSimulationState,
    BsvHomeExecuteResult, BsvHomeTransactionCheckpoint, BsvHomeTransactionCheckpointData, BsvHomeTransferAmountResult, SendInput,
};
use crate::services::wallet::addresses::testkit::MemoryBsvAddressStore;
use crate::transfer_amount::BsvHomeTransferAmount;

async fn loaded_session(
    kit: &ConfirmTestkit,
    input: &SendInput,
    authorization: Arc<MemoryConfirmAuthorization>,
    checkpoint: Option<Arc<dyn BsvHomeTransactionCheckpoint>>,
) -> Arc<BsvHomeConfirmSession> {
    let session = kit.service.clone().session_with_authorization(
        input.wallet.clone(),
        input.confirm.input.transfer.clone(),
        None,
        checkpoint,
        kit.wallet_session.clone(),
        authorization,
    );
    let asset = Asset::from_chain(Chain::BitcoinSv);
    let balance = BsvHomeAssetBalance {
        asset_id: asset.id.clone(),
        available: 100_000u32.into(),
        ..BsvHomeAssetBalance::mock()
    };
    *session.screen.lock().await = Some(BsvHomeConfirmLoad {
        sender: input.confirm.input.from.clone(),
        fee_asset: asset,
        metadata: BsvHomeConfirmMetadata {
            asset_balance: balance.clone(),
            fee_asset_balance: balance,
            prices: vec![],
        },
        fee_assets: vec![],
        simulation: BsvHomeConfirmSimulationState {
            chain: Chain::BitcoinSv,
            result: None,
            warnings: vec![],
            simulation: None,
            address_names: vec![],
        },
        address_name: None,
        preload: Some(BsvHomeConfirmPreload {
            confirm_data: input.confirm.clone(),
            amount: BsvHomeTransferAmountResult::Amount {
                amount: BsvHomeTransferAmount {
                    value: input.value.clone(),
                    network_fee: input.network_fee.clone(),
                    is_max_amount: input.confirm.input.transfer.use_max_amount,
                },
            },
        }),
    });
    session
}

fn assert_cancelled<T: Debug>(result: Result<T, BsvHomeConfirmError>) {
    match result {
        Err(BsvHomeConfirmError::Cancelled) => {}
        result => panic!("expected typed cancellation, received {result:?}"),
    }
}

fn assert_cancelled_poll<T: Debug>(result: Poll<Result<T, BsvHomeConfirmError>>) {
    match result {
        Poll::Ready(result) => assert_cancelled(result),
        Poll::Pending => panic!("cancelled execution must not wait for signing or broadcasting"),
    }
}

fn broadcasts(provider: &RecordingBroadcastProvider) -> usize {
    provider.requests.lock().unwrap().iter().filter(|request| request.url.ends_with("/tx/raw")).count()
}

#[test]
fn test_authorization_denial_prevents_real_signer_and_broadcast_ingress() {
    block_on(async {
        let input = bsv_send_input();
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let signer = Arc::new(BsvTransactionSigner::default());
        let kit = ConfirmTestkit::with_signer(input.wallet.clone(), provider.clone(), signer.clone());
        let authorization = Arc::new(MemoryConfirmAuthorization::default());
        authorization.denied.store(true, Ordering::SeqCst);
        let session = loaded_session(&kit, &input, authorization.clone(), None).await;
        assert_cancelled(session.execute().await);
        assert_eq!(authorization.calls.lock().unwrap().len(), 1);
        assert_eq!(signer.calls.load(Ordering::SeqCst), 0);
        assert_eq!(broadcasts(&provider), 0);
    });
}

#[test]
fn test_authorization_binds_wallet_root_and_rejects_cancelled_late_callbacks() {
    block_on(async {
        for change in ["cancel", "selection", "root"] {
            let input = bsv_send_input();
            let provider = Arc::new(RecordingBroadcastProvider::default());
            let signer = Arc::new(BsvTransactionSigner::default());
            let kit = ConfirmTestkit::with_signer(input.wallet.clone(), provider.clone(), signer.clone());
            let authorization = Arc::new(MemoryConfirmAuthorization::default());
            let (release, gate) = oneshot::channel();
            *authorization.gate.lock().unwrap() = Some(gate);
            let session = loaded_session(&kit, &input, authorization.clone(), None).await;
            let mut execution = Box::pin(session.execute());
            let waker = futures::task::noop_waker();
            let mut context = Context::from_waker(&waker);
            assert!(execution.as_mut().poll(&mut context).is_pending());
            assert_eq!(authorization.calls.lock().unwrap().len(), 1);
            match change {
                "cancel" => session.cancel(),
                "selection" => kit.wallet_session.set_current_wallet_id(None).unwrap(),
                "root" => kit.wallets.wallets.lock().unwrap()[0].accounts[0].address = "1BitcoinEaterAddressDontSendf59kuE".into(),
                _ => unreachable!(),
            }
            release.send(()).unwrap();
            assert_cancelled_poll(execution.as_mut().poll(&mut context));
            assert_eq!(signer.calls.load(Ordering::SeqCst), 0);
            assert_eq!(broadcasts(&provider), 0);
        }
    });
}

#[test]
fn test_authorizing_session_rejects_duplicate_execution_and_fee_reload() {
    block_on(async {
        let input = bsv_send_input();
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let kit = ConfirmTestkit::with_signer(input.wallet.clone(), provider.clone(), Arc::new(BsvTransactionSigner::default()));
        let authorization = Arc::new(MemoryConfirmAuthorization::default());
        let (release, gate) = oneshot::channel();
        *authorization.gate.lock().unwrap() = Some(gate);
        let session = loaded_session(&kit, &input, authorization.clone(), None).await;
        let mut execution = Box::pin(session.execute());
        let waker = futures::task::noop_waker();
        let mut context = Context::from_waker(&waker);
        assert!(execution.as_mut().poll(&mut context).is_pending());
        assert_cancelled(session.execute().await);
        assert_cancelled(
            session
                .load(BsvHomeConfirmLoadOptions {
                    fee_selection: BsvHomeConfirmFeeSelection::Custom { gas_price: 101.into() },
                    fee_asset_id: None,
                })
                .await,
        );
        let fee = session.state().await.unwrap().preload.unwrap().confirm_data.fee;
        assert_eq!(
            (fee.fee, fee.gas_price_type, fee.gas_limit, fee.fee_asset),
            (
                input.confirm.fee.fee.clone(),
                input.confirm.fee.gas_price_type.clone(),
                input.confirm.fee.gas_limit.clone(),
                input.confirm.fee.fee_asset.clone()
            )
        );
        assert_eq!(authorization.calls.lock().unwrap().len(), 1);
        assert_eq!(broadcasts(&provider), 0);
        session.cancel();
        release.send(()).unwrap();
        assert_cancelled_poll(execution.as_mut().poll(&mut context));
    });
}

#[test]
fn test_checkpoint_retry_requires_new_authorization_without_signing_again() {
    block_on(async {
        let input = bsv_send_input();
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let signer = Arc::new(BsvTransactionSigner::default());
        let kit = ConfirmTestkit::with_signer(input.wallet.clone(), provider.clone(), signer.clone());
        let authorization = Arc::new(MemoryConfirmAuthorization::default());
        let checkpoint = Arc::new(MemoryTransactionCheckpoint::default());
        let session = loaded_session(&kit, &input, authorization.clone(), Some(checkpoint.clone())).await;
        assert!(session.execute().await.is_err());
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(broadcasts(&provider), 1);
        assert!(checkpoint.saved.lock().unwrap().is_some());
        authorization.denied.store(true, Ordering::SeqCst);
        assert_cancelled(session.execute().await);
        assert_eq!(broadcasts(&provider), 1);
        authorization.denied.store(false, Ordering::SeqCst);
        assert!(session.execute().await.is_err());
        assert_eq!(authorization.calls.lock().unwrap().len(), 3);
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(broadcasts(&provider), 2);
    });
}

#[test]
fn test_hd_saved_retry_requires_new_authorization_without_signing_again() {
    block_on(async {
        let context = DerivedSendContext::new();
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let signer = Arc::new(DerivedBsvTransactionSigner::default());
        let kit = ConfirmTestkit::with_bsv_signer(
            context.input.wallet.clone(),
            provider.clone(),
            context.keystore.clone(),
            Arc::new(MemoryBsvAddressStore::default()),
            signer.clone(),
        );
        kit.bsv_addresses.initialize_import(&context.input.wallet, ABANDON_PHRASE).await.unwrap();
        let authorization = Arc::new(MemoryConfirmAuthorization::default());
        let session = loaded_session(&kit, &context.input, authorization.clone(), None).await;
        assert!(session.execute().await.is_err());
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(broadcasts(&provider), 1);
        authorization.denied.store(true, Ordering::SeqCst);
        assert_cancelled(session.execute().await);
        assert_eq!(broadcasts(&provider), 1);
        authorization.denied.store(false, Ordering::SeqCst);
        assert!(session.execute().await.is_err());
        assert_eq!(authorization.calls.lock().unwrap().len(), 3);
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(broadcasts(&provider), 2);
    });
}

#[test]
fn test_cancellation_during_system_signing_releases_hd_reservation_without_broadcast() {
    block_on(async {
        let context = DerivedSendContext::new();
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let signer = Arc::new(DerivedBsvTransactionSigner::default());
        let (release, gate) = oneshot::channel();
        let gated = Arc::new(GatedTransactionSigner {
            signer: signer.clone(),
            gate: Mutex::new(Some(gate)),
        });
        let kit = ConfirmTestkit::with_bsv_signer(
            context.input.wallet.clone(),
            provider.clone(),
            context.keystore.clone(),
            Arc::new(MemoryBsvAddressStore::default()),
            gated,
        );
        kit.bsv_addresses.initialize_import(&context.input.wallet, ABANDON_PHRASE).await.unwrap();
        let session = loaded_session(&kit, &context.input, Arc::new(MemoryConfirmAuthorization::default()), None).await;
        let mut execution = Box::pin(session.execute());
        let waker = futures::task::noop_waker();
        let mut task = Context::from_waker(&waker);
        assert!(execution.as_mut().poll(&mut task).is_pending());
        session.cancel();
        release.send(()).unwrap();
        assert_cancelled(execution.await);
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(broadcasts(&provider), 0);
        let transfer = &context.input.confirm.input.transfer;
        assert!(
            kit.bsv_addresses
                .pending_input(&context.input.wallet, &transfer.recipient.address, &transfer.value.to_string(), transfer.use_max_amount)
                .await
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn test_final_broadcast_guard_rechecks_scope_after_checkpoint_authorization() {
    block_on(async {
        let input = bsv_send_input();
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let signer = Arc::new(BsvTransactionSigner::default());
        let kit = ConfirmTestkit::with_signer(input.wallet.clone(), provider.clone(), signer.clone());
        let checkpoint = Arc::new(MemoryTransactionCheckpoint::default());
        let (release, gate) = oneshot::channel();
        *checkpoint.authorization_gate.lock().unwrap() = Some(gate);
        let session = loaded_session(&kit, &input, Arc::new(MemoryConfirmAuthorization::default()), Some(checkpoint.clone())).await;
        let mut execution = Box::pin(session.execute());
        let waker = futures::task::noop_waker();
        let mut context = Context::from_waker(&waker);
        assert!(execution.as_mut().poll(&mut context).is_pending());
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(checkpoint.authorizations.load(Ordering::SeqCst), 1);
        session.cancel();
        release.send(()).unwrap();
        assert_cancelled(execution.await);
        assert_eq!(broadcasts(&provider), 0);
        assert!(checkpoint.saved.lock().unwrap().is_some());
    });
}

#[derive(Clone, Copy)]
enum BsvBroadcastReply {
    Accepted,
    Timeout,
}

struct SchedulerBroadcastProvider {
    requests: Mutex<Vec<AlienTarget>>,
    reply: Mutex<BsvBroadcastReply>,
    after_post: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl Debug for SchedulerBroadcastProvider {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.debug_struct("SchedulerBroadcastProvider").finish()
    }
}

impl Default for SchedulerBroadcastProvider {
    fn default() -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            reply: Mutex::new(BsvBroadcastReply::Accepted),
            after_post: Mutex::new(None),
        }
    }
}

impl SchedulerBroadcastProvider {
    fn broadcasts(&self) -> Vec<Vec<u8>> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.url.ends_with("/tx/raw"))
            .map(|request| request.body.clone().unwrap())
            .collect()
    }
}

#[async_trait]
impl AlienProvider for SchedulerBroadcastProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        let broadcast = target.url.ends_with("/tx/raw");
        let hash = if broadcast {
            let body: Value = from_slice(target.body.as_deref().unwrap()).unwrap();
            let bytes = decode_hex(body["txhex"].as_str().unwrap()).unwrap();
            let transaction: BitcoinTransaction = deserialize(&bytes).unwrap();
            Some(transaction.compute_txid().to_string())
        } else {
            None
        };
        self.requests.lock().unwrap().push(target);
        if let Some(hash) = hash {
            if let Some(after_post) = self.after_post.lock().unwrap().take() {
                after_post();
            }
            return match *self.reply.lock().unwrap() {
                BsvBroadcastReply::Accepted => Ok(Arc::new(AlienResponse::new(Some(200), hash.into_bytes()))),
                BsvBroadcastReply::Timeout => Err(AlienError::RequestError {
                    msg: "BSV broadcast reply was not observed".into(),
                }),
            };
        }
        Ok(Arc::new(AlienResponse::new(Some(404), Vec::new())))
    }
}

async fn saved_bsv_checkpoint(kit: &ConfirmTestkit, input: &SendInput) -> BsvHomeTransactionCheckpointData {
    let transfer = &input.confirm.input.transfer;
    kit.bsv_addresses
        .saved_for_transfer(&input.wallet, &transfer.recipient.address, &transfer.value.to_string(), transfer.use_max_amount)
        .await
        .unwrap()
        .unwrap()
}

#[test]
fn test_bsv_confirm_rechecks_wallet_and_generation_after_actual_scheduler_queue_and_keeps_same_raw_retry() {
    block_on(async {
        for change in ["cancel", "selection", "root"] {
            let derived = DerivedSendContext::new();
            let input = derived.input.clone();
            let provider = Arc::new(SchedulerBroadcastProvider::default());
            let signer = Arc::new(DerivedBsvTransactionSigner::default());
            let kit = ConfirmTestkit::with_bsv_signer(
                input.wallet.clone(),
                provider.clone(),
                derived.keystore.clone(),
                Arc::new(MemoryBsvAddressStore::default()),
                signer.clone(),
            );
            kit.bsv_addresses.initialize_import(&input.wallet, ABANDON_PHRASE).await.unwrap();
            let authorization = Arc::new(MemoryConfirmAuthorization::default());
            let session = loaded_session(&kit, &input, authorization.clone(), None).await;
            let scheduler = BsvRequestScheduler::shared();
            let queue = scheduler.hold_request_queue().await;
            let mut execution = Box::pin(session.execute());
            let waker = noop_waker();
            let mut context = Context::from_waker(&waker);
            assert!(execution.as_mut().poll(&mut context).is_pending());
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert_eq!(authorization.calls.lock().unwrap().len(), 1);
            assert_eq!(provider.broadcasts().len(), 0);
            let saved = saved_bsv_checkpoint(&kit, &input).await;
            match change {
                "cancel" => session.cancel(),
                "selection" => kit.wallet_session.set_current_wallet_id(None).unwrap(),
                "root" => kit.wallets.wallets.lock().unwrap()[0].accounts[0].address = "1BitcoinEaterAddressDontSendf59kuE".into(),
                _ => unreachable!(),
            }
            drop(queue);
            assert_cancelled(execution.await);
            assert_eq!(provider.broadcasts().len(), 0);
            assert!(saved_bsv_checkpoint(&kit, &input).await == saved);
            *kit.wallets.wallets.lock().unwrap() = vec![input.wallet.clone()];
            kit.wallet_session.set_current_wallet_id(Some(input.wallet.id.clone())).unwrap();
            let retry = loaded_session(&kit, &input, authorization.clone(), None).await;
            let result = retry.execute().await.unwrap();
            assert_eq!(
                match result {
                    BsvHomeExecuteResult::Sent { hashes, .. } => hashes,
                    BsvHomeExecuteResult::Signed { .. } => Vec::new(),
                },
                vec![saved.transaction_id.clone()]
            );
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert_eq!(authorization.calls.lock().unwrap().len(), 2);
            assert_eq!(provider.broadcasts().len(), 1);
            assert!(saved_bsv_checkpoint(&kit, &input).await == saved);
        }
    });
}

#[test]
fn test_bsv_confirm_preserves_observed_acceptance_when_the_provider_cancels_the_scope() {
    block_on(async {
        let derived = DerivedSendContext::new();
        let input = derived.input.clone();
        let provider = Arc::new(SchedulerBroadcastProvider::default());
        let signer = Arc::new(DerivedBsvTransactionSigner::default());
        let kit = ConfirmTestkit::with_bsv_signer(
            input.wallet.clone(),
            provider.clone(),
            derived.keystore.clone(),
            Arc::new(MemoryBsvAddressStore::default()),
            signer.clone(),
        );
        kit.bsv_addresses.initialize_import(&input.wallet, ABANDON_PHRASE).await.unwrap();
        let authorization = Arc::new(MemoryConfirmAuthorization::default());
        let session = loaded_session(&kit, &input, authorization.clone(), None).await;
        let cancel = session.clone();
        *provider.after_post.lock().unwrap() = Some(Box::new(move || cancel.cancel()));
        let result = session.execute().await.unwrap();
        let saved = saved_bsv_checkpoint(&kit, &input).await;
        assert_eq!(
            match result {
                BsvHomeExecuteResult::Sent { hashes, .. } => hashes,
                BsvHomeExecuteResult::Signed { .. } => Vec::new(),
            },
            vec![saved.transaction_id]
        );
        assert_eq!(provider.broadcasts().len(), 1);
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(authorization.calls.lock().unwrap().len(), 1);
        assert_eq!(kit.transaction_status.tracked.lock().unwrap().len(), 1);
    });
}

#[test]
fn test_bsv_confirm_keeps_unknown_after_post_cancellation_and_retries_original_raw_without_resigning() {
    block_on(async {
        let derived = DerivedSendContext::new();
        let input = derived.input.clone();
        let provider = Arc::new(SchedulerBroadcastProvider::default());
        let signer = Arc::new(DerivedBsvTransactionSigner::default());
        let kit = ConfirmTestkit::with_bsv_signer(
            input.wallet.clone(),
            provider.clone(),
            derived.keystore.clone(),
            Arc::new(MemoryBsvAddressStore::default()),
            signer.clone(),
        );
        kit.bsv_addresses.initialize_import(&input.wallet, ABANDON_PHRASE).await.unwrap();
        let authorization = Arc::new(MemoryConfirmAuthorization::default());
        let session = loaded_session(&kit, &input, authorization.clone(), None).await;
        let cancel = session.clone();
        *provider.reply.lock().unwrap() = BsvBroadcastReply::Timeout;
        *provider.after_post.lock().unwrap() = Some(Box::new(move || cancel.cancel()));
        let result = session.execute().await;
        assert_eq!(
            match result {
                Err(BsvHomeConfirmError::Broadcast { hashes, .. }) => Some(hashes),
                _ => None,
            },
            Some(Vec::new())
        );
        let saved = saved_bsv_checkpoint(&kit, &input).await;
        assert_eq!(provider.broadcasts().len(), 1);
        *provider.reply.lock().unwrap() = BsvBroadcastReply::Accepted;
        let retry = loaded_session(&kit, &input, authorization.clone(), None).await;
        let result = retry.execute().await.unwrap();
        assert_eq!(
            match result {
                BsvHomeExecuteResult::Sent { hashes, .. } => hashes,
                BsvHomeExecuteResult::Signed { .. } => Vec::new(),
            },
            vec![saved.transaction_id.clone()]
        );
        let attempts = provider.broadcasts();
        assert_eq!(attempts.len(), 2);
        assert!(attempts.windows(2).all(|attempt| attempt[0] == attempt[1]));
        assert!(saved_bsv_checkpoint(&kit, &input).await == saved);
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(authorization.calls.lock().unwrap().len(), 2);
    });
}

fn mark_cold(store: &MemoryColdWalletStore, wallet: &Wallet) {
    store.records.lock().unwrap().insert(
        wallet.id.clone(),
        BsvHomeBsvColdWalletStoredState {
            revision: 0,
            value: "public metadata presence only".into(),
        },
    );
}

struct RoleQueuedSigner {
    store: Arc<MemoryColdWalletStore>,
    entered: Mutex<Option<oneshot::Sender<()>>>,
    resume: Mutex<Option<oneshot::Receiver<()>>>,
    signer: Arc<DerivedBsvTransactionSigner>,
}

#[async_trait]
impl BsvHomeTransactionSigner for RoleQueuedSigner {
    async fn sign(&self, wallet: Wallet, input: BsvHomeSignerInput) -> Result<Vec<BsvHomeSignedTransaction>, BsvHomeCoreError> {
        self.entered.lock().unwrap().take().unwrap().send(()).unwrap();
        let resume = self.resume.lock().unwrap().take().unwrap();
        resume.await.map_err(|_| BsvHomeCoreError::Cancelled)?;
        bsv_cold_wallet_require_hot_signing(wallet.clone(), input.input.input_type.get_asset().chain(), self.store.clone()).map_err(|_| BsvHomeCoreError::Cancelled)?;
        self.signer.sign(wallet, input).await
    }
}

#[test]
fn test_cold_marker_or_read_failure_while_the_signer_is_queued_prevents_key_call_and_post() {
    block_on(async {
        for read_error in [false, true] {
            let derived = DerivedSendContext::new();
            let input = derived.input.clone();
            let provider = Arc::new(SchedulerBroadcastProvider::default());
            let signer = Arc::new(DerivedBsvTransactionSigner::default());
            let kit = ConfirmTestkit::with_bsv_signer(
                input.wallet.clone(),
                provider.clone(),
                derived.keystore.clone(),
                Arc::new(MemoryBsvAddressStore::default()),
                signer.clone(),
            );
            kit.bsv_addresses.initialize_import(&input.wallet, ABANDON_PHRASE).await.unwrap();
            let (entered, entered_reply) = oneshot::channel();
            let (resume, resumed) = oneshot::channel();
            let queued = Arc::new(RoleQueuedSigner {
                store: kit.cold_store.clone(),
                entered: Mutex::new(Some(entered)),
                resume: Mutex::new(Some(resumed)),
                signer: signer.clone(),
            });
            let execute = kit.confirm.execute(input.clone(), queued);
            let role_change = async {
                entered_reply.await.unwrap();
                if read_error {
                    *kit.cold_store.read_fail.lock().unwrap() = true;
                } else {
                    mark_cold(kit.cold_store.as_ref(), &input.wallet);
                }
                resume.send(()).unwrap();
            };
            let (result, ()) = futures::join!(execute, role_change);
            assert_cancelled(result);
            assert_eq!(signer.calls.load(Ordering::SeqCst), 0);
            assert_eq!(provider.broadcasts().len(), 0);
        }
    });
}

#[test]
fn test_existing_cold_presence_blocks_actual_confirm_before_authorization_or_signing() {
    block_on(async {
        let derived = DerivedSendContext::new();
        let input = derived.input.clone();
        let provider = Arc::new(SchedulerBroadcastProvider::default());
        let signer = Arc::new(DerivedBsvTransactionSigner::default());
        let kit = ConfirmTestkit::with_bsv_signer(
            input.wallet.clone(),
            provider.clone(),
            derived.keystore.clone(),
            Arc::new(MemoryBsvAddressStore::default()),
            signer.clone(),
        );
        mark_cold(kit.cold_store.as_ref(), &input.wallet);
        let authorization = Arc::new(MemoryConfirmAuthorization::default());
        let session = loaded_session(&kit, &input, authorization.clone(), None).await;
        assert!(session.execute().await.is_err());
        assert_eq!(authorization.calls.lock().unwrap().len(), 0);
        assert_eq!(signer.calls.load(Ordering::SeqCst), 0);
        assert_eq!(provider.broadcasts().len(), 0);
    });
}

#[test]
fn test_cold_marker_after_actual_rpc_queue_entry_prevents_post_and_preserves_signed_checkpoint() {
    block_on(async {
        let derived = DerivedSendContext::new();
        let input = derived.input.clone();
        let provider = Arc::new(SchedulerBroadcastProvider::default());
        let signer = Arc::new(DerivedBsvTransactionSigner::default());
        let kit = ConfirmTestkit::with_bsv_signer(
            input.wallet.clone(),
            provider.clone(),
            derived.keystore.clone(),
            Arc::new(MemoryBsvAddressStore::default()),
            signer.clone(),
        );
        kit.bsv_addresses.initialize_import(&input.wallet, ABANDON_PHRASE).await.unwrap();
        let session = loaded_session(&kit, &input, Arc::new(MemoryConfirmAuthorization::default()), None).await;
        let scheduler = BsvRequestScheduler::shared();
        let queue = scheduler.hold_request_queue().await;
        let mut execution = Box::pin(session.execute());
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);
        assert!(execution.as_mut().poll(&mut context).is_pending());
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(provider.broadcasts().len(), 0);
        let saved = saved_bsv_checkpoint(&kit, &input).await;
        mark_cold(kit.cold_store.as_ref(), &input.wallet);
        drop(queue);
        assert!(execution.await.is_err());
        assert_eq!(provider.broadcasts().len(), 0);
        assert_eq!(saved_bsv_checkpoint(&kit, &input).await, saved);
    });
}
