use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use bsv_home_bsv::rpc::BsvClient;
use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use primitives::{Account, AssetBalance, Chain, Wallet, WalletId, WalletType};
use uuid::Uuid;

use super::state::pool_error;
use super::{BsvHomeBsvAddressPool, ScanPreparation, has_hd_account};
use crate::alien::AlienClient;
use crate::services::error::BsvHomeServiceError;

#[derive(Clone)]
pub(super) struct BalanceSnapshot {
    pub balance: AssetBalance,
    pub indexed: bool,
    pub revision: Option<u64>,
}

const MAX_DISPLAY_BALANCE_ATTEMPTS: usize = 2;

#[derive(Clone)]
struct DisplaySnapshot {
    balance: AssetBalance,
    revision: Option<u64>,
    receipt_generation: u64,
}

type DisplayFuture = Shared<BoxFuture<'static, Result<DisplaySnapshot, BsvHomeServiceError>>>;
type ActiveRefreshes = Arc<Mutex<DisplayRefreshState>>;

#[derive(Clone, PartialEq, Eq, Hash)]
enum RefreshWork {
    AddressesAndBalance,
    Balance(u64),
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct RefreshKey {
    wallet_id: WalletId,
    generation: u64,
    receipt_generation: u64,
    node_url: String,
    work: RefreshWork,
}

struct RefreshEntry {
    id: Uuid,
    wallet_type: WalletType,
    account: Option<Account>,
    future: Weak<DisplayFuture>,
}

#[derive(Default)]
pub(super) struct DisplayRefreshes {
    active: ActiveRefreshes,
}

#[derive(Default)]
struct DisplayRefreshState {
    flights: HashMap<RefreshKey, RefreshEntry>,
    receipt_generations: HashMap<WalletId, u64>,
}

struct RefreshLease {
    id: Uuid,
    key: RefreshKey,
    active: ActiveRefreshes,
}

impl Drop for RefreshLease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock()
            && active.flights.get(&self.key).is_some_and(|entry| entry.id == self.id)
        {
            active.flights.remove(&self.key);
        }
    }
}

impl DisplayRefreshes {
    fn receipt_generation(&self, wallet_id: &WalletId) -> Result<u64, BsvHomeServiceError> {
        Ok(self.active.lock().map_err(pool_error)?.receipt_generations.get(wallet_id).copied().unwrap_or(0))
    }

    fn shared_refresh<F>(&self, key: RefreshKey, wallet: &Wallet, query: F) -> Result<Arc<DisplayFuture>, BsvHomeServiceError>
    where
        F: FnOnce() -> BoxFuture<'static, Result<DisplaySnapshot, BsvHomeServiceError>> + Send + 'static,
    {
        let account = wallet.account(Chain::BitcoinSv).cloned();
        let mut active = self.active.lock().map_err(pool_error)?;
        if let Some(flight) = active
            .flights
            .get(&key)
            .filter(|entry| entry.wallet_type == wallet.wallet_type && entry.account == account)
            .and_then(|entry| entry.future.upgrade())
        {
            return Ok(flight);
        }
        let lease = RefreshLease {
            id: Uuid::new_v4(),
            key: key.clone(),
            active: self.active.clone(),
        };
        let id = lease.id;
        let future = Arc::new(
            async move {
                let _lease = lease;
                query().await
            }
            .boxed()
            .shared(),
        );
        active.flights.insert(
            key,
            RefreshEntry {
                id,
                wallet_type: wallet.wallet_type,
                account,
                future: Arc::downgrade(&future),
            },
        );
        Ok(future)
    }
}

impl BsvHomeBsvAddressPool {
    pub(crate) fn invalidate_receipt_refresh(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        let mut active = self.display_refreshes.active.lock().map_err(pool_error)?;
        let generation = active.receipt_generations.entry(wallet_id).or_insert(0);
        *generation = generation.checked_add(1).ok_or_else(|| pool_error("BSV receipt refresh generation exhausted"))?;
        Ok(())
    }

