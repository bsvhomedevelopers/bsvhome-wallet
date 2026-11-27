use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use async_trait::async_trait;
use bsv_home_bsv::signer::{BsvChainSigner, plan_transfer};
use futures::executor::block_on;
use primitives::testkit::signer_mock::TEST_PRIVATE_KEY;
use primitives::{Chain, ChainSigner, SignerInput, TransactionLoadMetadata, TransactionType, Wallet, WalletId};
use tempfile::TempDir;

use super::authorization::ExecutionScope;
use super::testkit::{ConfirmTestkit, MemoryTransactionCheckpoint, RecordingBroadcastProvider, bsv_send_input};
use super::{BsvHomeConfirmError, BsvHomeTransactionCheckpoint, BsvHomeTransactionCheckpointData, BsvHomeTransactionSigner, SendInput};
use crate::BsvHomeCoreError;
use crate::alien::{AlienError, AlienProvider, AlienResponse, AlienTarget};
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore, decode_password};
use crate::models::transaction::{BsvHomeSignedTransaction, BsvHomeSignerInput};
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::addresses::{BsvHomeBsvAddressStore, testkit::MemoryBsvAddressStore};
use crate::services::wallet::{rules as wallet_rules, testkit::TEST_PASSWORD};

struct SingleSendContext {
    _directory: TempDir,
    keystore: Arc<BsvHomeKeystore>,
    input: SendInput,
}

impl SingleSendContext {
    fn new() -> Self {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let stored = keystore
            .create_store(
                BsvHomeImportType::PrivateKey {
                    chain: Chain::BitcoinSv,
                    value: ::signer::encode_private_key(&Chain::BitcoinSv, &TEST_PRIVATE_KEY).unwrap(),
                },
                decode_password(TEST_PASSWORD),
            )
            .unwrap();
        let wallet = Wallet {
            id: WalletId::from_id(&stored.wallet_id).unwrap(),
            wallet_type: stored.wallet_type,
            accounts: stored.accounts.into_iter().map(wallet_rules::account).collect(),
            ..Wallet::mock()
        };
        let mut input = bsv_send_input();
        input.confirm.input.from = wallet.account(Chain::BitcoinSv).unwrap().clone();
        input.wallet = wallet;
        input.confirm.input.transfer.recipient.address = "1BitcoinEaterAddressDontSendf59kuE".into();
        let signer_input: SignerInput = input.signer_input().unwrap().into();
        let fee = plan_transfer(&signer_input.input).unwrap().fee;
        input.network_fee = fee.into();
        input.confirm.fee.fee = fee.into();
        Self {
            _directory: directory,
            keystore,
            input,
        }
    }

    fn kit(&self, provider: Arc<dyn AlienProvider>, store: Arc<MemoryBsvAddressStore>) -> ConfirmTestkit {
        ConfirmTestkit::with_bsv_provider(self.input.wallet.clone(), provider, self.keystore.clone(), store)
    }

    async fn saved(&self, kit: &ConfirmTestkit) -> BsvHomeTransactionCheckpointData {
        let transfer = &self.input.confirm.input.transfer;
        kit.bsv_addresses
            .saved_for_transfer(&self.input.wallet, &transfer.recipient.address, &transfer.value.to_string(), transfer.use_max_amount)
            .await
            .unwrap()
            .unwrap()
    }
}

struct ReservationCheckingSigner {
    calls: AtomicUsize,
    store: Arc<MemoryBsvAddressStore>,
    fail_signed_save: bool,
    cancel_after_sign: Option<Arc<AtomicU64>>,
}

impl ReservationCheckingSigner {
    fn new(store: Arc<MemoryBsvAddressStore>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            store,
            fail_signed_save: false,
            cancel_after_sign: None,
        }
    }
}

