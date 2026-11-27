pub mod model;
pub mod rules;
pub mod store;
#[cfg(test)]
pub(crate) mod testkit;
pub mod tracker;

use crate::services::error::BsvHomeServiceError;
use crate::services::failures::record;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use primitives::{Asset, AssetId, JobConfiguration, Transaction, TransactionId, TransactionState, TransactionUpdate, Wallet, WalletId};

pub use model::{BsvHomePendingTransaction, BsvHomePostProcessingFailure, BsvHomePostProcessingStep, BsvHomeTransactionStateResult, BsvHomeTransactionStateUpdate};
pub use store::BsvHomeTransactionStateStore;
use tracker::{BsvHomeTransactionUpdater, Tracking, poll};

use crate::gateway::BsvHomeGateway;
use crate::services::assets::BsvHomeAssetsService;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::nft::BsvHomeNftService;
use crate::services::stake::BsvHomeStakeService;

#[uniffi::export(with_foreign)]
pub trait BsvHomeTransactionStatusService: Send + Sync {
    fn track(&self, wallet_id: WalletId, transactions: Vec<Transaction>);
}

#[derive(uniffi::Object)]
pub struct BsvHomeTransactionStateService {
    gateway: Arc<BsvHomeGateway>,
    store: Arc<dyn BsvHomeTransactionStateStore>,
    assets: Arc<BsvHomeAssetsService>,
    balance: Arc<BsvHomeBalanceService>,
    stake: Arc<BsvHomeStakeService>,
    nft: Arc<BsvHomeNftService>,
    tracking: Tracking,
}

#[uniffi::export]
impl BsvHomeTransactionStateService {
    #[uniffi::constructor]
    pub fn new(
        gateway: Arc<BsvHomeGateway>,
        store: Arc<dyn BsvHomeTransactionStateStore>,
        assets: Arc<BsvHomeAssetsService>,
        balance: Arc<BsvHomeBalanceService>,
        stake: Arc<BsvHomeStakeService>,
        nft: Arc<BsvHomeNftService>,
    ) -> Self {
        Self {
            gateway,
            store,
            assets,
            balance,
            stake,
            nft,
            tracking: Tracking::default(),
        }
    }

    pub async fn track_pending(&self) -> Result<(), BsvHomeServiceError> {
        let generation = self.tracking_generation();
        self.tracking.check_generation(generation)?;
        let pending = self.store.get_pending_transactions().await?;
        self.tracking.check_generation(generation)?;
        let tracked = pending
            .into_iter()
            .filter(|pending| self.gateway.allows_asset(&pending.transaction.asset_id))
            .map(|pending| self.track_transaction(pending.wallet.id, pending.transaction, generation));
        futures::future::join_all(tracked).await;
        self.tracking.check_generation(generation)
    }

    pub async fn track(&self, wallet_id: WalletId, transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError> {
        self.track_with_generation(wallet_id, transactions, self.tracking_generation()).await
    }

    pub fn tracking_generation(&self) -> u64 {
        self.tracking.generation()
    }

    pub async fn track_with_generation(&self, wallet_id: WalletId, transactions: Vec<Transaction>, generation: u64) -> Result<(), BsvHomeServiceError> {
        self.tracking.check_generation(generation)?;
        for transaction in &transactions {
            self.gateway.require_asset(&transaction.asset_id)?;
        }
        self.enable_transaction_assets(wallet_id.clone(), transactions.clone()).await?;
        self.tracking.check_generation(generation)?;
        let tracked = transactions
            .into_iter()
            .map(|transaction| self.track_transaction(wallet_id.clone(), transaction, generation));
        futures::future::join_all(tracked).await;
        self.tracking.check_generation(generation)
    }

    pub fn stop_tracking(&self) {
        self.tracking.cancel();
    }

    pub fn pause_tracking(&self) {
        self.tracking.pause();
    }

    pub fn resume_tracking(&self) {
        self.tracking.resume();
    }

    pub async fn add_notification_transaction(&self, wallet: Wallet, asset_id: AssetId, transaction: Transaction) -> Result<Option<Asset>, BsvHomeServiceError> {
        let Some(asset) = self.assets.open_wallet_asset(wallet.clone(), asset_id).await? else {
            return Ok(None);
        };
        self.add_transactions(wallet.id, vec![transaction]).await?;
        Ok(Some(asset))
    }
}

impl BsvHomeTransactionStateService {
    pub(crate) async fn publish_bsv_receipt(&self, wallet_id: WalletId, transaction: Transaction) -> Result<Option<Transaction>, BsvHomeServiceError> {
        self.gateway.require_asset(&transaction.asset_id)?;
        let previous = self.store.get_state(wallet_id.clone(), transaction.id.clone()).await?;
        if previous.is_some_and(|state| state == transaction.state || !rules::can_replace_stored_transaction(transaction.asset_id.chain, state, transaction.state)) {
            return Ok(None);
        }
        self.store.add_transactions(wallet_id, vec![transaction.clone()]).await?;
        Ok(Some(transaction))
    }

