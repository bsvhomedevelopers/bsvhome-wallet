use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use bsv_home_bitcoin::signer::verify_fork_id_plan;
use bsv_home_bsv::BsvAddress;
use bsv_home_bsv::multisig::{Funding, Policy, review_funding};
use bsv_home_bsv::signer::{BsvChainSigner, plan_transfer};
use bsv_home_derivation::{bsv_account_xpub, bsv_private_key};
use futures::channel::oneshot;
use futures::executor::block_on;
use primitives::testkit::ABANDON_PHRASE;
use primitives::{
    Account, Address, Asset, BsvAddressBranch, BsvAddressLocator, BsvSpendChange, BsvSpendInput, BsvSpendMetadata, Chain, ChainSigner, GasPriceType, SignerError, SignerInput,
    TransactionFee, TransactionInputType, TransactionLoadInput, TransactionLoadMetadata, UTXO, Wallet, WalletId, WalletType,
};
use signer::secp256k1_public_key;
use tempfile::TempDir;

use super::ledger::{PayerFundingBinding, PayerReservationPurpose, PayerReservationRequest, PayerState};
use super::state::{PoolState, SpendReservation, can_add_receive_address};
use super::testkit::{BsvAddressProvider, MemoryBsvAddressStore, pool_with_keystore};
use super::{BsvHomeBsvAddressPool, BsvHomeBsvAddressStatus, BsvHomeBsvDiscoveryState, has_hd_account};
use crate::alien::{AlienError, AlienProvider, AlienResponse, AlienTarget};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore, decode_password};
use crate::services::confirm::BsvHomeTransactionCheckpointData;
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::testkit::{MemoryKeystorePassword, MemoryWalletStore, TEST_PASSWORD};
use crate::services::wallet::{BsvHomeWalletStore, rules as wallet_rules};
use crate::testkit::TestAlienProvider;

fn locator(branch: BsvAddressBranch, index: u32) -> BsvAddressLocator {
    BsvAddressLocator { branch, index }
}

fn state() -> PoolState {
    let mut state = PoolState::new(Wallet::mock().id, bsv_account_xpub(ABANDON_PHRASE).unwrap()).unwrap();
    state.receive_count = None;
    state.selected_index = None;
    state
}

fn receive_indexes(state: &PoolState, status: BsvHomeBsvAddressStatus) -> Vec<u32> {
    state
        .records
        .iter()
        .filter(|record| record.locator.branch == BsvAddressBranch::Receive && record.status == status)
        .map(|record| record.locator.index)
        .collect()
}

fn reservation(state: &PoolState, change_index: u32) -> SpendReservation {
    let change_locator = locator(BsvAddressBranch::Change, change_index);
    let address = state.address(change_locator).unwrap();
    SpendReservation {
        id: format!("reservation-{change_index}"),
        purpose: PayerReservationPurpose::Transfer,
        recipient: "recipient".into(),
        original_value: "10000".into(),
        is_max: false,
        value: 10000,
        fee: 100,
        gas_price: GasPriceType::regular(100),
        spend: BsvSpendMetadata {
            inputs: Vec::new(),
            change: Some(BsvSpendChange {
                script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
                address,
                locator: change_locator,
            }),
        },
        checkpoint: None,
        confirmed: false,
    }
}

#[test]
fn test_initial_receive_reserve_and_current_address_use_separate_gap_boundary() {
    let state = state();
    let snapshot = state.snapshot();
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Unused), (0..10).collect::<Vec<_>>());
    assert_eq!(state.records.len(), 10);
    assert_eq!(snapshot.current_address, Some(state.address(locator(BsvAddressBranch::Receive, 0)).unwrap()));
    assert_eq!(
        snapshot.records.iter().filter(|record| record.is_current).map(|record| record.index).collect::<Vec<_>>(),
        vec![0]
    );
    assert!(!snapshot.allocation_paused);
    assert_eq!(snapshot.discovery, BsvHomeBsvDiscoveryState::NotStarted);
    assert_eq!(state.allocation_limit(BsvAddressBranch::Receive), 19);
    assert_eq!(state.next_change().unwrap(), locator(BsvAddressBranch::Change, 0));
}

#[test]
fn test_unconfirmed_receive_usage_rotates_and_remains_used_after_history_rollback() {
    let mut state = state();
    let received = locator(BsvAddressBranch::Receive, 0);
    state.observe(received, true, false).unwrap();
    state.refill().unwrap();
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Unused), (1..11).collect::<Vec<_>>());
    assert_eq!(state.snapshot().current_address, Some(state.address(locator(BsvAddressBranch::Receive, 1)).unwrap()));
    state.observe(received, false, false).unwrap();
    state.refill().unwrap();
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Used), vec![0]);
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Unused), (1..11).collect::<Vec<_>>());
    assert_eq!(state.allocation_limit(BsvAddressBranch::Receive), 19);
}

#[test]
fn test_receive_allocation_pauses_at_gap_and_can_keep_less_than_ten_unused_addresses() {
    let mut state = state();
    for index in 0..20 {
        state.observe(locator(BsvAddressBranch::Receive, index), true, false).unwrap();
        state.refill().unwrap();
    }
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Used), (0..20).collect::<Vec<_>>());
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Unused), Vec::<u32>::new());
    assert_eq!((state.snapshot().current_address, state.snapshot().allocation_paused), (None, true));

    state.observe(locator(BsvAddressBranch::Receive, 7), true, true).unwrap();
    state.refill().unwrap();
    assert_eq!(state.allocation_limit(BsvAddressBranch::Receive), 27);
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Unused), (20..28).collect::<Vec<_>>());
    assert!(state.snapshot().allocation_paused);

    state.observe(locator(BsvAddressBranch::Receive, 19), true, true).unwrap();
    state.refill().unwrap();
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Unused), (20..30).collect::<Vec<_>>());
    assert!(!state.snapshot().allocation_paused);
    state.observe(locator(BsvAddressBranch::Receive, 19), false, false).unwrap();
    state.observe(locator(BsvAddressBranch::Receive, 7), false, false).unwrap();
    state.refill().unwrap();
    assert_eq!(state.allocation_limit(BsvAddressBranch::Receive), 19);
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Used), (0..20).collect::<Vec<_>>());
    assert_eq!(state.records.len(), 30);
}

#[test]
fn test_sparse_discovered_receive_address_remains_owned_and_does_not_skip_lower_unused_addresses() {
    let mut state = state();
    state.observe(locator(BsvAddressBranch::Receive, 30), true, true).unwrap();
    state.refill().unwrap();
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Used), vec![30]);
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Unused), (0..10).collect::<Vec<_>>());
    assert_eq!(state.snapshot().current_address, Some(state.address(locator(BsvAddressBranch::Receive, 0)).unwrap()));
    state.receive.next_index = 51;
    state.receive.empty_count = 20;
    state.receive.stopped = true;
    state.change.next_index = 20;
    state.change.empty_count = 20;
    state.change.stopped = true;
    state.discovery = BsvHomeBsvDiscoveryState::Complete;
    let main = state.address(locator(BsvAddressBranch::Receive, 0)).unwrap();
    state.validate(&state.wallet_id, &main).unwrap();
    assert_eq!((state.snapshot().receive_scanned, state.snapshot().change_scanned), (51, 20));
    assert_eq!(state.allocation_limit(BsvAddressBranch::Receive), 50);
    state.observe(locator(BsvAddressBranch::Receive, 30), false, false).unwrap();
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Used), vec![30]);
}

#[test]
fn test_change_reservation_reuses_unsigned_cancel_but_used_addresses_pause_at_gap_boundary() {
    let mut state = state();
    state.reservations.push(reservation(&state, 0));
    assert_eq!(state.next_change().unwrap(), locator(BsvAddressBranch::Change, 1));
    state.reservations.clear();
    assert_eq!(state.next_change().unwrap(), locator(BsvAddressBranch::Change, 0));
    for index in 0..20 {
        state.observe(locator(BsvAddressBranch::Change, index), true, false).unwrap();
    }
    assert!(state.next_change().is_none());
    state.observe(locator(BsvAddressBranch::Change, 5), true, true).unwrap();
    assert_eq!(state.next_change().unwrap(), locator(BsvAddressBranch::Change, 20));
    state.observe(locator(BsvAddressBranch::Change, 5), false, false).unwrap();
    assert!(state.next_change().is_none());
}

#[test]
fn test_pool_validation_rejects_wrong_wallet_duplicates_and_unsafe_locator() {
    let state = state();
    let main = state.address(locator(BsvAddressBranch::Receive, 0)).unwrap();
    state.validate(&state.wallet_id, &main).unwrap();
    assert!(state.validate(&WalletId::Multicoin("other".into()), &main).is_err());
    assert!(state.validate(&state.wallet_id, &state.address(locator(BsvAddressBranch::Receive, 1)).unwrap()).is_err());
    let mut duplicate = state.clone();
    duplicate.records.push(duplicate.records[0].clone());
    assert!(duplicate.validate(&duplicate.wallet_id, &main).is_err());
    let mut corrupted = state.clone();
    corrupted.records[1].address = main.clone();
    assert!(corrupted.validate(&corrupted.wallet_id, &main).is_err());
    let mut unsafe_locator = state.clone();
    unsafe_locator.records[0].locator.index = 1 << 31;
    assert!(unsafe_locator.validate(&unsafe_locator.wallet_id, &main).is_err());
    assert!(unsafe_locator.add_address(locator(BsvAddressBranch::Change, 1 << 31)).is_err());
    let mut invalid_progress = state.clone();
    invalid_progress.receive.empty_count = 1;
    assert!(invalid_progress.validate(&invalid_progress.wallet_id, &main).is_err());
    let mut spend = BsvSpendMetadata {
        inputs: vec![BsvSpendInput {
            transaction_id: "1".repeat(64),
            vout: 0,
            value: 100,
            address: main.clone(),
            script_pubkey: hex::encode(BsvAddress::from_str(&main).unwrap().script_pubkey().as_bytes()),
            locator: locator(BsvAddressBranch::Receive, 0),
        }],
        change: None,
    };
    state.validate_spend(&spend).unwrap();
    spend.inputs[0].locator.index = 1;
    assert!(state.validate_spend(&spend).is_err());
}

