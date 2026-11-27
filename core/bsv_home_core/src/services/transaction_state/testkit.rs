use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::channel::oneshot;
use primitives::{AssetId, Chain, Transaction, TransactionId, TransactionState, Wallet, WalletId};

use super::{BsvHomePendingTransaction, BsvHomeTransactionStateService, BsvHomeTransactionStateStore, BsvHomeTransactionStateUpdate, BsvHomeTransactionStatusService};
use crate::api::{BsvHomeDeviceApiClient, BsvHomeStaticApiClient};
use crate::gateway::EmptyPreferences;
use crate::services::balance::{BsvHomeAssetBalance, BsvHomeBalanceRecord, BsvHomeBalanceService, BsvHomeBalanceStore};
use crate::services::confirm::testkit::UnusedStakeStore;
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::error::BsvHomeServiceError;
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::nft::{BsvHomeNftService, testkit::MemoryNftStore};
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::stake::BsvHomeStakeService;
use crate::services::stream::testkit::{MemoryBalanceStore, SubscriptionTestkit};
use crate::services::transactions::testkit::{MemoryTransactionStore, TransactionsTestkit};
use crate::services::transactions::{BsvHomeTransactionStore, BsvHomeTransactionsService};
use crate::services::wallet::addresses::testkit::pool;
use crate::services::wallet::testkit::{MemoryAddressStore, MemoryWalletStore};
use crate::services::wallet_preferences::{BsvHomeWalletPreferencesService, testkit::MemoryWalletPreferencesStore};
use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
use crate::testkit::TestAlienProvider;

#[derive(Default)]
pub struct MemoryTransactionStateStore {
    pub states: Mutex<Vec<(TransactionId, TransactionState)>>,
    pub updates: Mutex<Vec<(TransactionId, BsvHomeTransactionStateUpdate)>>,
    pub hash_updates: Mutex<Vec<(TransactionId, TransactionId)>>,
    pub deleted: Mutex<Vec<TransactionId>>,
    pub pending: Mutex<Vec<BsvHomePendingTransaction>>,
    pub pending_gate: Mutex<Option<oneshot::Receiver<()>>>,
}

impl MemoryTransactionStateStore {
    pub fn with(states: Vec<(TransactionId, TransactionState)>) -> Arc<Self> {
        Arc::new(Self {
            states: Mutex::new(states),
            ..Default::default()
        })
    }
}

#[async_trait::async_trait]
impl BsvHomeTransactionStateStore for MemoryTransactionStateStore {
    async fn get_pending_transactions(&self) -> Result<Vec<BsvHomePendingTransaction>, BsvHomeServiceError> {
        let gate = self.pending_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.await.unwrap();
        }
        Ok(self.pending.lock().unwrap().clone())
    }

    async fn get_transaction(&self, _wallet_id: WalletId, _transaction_id: TransactionId) -> Result<Option<BsvHomePendingTransaction>, BsvHomeServiceError> {
        Ok(None)
    }

    async fn add_transactions(&self, _wallet_id: WalletId, _transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }

    async fn get_state(&self, _wallet_id: WalletId, transaction_id: TransactionId) -> Result<Option<TransactionState>, BsvHomeServiceError> {
        Ok(self.states.lock().unwrap().iter().find(|(id, _)| *id == transaction_id).map(|(_, state)| *state))
    }
    async fn update_transaction_hash(&self, _wallet_id: WalletId, transaction_id: TransactionId, hash: String) -> Result<(), BsvHomeServiceError> {
        let new_transaction_id = TransactionId::new(transaction_id.chain, hash);
        if transaction_id == new_transaction_id {
            return Ok(());
        }
        let mut states = self.states.lock().unwrap();
        let Some(index) = states.iter().position(|(id, _)| *id == transaction_id) else {
            return Ok(());
        };
        let (_, state) = states.remove(index);
        if !states.iter().any(|(id, _)| *id == new_transaction_id) {
            states.push((new_transaction_id.clone(), state));
        }
        self.hash_updates.lock().unwrap().push((transaction_id, new_transaction_id));
        Ok(())
    }
    async fn delete_transaction(&self, _wallet_id: WalletId, transaction_id: TransactionId) -> Result<(), BsvHomeServiceError> {
        self.states.lock().unwrap().retain(|(id, _)| *id != transaction_id);
        self.deleted.lock().unwrap().push(transaction_id);
        Ok(())
    }
    async fn update_transaction(&self, _wallet_id: WalletId, transaction_id: TransactionId, update: BsvHomeTransactionStateUpdate) -> Result<bool, BsvHomeServiceError> {
        let mut states = self.states.lock().unwrap();
        let Some(entry) = states.iter_mut().find(|(id, _)| *id == transaction_id) else {
            return Ok(false);
        };
        entry.1 = update.state;
        self.updates.lock().unwrap().push((transaction_id, update));
        Ok(true)
    }
}

#[derive(Default)]
pub struct MemoryTrackingBalanceStore {
    pub enabled_gate: Mutex<Option<oneshot::Receiver<()>>>,
    pub enabled_calls: Mutex<usize>,
    balances: MemoryBalanceStore,
}

#[async_trait]
impl BsvHomeBalanceStore for MemoryTrackingBalanceStore {
    async fn get_available_balances(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>) -> Result<Vec<BsvHomeAssetBalance>, BsvHomeServiceError> {
        self.balances.get_available_balances(wallet_id, asset_ids).await
    }

    async fn update_balances(&self, wallet_id: WalletId, balances: Vec<BsvHomeBalanceRecord>) -> Result<(), BsvHomeServiceError> {
        self.balances.update_balances(wallet_id, balances).await
    }