#[async_trait]
impl BsvHomeTransactionSigner for ReservationCheckingSigner {
    async fn sign(&self, wallet: Wallet, input: BsvHomeSignerInput) -> Result<Vec<BsvHomeSignedTransaction>, BsvHomeCoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let record = self.store.get(wallet.id).await.unwrap().unwrap();
        let json: serde_json::Value = serde_json::from_str(&record.value).unwrap();
        assert_eq!(json["kind"], "singleAddress");
        assert_eq!(json["reservations"].as_array().unwrap().len(), 1);
        assert!(json["reservations"][0]["checkpoint"].is_null());
        let input: SignerInput = input.into();
        assert!(matches!(input.metadata, TransactionLoadMetadata::Bitcoin { .. }));
        let data = BsvChainSigner
            .sign_transfer(&input, &TEST_PRIVATE_KEY)
            .map_err(|error| BsvHomeCoreError::AnyError { msg: error.to_string() })?;
        if self.fail_signed_save {
            *self.store.save_error.lock().unwrap() = Some(BsvHomeServiceError::Store {
                msg: "Single signed checkpoint storage failed".into(),
            });
        }
        if let Some(revision) = &self.cancel_after_sign {
            revision.fetch_add(1, Ordering::SeqCst);
        }
        Ok(vec![BsvHomeSignedTransaction {
            data,
            transaction_type: TransactionType::Transfer,
        }])
    }
}

struct DurableBroadcastProvider {
    inner: RecordingBroadcastProvider,
    store: Arc<MemoryBsvAddressStore>,
    wallet_id: WalletId,
}

impl std::fmt::Debug for DurableBroadcastProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("DurableBroadcastProvider").finish()
    }
}

#[async_trait]
impl AlienProvider for DurableBroadcastProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        if target.url.ends_with("/tx/raw") {
            let record = self.store.get(self.wallet_id.clone()).await.unwrap().unwrap();
            let json: serde_json::Value = serde_json::from_str(&record.value).unwrap();
            let checkpoint = &json["reservations"][0]["checkpoint"];
            assert_eq!(checkpoint["version"], 3);
            let body: serde_json::Value = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
            assert_eq!(body["txhex"], checkpoint["signed_data"]);
        }
        self.inner.request(target).await
    }
}

fn broadcasts(provider: &RecordingBroadcastProvider) -> Vec<Option<Vec<u8>>> {
    provider
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| request.url.ends_with("/tx/raw"))
        .map(|request| request.body.clone())
        .collect()
}

#[test]
fn test_single_actual_confirm_writer_saves_before_broadcast_and_reopens_the_same_raw() {
    block_on(async {
        let context = SingleSendContext::new();
        let store = Arc::new(MemoryBsvAddressStore::default());
        let provider = Arc::new(DurableBroadcastProvider {
            inner: RecordingBroadcastProvider::default(),
            store: store.clone(),
            wallet_id: context.input.wallet.id.clone(),
        });
        let signer = Arc::new(ReservationCheckingSigner::new(store.clone()));
        let kit = context.kit(provider.clone(), store.clone());
        assert!(kit.confirm.execute(context.input.clone(), signer.clone()).await.is_err());
        assert_eq!(broadcasts(&provider.inner).len(), 1);
        let saved = context.saved(&kit).await;
        assert_eq!(saved.version, 3);
        assert_eq!(saved.original_value, Some(context.input.confirm.input.transfer.value.clone()));
        assert!(saved.bsv_spend.as_ref().unwrap().change.is_none());
        saved.validate_transfer(&context.input.wallet, &context.input.confirm.input.transfer).unwrap();
        let transfer = &context.input.confirm.input.transfer;
        let pending = kit
            .bsv_addresses
            .pending_input(&context.input.wallet, &transfer.recipient.address, &transfer.value.to_string(), false)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(pending.metadata, TransactionLoadMetadata::Bitcoin { .. }));
        assert_eq!(pending.fee.fee, saved.network_fee.clone().into());
        drop(kit);

        let restarted = context.kit(provider.clone(), store.clone());
        assert!(restarted.confirm.execute(context.input.clone(), signer.clone()).await.is_err());
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        let attempts = broadcasts(&provider.inner);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0], attempts[1]);
        assert_eq!(context.saved(&restarted).await, saved);

        let mut changed = context.input.clone();
        changed.confirm.fee.gas_price_type = primitives::GasPriceType::regular(101);
        assert!(restarted.confirm.execute(changed, signer.clone()).await.is_err());
        assert_eq!(broadcasts(&provider.inner).len(), 2);
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);

        let original = store.get(context.input.wallet.id.clone()).await.unwrap().unwrap();
        let mut corrupt: serde_json::Value = serde_json::from_str(&original.value).unwrap();
        corrupt["reservations"][0]["checkpoint"]["signed_data"] = "00".into();
        let corrupt_value = serde_json::to_string(&corrupt).unwrap();
        store.states.lock().unwrap().get_mut(&context.input.wallet.id).unwrap().value = corrupt_value.clone();
        assert!(restarted.confirm.execute(context.input.clone(), signer.clone()).await.is_err());
        assert_eq!(store.get(context.input.wallet.id.clone()).await.unwrap().unwrap().value, corrupt_value);
        assert_eq!(broadcasts(&provider.inner).len(), 2);
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn test_single_actual_confirm_writer_stops_on_durable_signed_save_failure() {
    block_on(async {
        let context = SingleSendContext::new();
        let store = Arc::new(MemoryBsvAddressStore::default());
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let kit = context.kit(provider.clone(), store.clone());
        let signer = Arc::new(ReservationCheckingSigner {
            fail_signed_save: true,
            ..ReservationCheckingSigner::new(store.clone())
        });
        let result = kit.confirm.execute(context.input.clone(), signer.clone()).await;
        assert_eq!(result.unwrap_err().to_string(), "Single signed checkpoint storage failed");
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert!(broadcasts(&provider).is_empty());
        *store.save_error.lock().unwrap() = None;
        let transfer = &context.input.confirm.input.transfer;
        assert!(
            kit.bsv_addresses
                .saved_for_transfer(&context.input.wallet, &transfer.recipient.address, &transfer.value.to_string(), false)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            kit.bsv_addresses
                .pending_input(&context.input.wallet, &transfer.recipient.address, &transfer.value.to_string(), false)
                .await
                .unwrap()
                .is_some()
        );
    });
}

