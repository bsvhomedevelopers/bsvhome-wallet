use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;

use async_trait::async_trait;
use bsv_home_bsv::rpc::BsvRequestScheduler;
use bsv_home_derivation::{bsv_account_xpub, derive_accounts_from_mnemonic};
use futures::channel::oneshot;
use futures::executor::block_on;
use futures::future::{Either, select};
use futures::poll;
use primitives::testkit::ABANDON_PHRASE;
use primitives::{AssetBalance, Chain, Wallet, WalletId, WalletType};
use serde_json::Value;

use super::testkit::{BsvAddressProvider, MemoryBsvAddressStore, complete_record, pool};
use super::{BsvHomeBsvAddressPool, BsvHomeBsvAddressStatus};
use crate::alien::{AlienError, AlienProvider, AlienResponse, AlienTarget};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomePreferencesStore;
use crate::services::preferences::testkit::MemoryPreferencesStore;
use crate::services::wallet::testkit::MemoryWalletStore;

const DEFAULT_NODE: &str = "https://api.whatsonchain.com/v1/bsv/main";
const ALTERNATE_NODE: &str = "https://alternate.example/v1/bsv/main";

#[derive(Clone, Copy, Debug, PartialEq)]
enum RequestKind {
    History,
    Balance,
}

#[derive(Debug)]
struct PausedRequest {
    kind: RequestKind,
    entered: oneshot::Sender<()>,
    release: oneshot::Receiver<()>,
}

#[derive(Debug)]
struct RefreshProvider {
    inner: BsvAddressProvider,
    root: String,
    observed: Mutex<Vec<(RequestKind, String)>>,
    pauses: Mutex<Vec<PausedRequest>>,
    failure: Mutex<Option<RequestKind>>,
}

impl RefreshProvider {
    fn new(root: String) -> Self {
        Self {
            inner: BsvAddressProvider::default(),
            root,
            observed: Mutex::new(Vec::new()),
            pauses: Mutex::new(Vec::new()),
            failure: Mutex::new(None),
        }
    }

    fn pause(&self, kind: RequestKind) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (entered, started) = oneshot::channel();
        let (release, receiver) = oneshot::channel();
        self.pauses.lock().unwrap().push(PausedRequest { kind, entered, release: receiver });
        (started, release)
    }

    fn counts(&self) -> (usize, usize) {
        let observed = self.observed.lock().unwrap();
        (
            observed.iter().filter(|(kind, _)| *kind == RequestKind::History).count(),
            observed.iter().filter(|(kind, _)| *kind == RequestKind::Balance).count(),
        )
    }

    fn request_kind(&self, target: &AlienTarget) -> Option<RequestKind> {
        if target.url.ends_with("/addresses/history/all") {
            let body: Value = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
            return body["addresses"]
                .as_array()
                .unwrap()
                .iter()
                .any(|address| address.as_str() == Some(self.root.as_str()))
                .then_some(RequestKind::History);
        }
        target.url.ends_with(&format!("/address/{}/unspent/all", self.root)).then_some(RequestKind::Balance)
    }
}

#[async_trait]
impl AlienProvider for RefreshProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        let kind = self.request_kind(&target);
        let node = [DEFAULT_NODE, ALTERNATE_NODE].into_iter().find(|node| target.url.starts_with(*node)).unwrap();
        let target = AlienTarget {
            url: format!("{DEFAULT_NODE}{}", target.url.strip_prefix(node).unwrap()),
            ..target
        };
        let response = self.inner.request(target).await;
        let Some(kind) = kind else { return response };
        self.observed.lock().unwrap().push((kind, node.into()));
        let failed = {
            let mut failure = self.failure.lock().unwrap();
            if *failure == Some(kind) {
                *failure = None;
                true
            } else {
                false
            }
        };
        let response = if failed { Err(AlienError::Http { status: 400, len: 0 }) } else { response };
        let pause = {
            let mut pauses = self.pauses.lock().unwrap();
            pauses.iter().position(|pause| pause.kind == kind).map(|index| pauses.remove(index))
        };
        if let Some(pause) = pause {
            pause.entered.send(()).unwrap();
            pause.release.await.unwrap();
        }
        response
    }
}