    async fn get_enabled_asset_ids(&self, _wallet_id: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        let gate = self.enabled_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.await.unwrap();
        }
        Ok(vec![Chain::Ethereum.as_asset_id(), Chain::Bitcoin.as_asset_id()])
    }

    async fn set_assets_enabled(&self, _wallet_id: WalletId, _asset_ids: Vec<AssetId>, _enabled: bool) -> Result<(), BsvHomeServiceError> {
        *self.enabled_calls.lock().unwrap() += 1;
        Ok(())
    }

    async fn set_asset_pinned(&self, wallet_id: WalletId, asset_id: AssetId, pinned: bool) -> Result<(), BsvHomeServiceError> {
        self.balances.set_asset_pinned(wallet_id, asset_id, pinned).await
    }
}

pub struct TrackingTestkit {
    pub service: Arc<BsvHomeTransactionStateService>,
    pub balances: Arc<MemoryTrackingBalanceStore>,
    pub store: Arc<MemoryTransactionStateStore>,
    pub provider: Arc<TestAlienProvider>,
}

impl TrackingTestkit {
    pub async fn new() -> Self {
        let wallet = Wallet::mock();
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let kit = TransactionsTestkit::new(wallet.clone(), provider.clone());
        kit.assets.ensure_default_assets().await.unwrap();
        let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
        let session = Arc::new(BsvHomeWalletSessionService::new(
            Arc::new(MemoryWalletSessionStore {
                current: Mutex::new(Some(wallet.id)),
            }),
            kit.wallet_store.clone(),
        ));
        let balances = Arc::new(MemoryTrackingBalanceStore::default());
        let balance = Arc::new(BsvHomeBalanceService::new(
            kit.gateway.clone(),
            kit.wallet_store.clone(),
            kit.asset_store,
            balances.clone(),
            kit.assets.clone(),
            Arc::new(SubscriptionTestkit::new(&[], &[]).service),
            pool(kit.gateway.clone(), kit.wallet_store),
        ));
        let stake = Arc::new(BsvHomeStakeService::new(
            kit.gateway.clone(),
            Arc::new(BsvHomeStaticApiClient::new(provider.clone())),
            Arc::new(UnusedStakeStore),
            Arc::new(MemoryAddressStore::default()),
            Arc::new(BsvHomeExplorerService::new(preferences.clone())),
            preferences,
            session.clone(),
        ));
        let api = Arc::new(BsvHomeDeviceApiClient::new(
            provider.clone(),
            Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
        ));
        let store = Arc::new(MemoryTransactionStateStore::default());
        let service = Arc::new(BsvHomeTransactionStateService::new(
            kit.gateway,
            store.clone(),
            kit.assets,
            balance,
            stake,
            Arc::new(BsvHomeNftService::new(api, Arc::new(MemoryNftStore::default()), session)),
        ));
        Self {
            service,
            balances,
            store,
            provider,
        }
    }

    pub fn history(&self, wallet: Wallet) -> TrackingHistoryTestkit {
        let wallets = Arc::new(MemoryWalletStore {
            wallets: Mutex::new(vec![wallet.clone()]),
            ..Default::default()
        });
        let session = Arc::new(BsvHomeWalletSessionService::new(
            Arc::new(MemoryWalletSessionStore {
                current: Mutex::new(Some(wallet.id.clone())),
            }),
            wallets.clone(),
        ));
        let store = Arc::new(TrackingHistoryStore {
            wallet,
            pending: self.store.clone(),
            saved: MemoryTransactionStore::default(),
            gate: Mutex::new(None),
        });
        let status = Arc::new(QueuedTransactionStatus {
            service: self.service.clone(),
            callbacks: Mutex::new(Vec::new()),
        });
        let service = BsvHomeTransactionsService::new(
            Arc::new(BsvHomeDeviceApiClient::new(
                self.provider.clone(),
                Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
            )),
            self.service.gateway.clone(),
            self.service.assets.clone(),
            store.clone(),
            Arc::new(MemoryAddressStore::default()),
            Arc::new(BsvHomeWalletPreferencesService::new(Arc::new(MemoryWalletPreferencesStore::default()))),
            Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default()))),
            session,
            status.clone(),
            pool(self.service.gateway.clone(), wallets),
        );
        TrackingHistoryTestkit { service, store, status }
    }
}

pub struct TrackingHistoryTestkit {
    pub service: BsvHomeTransactionsService,
    pub store: Arc<TrackingHistoryStore>,
    pub status: Arc<QueuedTransactionStatus>,
}

pub struct TrackingHistoryStore {
    wallet: Wallet,
    pending: Arc<MemoryTransactionStateStore>,
    pub saved: MemoryTransactionStore,
    pub gate: Mutex<Option<oneshot::Receiver<()>>>,
}

#[async_trait]
impl BsvHomeTransactionStore for TrackingHistoryStore {
    async fn save_transactions(&self, wallet_id: WalletId, transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError> {
        let gate = self.gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.await.unwrap();
        }
        self.saved.save_transactions(wallet_id, transactions.clone()).await?;
        *self.pending.pending.lock().unwrap() = transactions
            .into_iter()
            .map(|transaction| BsvHomePendingTransaction {
                wallet: self.wallet.clone(),
                transaction,
            })
            .collect();
        Ok(())
    }
}

pub struct QueuedTransactionStatus {
    service: Arc<BsvHomeTransactionStateService>,
    pub callbacks: Mutex<Vec<(WalletId, Vec<Transaction>, u64)>>,
}

impl BsvHomeTransactionStatusService for QueuedTransactionStatus {
    fn track(&self, wallet_id: WalletId, transactions: Vec<Transaction>) {
        self.callbacks.lock().unwrap().push((wallet_id, transactions, self.service.tracking_generation()));
    }
}