    pub async fn add_transactions(&self, wallet_id: WalletId, transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError> {
        self.store.add_transactions(wallet_id, transactions).await
    }

    pub async fn enable_transaction_assets(&self, wallet_id: WalletId, transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError> {
        let asset_ids = rules::assets_to_enable(&transactions)
            .into_iter()
            .filter(|asset_id| self.gateway.allows_asset(asset_id))
            .collect::<Vec<_>>();
        if asset_ids.is_empty() {
            return Ok(());
        }
        self.balance.set_assets_enabled(wallet_id, asset_ids, true).await
    }

    pub async fn update(&self, wallet_id: WalletId, transaction: Transaction) -> Result<Option<BsvHomeTransactionStateResult>, BsvHomeServiceError> {
        self.gateway.require_asset(&transaction.asset_id)?;
        let update = self.gateway.get_transaction_update(transaction.clone()).await.map_err(|error| error.to_string());
        let previous_state = transaction.state;
        let result = apply(self.store.as_ref(), wallet_id.clone(), transaction.clone(), update, Utc::now()).await?;
        let Some(mut result) = result else {
            return Ok(None);
        };
        result.failures = self.post_process(wallet_id, &transaction, previous_state, result.state).await;
        Ok(Some(result))
    }

    async fn track_transaction(&self, wallet_id: WalletId, transaction: Transaction, generation: u64) {
        let Some(tracked) = self.tracking.start(&transaction.id, generation) else {
            return;
        };
        let configuration = JobConfiguration::transaction_state(transaction.asset_id.chain);
        poll(self, self.store.as_ref(), tracked, configuration, wallet_id, transaction).await;
    }

    async fn post_process(&self, wallet_id: WalletId, transaction: &Transaction, previous_state: TransactionState, state: TransactionState) -> Vec<BsvHomePostProcessingFailure> {
        let Some(processing) = rules::post_processing(transaction, previous_state, state) else {
            return Vec::new();
        };
        let mut failures = Vec::new();
        record(
            &mut failures,
            BsvHomePostProcessingStep::Balances,
            self.balance.update(wallet_id.clone(), processing.balance_asset_ids),
        )
        .await;
        for chain in processing.stake_chains {
            record(
                &mut failures,
                BsvHomePostProcessingStep::Stake,
                self.stake.sync_wallet(wallet_id.clone(), chain, transaction.from.clone()),
            )
            .await;
        }
        for asset_id in processing.earn_asset_ids {
            record(
                &mut failures,
                BsvHomePostProcessingStep::Earn,
                self.stake.sync_earn_wallet(wallet_id.clone(), asset_id, transaction.from.clone()),
            )
            .await;
        }
        if processing.sync_nfts {
            record(&mut failures, BsvHomePostProcessingStep::Nfts, async { self.nft.sync_wallet(wallet_id).await.map(|_| ()) }).await;
        }
        failures
    }
}

async fn apply(
    store: &dyn BsvHomeTransactionStateStore,
    wallet_id: WalletId,
    transaction: Transaction,
    update: Result<TransactionUpdate, String>,
    now: DateTime<Utc>,
) -> Result<Option<BsvHomeTransactionStateResult>, BsvHomeServiceError> {
    let timed_out = rules::has_timed_out(&transaction, now);
    let update = match update {
        Ok(update) => update,
        Err(_) if timed_out => TransactionUpdate::new_state(TransactionState::Failed),
        Err(msg) => return Err(BsvHomeServiceError::Gateway { msg }),
    };
    let (transaction_id, current_state) = match rules::new_hash(&update.changes) {
        Some(hash) => {
            let new_transaction_id = TransactionId::new(transaction.id.chain, hash.clone());
            store.update_transaction_hash(wallet_id.clone(), transaction.id.clone(), hash).await?;
            let Some(state) = store.get_state(wallet_id.clone(), new_transaction_id.clone()).await? else {
                return Ok(None);
            };
            (new_transaction_id, state)
        }
        None => (transaction.id.clone(), transaction.state),
    };
    let next_state = match current_state.merged_with(update.state) {
        state if timed_out && !state.is_completed() => TransactionState::Failed,
        state => state,
    };
    let fields = rules::state_update(next_state, &update.changes).map_err(|error| BsvHomeServiceError::Core { msg: error.to_string() })?;
    if next_state == current_state && !fields.has_field_changes() {
        let state = store.get_state(wallet_id, transaction_id.clone()).await?;
        return Ok(state.map(|state| BsvHomeTransactionStateResult {
            transaction_id,
            state,
            failures: Vec::new(),
        }));
    }
    if !store.update_transaction(wallet_id, transaction_id.clone(), fields).await? {
        return Ok(None);
    }
    Ok(Some(BsvHomeTransactionStateResult {
        transaction_id,
        state: next_state,
        failures: Vec::new(),
    }))
}

#[async_trait]
impl BsvHomeTransactionUpdater for BsvHomeTransactionStateService {
    async fn update(&self, wallet_id: WalletId, transaction: Transaction) -> Result<Option<BsvHomeTransactionStateResult>, BsvHomeServiceError> {
        BsvHomeTransactionStateService::update(self, wallet_id, transaction).await
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::{MemoryTransactionStateStore, TrackingTestkit};
    use super::*;
    use std::future::Future;
    use std::task::{Context, Poll};

    use futures::channel::oneshot;
    use futures::executor::block_on;
    use num_bigint::BigUint;
    use primitives::{AssetId, Chain, TransactionChange, TransactionMetadata, TransactionSwapMetadata, TransactionType};

    fn swap_metadata(to_value: BigUint) -> TransactionSwapMetadata {
        TransactionSwapMetadata {
            from_asset: AssetId::from_chain(Chain::Ethereum),
            from_value: BigUint::parse_bytes(b"1000000000000000000", 10).unwrap(),
            to_asset: AssetId::from_chain(Chain::Bitcoin),
            to_value,
            provider: Some("thorchain".into()),
        }
    }

    fn transaction(hash: &str, state: TransactionState, created_at: DateTime<Utc>) -> Transaction {
        Transaction::new(
            hash.into(),
            AssetId::from_chain(Chain::Ethereum),
            "from".into(),
            "to".into(),
            None,
            TransactionType::Swap,
            state,
            BigUint::from(1u64),
            AssetId::from_chain(Chain::Ethereum),
            BigUint::from(1000000000000000000u64),
            None,
            serde_json::to_value(swap_metadata(BigUint::parse_bytes(b"10000000000000000000", 10).unwrap())).ok(),
            created_at,
        )
    }

    fn update(state: TransactionState, changes: Vec<TransactionChange>) -> Result<TransactionUpdate, String> {
        Ok(TransactionUpdate::new(state, changes))
    }

    fn apply_update(
        store: &MemoryTransactionStateStore,
        transaction: Transaction,
        update: Result<TransactionUpdate, String>,
        now: DateTime<Utc>,
    ) -> Result<Option<BsvHomeTransactionStateResult>, BsvHomeServiceError> {
        futures::executor::block_on(apply(store, WalletId::Multicoin("wallet".into()), transaction, update, now))
    }

    fn id(hash: &str) -> TransactionId {
        TransactionId::new(Chain::Ethereum, hash.into())
    }

    #[test]
    fn test_paused_lifecycle_rejects_fresh_callbacks_and_invalidates_background_tickets_on_resume() {
        block_on(async {
            let kit = TrackingTestkit::new().await;
            kit.service.pause_tracking();
            let background = kit.service.tracking_generation();
            assert_eq!(
                kit.service.track_with_generation(Wallet::mock().id, vec![], background).await,
                Err(BsvHomeServiceError::Cancelled)
            );
            kit.service.resume_tracking();
            let foreground = kit.service.tracking_generation();
            assert_ne!(foreground, background);
            kit.service.resume_tracking();
            assert_eq!(kit.service.tracking_generation(), foreground);
            assert_eq!(
                kit.service.track_with_generation(Wallet::mock().id, vec![], background).await,
                Err(BsvHomeServiceError::Cancelled)
            );
            assert_eq!(kit.service.track_with_generation(Wallet::mock().id, vec![], foreground).await, Ok(()));
            kit.service.stop_tracking();
            assert_eq!(kit.service.track(Wallet::mock().id, vec![]).await, Ok(()));
        });
    }

    #[test]
    fn test_late_history_status_callback_stays_paused_and_foreground_pending_tracking_recovers() {
        block_on(async {
            let kit = TrackingTestkit::new().await;
            let wallet = Wallet::mock();
            let pending = transaction("late-history", TransactionState::Pending, Utc::now());
            let history = kit.history(wallet.clone());
            let (release, gate) = oneshot::channel();
            *history.store.gate.lock().unwrap() = Some(gate);
            let mut sync = Box::pin(history.service.save_address_history(wallet.id.clone(), vec![pending.clone()]));
            let waker = futures::task::noop_waker();
            let mut context = Context::from_waker(&waker);
            assert!(sync.as_mut().poll(&mut context).is_pending());
            kit.service.pause_tracking();
            release.send(()).unwrap();
            assert_eq!(sync.await, Ok(()));
            assert_eq!(history.store.saved.transactions.lock().unwrap().as_slice(), &[pending.clone()]);
            let callbacks = history.status.callbacks.lock().unwrap().clone();
            assert_eq!(callbacks.len(), 1);
            let (wallet_id, transactions, background) = callbacks[0].clone();
            assert_eq!(background, kit.service.tracking_generation());
            let mut callback = Box::pin(kit.service.track_with_generation(wallet_id.clone(), transactions.clone(), background));
            assert_eq!(callback.as_mut().poll(&mut context), Poll::Ready(Err(BsvHomeServiceError::Cancelled)));
            drop(callback);
            assert_eq!(kit.service.track_pending().await, Err(BsvHomeServiceError::Cancelled));
            assert_eq!(*kit.balances.enabled_calls.lock().unwrap(), 0);
            assert_eq!(kit.provider.requested_paths(), Vec::<String>::new());
            kit.service.resume_tracking();
            let foreground = kit.service.tracking_generation();
            assert_ne!(foreground, background);
            assert_eq!(
                kit.service.track_with_generation(wallet_id, transactions, background).await,
                Err(BsvHomeServiceError::Cancelled)
            );
            let mut polling = Box::pin(kit.service.track_pending());
            assert!(polling.as_mut().poll(&mut context).is_pending());
            assert_eq!(kit.service.tracking.check_generation(foreground), Ok(()));
            assert_eq!(kit.service.tracking.start(&pending.id, foreground).is_none(), true);
            drop(polling);
            assert_eq!(kit.service.tracking.start(&pending.id, foreground).is_some(), true);
            assert_eq!(kit.provider.requested_paths(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_stop_tracking_during_asset_enable_does_not_restart_polling() {
        block_on(async {
            let kit = TrackingTestkit::new().await;
            let (release, gate) = oneshot::channel();
            *kit.balances.enabled_gate.lock().unwrap() = Some(gate);
            let mut tracking = Box::pin(kit.service.track(Wallet::mock().id, vec![transaction("hash", TransactionState::Pending, Utc::now())]));
            let waker = futures::task::noop_waker();
            let mut context = Context::from_waker(&waker);
            assert!(tracking.as_mut().poll(&mut context).is_pending());
            kit.service.stop_tracking();
            release.send(()).unwrap();
            assert_eq!(tracking.as_mut().poll(&mut context), Poll::Ready(Err(BsvHomeServiceError::Cancelled)));
            assert_eq!(kit.provider.requested_paths(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_stop_tracking_during_pending_read_does_not_register_a_poll() {
        block_on(async {
            let kit = TrackingTestkit::new().await;
            let (release, gate) = oneshot::channel();
            *kit.store.pending_gate.lock().unwrap() = Some(gate);
            kit.store.pending.lock().unwrap().push(BsvHomePendingTransaction {
                wallet: Wallet::mock(),
                transaction: transaction("hash", TransactionState::Pending, Utc::now()),
            });
            let mut tracking = Box::pin(kit.service.track_pending());
            let waker = futures::task::noop_waker();
            let mut context = Context::from_waker(&waker);
            assert!(tracking.as_mut().poll(&mut context).is_pending());
            kit.service.stop_tracking();
            release.send(()).unwrap();
            assert_eq!(tracking.as_mut().poll(&mut context), Poll::Ready(Err(BsvHomeServiceError::Cancelled)));
            assert_eq!(kit.provider.requested_paths(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_tracking_generation_rejects_a_queued_callback_but_allows_a_new_foreground_generation() {
        block_on(async {
            let kit = TrackingTestkit::new().await;
            let generation = kit.service.tracking_generation();
            let queued = kit
                .service
                .track_with_generation(Wallet::mock().id, vec![transaction("hash", TransactionState::Pending, Utc::now())], generation);
            kit.service.stop_tracking();
            assert_eq!(queued.await, Err(BsvHomeServiceError::Cancelled));
            assert_eq!(*kit.balances.enabled_calls.lock().unwrap(), 0);
            assert_eq!(kit.provider.requested_paths(), Vec::<String>::new());
            assert_ne!(kit.service.tracking_generation(), generation);
            assert_eq!(
                kit.service.track_with_generation(Wallet::mock().id, vec![], kit.service.tracking_generation()).await,
                Ok(())
            );
        });
    }

    #[test]
    fn test_in_transit_saves_metadata_and_keeps_polling() {
        let now = Utc::now();
        let store = MemoryTransactionStateStore::with(vec![(id("hash"), TransactionState::Pending)]);

        let result = apply_update(
            &store,
            transaction("hash", TransactionState::Pending, now),
            update(
                TransactionState::InTransit,
                vec![TransactionChange::Metadata(TransactionMetadata::Swap(swap_metadata(
                    BigUint::parse_bytes(b"9900000000000000000", 10).unwrap(),
                )))],
            ),
            now,
        )
        .unwrap()
        .unwrap();

        assert_eq!(result.state, TransactionState::InTransit);
        let (_, saved) = store.updates.lock().unwrap()[0].clone();
        assert_eq!(saved.state, TransactionState::InTransit);
        assert!(saved.metadata.unwrap().contains("9900000000000000000"));
    }

    #[test]
    fn test_hash_change_renames_when_no_existing_row() {
        let now = Utc::now();
        let store = MemoryTransactionStateStore::with(vec![(id("hash"), TransactionState::Pending)]);

        let result = apply_update(
            &store,
            transaction("hash", TransactionState::Pending, now),
            update(
                TransactionState::InTransit,
                vec![TransactionChange::HashChange {
                    old: "hash".into(),
                    new: "new-hash".into(),
                }],
            ),
            now,
        )
        .unwrap()
        .unwrap();

        assert_eq!(result.transaction_id, id("new-hash"));
        assert_eq!(store.hash_updates.lock().unwrap().as_slice(), &[(id("hash"), id("new-hash"))]);
        assert_eq!(store.updates.lock().unwrap()[0].0, id("new-hash"));
    }

    #[test]
    fn test_hash_change_merges_into_existing_row_without_downgrade() {
        let now = Utc::now();
        let store = MemoryTransactionStateStore::with(vec![(id("hash"), TransactionState::Pending), (id("new-hash"), TransactionState::Confirmed)]);

        let result = apply_update(
            &store,
            transaction("hash", TransactionState::Pending, now),
            update(
                TransactionState::InTransit,
                vec![TransactionChange::HashChange {
                    old: "hash".into(),
                    new: "new-hash".into(),
                }],
            ),
            now,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            result,
            BsvHomeTransactionStateResult {
                transaction_id: id("new-hash"),
                state: TransactionState::Confirmed,
                failures: Vec::new(),
            }
        );
        assert_eq!(store.deleted.lock().unwrap().as_slice(), &[]);
        assert_eq!(store.hash_updates.lock().unwrap().as_slice(), &[(id("hash"), id("new-hash"))]);
        assert_eq!(store.states.lock().unwrap().as_slice(), &[(id("new-hash"), TransactionState::Confirmed)]);
        assert!(store.updates.lock().unwrap().is_empty());
    }

    #[test]
    fn test_hash_change_is_idempotent() {
        let now = Utc::now();
        let store = MemoryTransactionStateStore::with(vec![(id("hash"), TransactionState::Pending)]);
        for hash in ["hash", "new-hash", "new-hash"] {
            let result = apply_update(
                &store,
                transaction("hash", TransactionState::Pending, now),
                update(
                    TransactionState::Confirmed,
                    vec![TransactionChange::HashChange {
                        old: "hash".into(),
                        new: hash.into(),
                    }],
                ),
                now,
            )
            .unwrap()
            .unwrap();

            assert_eq!(result.transaction_id, id(hash));
            assert_eq!(result.state, TransactionState::Confirmed);
            assert_eq!(store.states.lock().unwrap().as_slice(), &[(id(hash), TransactionState::Confirmed)]);
            assert_eq!(store.deleted.lock().unwrap().as_slice(), &[]);
        }
    }

    #[test]
    fn test_in_transit_is_not_downgraded_to_pending() {
        let now = Utc::now();
        let store = MemoryTransactionStateStore::with(vec![(id("hash"), TransactionState::InTransit)]);

        let result = apply_update(
            &store,
            transaction("hash", TransactionState::InTransit, now),
            update(TransactionState::Pending, vec![]),
            now,
        )
        .unwrap()
        .unwrap();

        assert_eq!(result.state, TransactionState::InTransit);
        assert!(store.updates.lock().unwrap().is_empty());
    }

    #[test]
    fn test_removed_row_stops_polling() {
        let now = Utc::now();
        let store = MemoryTransactionStateStore::with(vec![]);

        for changes in [
            vec![],
            vec![TransactionChange::HashChange {
                old: "hash".into(),
                new: "new-hash".into(),
            }],
        ] {
            let result = apply_update(
                &store,
                transaction("hash", TransactionState::Pending, now),
                update(TransactionState::Confirmed, changes),
                now,
            )
            .unwrap();

            assert_eq!(result, None);
        }
    }

    #[test]
    fn test_status_failure_fails_only_after_timeout() {
        let now = Utc::now();
        let store = MemoryTransactionStateStore::with(vec![(id("hash"), TransactionState::Pending)]);

        let fresh = apply_update(&store, transaction("hash", TransactionState::Pending, now), Err("offline".into()), now);
        assert!(matches!(fresh, Err(BsvHomeServiceError::Gateway { .. })));

        let stale = apply_update(
            &store,
            transaction("hash", TransactionState::Pending, now - chrono::Duration::hours(2)),
            Err("offline".into()),
            now,
        )
        .unwrap()
        .unwrap();
        assert_eq!(stale.state, TransactionState::Failed);
    }

    #[test]
    fn test_post_processing_by_state_transition() {
        let now = Utc::now();
        let swap = transaction("hash", TransactionState::Pending, now);

        assert_eq!(rules::post_processing(&swap, TransactionState::Pending, TransactionState::Pending), None);
        assert_eq!(rules::post_processing(&swap, TransactionState::InTransit, TransactionState::InTransit), None);

        let in_transit = rules::post_processing(&swap, TransactionState::Pending, TransactionState::InTransit).unwrap();
        assert_eq!(in_transit.balance_asset_ids.len(), 2);
        assert!(in_transit.stake_chains.is_empty() && in_transit.earn_asset_ids.is_empty() && !in_transit.sync_nfts);

        let mut stake = transaction("hash", TransactionState::Pending, now);
        stake.transaction_type = TransactionType::StakeFreeze;
        stake.metadata = None;
        let completed = rules::post_processing(&stake, TransactionState::Pending, TransactionState::Confirmed).unwrap();
        assert_eq!(completed.stake_chains, vec![Chain::Ethereum]);
        assert_eq!(completed.balance_asset_ids, vec![AssetId::from_chain(Chain::Ethereum)]);

        let mut nft = stake.clone();
        nft.transaction_type = TransactionType::TransferNFT;
        assert!(rules::post_processing(&nft, TransactionState::Pending, TransactionState::Failed).unwrap().sync_nfts);

        let mut earn = stake.clone();
        earn.transaction_type = TransactionType::EarnDeposit;
        let completed = rules::post_processing(&earn, TransactionState::Pending, TransactionState::Confirmed).unwrap();
        assert_eq!(completed.earn_asset_ids, vec![AssetId::from_chain(Chain::Ethereum)]);
        assert!(completed.stake_chains.is_empty());
    }

    #[test]
    fn test_in_transit_timeout_uses_destination_chain() {
        let now = Utc::now();
        let created_at = now - chrono::Duration::hours(2);
        let in_transit = transaction("hash", TransactionState::InTransit, created_at);

        assert_eq!(rules::destination_chain(&in_transit), Some(Chain::Bitcoin));
        assert!(!rules::has_timed_out(&in_transit, now));
        assert!(rules::has_timed_out(&transaction("hash", TransactionState::Pending, created_at), now));
        assert!(!rules::has_timed_out(
            &transaction("hash", TransactionState::Confirmed, now - chrono::Duration::days(30)),
            now
        ));
    }

    #[test]
    fn test_assets_to_enable_skips_hypercore_and_duplicates() {
        let swap = transaction("swap", TransactionState::Pending, Utc::now());
        let hypercore = Transaction::new(
            "perpetual".into(),
            AssetId::from_chain(Chain::HyperCore),
            "from".into(),
            "to".into(),
            None,
            TransactionType::Transfer,
            TransactionState::Pending,
            BigUint::from(1u64),
            AssetId::from_chain(Chain::HyperCore),
            BigUint::from(1u64),
            None,
            None,
            Utc::now(),
        );
        let asset_ids = rules::assets_to_enable(&[swap.clone(), swap, hypercore]);
        assert_eq!(asset_ids.len(), 2);
        assert!(asset_ids.contains(&AssetId::from_chain(Chain::Ethereum)));
        assert!(asset_ids.contains(&AssetId::from_chain(Chain::Bitcoin)));
    }
}