struct Context<P> {
    _directory: TempDir,
    pool: Arc<BsvHomeBsvAddressPool>,
    wallet: Wallet,
    wallets: Arc<MemoryWalletStore>,
    password: Arc<MemoryKeystorePassword>,
    store: Arc<MemoryBsvAddressStore>,
    provider: Arc<P>,
    gateway: Arc<BsvHomeGateway>,
    keystore: Arc<BsvHomeKeystore>,
}

impl<P: AlienProvider + 'static> Context<P> {
    fn new(provider: Arc<P>) -> Self {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let stored = keystore
            .create_store(
                BsvHomeImportType::SinglePhrase {
                    words: ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                    chain: Chain::BitcoinSv,
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
        let wallets = Arc::new(MemoryWalletStore::default());
        wallets.wallets.lock().unwrap().push(wallet.clone());
        let store = wallets.bsv_addresses.clone();
        let password = Arc::new(MemoryKeystorePassword::default());
        let gateway = Arc::new(BsvHomeGateway::new(provider.clone(), Arc::new(EmptyPreferences), Arc::new(EmptyPreferences)));
        let pool = pool_with_keystore(gateway.clone(), wallets.clone(), keystore.clone(), password.clone(), store.clone());
        Self {
            _directory: directory,
            pool,
            wallet,
            wallets,
            password,
            store,
            provider,
            gateway,
            keystore,
        }
    }

    fn rebuilt_pool(&self) -> Arc<BsvHomeBsvAddressPool> {
        pool_with_keystore(self.gateway.clone(), self.wallets.clone(), self.keystore.clone(), self.password.clone(), self.store.clone())
    }

    async fn mark_discovery_complete(&self) {
        let (revision, mut state) = self.pool.required_state(&self.wallet).await.unwrap();
        for cursor in [&mut state.receive, &mut state.change] {
            cursor.next_index = 20;
            cursor.empty_count = 20;
            cursor.stopped = true;
        }
        state.discovery = BsvHomeBsvDiscoveryState::Complete;
        self.pool.store_state(&self.wallet, Some(revision), &state).await.unwrap();
    }
}

#[test]
fn test_prepare_upgrades_existing_mnemonic_once_without_changing_wallet_identity() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        assert_eq!(context.pool.snapshot(context.wallet.id.clone()).await.unwrap(), None);
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        let snapshot = context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        assert_eq!(snapshot.records.len(), 10);
        assert_eq!(snapshot.current_address, Some(context.wallet.accounts[0].address.clone()));
        assert_eq!(context.pool.prepare(context.wallet.id.clone()).await.unwrap(), snapshot);
        assert_eq!(*context.password.create_requests.lock().unwrap(), vec![false]);
        assert_eq!(
            serde_json::to_value(context.wallets.get_wallet(context.wallet.id.clone()).await.unwrap()).unwrap(),
            serde_json::to_value(Some(&context.wallet)).unwrap()
        );
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_funding_preview_uses_the_verified_hd_owner_without_modifying_the_wallet() {
    block_on(async {
        let context = Context::new(Arc::new(BsvAddressProvider::default()));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        context.mark_discovery_complete().await;
        let root = &context.wallet.accounts[0].address;
        let preview = context.pool.preload_payer_spend(context.wallet.id.clone(), root, false).await.unwrap();
        let (_, state) = context.pool.required_state(&context.wallet).await.unwrap();
        assert_eq!(state.account_xpub, bsv_account_xpub(ABANDON_PHRASE).unwrap());
        assert_eq!(preview.change.unwrap().address, state.address(locator(BsvAddressBranch::Change, 0)).unwrap());
        assert_eq!(
            serde_json::to_value(context.wallets.get_wallet(context.wallet.id.clone()).await.unwrap()).unwrap(),
            serde_json::to_value(Some(&context.wallet)).unwrap()
        );
    });
}

#[derive(Debug)]
struct SuspendedAddressProvider {
    inner: BsvAddressProvider,
    requests: AtomicUsize,
    entered: StdMutex<Option<oneshot::Sender<()>>>,
}

#[async_trait]
impl AlienProvider for SuspendedAddressProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        if self.requests.fetch_add(1, Ordering::SeqCst) == 2 {
            self.entered.lock().unwrap().take().unwrap().send(()).unwrap();
            futures::future::pending::<()>().await;
        }
        self.inner.request(target).await
    }
}

#[test]
fn test_cancelled_scan_aborts_rpc_and_queued_scan_preserves_progress_and_can_resume() {
    block_on(async {
        let (entered, waiting) = oneshot::channel();
        let provider = Arc::new(SuspendedAddressProvider {
            inner: BsvAddressProvider::default(),
            requests: AtomicUsize::new(0),
            entered: StdMutex::new(Some(entered)),
        });
        let context = Context::new(provider.clone());
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let generation = context.pool.scan_generation(context.wallet.id.clone()).unwrap();
        let first = context.pool.sync_at_generation(context.wallet.id.clone(), generation, false);
        let queued = context.pool.sync_at_generation(context.wallet.id.clone(), generation, true);
        let cancel = async {
            waiting.await.unwrap();
            assert_eq!(context.pool.active_scans.lock().unwrap().handles.len(), 2);
            context.pool.cancel_scan(Wallet::mock().id).unwrap();
            assert_eq!(context.pool.scan_generation(context.wallet.id.clone()).unwrap(), generation);
            context.pool.cancel_scan(context.wallet.id.clone()).unwrap();
        };
        let (first, queued, ()) = futures::join!(first, queued, cancel);
        assert_eq!(first.unwrap_err(), BsvHomeServiceError::Cancelled);
        assert_eq!(queued.unwrap_err(), BsvHomeServiceError::Cancelled);
        assert!(context.pool.active_scans.lock().unwrap().handles.is_empty());
        assert_eq!(provider.requests.load(Ordering::SeqCst), 3);
        let progress = context.pool.snapshot(context.wallet.id.clone()).await.unwrap().unwrap();
        assert_eq!(progress.receive_scanned, 1);
        assert_eq!(progress.change_scanned, 0);
        assert_eq!(progress.discovery, BsvHomeBsvDiscoveryState::Incomplete);
        assert_eq!(progress.records.len(), 10);
        let resumed = context.pool.sync(context.wallet.id.clone()).await.unwrap();
        assert_eq!(resumed.discovery, BsvHomeBsvDiscoveryState::Complete);
        assert_eq!(resumed.receive_scanned, 20);
        assert_eq!(resumed.change_scanned, 20);
        assert_eq!(resumed.current_address, progress.current_address);
    });
}

#[test]
fn test_cancel_before_first_poll_rejects_old_generation_without_auth_or_rpc() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        let generation = context.pool.scan_generation(context.wallet.id.clone()).unwrap();
        let pending = context.pool.sync_at_generation(context.wallet.id.clone(), generation, false);
        context.pool.cancel_scan(context.wallet.id.clone()).unwrap();
        assert_eq!(pending.await.unwrap_err(), BsvHomeServiceError::Cancelled);
        assert!(context.password.create_requests.lock().unwrap().is_empty());
        assert_eq!(context.pool.snapshot(context.wallet.id.clone()).await.unwrap(), None);
        assert_eq!(context.pool.scan_generation(context.wallet.id.clone()).unwrap(), generation + 1);
        assert_eq!(context.pool.prepare(context.wallet.id.clone()).await.unwrap().records.len(), 10);
    });
}

#[test]
fn test_prepare_cancellation_and_storage_error_do_not_create_an_empty_pool() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        *context.password.password_error.lock().unwrap() = Some(BsvHomeServiceError::Cancelled);
        assert_eq!(context.pool.prepare(context.wallet.id.clone()).await.unwrap_err(), BsvHomeServiceError::Cancelled);
        assert_eq!(context.pool.snapshot(context.wallet.id.clone()).await.unwrap(), None);
        *context.password.password_error.lock().unwrap() = None;
        let failure = BsvHomeServiceError::Store {
            msg: "address storage failed".into(),
        };
        *context.store.save_error.lock().unwrap() = Some(failure.clone());
        assert_eq!(context.pool.prepare(context.wallet.id.clone()).await.unwrap_err(), failure);
        assert_eq!(context.pool.snapshot(context.wallet.id.clone()).await.unwrap(), None);
        assert_eq!(
            serde_json::to_value(context.wallets.get_wallet(context.wallet.id.clone()).await.unwrap()).unwrap(),
            serde_json::to_value(Some(&context.wallet)).unwrap()
        );
    });
}