struct RefreshTestkit {
    pool: Arc<BsvHomeBsvAddressPool>,
    provider: Arc<RefreshProvider>,
    wallets: Arc<MemoryWalletStore>,
    store: Arc<MemoryBsvAddressStore>,
    preferences: Arc<MemoryPreferencesStore>,
    wallet: Wallet,
}

impl RefreshTestkit {
    fn new() -> Self {
        let mut account = derive_accounts_from_mnemonic(ABANDON_PHRASE, vec![Chain::BitcoinSv]).unwrap().remove(0);
        account.extended_public_key = Some(bsv_account_xpub(ABANDON_PHRASE).unwrap());
        let wallet = Wallet {
            id: WalletId::Multicoin("display-refresh".into()),
            wallet_type: WalletType::Multicoin,
            accounts: vec![account],
            ..Wallet::mock()
        };
        let provider = Arc::new(RefreshProvider::new(wallet.accounts[0].address.clone()));
        let wallets = Arc::new(MemoryWalletStore::default());
        wallets.wallets.lock().unwrap().push(wallet.clone());
        let store = wallets.bsv_addresses.clone();
        store.states.lock().unwrap().insert(wallet.id.clone(), complete_record(&wallet, ABANDON_PHRASE));
        let preferences = Arc::new(MemoryPreferencesStore::default());
        let gateway = Arc::new(BsvHomeGateway::new(provider.clone(), preferences.clone(), Arc::new(EmptyPreferences)));
        Self {
            pool: pool(gateway, wallets.clone()),
            provider,
            wallets,
            store,
            preferences,
            wallet,
        }
    }

    fn add_wallet(&self) -> Wallet {
        let wallet = Wallet {
            id: WalletId::Multicoin("other-display-refresh".into()),
            ..self.wallet.clone()
        };
        self.store.states.lock().unwrap().insert(wallet.id.clone(), complete_record(&wallet, ABANDON_PHRASE));
        self.wallets.wallets.lock().unwrap().push(wallet.clone());
        wallet
    }

    fn change_revision(&self) {
        self.store.states.lock().unwrap().get_mut(&self.wallet.id).unwrap().revision += 1;
    }
}

fn zero_balance() -> AssetBalance {
    AssetBalance::new_zero_balance(Chain::BitcoinSv.as_asset_id())
}

async fn await_request<F: Future>(future: Pin<&mut F>, entered: oneshot::Receiver<()>) {
    match select(entered, future).await {
        Either::Left((entered, _)) => entered.unwrap(),
        Either::Right(_) => panic!("Refresh completed before reaching its request gate"),
    }
}

#[test]
fn test_display_refresh_shares_one_scan_and_balance_without_caching_completed_results() {
    block_on(async {
        let kit = RefreshTestkit::new();
        let before = kit.store.states.lock().unwrap().get(&kit.wallet.id).unwrap().clone();
        let (entered, release) = kit.provider.pause(RequestKind::History);
        let mut first = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        await_request(first.as_mut(), entered).await;
        let mut second = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        assert!(poll!(second.as_mut()).is_pending());
        assert_eq!(kit.provider.counts(), (1, 0));

        release.send(()).unwrap();
        assert_eq!(first.await.unwrap(), zero_balance());
        assert_eq!(second.await.unwrap(), zero_balance());
        assert_eq!(kit.provider.counts(), (1, 1));
        assert_eq!(kit.pool.refresh_display_balance(kit.wallet.id.clone()).await.unwrap(), zero_balance());
        assert_eq!(kit.provider.counts(), (2, 2));
        assert_eq!(kit.store.states.lock().unwrap().get(&kit.wallet.id), Some(&before));

        kit.provider.inner.add_output(&kit.wallet.accounts[0].address, &"1".repeat(64), 5_000, true);
        assert_eq!(
            kit.pool.refresh_display_balance(kit.wallet.id.clone()).await.unwrap(),
            AssetBalance::new(Chain::BitcoinSv.as_asset_id(), 5_000u32.into())
        );
        assert_eq!(kit.provider.counts(), (3, 3));
        assert_eq!(kit.store.states.lock().unwrap().get(&kit.wallet.id).unwrap().revision, before.revision + 1);
        assert_eq!(
            kit.pool.snapshot(kit.wallet.id.clone()).await.unwrap().unwrap().records[0].status,
            BsvHomeBsvAddressStatus::Used
        );
    });
}