#[test]
fn test_single_authoritative_raw_survives_secondary_checkpoint_failure_without_resigning() {
    block_on(async {
        let context = SingleSendContext::new();
        let store = Arc::new(MemoryBsvAddressStore::default());
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let kit = context.kit(provider.clone(), store.clone());
        let signer = Arc::new(ReservationCheckingSigner::new(store));
        let failed = Arc::new(MemoryTransactionCheckpoint {
            fail_writes: true,
            ..Default::default()
        });
        assert_eq!(
            kit.confirm
                .execute_with_checkpoint(context.input.clone(), signer.clone(), Some(failed))
                .await
                .unwrap_err()
                .to_string(),
            "checkpoint storage failed"
        );
        assert!(broadcasts(&provider).is_empty());
        let saved = context.saved(&kit).await;
        let repaired = Arc::new(MemoryTransactionCheckpoint::default());
        assert!(
            kit.confirm
                .execute_with_checkpoint(context.input.clone(), signer.clone(), Some(repaired.clone()))
                .await
                .is_err()
        );
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert_eq!(repaired.load().await.unwrap(), Some(saved));
        assert_eq!(repaired.authorizations.load(Ordering::SeqCst), 1);
        assert_eq!(broadcasts(&provider).len(), 1);
    });
}

#[test]
fn test_single_actual_confirm_writer_cancel_after_sdk_sign_releases_only_unsigned_and_does_not_broadcast() {
    block_on(async {
        let context = SingleSendContext::new();
        let store = Arc::new(MemoryBsvAddressStore::default());
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let kit = context.kit(provider.clone(), store.clone());
        let revision = Arc::new(AtomicU64::new(0));
        let signer = Arc::new(ReservationCheckingSigner {
            cancel_after_sign: Some(revision.clone()),
            ..ReservationCheckingSigner::new(store.clone())
        });
        let scope = Arc::new(ExecutionScope {
            revision,
            generation: 0,
            wallet: context.input.wallet.clone(),
            chain: Chain::BitcoinSv,
            session: Some(kit.wallet_session.clone()),
            cold_wallet_store: kit.cold_store.clone(),
        });
        assert!(matches!(
            kit.confirm.execute_scoped(context.input.clone(), signer.clone(), None, Some(scope)).await,
            Err(BsvHomeConfirmError::Cancelled)
        ));
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        assert!(broadcasts(&provider).is_empty());
        let stored = store.get(context.input.wallet.id.clone()).await.unwrap().unwrap();
        let json: serde_json::Value = serde_json::from_str(&stored.value).unwrap();
        assert!(json["reservations"].as_array().unwrap().is_empty());
    });
}

