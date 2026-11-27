use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bsv_home_bsv::cold_wallet::{Pairing, decode_json};
use bsv_home_derivation::derive_wallet_id_from_account;
use futures::{channel::oneshot, executor::block_on, join};
use primitives::{Chain, Wallet, WalletId, WalletSource, WalletType, testkit::ABANDON_PHRASE};

use super::state::State;
use super::testkit::{Context, MemoryColdWalletStore, sign_action};
use super::{
    BsvHomeBsvColdWalletRole, BsvHomeBsvColdWalletService, BsvHomeBsvColdWalletStateStamp, BsvHomeBsvColdWalletStore, BsvHomeBsvColdWalletStoredState,
    bsv_cold_wallet_require_hot_signing, bsv_cold_wallet_role,
};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::keystore::{decode_password, keystore_id_for_wallet};
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::testkit::MemoryPreferencesStore;
use crate::services::wallet::addresses::BsvHomeBsvAddressStoredState;
use crate::services::wallet::model::BsvHomeWalletSecretKind;
use crate::services::wallet::testkit::{MemoryWalletStore, TEST_PASSWORD};
use crate::services::wallet::{BsvHomeWalletImportResult, BsvHomeWalletImportType, BsvHomeWalletStore};

#[derive(Clone, Copy)]
enum ImportKind {
    Create,
    Recovery,
}

impl ImportKind {
    fn account_path(self) -> Option<String> {
        match self {
            Self::Create => None,
            Self::Recovery => Some("m/44'/236'/1'".into()),
        }
    }
}

fn unified(context: &Context, store: Arc<dyn BsvHomeBsvColdWalletStore>) -> BsvHomeBsvColdWalletService {
    BsvHomeBsvColdWalletService::unified(
        context.wallets.clone(),
        context.addresses.clone(),
        Arc::new(BsvHomeGateway::new(
            context.provider.clone(),
            Arc::new(MemoryPreferencesStore::default()),
            Arc::new(EmptyPreferences),
        )),
        context.keystore.clone(),
        store,
        context.signer.clone(),
    )
}

fn preview(context: &Context, kind: ImportKind) -> String {
    context.keystore.bsv_hd_preview(ABANDON_PHRASE.into(), kind.account_path(), None, None, false).unwrap()
}

async fn import(context: &Context, service: &BsvHomeBsvColdWalletService, kind: ImportKind) -> Result<super::BsvHomeBsvColdWalletSnapshot, BsvHomeServiceError> {
    let generation = service.capture_generation();
    match kind {
        ImportKind::Create => {
            service
                .create("Cold fixture".into(), ABANDON_PHRASE.split_whitespace().map(String::from).collect(), generation)
                .await
        }
        ImportKind::Recovery => {
            service
                .import_recovery(
                    "Recovered fixture".into(),
                    ABANDON_PHRASE.into(),
                    kind.account_path(),
                    None,
                    preview(context, kind),
                    None,
                    generation,
                )
                .await
        }
    }
}

async fn observer(context: &Context, address: &str) -> Wallet {
    match context
        .wallets
        .import_wallet(
            "Current fixture".into(),
            BsvHomeWalletImportType::Address {
                address: address.into(),
                chain: Chain::BitcoinSv,
            },
            WalletSource::Import,
        )
        .await
        .unwrap()
    {
        BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
    }
}