#[test]
fn test_private_key_and_view_wallets_keep_one_address_without_authentication() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        let main = context.wallet.accounts[0].address.clone();
        for wallet_type in [WalletType::PrivateKey, WalletType::View] {
            let id = match wallet_type {
                WalletType::PrivateKey => WalletId::PrivateKey(Chain::BitcoinSv, main.clone()),
                WalletType::View => WalletId::View(Chain::BitcoinSv, main.clone()),
                WalletType::Multicoin | WalletType::Single => unreachable!(),
            };
            let wallet = Wallet {
                id,
                wallet_type,
                ..context.wallet.clone()
            };
            context.wallets.add_wallet(wallet.clone()).await.unwrap();
            let snapshot = context.pool.prepare(wallet.id.clone()).await.unwrap();
            assert_eq!(snapshot.records.len(), 1);
            assert_eq!(snapshot.current_address, Some(main.clone()));
            assert_eq!(snapshot.discovery, BsvHomeBsvDiscoveryState::Complete);
            assert_eq!((snapshot.receive_count, snapshot.selected_index, snapshot.can_add), (1, Some(0), false));
            assert_eq!(context.pool.receipt_addresses(wallet.id.clone()).await.unwrap(), Some(vec![main.clone()]));
            assert_eq!(context.pool.select_address(wallet.id.clone(), 0).await.unwrap(), snapshot);
            assert_eq!(context.pool.select_address(wallet.id.clone(), 1).await.is_err(), true);
            assert_eq!(context.pool.add_address(wallet.id.clone()).await.is_err(), true);
            assert_eq!(context.pool.owned_addresses(wallet.id).await.unwrap(), vec![main.clone()]);
        }
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        assert_eq!(context.store.states.lock().unwrap().len(), 0);
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_ordinary_receive_selection_addition_and_restart_preserve_wallet_identity() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        assert_eq!(context.pool.receipt_addresses(context.wallet.id.clone()).await.unwrap(), None);
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        let initial = context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        assert_eq!((initial.receive_count, initial.selected_index, initial.can_add), (10, Some(0), true));
        for index in 10..20 {
            let added = context.pool.add_address(context.wallet.id.clone()).await.unwrap();
            assert_eq!((added.receive_count, added.selected_index, added.can_add), (index + 1, Some(index), index < 19));
            assert_eq!(
                added.records.iter().filter(|record| record.is_current).map(|record| record.index).collect::<Vec<_>>(),
                vec![index]
            );
        }
        let before = context.store.states.lock().unwrap().get(&context.wallet.id).unwrap().clone();
        assert_eq!(context.pool.add_address(context.wallet.id.clone()).await.is_err(), true);
        assert_eq!(context.pool.select_address(context.wallet.id.clone(), 20).await.is_err(), true);
        assert_eq!(context.store.states.lock().unwrap().get(&context.wallet.id).unwrap(), &before);
        let selected = context.pool.select_address(context.wallet.id.clone(), 3).await.unwrap();
        assert_eq!(selected.selected_index, Some(3));
        assert_eq!(
            selected.current_address,
            selected.records.iter().find(|record| record.is_current).map(|record| record.address.clone())
        );
        assert_eq!(context.rebuilt_pool().snapshot(context.wallet.id.clone()).await.unwrap(), Some(selected));
        assert_eq!(
            serde_json::to_value(context.wallets.get_wallet(context.wallet.id.clone()).await.unwrap()).unwrap(),
            serde_json::to_value(Some(&context.wallet)).unwrap()
        );
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_manual_receive_cap_keeps_the_last_add_and_rejects_further_issuance() {
    for (count, used, allowed) in [
        (9979, 9960, true),
        (9979, 9959, false),
        (9980, 9960, false),
        (9980, 9980, false),
        (10001, 10001, false),
        (20, 0, false),
        (20, 1, true),
        (21, 1, false),
    ] {
        assert_eq!(can_add_receive_address(count, used), allowed, "issued={count}, used={used}");
    }
}

#[test]
fn test_legacy_high_receive_records_remain_owned_and_visible_beyond_the_manual_cap() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let (revision, mut state) = context.pool.required_state(&context.wallet).await.unwrap();
        state.receive_count = None;
        state.selected_index = None;
        state.observe(locator(BsvAddressBranch::Receive, 10000), true, false).unwrap();
        state.validate(&context.wallet.id, &context.wallet.accounts[0].address).unwrap();
        context.pool.store_state(&context.wallet, Some(revision), &state).await.unwrap();
        let before = context.store.states.lock().unwrap().get(&context.wallet.id).unwrap().clone();
        let snapshot = context.rebuilt_pool().snapshot(context.wallet.id.clone()).await.unwrap().unwrap();
        assert_eq!((snapshot.receive_count, snapshot.selected_index, snapshot.can_add), (10001, Some(0), false));
        assert_eq!(snapshot.records.len(), 11);
        assert_eq!(snapshot.records.last().unwrap().index, 10000);
        assert_eq!(snapshot.current_address, Some(context.wallet.accounts[0].address.clone()));
        assert_eq!(context.pool.owned_addresses(context.wallet.id.clone()).await.unwrap().len(), 11);
        assert_eq!(context.pool.receipt_addresses(context.wallet.id.clone()).await.unwrap().unwrap().len(), 11);
        assert_eq!(context.pool.add_address(context.wallet.id.clone()).await.is_err(), true);
        assert_eq!(context.store.states.lock().unwrap().get(&context.wallet.id).unwrap(), &before);
        assert_eq!(
            serde_json::to_value(context.wallets.get_wallet(context.wallet.id.clone()).await.unwrap()).unwrap(),
            serde_json::to_value(Some(&context.wallet)).unwrap()
        );
    });
}

#[test]
fn test_manual_receive_issuance_uses_any_history_without_relaxing_automatic_or_change_limits() {
    let mut state = PoolState::new(Wallet::mock().id, bsv_account_xpub(ABANDON_PHRASE).unwrap()).unwrap();
    state.issue_receive_addresses(20).unwrap();
    assert_eq!(state.snapshot().can_add, false);
    state.observe(locator(BsvAddressBranch::Receive, 0), true, false).unwrap();
    assert_eq!(state.snapshot().can_add, true);
    assert_eq!(state.allocation_limit(BsvAddressBranch::Receive), 19);
    for index in 0..20 {
        state.observe(locator(BsvAddressBranch::Change, index), true, false).unwrap();
    }
    assert_eq!(state.allocation_limit(BsvAddressBranch::Change), 19);
    assert_eq!(state.next_change(), None);
    state.observe(locator(BsvAddressBranch::Receive, 0), false, false).unwrap();
    assert_eq!(state.snapshot().can_add, true);
    state.issue_receive_addresses(21).unwrap();
    assert_eq!(
        (state.snapshot().receive_count, state.snapshot().selected_index, state.snapshot().can_add),
        (21, Some(0), false)
    );
    assert_eq!(receive_indexes(&state, BsvHomeBsvAddressStatus::Used), vec![0]);
    assert_eq!(state.allocation_limit(BsvAddressBranch::Receive), 19);
}

#[test]
fn test_new_receive_selection_stays_fixed_while_legacy_records_rotate_until_explicit_selection() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let (revision, mut stored) = context.pool.required_state(&context.wallet).await.unwrap();
        stored.observe(locator(BsvAddressBranch::Receive, 0), true, false).unwrap();
        stored.refill().unwrap();
        assert_eq!(stored.snapshot().selected_index, Some(0));
        context.pool.store_state(&context.wallet, Some(revision), &stored).await.unwrap();
        let receipt_addresses = context.pool.receipt_addresses(context.wallet.id.clone()).await.unwrap().unwrap();
        assert_eq!(receipt_addresses.len(), 10);
        assert_eq!(stored.records.len(), 11);

        let mut legacy = serde_json::to_value(&stored).unwrap();
        legacy.as_object_mut().unwrap().remove("receive_count");
        legacy.as_object_mut().unwrap().remove("selected_index");
        let legacy: PoolState = serde_json::from_value(legacy).unwrap();
        let revision = context.store.states.lock().unwrap().get(&context.wallet.id).unwrap().revision;
        context.pool.store_state(&context.wallet, Some(revision), &legacy).await.unwrap();
        let before = context.store.states.lock().unwrap().get(&context.wallet.id).unwrap().clone();
        let snapshot = context.pool.snapshot(context.wallet.id.clone()).await.unwrap().unwrap();
        assert_eq!((snapshot.receive_count, snapshot.selected_index), (10, Some(1)));
        assert_eq!(context.store.states.lock().unwrap().get(&context.wallet.id).unwrap(), &before);
        let selected = context.pool.select_address(context.wallet.id.clone(), 0).await.unwrap();
        assert_eq!(selected.selected_index, Some(0));
        assert_eq!(context.rebuilt_pool().snapshot(context.wallet.id.clone()).await.unwrap(), Some(selected));
        let (_, stored) = context.pool.required_state(&context.wallet).await.unwrap();
        assert_eq!((stored.receive_count, stored.selected_index), (Some(10), Some(0)));
    });
}

#[test]
fn test_unconfirmed_receive_history_allows_twenty_first_address_after_rollback_and_restart() {
    block_on(async {
        let context = Context::new(Arc::new(BsvAddressProvider::default()));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        for _ in 10..20 {
            context.pool.add_address(context.wallet.id.clone()).await.unwrap();
        }
        context.pool.select_address(context.wallet.id.clone(), 3).await.unwrap();
        context.mark_discovery_complete().await;
        let root = &context.wallet.accounts[0].address;
        context.provider.add_output(root, &"1".repeat(64), 5000, false);
        let observed = context.pool.sync(context.wallet.id.clone()).await.unwrap();
        assert_eq!((observed.receive_count, observed.selected_index, observed.can_add), (20, Some(3), true));
        context.provider.remove_output(root, &"1".repeat(64));
        context.provider.remove_history(root, &"1".repeat(64));
        let rollback = context.pool.sync(context.wallet.id.clone()).await.unwrap();
        assert_eq!((rollback.receive_count, rollback.selected_index, rollback.can_add), (20, Some(3), true));
        assert_eq!(rollback.current_address, observed.current_address);
        let added = context.rebuilt_pool().add_address(context.wallet.id.clone()).await.unwrap();
        assert_eq!((added.receive_count, added.selected_index, added.can_add), (21, Some(20), false));
        assert_eq!(context.rebuilt_pool().snapshot(context.wallet.id.clone()).await.unwrap(), Some(added));
        assert_eq!(
            serde_json::to_value(context.wallets.get_wallet(context.wallet.id.clone()).await.unwrap()).unwrap(),
            serde_json::to_value(Some(&context.wallet)).unwrap()
        );
        assert_eq!(*context.password.create_requests.lock().unwrap(), vec![false]);
    });
}