#[test]
fn test_home_and_receipt_refresh_share_only_one_balance_in_either_order() {
    block_on(async {
        for receipt_first in [false, true] {
            let kit = RefreshTestkit::new();
            let (entered, release) = kit.provider.pause(RequestKind::Balance);
            if receipt_first {
                let mut receipt = Box::pin(kit.pool.refresh_receipt_balance(kit.wallet.id.clone()));
                await_request(receipt.as_mut(), entered).await;
                assert_eq!(kit.provider.counts(), (0, 1));

                let (entered, scan_release) = kit.provider.pause(RequestKind::History);
                let mut home = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
                await_request(home.as_mut(), entered).await;
                scan_release.send(()).unwrap();
                assert!(poll!(home.as_mut()).is_pending());
                release.send(()).unwrap();
                assert_eq!(receipt.await.unwrap(), zero_balance());
                assert_eq!(home.await.unwrap(), zero_balance());
            } else {
                let mut home = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
                await_request(home.as_mut(), entered).await;
                let mut receipt = Box::pin(kit.pool.refresh_receipt_balance(kit.wallet.id.clone()));
                assert!(poll!(receipt.as_mut()).is_pending());
                release.send(()).unwrap();
                assert_eq!(home.await.unwrap(), zero_balance());
                assert_eq!(receipt.await.unwrap(), zero_balance());
            }
            assert_eq!(kit.provider.counts(), (1, 1));
        }
    });
}

#[test]
fn test_display_refresh_shares_failures_and_can_retry() {
    block_on(async {
        let kit = RefreshTestkit::new();
        *kit.provider.failure.lock().unwrap() = Some(RequestKind::History);
        let (entered, release) = kit.provider.pause(RequestKind::History);
        let mut first = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        await_request(first.as_mut(), entered).await;
        let mut second = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        assert!(poll!(second.as_mut()).is_pending());
        release.send(()).unwrap();
        let expected = Err(BsvHomeServiceError::Gateway {
            msg: "Network error: HTTP error: status 400".into(),
        });
        assert_eq!(first.await, expected);
        assert_eq!(second.await, expected);
        assert_eq!(kit.provider.counts(), (1, 0));

        assert_eq!(kit.pool.refresh_display_balance(kit.wallet.id.clone()).await.unwrap(), zero_balance());
        assert_eq!(kit.provider.counts(), (2, 1));
    });
}

#[test]
fn test_display_refresh_keeps_different_wallets_independent() {
    block_on(async {
        let kit = RefreshTestkit::new();
        let other = kit.add_wallet();
        let (entered, release) = kit.provider.pause(RequestKind::History);
        let mut first = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        await_request(first.as_mut(), entered).await;

        let scheduler = BsvRequestScheduler::shared();
        let queue = scheduler.hold_request_queue().await;
        let other_id = other.id;
        let mut other = Box::pin(kit.pool.refresh_display_balance(other_id.clone()));
        assert!(poll!(other.as_mut()).is_pending());
        {
            let scans = kit.pool.active_scans.lock().unwrap();
            let first_lock = scans.discovery.get(&kit.wallet.id).unwrap().upgrade().unwrap();
            let other_lock = scans.discovery.get(&other_id).unwrap().upgrade().unwrap();
            assert!(!Arc::ptr_eq(&first_lock, &other_lock));
            assert!(other_lock.try_lock().is_none());
        }
        drop(queue);
        assert_eq!(other.await.unwrap(), zero_balance());
        assert_eq!(kit.provider.counts(), (2, 1));
        assert!(poll!(first.as_mut()).is_pending());
        release.send(()).unwrap();
        assert_eq!(first.await.unwrap(), zero_balance());
        assert_eq!(kit.provider.counts(), (2, 2));
    });
}