#[test]
fn test_unified_import_cancel_after_real_wallet_commit_keeps_encrypted_secret_cold_and_does_not_claim_current() {
    block_on(async {
        for kind in [ImportKind::Create, ImportKind::Recovery] {
            for switch_wallet in [false, true] {
                let gate = Arc::new(CommitGate::default());
                let writes = Arc::new(AtomicUsize::new(0));
                let context = Context::with_wallet_store({
                    let gate = gate.clone();
                    let writes = writes.clone();
                    move |inner| Arc::new(PersistingWalletStore { inner, gate, writes })
                });
                let previous = observer(&context, "1BoatSLRHtKNngkdXEeobR76b53LETtpyT").await;
                let other = observer(&context, "1BitcoinEaterAddressDontSendf59kuE").await;
                context.wallets.set_current_wallet_id(previous.id.clone()).unwrap();
                let baseline = writes.load(Ordering::SeqCst);
                let pairing: Pairing = decode_json(&preview(&context, kind)).unwrap();
                let expected_id = derive_wallet_id_from_account(&pairing.account().unwrap(), WalletType::Single).unwrap();
                let service = unified(&context, context.store.clone());
                let (entered, release) = gate.arm();
                let importing = import(&context, &service, kind);
                let cancel = async {
                    let committed_id = entered.await.unwrap();
                    assert_eq!(committed_id, expected_id);
                    let actual = context.wallet_store.get_wallet(committed_id.clone()).await.unwrap().unwrap();
                    let key_id = keystore_id_for_wallet(committed_id.id());
                    assert!(context.keystore.opens_with(key_id.clone(), decode_password(TEST_PASSWORD)));
                    assert!(!context.keystore.opens_with(key_id, vec![0; 32]));
                    assert_eq!(writes.load(Ordering::SeqCst), baseline + 1);
                    if switch_wallet {
                        context.wallets.set_current_wallet_id(other.id.clone()).unwrap();
                    } else {
                        service.invalidate_actions().unwrap();
                    }
                    release.send(()).unwrap();
                    actual
                };
                let (result, actual) = join!(importing, cancel);
                assert!(result.is_err());
                assert_eq!(bsv_cold_wallet_role(actual.clone(), context.store.clone()).unwrap(), BsvHomeBsvColdWalletRole::Signer);
                assert!(bsv_cold_wallet_require_hot_signing(actual.clone(), Chain::BitcoinSv, context.store.clone()).is_err());
                assert_eq!(context.wallets.current_wallet_id().unwrap(), Some(if switch_wallet { other.id } else { previous.id }));
                assert!(!service.consume_wallet_selection(actual.id.clone()).unwrap());
                assert!(context.keystore.opens_with(keystore_id_for_wallet(actual.id.id()), decode_password(TEST_PASSWORD)));
                assert_eq!(
                    context.keystore.wallet_secret_kind(actual).unwrap(),
                    Some(match kind {
                        ImportKind::Create => BsvHomeWalletSecretKind::Phrase,
                        ImportKind::Recovery => BsvHomeWalletSecretKind::Recovery,
                    })
                );
                assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
            }
        }
    });
}

#[test]
fn test_unified_import_bad_cold_storage_never_starts_actual_secret_or_wallet_writer() {
    block_on(async {
        for kind in [ImportKind::Create, ImportKind::Recovery] {
            for failure in [StorageFailure::Read, StorageFailure::Write, StorageFailure::Corrupt, StorageFailure::Conflict] {
                let writes = Arc::new(AtomicUsize::new(0));
                let context = Context::with_wallet_store({
                    let writes = writes.clone();
                    move |inner| {
                        Arc::new(PersistingWalletStore {
                            inner,
                            gate: Arc::new(CommitGate::default()),
                            writes,
                        })
                    }
                });
                let pairing: Pairing = decode_json(&preview(&context, kind)).unwrap();
                let id = derive_wallet_id_from_account(&pairing.account().unwrap(), WalletType::Single).unwrap();
                match failure {
                    StorageFailure::Read => *context.store.read_fail.lock().unwrap() = true,
                    StorageFailure::Write => *context.store.fail.lock().unwrap() = true,
                    StorageFailure::Corrupt => {
                        context
                            .store
                            .records
                            .lock()
                            .unwrap()
                            .insert(id.clone(), BsvHomeBsvColdWalletStoredState { revision: 9, value: "{".into() });
                    }
                    StorageFailure::Conflict => {
                        let other = context
                            .keystore
                            .bsv_hd_preview(ABANDON_PHRASE.into(), Some("m/44'/236'/7'".into()), None, None, false)
                            .unwrap();
                        let state = State::new(other).unwrap();
                        context.store.records.lock().unwrap().insert(
                            id.clone(),
                            BsvHomeBsvColdWalletStoredState {
                                revision: 8,
                                value: serde_json::to_string(&state).unwrap(),
                            },
                        );
                    }
                }
                let before = context.store.records.lock().unwrap().get(&id).map(|record| (record.revision, record.value.clone()));
                let service = unified(&context, context.store.clone());
                assert!(import(&context, &service, kind).await.is_err());
                assert_eq!(writes.load(Ordering::SeqCst), 0);
                assert!(context.wallet_store.get_wallets().await.unwrap().is_empty());
                assert!(!context.keystore.has_stored_wallets().unwrap());
                let after = context.store.records.lock().unwrap().get(&id).map(|record| (record.revision, record.value.clone()));
                assert!(after == before);
                assert_eq!(context.wallets.current_wallet_id().unwrap(), None);
                assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
            }
        }
    });
}