#[test]
fn test_legacy_auto_receive_history_allows_first_manual_add_without_a_prior_selection() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let (revision, mut state) = context.pool.required_state(&context.wallet).await.unwrap();
        state.issue_receive_addresses(20).unwrap();
        state.receive_count = None;
        state.selected_index = None;
        for index in 0..19 {
            state.observe(locator(BsvAddressBranch::Receive, index), true, false).unwrap();
        }
        state.refill().unwrap();
        context.pool.store_state(&context.wallet, Some(revision), &state).await.unwrap();
        let before = context.store.states.lock().unwrap().get(&context.wallet.id).unwrap().clone();
        let snapshot = context.pool.snapshot(context.wallet.id.clone()).await.unwrap().unwrap();
        assert_eq!((snapshot.receive_count, snapshot.selected_index, snapshot.can_add), (20, Some(19), true));
        assert_eq!(context.store.states.lock().unwrap().get(&context.wallet.id).unwrap(), &before);
        assert_eq!(
            (state.receive_count, state.selected_index, state.allocation_limit(BsvAddressBranch::Receive)),
            (None, None, 19)
        );
        let added = context.pool.add_address(context.wallet.id.clone()).await.unwrap();
        assert_eq!((added.receive_count, added.selected_index, added.can_add), (21, Some(20), true));
        assert_eq!(context.rebuilt_pool().snapshot(context.wallet.id.clone()).await.unwrap(), Some(added));
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
        assert_eq!(
            serde_json::to_value(context.wallets.get_wallet(context.wallet.id.clone()).await.unwrap()).unwrap(),
            serde_json::to_value(Some(&context.wallet)).unwrap()
        );
    });
}

#[test]
fn test_receive_history_errors_cannot_expand_the_manual_gap_or_change_selection() {
    block_on(async {
        let context = Context::new(Arc::new(BsvAddressProvider::default()));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        for _ in 10..20 {
            context.pool.add_address(context.wallet.id.clone()).await.unwrap();
        }
        context.pool.select_address(context.wallet.id.clone(), 3).await.unwrap();
        context.mark_discovery_complete().await;
        context.provider.add_output(&context.wallet.accounts[0].address, &"1".repeat(64), 5000, false);
        *context.provider.history_error.lock().unwrap() = Some("history unavailable");
        let before = context.store.states.lock().unwrap().get(&context.wallet.id).unwrap().clone();
        assert_eq!(context.pool.sync(context.wallet.id.clone()).await.is_err(), true);
        assert_eq!(context.store.states.lock().unwrap().get(&context.wallet.id).unwrap(), &before);
        assert_eq!(context.pool.add_address(context.wallet.id.clone()).await.is_err(), true);
        let snapshot = context.pool.snapshot(context.wallet.id.clone()).await.unwrap().unwrap();
        assert_eq!((snapshot.receive_count, snapshot.selected_index, snapshot.can_add), (20, Some(3), false));
        assert_eq!(snapshot.records.iter().filter(|record| record.status == BsvHomeBsvAddressStatus::Used).count(), 0);
    });
}

#[test]
fn test_receive_mutations_reject_store_conflicts_and_invalid_selection_metadata() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        for add in [false, true] {
            let before = context.store.states.lock().unwrap().get(&context.wallet.id).unwrap().clone();
            *context.store.conflict_on_save.lock().unwrap() = true;
            let result = if add {
                context.pool.add_address(context.wallet.id.clone()).await
            } else {
                context.pool.select_address(context.wallet.id.clone(), 3).await
            };
            assert_eq!(result.unwrap_err().to_string(), "BSV address state changed concurrently; retry");
            let after = context.store.states.lock().unwrap().get(&context.wallet.id).unwrap().clone();
            assert_eq!((after.revision, after.value), (before.revision + 1, before.value));
        }
        let (_, original) = context.pool.required_state(&context.wallet).await.unwrap();
        for (count, selected) in [(Some(10), None), (None, Some(0)), (Some(0), Some(0)), (Some(10), Some(10)), (Some(11), Some(0))] {
            let mut invalid = original.clone();
            invalid.receive_count = count;
            invalid.selected_index = selected;
            assert_eq!(invalid.validate(&context.wallet.id, &context.wallet.accounts[0].address).is_err(), true);
        }
    });
}

#[test]
fn test_receipt_subscription_excludes_unissued_reserves_change_and_foreign_wallets() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let (revision, mut state) = context.pool.required_state(&context.wallet).await.unwrap();
        state.add_address(locator(BsvAddressBranch::Receive, 10)).unwrap();
        state.add_address(locator(BsvAddressBranch::Change, 0)).unwrap();
        context.pool.store_state(&context.wallet, Some(revision), &state).await.unwrap();
        let issued = context.pool.receipt_addresses(context.wallet.id.clone()).await.unwrap().unwrap();
        assert_eq!(
            issued,
            state
                .records
                .iter()
                .filter(|record| record.locator.branch == BsvAddressBranch::Receive && record.locator.index < 10)
                .map(|record| record.address.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(context.pool.owned_addresses(context.wallet.id.clone()).await.unwrap().len(), 12);
        let (revision, mut state) = context.pool.required_state(&context.wallet).await.unwrap();
        state.cold_receive_count = Some(12);
        state.cold_selected_index = Some(0);
        state.add_address(locator(BsvAddressBranch::Receive, 11)).unwrap();
        context.pool.store_state(&context.wallet, Some(revision), &state).await.unwrap();
        assert_eq!(context.pool.receipt_addresses(context.wallet.id.clone()).await.unwrap().unwrap().len(), 12);
        let (revision, mut state) = context.pool.required_state(&context.wallet).await.unwrap();
        state.issue_receive_addresses(14).unwrap();
        state.add_address(locator(BsvAddressBranch::Receive, 14)).unwrap();
        context.pool.store_state(&context.wallet, Some(revision), &state).await.unwrap();
        assert_eq!(context.pool.receipt_addresses(context.wallet.id.clone()).await.unwrap().unwrap().len(), 14);
        assert_eq!(
            context
                .pool
                .receipt_addresses(WalletId::View(Chain::BitcoinSv, "absent-public-fixture".into()))
                .await
                .is_err(),
            true
        );
        context.wallets.wallets.lock().unwrap()[0].accounts[0].address = "1BitcoinEaterAddressDontSendf59kuE".into();
        assert_eq!(context.pool.receipt_addresses(context.wallet.id.clone()).await.is_err(), true);
        assert_eq!(*context.password.create_requests.lock().unwrap(), vec![false]);
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_cold_public_observer_addresses_are_local_selected_and_gap_limited() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        let mut account = context.wallet.accounts[0].clone();
        account.extended_public_key = Some(bsv_account_xpub(ABANDON_PHRASE).unwrap());
        let wallet = Wallet {
            id: WalletId::View(Chain::BitcoinSv, account.address.clone()),
            wallet_type: WalletType::View,
            accounts: vec![account.clone()],
            ..context.wallet.clone()
        };
        context.wallets.add_wallet(wallet.clone()).await.unwrap();
        let initial = context.pool.cold_addresses(wallet.id.clone()).await.unwrap();
        assert_eq!(initial.addresses.len(), 10);
        assert_eq!((initial.selected_index, initial.can_add), (Some(0), true));
        for index in 10..20 {
            let state = context.pool.cold_add_address(wallet.id.clone()).await.unwrap();
            assert_eq!(state.selected_index, Some(index));
            assert_eq!(state.addresses.len(), index as usize + 1);
            assert_eq!(state.can_add, index < 19);
        }
        assert_eq!(context.pool.cold_add_address(wallet.id.clone()).await.is_err(), true);
        assert_eq!(context.pool.cold_select_address(wallet.id.clone(), 20).await.is_err(), true);
        let selected = context.pool.cold_select_address(wallet.id.clone(), 3).await.unwrap();
        assert_eq!(selected.selected_index, Some(3));
        assert_eq!(
            selected
                .addresses
                .iter()
                .filter(|address| address.is_current)
                .map(|address| address.index)
                .collect::<Vec<_>>(),
            vec![3]
        );
        assert_eq!(context.pool.cold_addresses(wallet.id.clone()).await.unwrap(), selected);
        assert_eq!(context.pool.cold_account(wallet.id.clone()).await.unwrap(), account);
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
        let current = context.wallets.get_wallet(wallet.id.clone()).await.unwrap().unwrap();
        assert_eq!(serde_json::to_value(current).unwrap(), serde_json::to_value(wallet).unwrap());
    });
}

#[test]
fn test_legacy_pool_records_without_cold_metadata_preserve_identity_and_reservations() {
    let state = state();
    let mut value = serde_json::to_value(&state).unwrap();
    for key in ["account_path", "cold_receive_count", "cold_selected_index"] {
        value.as_object_mut().unwrap().remove(key);
    }
    let legacy: PoolState = serde_json::from_value(value).unwrap();
    let main = legacy.address(locator(BsvAddressBranch::Receive, 0)).unwrap();
    legacy.validate(&legacy.wallet_id, &main).unwrap();
    assert_eq!(legacy.account_path, None);
    assert_eq!(legacy.snapshot(), state.snapshot());
    assert_eq!(legacy.reservations.len(), state.reservations.len());
}

