use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use primitives::{JobConfiguration, Transaction, TransactionId, WalletId};

use crate::services::clock::sleep;
use crate::services::error::BsvHomeServiceError;

use super::model::BsvHomeTransactionStateResult;
use super::store::BsvHomeTransactionStateStore;

#[async_trait]
pub trait BsvHomeTransactionUpdater: Send + Sync {
    async fn update(&self, wallet_id: WalletId, transaction: Transaction) -> Result<Option<BsvHomeTransactionStateResult>, BsvHomeServiceError>;
}

#[derive(Default)]
pub struct Tracking {
    state: Mutex<TrackingState>,
}

#[derive(Default)]
struct TrackingState {
    phase: Phase,
    generation: u64,
    last_poll: u64,
    polls: HashMap<TransactionId, u64>,
}

#[derive(Default, PartialEq)]
enum Phase {
    #[default]
    Active,
    Paused,
}

pub struct TrackedTransactions<'a> {
    tracking: &'a Tracking,
    poll: u64,
}

impl Tracking {
    pub fn generation(&self) -> u64 {
        self.state.lock().unwrap().generation
    }

    pub fn check_generation(&self, generation: u64) -> Result<(), BsvHomeServiceError> {
        let state = self.state.lock().map_err(|_| BsvHomeServiceError::Cancelled)?;
        if state.phase == Phase::Paused || state.generation != generation {
            return Err(BsvHomeServiceError::Cancelled);
        }
        Ok(())
    }

    pub fn start(&self, transaction_id: &TransactionId, generation: u64) -> Option<TrackedTransactions<'_>> {
        let mut state = self.state.lock().ok()?;
        if state.phase == Phase::Paused || state.generation != generation || state.polls.contains_key(transaction_id) {
            return None;
        }
        state.last_poll += 1;
        let poll = state.last_poll;
        state.polls.insert(transaction_id.clone(), poll);
        Some(TrackedTransactions { tracking: self, poll })
    }

    pub fn cancel(&self) {
        let mut state = self.state.lock().unwrap();
        state.generation = state.generation.wrapping_add(1);
        state.polls.clear();
    }

    pub fn pause(&self) {
        let Ok(mut state) = self.state.lock() else { return };
        state.phase = Phase::Paused;
        state.generation = state.generation.wrapping_add(1);
        state.polls.clear();
    }

    pub fn resume(&self) {
        let Ok(mut state) = self.state.lock() else { return };
        match state.phase {
            Phase::Active => {}
            Phase::Paused => {
                state.phase = Phase::Active;
                state.generation = state.generation.wrapping_add(1);
            }
        }
    }
}

impl TrackedTransactions<'_> {
    fn is_tracking(&self) -> bool {
        self.tracking.state.lock().unwrap().polls.values().any(|poll| *poll == self.poll)
    }

    fn follow(&self, transaction_id: &TransactionId) {
        let mut state = self.tracking.state.lock().unwrap();
        if state.polls.values().any(|poll| *poll == self.poll) {
            state.polls.insert(transaction_id.clone(), self.poll);
        }
    }
}

impl Drop for TrackedTransactions<'_> {
    fn drop(&mut self) {
        self.tracking.state.lock().unwrap().polls.retain(|_, poll| *poll != self.poll);
    }
}