#[test]
fn test_single_external_v3_checkpoint_cannot_bypass_the_authoritative_payer_store() {
    block_on(async {
        let context = SingleSendContext::new();
        let store = Arc::new(MemoryBsvAddressStore::default());
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let kit = context.kit(provider.clone(), store);
        let input: SignerInput = context.input.signer_input().unwrap().into();
        let transaction = BsvHomeSignedTransaction {
            data: BsvChainSigner.sign_transfer(&input, &TEST_PRIVATE_KEY).unwrap(),
            transaction_type: TransactionType::Transfer,
        };
        let saved = BsvHomeTransactionCheckpointData::from_signed(&context.input, &transaction).unwrap();
        assert_eq!(saved.version, 3);
        let checkpoint = Arc::new(MemoryTransactionCheckpoint {
            saved: std::sync::Mutex::new(Some(saved)),
            ..Default::default()
        });
        let signer = Arc::new(ReservationCheckingSigner::new(kit.wallets.bsv_addresses.clone()));
        let error = kit
            .confirm
            .execute_with_checkpoint(context.input.clone(), signer.clone(), Some(checkpoint.clone()))
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), "BSV external payment requires validated payer-ledger recovery before broadcast");
        assert_eq!(signer.calls.load(Ordering::SeqCst), 0);
        assert!(broadcasts(&provider).is_empty());
        assert!(checkpoint.load().await.unwrap().is_some());
    });
}

#[test]
fn test_single_max_actual_confirm_freezes_original_intent_and_replays_its_exact_legacy_plan() {
    block_on(async {
        let mut context = SingleSendContext::new();
        context.input.confirm.input.transfer.use_max_amount = true;
        context.input.confirm.input.transfer.value = 50_000.into();
        context.input.value = 50_000.into();
        let candidate: SignerInput = context.input.signer_input().unwrap().into();
        let plan = plan_transfer(&candidate.input).unwrap();
        context.input.value = plan.outputs[0].value.to_sat().into();
        context.input.network_fee = plan.fee.into();
        context.input.confirm.fee.fee = plan.fee.into();
        let store = Arc::new(MemoryBsvAddressStore::default());
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let kit = context.kit(provider.clone(), store.clone());
        let signer = Arc::new(ReservationCheckingSigner::new(store.clone()));
        assert!(kit.confirm.execute(context.input.clone(), signer.clone()).await.is_err());
        let saved = context.saved(&kit).await;
        assert_eq!(saved.version, 3);
        assert_eq!(saved.original_value, Some(50_000.into()));
        assert!(saved.use_max_amount);
        assert_eq!(saved.value, context.input.value);
        saved.validate_confirmation(&context.input).unwrap();
        drop(kit);
        let restarted = context.kit(provider.clone(), store);
        assert!(restarted.confirm.execute(context.input.clone(), signer.clone()).await.is_err());
        assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        let attempts = broadcasts(&provider);
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0], attempts[1]);
    });
}

#[test]
fn test_single_unsupported_payment_references_stop_at_the_owner_before_signing() {
    block_on(async {
        let mut context = SingleSendContext::new();
        context.input.confirm.input.transfer.recipient.references = vec!["unsupported-reference".into()];
        let store = Arc::new(MemoryBsvAddressStore::default());
        let provider = Arc::new(RecordingBroadcastProvider::default());
        let kit = context.kit(provider.clone(), store.clone());
        let signer = Arc::new(ReservationCheckingSigner::new(store.clone()));
        let error = kit.confirm.execute(context.input.clone(), signer.clone()).await.unwrap_err();
        assert_eq!(error.to_string(), "BSV checkpoint requires a positive native transfer without memo");
        assert_eq!(signer.calls.load(Ordering::SeqCst), 0);
        assert!(store.states.lock().unwrap().is_empty());
        assert!(broadcasts(&provider).is_empty());
    });
}