#[test]
fn test_scan_error_preserves_discovery_cursor_and_saved_address_records() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        let snapshot = context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let saved = context.store.states.lock().unwrap().clone();
        assert!(context.pool.discover(&context.wallet, false, 0, &context.pool.gateway.bsv_client()).await.is_err());
        assert_eq!(*context.store.states.lock().unwrap(), saved);
        assert_eq!(context.pool.snapshot(context.wallet.id.clone()).await.unwrap(), Some(snapshot));
        assert_eq!(context.provider.requested_paths(), vec!["/v1/bsv/main/addresses/history/all".to_string()]);
    });
}

#[test]
fn test_multi_address_balance_error_does_not_return_partial_balance_or_change_the_pool() {
    block_on(async {
        let main = state().address(locator(BsvAddressBranch::Receive, 0)).unwrap();
        let provider = Arc::new(TestAlienProvider::with_responses(vec![(
            format!("https://api.whatsonchain.com/v1/bsv/main/address/{main}/unspent/all"),
            AlienResponse::new(Some(200), format!(r#"{{"address":"{main}","result":[],"error":""}}"#).into_bytes()),
        )]));
        let context = Context::new(provider);
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        context.mark_discovery_complete().await;
        let saved = context.store.states.lock().unwrap().clone();
        context.password.create_requests.lock().unwrap().clear();
        assert!(context.pool.balance(context.wallet.id.clone(), false).await.is_err());
        assert_eq!(*context.store.states.lock().unwrap(), saved);
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        assert_eq!(context.provider.requested_paths().len(), 2);
    });
}

#[test]
fn test_atomic_wallet_import_failure_preserves_both_wallet_and_address_stores() {
    block_on(async {
        let wallets = MemoryWalletStore::default();
        let state = state();
        let wallet = Wallet {
            id: state.wallet_id.clone(),
            wallet_type: WalletType::Multicoin,
            accounts: vec![Account::mock(Chain::BitcoinSv, &state.address(locator(BsvAddressBranch::Receive, 0)).unwrap())],
            ..Wallet::mock()
        };
        let initial = BsvHomeBsvAddressPool::initial_record(&wallet, ABANDON_PHRASE).unwrap().unwrap();
        *wallets.bsv_addresses.save_error.lock().unwrap() = Some(BsvHomeServiceError::Store {
            msg: "import storage failed".into(),
        });
        assert!(wallets.add_wallet_with_bsv_addresses(wallet.clone(), Some(initial.clone())).await.is_err());
        assert_eq!(wallets.get_wallets().await.unwrap().len(), 0);
        assert_eq!(wallets.bsv_addresses.states.lock().unwrap().len(), 0);
        *wallets.bsv_addresses.save_error.lock().unwrap() = None;
        wallets.add_wallet_with_bsv_addresses(wallet.clone(), Some(initial.clone())).await.unwrap();
        assert_eq!(
            serde_json::to_value(wallets.get_wallets().await.unwrap()).unwrap(),
            serde_json::to_value(vec![&wallet]).unwrap()
        );
        assert_eq!(wallets.bsv_addresses.states.lock().unwrap().get(&wallet.id).cloned(), Some(initial));
    });
}

#[test]
fn test_signed_change_and_checkpoint_survive_cancel_and_reject_replacement_or_wrong_owner() {
    block_on(async {
        let mut context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let (_, state) = context.pool.required_state(&context.wallet).await.unwrap();
        let main = state.address(locator(BsvAddressBranch::Receive, 0)).unwrap();
        let mut reserved = reservation(&state, 0);
        reserved.spend.inputs.push(BsvSpendInput {
            transaction_id: "1".repeat(64),
            vout: 0,
            value: 50_000,
            address: main.clone(),
            script_pubkey: hex::encode(BsvAddress::from_str(&main).unwrap().script_pubkey().as_bytes()),
            locator: locator(BsvAddressBranch::Receive, 0),
        });
        let recipient = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".to_string();
        reserved.recipient = recipient.clone();
        let load = TransactionLoadInput {
            input_type: TransactionInputType::Transfer {
                asset: Asset::from_chain(Chain::BitcoinSv),
            },
            sender_address: main.clone(),
            destination_address: recipient.clone(),
            value: 10_000u32.into(),
            gas_price: GasPriceType::regular(100),
            memo: None,
            is_max_value: false,
            metadata: TransactionLoadMetadata::BitcoinSv { spend: reserved.spend.clone() },
        };
        let plan = plan_transfer(&load).unwrap();
        let input = SignerInput::new(
            load,
            TransactionFee {
                fee: plan.fee.into(),
                gas_price_type: GasPriceType::regular(100),
                gas_limit: 1.into(),
                options: Default::default(),
                fee_asset: Chain::BitcoinSv.as_asset_id(),
            },
        );
        let signed_data = BsvChainSigner
            .sign_transfer_with_keys(&input, |locator| bsv_private_key(ABANDON_PHRASE, locator).map_err(SignerError::from_display))
            .unwrap();
        let transaction_id = BsvChainSigner::validate_signed_spend(&signed_data, &input).unwrap();
        let saved = BsvHomeTransactionCheckpointData {
            version: 1,
            bsv_spend: Some(reserved.spend.clone()),
            original_value: Some(10_000.into()),
            use_max_amount: false,
            gas_price: Some(GasPriceType::regular(100)),
            wallet_id: context.wallet.id.clone(),
            asset_id: Chain::BitcoinSv.as_asset_id(),
            sender_address: main,
            recipient_address: recipient,
            value: 10_000.into(),
            network_fee: plan.fee.into(),
            fee_asset_id: Chain::BitcoinSv.as_asset_id(),
            transaction_id,
            signed_data,
        };
        let lease = context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.unwrap();
        let reservation_id = lease.id.clone();
        assert_eq!(
            context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.err().unwrap().to_string(),
            "BSV payment is already being signed"
        );
        let prepared_state = context.store.states.lock().unwrap().clone();
        drop(lease);
        context.pool = context.rebuilt_pool();
        let resumed = context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.unwrap();
        assert_eq!(resumed.id, reservation_id);
        assert_eq!(*context.store.states.lock().unwrap(), prepared_state);
        drop(resumed);
        context.pool.release_unsigned(&context.wallet, &reservation_id).await.unwrap();
        let (_, released) = context.pool.required_state(&context.wallet).await.unwrap();
        assert_eq!(released.reservations.len(), 0);
        assert_eq!(released.next_change().unwrap(), locator(BsvAddressBranch::Change, 0));

        let signing = context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.unwrap();
        context.pool.validate_checkpoint(&context.wallet, &saved).await.unwrap();
        context.pool.save_signed(&context.wallet, &signing.id, saved.clone()).await.unwrap();
        context.pool.release_unsigned(&context.wallet, &signing.id).await.unwrap();
        let (_, mut signed) = context.pool.required_state(&context.wallet).await.unwrap();
        assert_eq!(signed.reservations[0].checkpoint, Some(saved.clone()));
        signed.observe(locator(BsvAddressBranch::Change, 0), false, false).unwrap();
        assert_eq!(signed.next_change().unwrap(), locator(BsvAddressBranch::Change, 1));
        let stored = context.store.states.lock().unwrap().clone();
        let mut replacement = saved.clone();
        replacement.transaction_id = "2".repeat(64);
        assert!(context.pool.save_signed(&context.wallet, &signing.id, replacement).await.is_err());
        let mut fee_tampered = saved.clone();
        fee_tampered.network_fee += 1u32;
        assert!(context.pool.save_signed(&context.wallet, &signing.id, fee_tampered).await.is_err());
        assert_eq!(*context.store.states.lock().unwrap(), stored);
        let mut wrong_owner = saved;
        wrong_owner.bsv_spend.as_mut().unwrap().inputs[0].locator.index = 1;
        assert_eq!(
            context.pool.validate_checkpoint(&context.wallet, &wrong_owner).await.unwrap_err().to_string(),
            "BSV input does not belong to wallet"
        );
    });
}

#[test]
fn test_automatic_and_continued_discovery_find_funds_beyond_initial_receive_reserve() {
    block_on(async {
        let provider = Arc::new(BsvAddressProvider::default());
        let context = Context::new(provider.clone());
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let (_, state) = context.pool.required_state(&context.wallet).await.unwrap();
        let first = state.address(locator(BsvAddressBranch::Receive, 15)).unwrap();
        let later = state.address(locator(BsvAddressBranch::Receive, 45)).unwrap();
        provider.add_output(&first, &"1".repeat(64), 50_000, true);
        provider.add_output(&later, &"2".repeat(64), 7_000, true);

        let automatic = context.pool.sync(context.wallet.id.clone()).await.unwrap();
        assert_eq!(automatic.discovery, BsvHomeBsvDiscoveryState::Complete);
        assert_eq!((automatic.receive_scanned, automatic.change_scanned), (36, 20));
        assert_eq!(
            automatic
                .records
                .iter()
                .filter(|record| record.status == BsvHomeBsvAddressStatus::Used)
                .map(|record| (record.index, record.address.clone()))
                .collect::<Vec<_>>(),
            vec![(15, first.clone())]
        );

        let continued = context.pool.continue_scan(context.wallet.id.clone()).await.unwrap();
        assert_eq!(continued.discovery, BsvHomeBsvDiscoveryState::Complete);
        assert_eq!((continued.receive_scanned, continued.change_scanned), (66, 40));
        assert_eq!(
            continued
                .records
                .iter()
                .filter(|record| record.status == BsvHomeBsvAddressStatus::Used)
                .map(|record| (record.index, record.address.clone()))
                .collect::<Vec<_>>(),
            vec![(15, first), (45, later)]
        );
        assert_eq!(continued.current_address, automatic.current_address);
        assert_eq!(*context.password.create_requests.lock().unwrap(), vec![false]);
        let (_, saved) = context.pool.required_state(&context.wallet).await.unwrap();
        assert_eq!(
            receive_indexes(&saved, BsvHomeBsvAddressStatus::Unused),
            (0..45).filter(|index| *index != 15).collect::<Vec<_>>()
        );
        assert_eq!((continued.receive_count, continued.selected_index), (46, Some(0)));
    });
}

#[test]
fn test_incomplete_discovery_keeps_balance_and_history_from_publishing_a_partial_wallet_scope() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let saved = context.store.states.lock().unwrap().clone();
        let expected = "BSV automatic discovery is incomplete; continue scanning";
        assert_eq!(context.pool.balance(context.wallet.id.clone(), false).await.unwrap_err().to_string(), expected);
        assert_eq!(context.pool.transactions(context.wallet.id.clone(), 1000).await.unwrap_err().to_string(), expected);
        assert_eq!(*context.store.states.lock().unwrap(), saved);
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_rpc_revision_change_rejects_stale_balance_history_and_address_refresh() {
    enum Query {
        Balance,
        History,
        Refresh,
    }

    block_on(async {
        for query in [Query::Balance, Query::History, Query::Refresh] {
            let provider = Arc::new(BsvAddressProvider::default());
            let context = Context::new(provider.clone());
            context.pool.prepare(context.wallet.id.clone()).await.unwrap();
            context.mark_discovery_complete().await;
            let before = context.store.states.lock().unwrap().get(&context.wallet.id).unwrap().clone();
            *provider.revision_change.lock().unwrap() = Some((context.store.clone(), context.wallet.id.clone()));
            let (result, expected, requests) = match query {
                Query::Balance => (
                    context.pool.balance(context.wallet.id.clone(), false).await.map(|_| ()),
                    "BSV address scope changed during synchronization; retry",
                    10,
                ),
                Query::History => (
                    context.pool.transactions(context.wallet.id.clone(), 1000).await.map(|_| ()),
                    "BSV address scope changed during synchronization; retry",
                    1,
                ),
                Query::Refresh => (
                    context.pool.refresh_known(&context.wallet, 0, &context.pool.gateway.bsv_client()).await,
                    "BSV address state changed concurrently; retry",
                    1,
                ),
            };
            assert_eq!(result.unwrap_err().to_string(), expected);
            let after = context.store.states.lock().unwrap().get(&context.wallet.id).unwrap().clone();
            assert_eq!((after.revision, after.value), (before.revision + 1, before.value));
            assert_eq!(provider.requested.lock().unwrap().len(), requests);
        }
    });
}

#[test]
fn test_reserved_balance_counts_only_verified_unspent_inputs_and_preserves_available_and_pending() {
    block_on(async {
        let provider = Arc::new(BsvAddressProvider::default());
        let context = Context::new(provider.clone());
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        context.mark_discovery_complete().await;
        let (revision, mut state) = context.pool.required_state(&context.wallet).await.unwrap();
        let main_locator = locator(BsvAddressBranch::Receive, 0);
        let main = state.address(main_locator).unwrap();
        let available = state.address(locator(BsvAddressBranch::Receive, 1)).unwrap();
        let pending = state.address(locator(BsvAddressBranch::Receive, 2)).unwrap();
        let reserved_transaction = "1".repeat(64);
        provider.add_output(&main, &reserved_transaction, 50_000, true);
        provider.add_output(&available, &"2".repeat(64), 7_000, true);
        provider.add_output(&pending, &"3".repeat(64), 3_000, false);
        let mut reserved = reservation(&state, 0);
        reserved.recipient = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".into();
        reserved.spend.inputs.push(BsvSpendInput {
            transaction_id: reserved_transaction.clone(),
            vout: 0,
            value: 50_000,
            address: main.clone(),
            script_pubkey: hex::encode(BsvAddress::from_str(&main).unwrap().script_pubkey().as_bytes()),
            locator: main_locator,
        });
        state.add_address(locator(BsvAddressBranch::Change, 0)).unwrap();
        state.reservations.push(reserved);
        context.pool.store_state(&context.wallet, Some(revision), &state).await.unwrap();
        let balance = context.pool.balance(context.wallet.id.clone(), false).await.unwrap().balance;
        assert_eq!(
            (balance.available, balance.pending_unconfirmed, balance.reserved),
            (7_000u32.into(), 3_000u32.into(), 50_000u32.into())
        );
        provider.remove_output(&main, &reserved_transaction);
        let balance = context.pool.balance(context.wallet.id.clone(), false).await.unwrap().balance;
        assert_eq!(
            (balance.available, balance.pending_unconfirmed, balance.reserved),
            (7_000u32.into(), 3_000u32.into(), 0u32.into())
        );
    });
}

fn single_address_wallet() -> Wallet {
    let address = BsvAddress::from_public_key(&secp256k1_public_key(&[9; 32]).unwrap()).unwrap().encode();
    Wallet {
        id: WalletId::PrivateKey(Chain::BitcoinSv, address.clone()),
        wallet_type: WalletType::PrivateKey,
        accounts: vec![Account {
            chain: Chain::BitcoinSv,
            address,
            derivation_path: String::new(),
            extended_public_key: None,
        }],
        ..Wallet::mock()
    }
}

pub(super) fn payer_transfer_input(wallet: &Wallet, hd: Option<&PoolState>) -> SignerInput {
    let sender = wallet.account(Chain::BitcoinSv).unwrap().address.clone();
    let metadata = match hd {
        Some(state) => {
            let change_locator = locator(BsvAddressBranch::Change, 0);
            let change_address = state.address(change_locator).unwrap();
            TransactionLoadMetadata::BitcoinSv {
                spend: BsvSpendMetadata {
                    inputs: vec![BsvSpendInput {
                        transaction_id: "1".repeat(64),
                        vout: 0,
                        value: 50_000,
                        address: sender.clone(),
                        script_pubkey: hex::encode(BsvAddress::from_str(&sender).unwrap().script_pubkey().as_bytes()),
                        locator: locator(BsvAddressBranch::Receive, 0),
                    }],
                    change: Some(BsvSpendChange {
                        script_pubkey: hex::encode(BsvAddress::from_str(&change_address).unwrap().script_pubkey().as_bytes()),
                        address: change_address,
                        locator: change_locator,
                    }),
                },
            }
        }
        None => TransactionLoadMetadata::Bitcoin {
            utxos: vec![UTXO {
                transaction_id: "1".repeat(64),
                vout: 0,
                value: 50_000u64.into(),
                address: sender.clone(),
            }],
        },
    };
    let load = TransactionLoadInput {
        input_type: TransactionInputType::Transfer {
            asset: Asset::from_chain(Chain::BitcoinSv),
        },
        sender_address: sender,
        destination_address: "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".into(),
        value: 10_000u64.into(),
        gas_price: GasPriceType::regular(100),
        memo: None,
        is_max_value: false,
        metadata,
    };
    let plan = plan_transfer(&load).unwrap();
    let fee = TransactionFee {
        fee: plan.fee.into(),
        gas_price_type: load.gas_price.clone(),
        gas_limit: 1.into(),
        options: HashMap::new(),
        fee_asset: Chain::BitcoinSv.as_asset_id(),
    };
    SignerInput::new(load, fee)
}

fn payer_funding_binding(wallet: &Wallet, hd: Option<&PoolState>, operation: &str) -> PayerFundingBinding {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../../../../crates/bsv_home_bsv/testdata/bsv-multisig-interop.json")).unwrap();
    let policy: Policy = serde_json::from_value(fixture["proposal"]["policy"].clone()).unwrap();
    let account = wallet.account(Chain::BitcoinSv).unwrap();
    assert_eq!(has_hd_account(wallet), hd.is_some());
    let change = hd.map(|state| {
        let at = state.next_change().unwrap();
        let address = state.address(at).unwrap();
        BsvSpendChange {
            script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
            address,
            locator: at,
        }
    });
    let funding = Funding::from_spend(
        policy,
        account.address.clone(),
        10_000,
        100,
        BsvSpendMetadata {
            inputs: vec![BsvSpendInput {
                transaction_id: "1".repeat(64),
                vout: 0,
                value: 50_000,
                address: account.address.clone(),
                script_pubkey: hex::encode(BsvAddress::from_str(&account.address).unwrap().script_pubkey().as_bytes()),
                locator: locator(BsvAddressBranch::Receive, 0),
            }],
            change,
        },
    )
    .unwrap();
    PayerFundingBinding {
        scope: format!("https://ledger-fixture.invalid/api/portal|{}|team", wallet.id),
        operation_id: operation.into(),
        payer_wallet_id: wallet.id.clone(),
        target: crate::services::bsv_multisig::BsvHomeBsvMultisigFundingTarget::Member { wallet_id: "team".into() },
        review_json: serde_json::to_string(&review_funding(&funding).unwrap()).unwrap(),
        funding_json: serde_json::to_string(&funding).unwrap(),
    }
}

#[test]
fn test_single_address_real_legacy_send_reserves_before_sdk_sign_without_xpub_or_derived_change() {
    block_on(async {
        let mut context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.wallet = single_address_wallet();
        *context.wallets.wallets.lock().unwrap() = vec![context.wallet.clone()];
        let input = payer_transfer_input(&context.wallet, None);
        let lease = context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.unwrap();
        let (_, state) = context.pool.payer_state(&context.wallet).await.unwrap().unwrap();
        assert!(matches!(state, PayerState::SingleAddress(_)));
        assert_eq!(state.reservations().len(), 1);
        assert_eq!(state.reservations()[0].spend.change, None);
        assert_eq!(state.reservations()[0].spend.inputs[0].locator, locator(BsvAddressBranch::Receive, 0));
        context.pool.release_unsigned(&context.wallet, &lease.id).await.unwrap();
        drop(lease);
        assert!(context.pool.payer_state(&context.wallet).await.unwrap().unwrap().1.reservations().is_empty());
        let lease = context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.unwrap();
        let raw = BsvChainSigner.sign_transfer(&input, &[9; 32]).unwrap();
        let plan = plan_transfer(&input.input).unwrap();
        assert!(verify_fork_id_plan(Chain::BitcoinSv, &plan, &raw).is_ok());
        let stored = context.store.states.lock().unwrap().get(&context.wallet.id).cloned().unwrap();
        let json: serde_json::Value = serde_json::from_str(&stored.value).unwrap();
        assert_eq!(json["kind"], "singleAddress");
        assert!(json.get("account_xpub").is_none());
        assert!(context.provider.requested_paths().is_empty());
        drop(lease);
        assert_eq!(context.pool.payer_state(&context.wallet).await.unwrap().unwrap().1.reservations().len(), 1);
        assert!(
            context
                .rebuilt_pool()
                .reserve_funding(&context.wallet, payer_funding_binding(&context.wallet, None, "after-sign"))
                .await
                .is_err()
        );
    });
}

#[test]
fn test_hd_and_single_payers_share_transfer_funding_outpoint_conflicts_in_both_orders() {
    block_on(async {
        for single in [false, true] {
            for funding_first in [false, true] {
                let mut context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
                let hd = if single {
                    context.wallet = single_address_wallet();
                    *context.wallets.wallets.lock().unwrap() = vec![context.wallet.clone()];
                    None
                } else {
                    context.pool.prepare(context.wallet.id.clone()).await.unwrap();
                    Some(context.pool.required_state(&context.wallet).await.unwrap().1)
                };
                let input = payer_transfer_input(&context.wallet, hd.as_ref());
                if funding_first {
                    let _lease = context
                        .pool
                        .reserve_funding(&context.wallet, payer_funding_binding(&context.wallet, hd.as_ref(), "funding-one"))
                        .await
                        .unwrap();
                    assert!(context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.is_err());
                    assert!(
                        context
                            .rebuilt_pool()
                            .reserve_funding(&context.wallet, payer_funding_binding(&context.wallet, hd.as_ref(), "funding-two"))
                            .await
                            .is_err()
                    );
                } else {
                    let _lease = context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.unwrap();
                    assert!(
                        context
                            .rebuilt_pool()
                            .reserve_funding(&context.wallet, payer_funding_binding(&context.wallet, hd.as_ref(), "funding-one"))
                            .await
                            .is_err()
                    );
                }
                let (_, state) = context.pool.payer_state(&context.wallet).await.unwrap().unwrap();
                assert_eq!(state.reservations().len(), usize::from(!funding_first));
                assert_eq!(state.funding().len(), usize::from(funding_first));
                assert!(context.provider.requested_paths().is_empty());
            }
        }
    });
}

#[test]
fn test_payer_ledger_decodes_legacy_hd_without_rewriting_root_selection_or_schema() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let stored = context.store.states.lock().unwrap().get(&context.wallet.id).cloned().unwrap();
        let (_, payer) = context.pool.payer_state(&context.wallet).await.unwrap().unwrap();
        assert!(matches!(payer, PayerState::Hd(_)));
        let legacy: PoolState = serde_json::from_str(&stored.value).unwrap();
        assert_eq!(legacy.snapshot().selected_index, Some(0));
        assert_eq!(legacy.snapshot().receive_count, 10);
        assert_eq!(serde_json::to_string(&legacy).unwrap(), stored.value);
        assert_eq!(context.store.states.lock().unwrap().get(&context.wallet.id).cloned(), Some(stored));
    });
}