pub async fn poll(
    updater: &dyn BsvHomeTransactionUpdater,
    store: &dyn BsvHomeTransactionStateStore,
    tracked: TrackedTransactions<'_>,
    configuration: JobConfiguration,
    wallet_id: WalletId,
    transaction: Transaction,
) {
    let mut current = transaction;
    let mut interval = configuration.initial_interval_ms;

    loop {
        sleep(Duration::from_millis(u64::from(interval))).await;
        interval = configuration.next_interval_ms(interval);
        if !tracked.is_tracking() {
            break;
        }
        let result = match updater.update(wallet_id.clone(), current.clone()).await {
            Ok(Some(result)) => result,
            Ok(None) => break,
            Err(_) => continue,
        };
        if result.transaction_id != current.id {
            tracked.follow(&result.transaction_id);
        }
        let stored = store.get_transaction(wallet_id.clone(), result.transaction_id.clone()).await;
        let Ok(Some(pending)) = stored else {
            break;
        };
        current = pending.transaction;
        if current.state.is_completed() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::{AssetId, Chain, TransactionState, Wallet};
    use std::future::Future;
    use std::task::Context;

    use crate::services::transaction_state::model::{BsvHomePendingTransaction, BsvHomeTransactionStateUpdate};

    #[derive(Default)]
    struct StubStore {
        transactions: Mutex<Vec<Transaction>>,
    }

    #[async_trait]
    impl BsvHomeTransactionStateStore for StubStore {
        async fn get_pending_transactions(&self) -> Result<Vec<BsvHomePendingTransaction>, BsvHomeServiceError> {
            Ok(Vec::new())
        }

        async fn get_transaction(&self, _wallet_id: WalletId, transaction_id: TransactionId) -> Result<Option<BsvHomePendingTransaction>, BsvHomeServiceError> {
            let transactions = self.transactions.lock().unwrap();
            Ok(transactions
                .iter()
                .find(|transaction| transaction.id == transaction_id)
                .map(|transaction| BsvHomePendingTransaction {
                    wallet: wallet(),
                    transaction: transaction.clone(),
                }))
        }

        async fn add_transactions(&self, _wallet_id: WalletId, _transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError> {
            Ok(())
        }

        async fn get_state(&self, _wallet_id: WalletId, _transaction_id: TransactionId) -> Result<Option<TransactionState>, BsvHomeServiceError> {
            Ok(None)
        }

        async fn update_transaction_hash(&self, _wallet_id: WalletId, _transaction_id: TransactionId, _hash: String) -> Result<(), BsvHomeServiceError> {
            Ok(())
        }

        async fn delete_transaction(&self, _wallet_id: WalletId, _transaction_id: TransactionId) -> Result<(), BsvHomeServiceError> {
            Ok(())
        }

        async fn update_transaction(&self, _wallet_id: WalletId, _transaction_id: TransactionId, _update: BsvHomeTransactionStateUpdate) -> Result<bool, BsvHomeServiceError> {
            Ok(true)
        }
    }

    #[derive(Default)]
    struct StubUpdater {
        results: Mutex<Vec<Result<Option<BsvHomeTransactionStateResult>, BsvHomeServiceError>>>,
        requested: Mutex<Vec<TransactionId>>,
    }

    #[async_trait]
    impl BsvHomeTransactionUpdater for StubUpdater {
        async fn update(&self, _wallet_id: WalletId, transaction: Transaction) -> Result<Option<BsvHomeTransactionStateResult>, BsvHomeServiceError> {
            self.requested.lock().unwrap().push(transaction.id.clone());
            let mut results = self.results.lock().unwrap();
            if results.is_empty() {
                return Ok(None);
            }
            results.remove(0)
        }
    }

    fn wallet() -> Wallet {
        Wallet {
            id: WalletId::Multicoin("wallet".into()),
            ..Wallet::mock()
        }
    }

    fn configuration() -> JobConfiguration {
        JobConfiguration {
            initial_interval_ms: 1,
            max_interval_ms: 1,
            step_factor: 1.0,
        }
    }

    fn transaction(hash: &str, state: TransactionState) -> Transaction {
        let mut transaction = Transaction::mock();
        transaction.id = TransactionId::new(Chain::Ethereum, hash.to_string());
        transaction.asset_id = AssetId::from_chain(Chain::Ethereum);
        transaction.state = state;
        transaction
    }

    fn state_result(transaction_id: &TransactionId, state: TransactionState) -> BsvHomeTransactionStateResult {
        BsvHomeTransactionStateResult {
            transaction_id: transaction_id.clone(),
            state,
            failures: Vec::new(),
        }
    }

    fn run(updater: &StubUpdater, store: &StubStore, tracking: &Tracking, transaction: Transaction) {
        if let Some(tracked) = tracking.start(&transaction.id, tracking.generation()) {
            futures::executor::block_on(poll(updater, store, tracked, configuration(), WalletId::Multicoin("wallet".into()), transaction));
        }
    }

    #[test]
    fn test_poll_follows_a_replaced_hash_and_stops_once_confirmed() {
        let pending = transaction("hash", TransactionState::Pending);
        let replaced = transaction("new-hash", TransactionState::Confirmed);
        let store = StubStore {
            transactions: Mutex::new(vec![replaced.clone()]),
        };
        let updater = StubUpdater {
            results: Mutex::new(vec![Ok(Some(state_result(&replaced.id, TransactionState::Confirmed)))]),
            ..Default::default()
        };
        let tracking = Tracking::default();

        run(&updater, &store, &tracking, pending.clone());

        assert_eq!(*updater.requested.lock().unwrap(), vec![pending.id.clone()]);
        assert!(tracking.start(&pending.id, tracking.generation()).is_some());
        assert!(tracking.start(&replaced.id, tracking.generation()).is_some());
    }

    #[test]
    fn test_poll_retries_after_an_update_error_and_stops_when_the_transaction_is_gone() {
        let pending = transaction("hash", TransactionState::Pending);
        let updater = StubUpdater {
            results: Mutex::new(vec![Err(BsvHomeServiceError::Gateway { msg: "offline".to_string() }), Ok(None)]),
            ..Default::default()
        };
        let tracking = Tracking::default();

        run(&updater, &StubStore::default(), &tracking, pending.clone());

        assert_eq!(updater.requested.lock().unwrap().len(), 2);
        assert!(tracking.start(&pending.id, tracking.generation()).is_some());
    }

    #[test]
    fn test_poll_skips_a_transaction_that_is_already_tracked() {
        let pending = transaction("hash", TransactionState::Pending);
        let updater = StubUpdater::default();
        let tracking = Tracking::default();
        let _owner = tracking.start(&pending.id, tracking.generation()).unwrap();

        run(&updater, &StubStore::default(), &tracking, pending);

        assert!(updater.requested.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pause_closes_fresh_registration_and_resume_discards_background_generations() {
        let pending = transaction("hash", TransactionState::Pending);
        let tracking = Tracking::default();
        let initial = tracking.generation();
        let old = tracking.start(&pending.id, initial).unwrap();
        tracking.pause();
        let background = tracking.generation();
        assert_eq!(old.is_tracking(), false);
        assert_eq!(tracking.check_generation(initial), Err(BsvHomeServiceError::Cancelled));
        assert_eq!(tracking.check_generation(background), Err(BsvHomeServiceError::Cancelled));
        assert_eq!(tracking.start(&pending.id, background).is_none(), true);
        tracking.cancel();
        assert_eq!(tracking.start(&pending.id, tracking.generation()).is_none(), true);
        tracking.resume();
        let foreground = tracking.generation();
        assert_ne!(foreground, background);
        assert_eq!(tracking.check_generation(background), Err(BsvHomeServiceError::Cancelled));
        assert_eq!(tracking.check_generation(foreground), Ok(()));
        let new = tracking.start(&pending.id, foreground).unwrap();
        drop(old);
        assert_eq!(new.is_tracking(), true);
        tracking.resume();
        assert_eq!(tracking.generation(), foreground);
    }

    #[test]
    fn test_cancel_stops_the_running_poll_and_frees_its_transactions() {
        let pending = transaction("hash", TransactionState::Pending);
        let tracking = Tracking::default();
        let tracked = tracking.start(&pending.id, tracking.generation()).unwrap();

        assert!(tracking.start(&pending.id, tracking.generation()).is_none());
        assert!(tracked.is_tracking());

        tracking.cancel();

        assert!(!tracked.is_tracking());
        assert!(tracking.start(&pending.id, tracking.generation()).is_some());
    }

    #[test]
    fn test_a_poll_dropped_mid_flight_releases_the_transaction() {
        let pending = transaction("hash", TransactionState::Pending);
        let updater = StubUpdater::default();
        let store = StubStore::default();
        let tracking = Tracking::default();

        {
            let tracked = tracking.start(&pending.id, tracking.generation()).unwrap();
            let mut polling = Box::pin(poll(&updater, &store, tracked, configuration(), WalletId::Multicoin("wallet".into()), pending.clone()));
            let waker = futures::task::noop_waker();
            assert!(polling.as_mut().poll(&mut Context::from_waker(&waker)).is_pending());
            assert!(tracking.start(&pending.id, tracking.generation()).is_none(), "the poll owns the transaction while it runs");
        }

        assert!(
            tracking.start(&pending.id, tracking.generation()).is_some(),
            "a poll dropped at its first sleep must free the transaction, or it is never tracked again"
        );
    }

    #[test]
    fn test_a_poll_dropped_after_a_restart_leaves_the_new_owner_alone() {
        let pending = transaction("hash", TransactionState::Pending);
        let tracking = Tracking::default();
        let stopped = tracking.start(&pending.id, tracking.generation()).unwrap();

        tracking.cancel();
        let restarted = tracking.start(&pending.id, tracking.generation()).unwrap();
        drop(stopped);

        assert!(
            tracking.start(&pending.id, tracking.generation()).is_none(),
            "the restarted poll still owns the transaction"
        );

        drop(restarted);
        assert!(tracking.start(&pending.id, tracking.generation()).is_some());
    }

    #[test]
    fn test_cancelled_generation_cannot_register_or_follow_a_replaced_hash() {
        let pending = transaction("hash", TransactionState::Pending);
        let replaced = transaction("new-hash", TransactionState::Confirmed);
        let tracking = Tracking::default();
        let generation = tracking.generation();
        let stopped = tracking.start(&pending.id, generation).unwrap();

        tracking.cancel();
        stopped.follow(&replaced.id);

        assert!(tracking.start(&pending.id, generation).is_none());
        assert!(tracking.start(&replaced.id, tracking.generation()).is_some());
        assert!(tracking.start(&pending.id, tracking.generation()).is_some());
    }
}