#[test]
fn test_unified_import_preserves_existing_exact_pending_and_signed_state_without_revision_rewrite() {
    block_on(async {
        for kind in [ImportKind::Create, ImportKind::Recovery] {
            for signed in [false, true] {
                let context = Context::new();
                let service = unified(&context, context.store.clone());
                let wallet = import(&context, &service, kind).await.unwrap().wallet;
                let request = context.request(wallet.id.clone(), 75000).await;
                let pending = service.import_request(wallet.id.clone(), request).await.unwrap();
                if signed {
                    sign_action(&service, wallet.id.clone(), pending.review.commitment).await.unwrap();
                }
                let before = context.store.records.lock().unwrap().get(&wallet.id).unwrap().clone();
                let writes = *context.store.writes.lock().unwrap();
                let imported = import(&context, &service, kind).await.unwrap();
                assert_eq!(imported.wallet.id, wallet.id);
                let after = context.store.records.lock().unwrap().get(&wallet.id).unwrap().clone();
                assert_eq!(after.revision, before.revision);
                assert!(after.value.as_bytes() == before.value.as_bytes());
                assert_eq!(*context.store.writes.lock().unwrap(), writes);
                assert!(imported.pending.is_some());
                assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
            }
        }
    });
}

#[test]
fn test_unified_import_cancel_after_public_reservation_retains_only_valid_public_orphan() {
    block_on(async {
        for kind in [ImportKind::Create, ImportKind::Recovery] {
            let context = Context::new();
            let gate = Arc::new(CommitGate::default());
            let store = Arc::new(ReservingColdStore {
                inner: context.store.clone(),
                gate: gate.clone(),
            });
            let service = unified(&context, store);
            let (entered, release) = gate.arm();
            let importing = import(&context, &service, kind);
            let cancel = async {
                let id = entered.await.unwrap();
                service.invalidate_actions().unwrap();
                release.send(()).unwrap();
                id
            };
            let (result, id) = join!(importing, cancel);
            assert_eq!(result.err(), Some(BsvHomeServiceError::Cancelled));
            let record = context.store.records.lock().unwrap().get(&id).unwrap().clone();
            let state: State = serde_json::from_str(&record.value).unwrap();
            let pairing = state.validate().unwrap();
            assert_eq!(derive_wallet_id_from_account(&pairing.account().unwrap(), WalletType::Single).unwrap(), id);
            assert!(state.pending.is_none() && state.next.is_none());
            assert!(context.wallet_store.get_wallets().await.unwrap().is_empty());
            assert!(!context.keystore.has_stored_wallets().unwrap());
            assert_eq!(context.wallets.current_wallet_id().unwrap(), None);
            assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
        }
    });
}