#[test]
fn test_corrupt_or_wrong_family_payer_state_fails_without_creating_a_replacement() {
    block_on(async {
        for kind in ["invalid-xpub", "unknown-kind", "hd-for-private"] {
            let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
            context.pool.prepare(context.wallet.id.clone()).await.unwrap();
            let original = context.store.states.lock().unwrap().get(&context.wallet.id).cloned().unwrap();
            let mut value: serde_json::Value = serde_json::from_str(&original.value).unwrap();
            let mut wallet = context.wallet.clone();
            match kind {
                "invalid-xpub" => value["account_xpub"] = "not-an-xpub".into(),
                "unknown-kind" => value["kind"] = "unknown".into(),
                "hd-for-private" => wallet = single_address_wallet(),
                _ => unreachable!(),
            }
            let saved = super::BsvHomeBsvAddressStoredState {
                revision: original.revision,
                value: serde_json::to_string(&value).unwrap(),
            };
            context.store.states.lock().unwrap().insert(wallet.id.clone(), saved.clone());
            assert!(context.pool.payer_state(&wallet).await.is_err());
            assert_eq!(context.store.states.lock().unwrap().get(&wallet.id).cloned(), Some(saved));
        }
    });
}

#[test]
fn test_single_address_ledger_corruption_never_becomes_hd_or_erases_reservations() {
    block_on(async {
        for corruption in ["root", "kind", "locator", "script", "confirmed"] {
            let mut context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
            context.wallet = single_address_wallet();
            *context.wallets.wallets.lock().unwrap() = vec![context.wallet.clone()];
            let input = payer_transfer_input(&context.wallet, None);
            let _lease = context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.unwrap();
            let original = context.store.states.lock().unwrap().get(&context.wallet.id).cloned().unwrap();
            let mut value: serde_json::Value = serde_json::from_str(&original.value).unwrap();
            match corruption {
                "root" => value["root_address"] = "1BitcoinEaterAddressDontSendf59kuE".into(),
                "kind" => value["kind"] = "hd".into(),
                "locator" => value["reservations"][0]["spend"]["inputs"][0]["locator"]["branch"] = "change".into(),
                "script" => value["reservations"][0]["spend"]["inputs"][0]["scriptPubkey"] = "00".into(),
                "confirmed" => value["reservations"][0]["confirmed"] = true.into(),
                _ => unreachable!(),
            }
            let corrupt = super::BsvHomeBsvAddressStoredState {
                revision: original.revision,
                value: serde_json::to_string(&value).unwrap(),
            };
            context.store.states.lock().unwrap().insert(context.wallet.id.clone(), corrupt.clone());
            assert!(context.pool.payer_state(&context.wallet).await.is_err());
            assert!(
                context
                    .rebuilt_pool()
                    .reserve_funding(&context.wallet, payer_funding_binding(&context.wallet, None, "corrupt-retry"))
                    .await
                    .is_err()
            );
            assert_eq!(context.store.states.lock().unwrap().get(&context.wallet.id).cloned(), Some(corrupt));
        }
    });
}