#[test]
fn test_display_refresh_waiter_cancellation_preserves_other_waiters_and_releases_the_last_flight() {
    block_on(async {
        let kit = RefreshTestkit::new();
        let (entered, release) = kit.provider.pause(RequestKind::Balance);
        let mut first = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        await_request(first.as_mut(), entered).await;
        let mut second = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        assert!(poll!(second.as_mut()).is_pending());
        drop(first);
        release.send(()).unwrap();
        assert_eq!(second.await.unwrap(), zero_balance());
        assert_eq!(kit.provider.counts(), (1, 1));

        let (entered, release) = kit.provider.pause(RequestKind::Balance);
        let mut abandoned = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        await_request(abandoned.as_mut(), entered).await;
        drop(abandoned);
        assert_eq!(release.send(()), Err(()));
        assert_eq!(kit.pool.refresh_display_balance(kit.wallet.id.clone()).await.unwrap(), zero_balance());
        assert_eq!(kit.provider.counts(), (3, 3));
    });
}

#[test]
fn test_display_refresh_generation_cancellation_rejects_shared_waiters_and_allows_a_new_scan() {
    block_on(async {
        let kit = RefreshTestkit::new();
        let (entered, release) = kit.provider.pause(RequestKind::History);
        let mut first = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        await_request(first.as_mut(), entered).await;
        let mut second = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        assert!(poll!(second.as_mut()).is_pending());

        kit.pool.cancel_scan(kit.wallet.id.clone()).unwrap();
        assert_eq!(first.await, Err(BsvHomeServiceError::Cancelled));
        assert_eq!(second.await, Err(BsvHomeServiceError::Cancelled));
        assert_eq!(release.send(()), Err(()));
        assert_eq!(kit.pool.refresh_display_balance(kit.wallet.id.clone()).await.unwrap(), zero_balance());
        assert_eq!(kit.provider.counts(), (2, 1));
    });
}

#[test]
fn test_display_refresh_rechecks_revision_for_every_waiter() {
    block_on(async {
        for change_after_first_result in [false, true] {
            let kit = RefreshTestkit::new();
            let (entered, release) = kit.provider.pause(RequestKind::Balance);
            let mut first = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
            await_request(first.as_mut(), entered).await;
            let mut second = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
            assert!(poll!(second.as_mut()).is_pending());
            if !change_after_first_result {
                kit.change_revision();
            }
            release.send(()).unwrap();
            let first = first.await;
            assert_eq!(first.is_ok(), change_after_first_result);
            if change_after_first_result {
                kit.change_revision();
            }
            assert!(second.await.is_err());
            assert_eq!(kit.provider.counts(), (1, 1));
            assert_eq!(kit.pool.refresh_display_balance(kit.wallet.id.clone()).await.unwrap(), zero_balance());
            assert_eq!(kit.provider.counts(), (2, 2));
        }
    });
}

#[test]
fn test_changed_wallet_identity_does_not_join_or_remove_a_replacement_display_refresh() {
    block_on(async {
        let kit = RefreshTestkit::new();
        let (entered, old_release) = kit.provider.pause(RequestKind::Balance);
        let mut old = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        await_request(old.as_mut(), entered).await;
        kit.wallets.wallets.lock().unwrap()[0].accounts[0].extended_public_key = None;

        let (entered, release) = kit.provider.pause(RequestKind::Balance);
        let mut replacement = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        assert!(poll!(replacement.as_mut()).is_pending());
        old_release.send(()).unwrap();
        assert!(old.await.is_err());
        await_request(replacement.as_mut(), entered).await;
        assert_eq!(kit.provider.counts(), (2, 2));

        let mut joined = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        assert!(poll!(joined.as_mut()).is_pending());
        assert_eq!(kit.provider.counts(), (2, 2));
        release.send(()).unwrap();
        assert_eq!(replacement.await.unwrap(), zero_balance());
        assert_eq!(joined.await.unwrap(), zero_balance());
    });
}