#[derive(Clone, Copy)]
enum StorageFailure {
    Read,
    Write,
    Corrupt,
    Conflict,
}

#[derive(Default)]
struct CommitGate {
    entered: Mutex<Option<oneshot::Sender<WalletId>>>,
    release: Mutex<Option<oneshot::Receiver<()>>>,
}

impl CommitGate {
    fn arm(&self) -> (oneshot::Receiver<WalletId>, oneshot::Sender<()>) {
        let (entered, wait) = oneshot::channel();
        let (release, gate) = oneshot::channel();
        *self.entered.lock().unwrap() = Some(entered);
        *self.release.lock().unwrap() = Some(gate);
        (wait, release)
    }

    async fn persisted(&self, wallet_id: WalletId) {
        let entered = self.entered.lock().unwrap().take();
        if let Some(entered) = entered {
            entered.send(wallet_id).unwrap();
        }
        let wait = self.release.lock().unwrap().take();
        if let Some(wait) = wait {
            wait.await.unwrap();
        }
    }
}

struct PersistingWalletStore {
    inner: Arc<MemoryWalletStore>,
    gate: Arc<CommitGate>,
    writes: Arc<AtomicUsize>,
}

#[async_trait]
impl BsvHomeWalletStore for PersistingWalletStore {
    async fn get_wallets(&self) -> Result<Vec<Wallet>, BsvHomeServiceError> {
        self.inner.get_wallets().await
    }
    async fn get_wallet(&self, wallet_id: WalletId) -> Result<Option<Wallet>, BsvHomeServiceError> {
        self.inner.get_wallet(wallet_id).await
    }
    async fn add_wallet(&self, wallet: Wallet) -> Result<(), BsvHomeServiceError> {
        self.inner.add_wallet(wallet).await
    }
    async fn add_wallet_with_bsv_addresses(&self, wallet: Wallet, addresses: Option<BsvHomeBsvAddressStoredState>) -> Result<(), BsvHomeServiceError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.inner.add_wallet_with_bsv_addresses(wallet.clone(), addresses).await?;
        self.gate.persisted(wallet.id).await;
        Ok(())
    }
    async fn delete_wallet(&self, wallet_id: WalletId) -> Result<bool, BsvHomeServiceError> {
        self.inner.delete_wallet(wallet_id).await
    }
    async fn set_pinned(&self, wallet_id: WalletId, pinned: bool) -> Result<(), BsvHomeServiceError> {
        self.inner.set_pinned(wallet_id, pinned).await
    }
    async fn set_name(&self, wallet_id: WalletId, name: String) -> Result<(), BsvHomeServiceError> {
        self.inner.set_name(wallet_id, name).await
    }
    async fn set_image_url(&self, wallet_id: WalletId, image_url: Option<String>) -> Result<(), BsvHomeServiceError> {
        self.inner.set_image_url(wallet_id, image_url).await
    }
}

struct ReservingColdStore {
    inner: Arc<MemoryColdWalletStore>,
    gate: Arc<CommitGate>,
}

#[async_trait]
impl BsvHomeBsvColdWalletStore for ReservingColdStore {
    fn state_stamp(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvColdWalletStateStamp>, BsvHomeServiceError> {
        self.inner.state_stamp(wallet_id)
    }
    fn has_state(&self, wallet_id: WalletId) -> Result<bool, BsvHomeServiceError> {
        self.inner.has_state(wallet_id)
    }
    async fn get(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvColdWalletStoredState>, BsvHomeServiceError> {
        self.inner.get(wallet_id).await
    }
    async fn compare_and_swap(&self, wallet_id: WalletId, revision: Option<u64>, state: BsvHomeBsvColdWalletStoredState) -> Result<bool, BsvHomeServiceError> {
        let saved = self.inner.compare_and_swap(wallet_id.clone(), revision, state).await?;
        if saved {
            self.gate.persisted(wallet_id).await;
        }
        Ok(saved)
    }
}