#[test]
fn test_common_payer_reservation_cas_rejects_stale_writes_for_both_ownership_families() {
    block_on(async {
        for single in [false, true] {
            let mut context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
            if single {
                context.wallet = single_address_wallet();
                *context.wallets.wallets.lock().unwrap() = vec![context.wallet.clone()];
                let input = payer_transfer_input(&context.wallet, None);
                let lease = context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.unwrap();
                context.pool.release_unsigned(&context.wallet, &lease.id).await.unwrap();
            } else {
                context.pool.prepare(context.wallet.id.clone()).await.unwrap();
            }
            let hd = if single {
                None
            } else {
                Some(context.pool.required_state(&context.wallet).await.unwrap().1)
            };
            let before = context.store.states.lock().unwrap().get(&context.wallet.id).cloned().unwrap();
            *context.store.conflict_on_save.lock().unwrap() = true;
            assert!(
                context
                    .pool
                    .reserve_funding(&context.wallet, payer_funding_binding(&context.wallet, hd.as_ref(), "stale-request"))
                    .await
                    .is_err()
            );
            let after = context.store.states.lock().unwrap().get(&context.wallet.id).cloned().unwrap();
            assert_eq!(after.revision, before.revision + 1);
            assert_eq!(after.value, before.value);
            assert!(context.pool.payer_state(&context.wallet).await.unwrap().unwrap().1.reservations().is_empty());
        }
    });
}

#[test]
fn test_common_payer_reservation_rejects_duplicate_outpoint_aliases_without_a_write() {
    block_on(async {
        for single in [false, true] {
            let mut context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
            if single {
                context.wallet = single_address_wallet();
                *context.wallets.wallets.lock().unwrap() = vec![context.wallet.clone()];
            } else {
                context.pool.prepare(context.wallet.id.clone()).await.unwrap();
            }
            let before = context.store.states.lock().unwrap().get(&context.wallet.id).cloned();
            let hd = if single {
                None
            } else {
                Some(context.pool.required_state(&context.wallet).await.unwrap().1)
            };
            let input = payer_transfer_input(&context.wallet, hd.as_ref());
            let mut request = PayerReservationRequest::transfer(&context.wallet, &input, "10000".into(), false).unwrap();
            request.spend.inputs[0].transaction_id = "a".repeat(64);
            let mut alias = request.spend.inputs[0].clone();
            alias.transaction_id = "A".repeat(64);
            request.spend.inputs.push(alias);
            assert!(context.pool.reserve_payer_spend(&context.wallet, request).await.is_err());
            assert_eq!(context.store.states.lock().unwrap().get(&context.wallet.id).cloned(), before);
        }
    });
}