#[test]
fn test_display_refresh_isolates_node_scopes_and_rechecks_completed_results() {
    block_on(async {
        for complete_first_result in [false, true] {
            let kit = RefreshTestkit::new();
            let (entered, release) = kit.provider.pause(RequestKind::Balance);
            let mut first = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
            await_request(first.as_mut(), entered).await;
            let mut second = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
            assert!(poll!(second.as_mut()).is_pending());

            if complete_first_result {
                release.send(()).unwrap();
                assert_eq!(first.await.unwrap(), zero_balance());
                kit.preferences.set(format!("node_{}", Chain::BitcoinSv.as_ref()), ALTERNATE_NODE.into()).unwrap();
                assert_eq!(second.await, Err(BsvHomeServiceError::Cancelled));
                assert_eq!(kit.pool.refresh_display_balance(kit.wallet.id.clone()).await.unwrap(), zero_balance());
            } else {
                kit.preferences.set(format!("node_{}", Chain::BitcoinSv.as_ref()), ALTERNATE_NODE.into()).unwrap();
                let (entered, replacement_release) = kit.provider.pause(RequestKind::Balance);
                let mut replacement = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
                assert!(poll!(replacement.as_mut()).is_pending());
                release.send(()).unwrap();
                assert!(first.await.is_err());
                assert!(second.await.is_err());
                await_request(replacement.as_mut(), entered).await;
                assert_eq!(kit.provider.counts(), (2, 2));
                replacement_release.send(()).unwrap();
                assert_eq!(replacement.await.unwrap(), zero_balance());
            }
            assert_eq!(kit.provider.counts(), (2, 2));
            assert_eq!(
                *kit.provider.observed.lock().unwrap(),
                vec![
                    (RequestKind::History, DEFAULT_NODE.into()),
                    (RequestKind::Balance, DEFAULT_NODE.into()),
                    (RequestKind::History, ALTERNATE_NODE.into()),
                    (RequestKind::Balance, ALTERNATE_NODE.into()),
                ]
            );
        }
    });
}

#[test]
fn test_receipt_resync_prevents_a_captured_display_balance_from_overwriting_new_values() {
    block_on(async {
        let kit = RefreshTestkit::new();
        let (entered, release) = kit.provider.pause(RequestKind::Balance);
        let mut display = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        await_request(display.as_mut(), entered).await;
        assert_eq!(kit.provider.counts(), (1, 1));

        kit.provider.inner.add_output(&kit.wallet.accounts[0].address, &"1".repeat(64), 5_000, true);
        kit.pool.invalidate_receipt_refresh(kit.wallet.id.clone()).unwrap();
        let expected = AssetBalance::new(Chain::BitcoinSv.as_asset_id(), 5_000u32.into());
        let mut receipt = Box::pin(kit.pool.refresh_receipt_balance(kit.wallet.id.clone()));
        let receipt_result = poll!(receipt.as_mut());

        release.send(()).unwrap();
        assert_eq!(display.await.unwrap(), expected.clone());
        let receipt = match receipt_result {
            Poll::Ready(result) => result,
            Poll::Pending => receipt.await,
        };
        assert_eq!(receipt.unwrap(), expected);
        assert_eq!(kit.provider.counts(), (1, 2));
    });
}

#[test]
fn test_receipt_balance_does_not_join_a_display_response_captured_before_the_receipt() {
    block_on(async {
        let kit = RefreshTestkit::new();
        let (entered, release) = kit.provider.pause(RequestKind::Balance);
        let mut display = Box::pin(kit.pool.refresh_display_balance(kit.wallet.id.clone()));
        await_request(display.as_mut(), entered).await;
        let hash = "1".repeat(64);
        kit.provider.inner.add_output(&kit.wallet.accounts[0].address, &hash, 5_000, true);

        let expected = AssetBalance::new(Chain::BitcoinSv.as_asset_id(), 5_000u32.into());
        assert_eq!(kit.pool.receipt_balance(kit.wallet.id.clone(), &hash).await.unwrap(), Some(expected.clone()));
        release.send(()).unwrap();
        assert_eq!(display.await.unwrap(), zero_balance());
        assert_eq!(kit.provider.counts(), (1, 2));
        assert_eq!(kit.pool.refresh_display_balance(kit.wallet.id.clone()).await.unwrap(), expected);
        assert_eq!(kit.provider.counts(), (2, 3));
    });
}