    pub(crate) async fn refresh_receipt_balance(self: &Arc<Self>, wallet_id: WalletId) -> Result<AssetBalance, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) || self.load(&wallet).await?.is_none() {
            return self.balance(wallet_id, false).await;
        }
        let (node_url, client) = self.gateway.bsv_read_scope();
        self.display_balance_snapshot(wallet, node_url, client).await.map(|snapshot| snapshot.balance)
    }

    pub(crate) async fn refresh_display_balance(self: &Arc<Self>, wallet_id: WalletId) -> Result<AssetBalance, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) {
            self.sync(wallet_id.clone()).await?;
            return self.balance(wallet_id, false).await;
        }
        if self.load(&wallet).await?.is_none() {
            return self.balance(wallet_id, false).await;
        }

        let (node_url, client) = self.gateway.bsv_read_scope();
        let key = RefreshKey {
            wallet_id: wallet_id.clone(),
            generation: self.scan_generation(wallet_id.clone())?,
            receipt_generation: self.display_refreshes.receipt_generation(&wallet_id)?,
            node_url: node_url.clone(),
            work: RefreshWork::AddressesAndBalance,
        };
        let pool = self.clone();
        let refresh_wallet = wallet.clone();
        let generation = key.generation;
        let flight = self.display_refreshes.shared_refresh(key.clone(), &wallet, move || {
            async move {
                pool.scan(wallet_id, false, Some(generation), &client, ScanPreparation::RequireExisting).await?;
                pool.display_balance_snapshot(refresh_wallet, node_url, client).await
            }
            .boxed()
        })?;
        let result = flight.as_ref().clone().await;
        drop(flight);
        let snapshot = result?;
        self.validate_display_snapshot(&wallet, &key, &snapshot).await?;
        if snapshot.receipt_generation != self.display_refreshes.receipt_generation(&wallet.id)? {
            let (node_url, client) = self.gateway.bsv_read_scope();
            return self.display_balance_snapshot(wallet, node_url, client).await.map(|snapshot| snapshot.balance);
        }
        Ok(snapshot.balance)
    }

    async fn display_balance_snapshot(self: &Arc<Self>, wallet: Wallet, node_url: String, client: BsvClient<AlienClient>) -> Result<DisplaySnapshot, BsvHomeServiceError> {
        let generation = self.scan_generation(wallet.id.clone())?;
        let client = Arc::new(client);
        for _ in 0..MAX_DISPLAY_BALANCE_ATTEMPTS {
            self.check_scan(&wallet.id, generation)?;
            let (revision, _) = self.required_state(&wallet).await?;
            let key = RefreshKey {
                wallet_id: wallet.id.clone(),
                generation,
                receipt_generation: self.display_refreshes.receipt_generation(&wallet.id)?,
                node_url: node_url.clone(),
                work: RefreshWork::Balance(revision),
            };
            let pool = self.clone();
            let wallet_id = wallet.id.clone();
            let client = client.clone();
            let receipt_generation = key.receipt_generation;
            let flight = self.display_refreshes.shared_refresh(key.clone(), &wallet, move || {
                async move {
                    let snapshot = pool.wallet_balance(wallet_id, None, &client).await?;
                    Ok(DisplaySnapshot {
                        balance: snapshot.balance,
                        revision: snapshot.revision,
                        receipt_generation,
                    })
                }
                .boxed()
            })?;
            let result = flight.as_ref().clone().await;
            drop(flight);
            let snapshot = result?;
            if snapshot.revision != Some(revision) {
                return Err(BsvHomeServiceError::Cancelled);
            }
            self.validate_display_snapshot(&wallet, &key, &snapshot).await?;
            if snapshot.receipt_generation == self.display_refreshes.receipt_generation(&wallet.id)? {
                return Ok(snapshot);
            }
        }
        Err(BsvHomeServiceError::Cancelled)
    }

    async fn validate_display_snapshot(&self, wallet: &Wallet, key: &RefreshKey, snapshot: &DisplaySnapshot) -> Result<(), BsvHomeServiceError> {
        self.check_scan(&key.wallet_id, key.generation)?;
        let current = self.wallet(&key.wallet_id).await?;
        if current.wallet_type != wallet.wallet_type || current.account(Chain::BitcoinSv) != wallet.account(Chain::BitcoinSv) {
            return Err(BsvHomeServiceError::Cancelled);
        }
        self.check_revision(&key.wallet_id, snapshot.revision).await?;
        self.check_scan(&key.wallet_id, key.generation)?;
        if self.gateway.bsv_read_scope().0 != key.node_url {
            return Err(BsvHomeServiceError::Cancelled);
        }
        Ok(())
    }
}