#[test]
fn test_hd_and_single_shared_preview_excludes_common_claims_and_unconfirmed_outputs() {
    block_on(async {
        for single in [false, true] {
            let provider = Arc::new(BsvAddressProvider::default());
            let mut context = Context::new(provider.clone());
            if single {
                context.wallet = single_address_wallet();
                *context.wallets.wallets.lock().unwrap() = vec![context.wallet.clone()];
            } else {
                context.pool.prepare(context.wallet.id.clone()).await.unwrap();
                context.mark_discovery_complete().await;
            }
            let root = context.wallet.account(Chain::BitcoinSv).unwrap().address.clone();
            provider.add_output(&root, &"1".repeat(64), 50_000, true);
            provider.add_output(&root, &"2".repeat(64), 3_000, false);
            provider.add_output(&root, &"3".repeat(64), 7_000, true);
            let preview = context.pool.preload_payer_spend(context.wallet.id.clone(), &root, false).await.unwrap();
            assert_eq!(preview.inputs.len(), 2);
            assert_eq!(preview.change.is_some(), !single);
            let hd = if single {
                None
            } else {
                Some(context.pool.required_state(&context.wallet).await.unwrap().1)
            };
            let _claim = context
                .pool
                .reserve_funding(&context.wallet, payer_funding_binding(&context.wallet, hd.as_ref(), "member-claim"))
                .await
                .unwrap();
            let preview = context.rebuilt_pool().preload_payer_spend(context.wallet.id.clone(), &root, false).await.unwrap();
            assert_eq!(preview.inputs.len(), 1);
            assert_eq!(preview.inputs[0].transaction_id, "3".repeat(64));
            assert_eq!(preview.inputs[0].locator, locator(BsvAddressBranch::Receive, 0));
            let balance = context.pool.balance(context.wallet.id.clone(), false).await.unwrap().balance;
            assert_eq!(
                (balance.available, balance.pending_unconfirmed, balance.reserved),
                (7_000u32.into(), 3_000u32.into(), 50_000u32.into())
            );
            let snapshot = context.pool.snapshot(context.wallet.id.clone()).await.unwrap().unwrap();
            assert_eq!(snapshot.selected_index, Some(0));
            assert_eq!(snapshot.current_address, Some(root));
        }
    });
}

fn legacy_funding_record(wallet: &Wallet) -> super::BsvHomeBsvFundingStoredState {
    let binding = payer_funding_binding(wallet, None, "legacy-original");
    let funding: Funding = serde_json::from_str(&binding.funding_json).unwrap();
    let review: bsv_home_bsv::multisig::FundingReview = serde_json::from_str(&binding.review_json).unwrap();
    let raw = bsv_home_bsv::multisig::sign_funding_with_keys(&funding, &review, |_| Ok(zeroize::Zeroizing::new(vec![9; 32]))).unwrap();
    let transaction_id = bsv_home_bsv::multisig::funding_transaction_id(&funding, &review, &raw).unwrap();
    let old_signer = WalletId::Single(Chain::BitcoinSv, "1BitcoinEaterAddressDontSendf59kuE".into());
    let original_scope = format!("https://ledger-fixture.invalid/api/portal|{old_signer}");
    let scope = format!("{original_scope}|team");
    let checkpoint = serde_json::json!({
        "scope":scope,"walletId":"team","payerWalletId":wallet.id,"fundingJson":binding.funding_json,"reviewJson":binding.review_json,
        "raw":raw,"transactionId":transaction_id,"status":"Pending","errorCode":null,
    });
    let mut funding_map = serde_json::Map::new();
    funding_map.insert(scope, checkpoint);
    let state = serde_json::json!({"trusted":{},"memberNames":{},"intentions":{},"funding":funding_map});
    super::BsvHomeBsvFundingStoredState {
        identifier: hex::encode(bsv_home_hash::sha2::sha256(original_scope.as_bytes())),
        revision: 1,
        json: serde_json::to_string(&state).unwrap(),
    }
}

#[test]
fn test_legacy_all_scope_barrier_preserves_deleted_signer_funding_and_rejects_forged_payer_ids() {
    block_on(async {
        let mut context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.wallet = single_address_wallet();
        *context.wallets.wallets.lock().unwrap() = vec![context.wallet.clone()];
        let record = legacy_funding_record(&context.wallet);
        *context.store.funding_states.lock().unwrap() = vec![record.clone()];
        context.pool.reconcile_funding(&context.wallet).await.unwrap();
        let adopted = context.pool.funding_checkpoints(&context.wallet).await.unwrap();
        assert_eq!(adopted.len(), 1);
        let before = context.store.states.lock().unwrap().get(&context.wallet.id).cloned();
        assert_eq!(*context.store.funding_states.lock().unwrap(), vec![record.clone()]);
        // A stored payer ID cannot hide this wallet's actual signed input root.
        let mut state: serde_json::Value = serde_json::from_str(&record.json).unwrap();
        state["funding"].as_object_mut().unwrap().values_mut().next().unwrap()["payerWalletId"] =
            WalletId::PrivateKey(Chain::BitcoinSv, "1BitcoinEaterAddressDontSendf59kuE".into()).id().into();
        let forged = super::BsvHomeBsvFundingStoredState {
            json: serde_json::to_string(&state).unwrap(),
            ..record
        };
        *context.store.funding_states.lock().unwrap() = vec![forged.clone()];
        assert!(context.pool.reconcile_funding(&context.wallet).await.is_err());
        assert_eq!(context.store.states.lock().unwrap().get(&context.wallet.id).cloned(), before);
        assert_eq!(*context.store.funding_states.lock().unwrap(), vec![forged]);
        assert!(context.provider.requested_paths().is_empty());
    });
}

#[test]
fn test_global_funding_enumeration_errors_and_corrupt_scopes_never_become_no_legacy_funding() {
    block_on(async {
        for corruption in ["read-error", "bad-json", "wrong-file", "bad-revision", "duplicate-file", "raw"] {
            let mut context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
            context.wallet = single_address_wallet();
            *context.wallets.wallets.lock().unwrap() = vec![context.wallet.clone()];
            let mut record = legacy_funding_record(&context.wallet);
            match corruption {
                "read-error" => {
                    *context.store.funding_error.lock().unwrap() = Some(BsvHomeServiceError::Store {
                        msg: "funding enumeration failed".into(),
                    })
                }
                "bad-json" => record.json = "{".into(),
                "wrong-file" => record.identifier = "0".repeat(64),
                "bad-revision" => record.revision = 0,
                "duplicate-file" => {}
                "raw" => {
                    let mut state: serde_json::Value = serde_json::from_str(&record.json).unwrap();
                    state["funding"].as_object_mut().unwrap().values_mut().next().unwrap()["raw"] = "00".into();
                    record.json = serde_json::to_string(&state).unwrap();
                }
                _ => unreachable!(),
            }
            let records = if corruption == "duplicate-file" { vec![record.clone(), record] } else { vec![record] };
            *context.store.funding_states.lock().unwrap() = records.clone();
            let input = payer_transfer_input(&context.wallet, None);
            assert!(context.pool.reserve(&context.wallet, &input, "10000".into(), false).await.is_err());
            assert!(context.store.states.lock().unwrap().is_empty());
            assert_eq!(*context.store.funding_states.lock().unwrap(), records);
            assert!(context.provider.requested_paths().is_empty());
        }
    });
}

#[test]
fn test_shared_issued_receive_range_keeps_cold_history_without_reusing_addresses_or_changing_reservations() {
    block_on(async {
        let context = Context::new(Arc::new(TestAlienProvider::with_status(403)));
        context.pool.prepare(context.wallet.id.clone()).await.unwrap();
        let (revision, mut state) = context.pool.required_state(&context.wallet).await.unwrap();
        for index in 0..14 {
            state.add_address(locator(BsvAddressBranch::Receive, index)).unwrap();
        }
        state.selected_index = Some(3);
        state.cold_receive_count = Some(14);
        state.cold_selected_index = Some(12);
        state.reservations.push(reservation(&state, 0));
        let reservations = serde_json::to_value(&state.reservations).unwrap();
        let next_change = state.next_change();
        let stable = state.public_account(&context.wallet.accounts[0]);
        context.pool.store_state(&context.wallet, Some(revision), &state).await.unwrap();
        let initial = context.pool.snapshot(context.wallet.id.clone()).await.unwrap().unwrap();
        assert_eq!((initial.receive_count, initial.selected_index), (14, Some(3)));
        assert_eq!(context.pool.receipt_addresses(context.wallet.id.clone()).await.unwrap().unwrap().len(), 14);
        let selected = context.pool.select_address(context.wallet.id.clone(), 13).await.unwrap();
        assert_eq!((selected.receive_count, selected.selected_index), (14, Some(13)));
        let added = context.pool.add_address(context.wallet.id.clone()).await.unwrap();
        assert_eq!((added.receive_count, added.selected_index), (15, Some(14)));
        assert_eq!(context.rebuilt_pool().snapshot(context.wallet.id.clone()).await.unwrap(), Some(added));
        let (_, after) = context.pool.required_state(&context.wallet).await.unwrap();
        assert_eq!(serde_json::to_value(&after.reservations).unwrap(), reservations);
        assert_eq!(after.next_change(), next_change);
        assert_eq!(after.public_account(&context.wallet.accounts[0]), stable);
        assert_eq!((after.cold_receive_count, after.cold_selected_index), (Some(14), Some(12)));
        assert_eq!(context.pool.receipt_addresses(context.wallet.id.clone()).await.unwrap().unwrap().len(), 15);
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}
