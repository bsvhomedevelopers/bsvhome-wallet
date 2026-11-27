mod cold;
mod funding;
#[cfg(test)]
mod funding_tests;
pub(crate) mod ledger;
mod model;
mod payer;
mod refresh;
#[cfg(test)]
mod refresh_tests;
mod selection;
pub(crate) mod single;
mod state;
#[cfg(test)]
pub(crate) mod testkit;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex as StdMutex, Weak};

use bsv_home_bsv::rpc::BsvClient;
use bsv_home_derivation::bsv_account_xpub;
use futures::future::{AbortHandle, Abortable};
use futures::lock::Mutex;
use num_bigint::BigUint;
use primitives::{
    Account, AssetBalance, Balance, BsvAddressBranch, BsvAddressLocator, BsvSpendMetadata, Chain, SignerInput, Transaction, TransactionIdRequest, TransactionState, Wallet,
    WalletId, WalletType,
};

use crate::alien::AlienClient;
use crate::gateway::BsvHomeGateway;
use crate::keystore::{BsvHomeKeystore, decode_password, keystore_id_for_wallet};
use crate::services::confirm::BsvHomeTransactionCheckpointData;
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::{BsvHomeKeystorePassword, BsvHomeWalletStore};

use ledger::PayerReservationRequest;
pub use model::*;
use refresh::{BalanceSnapshot, DisplayRefreshes};
use state::{GAP_LIMIT, PoolState, pool_error};

const MAX_DISCOVERY_STEPS: u32 = 1000;

enum ScanPreparation {
    Initialize,
    RequireExisting,
}

#[derive(Default)]
struct BsvScans {
    generations: HashMap<WalletId, u64>,
    handles: HashMap<String, (WalletId, AbortHandle)>,
    discovery: HashMap<WalletId, Weak<Mutex<()>>>,
}

type ActiveScans = Arc<StdMutex<BsvScans>>;

struct BsvScanLease {
    id: String,
    active: ActiveScans,
}

impl Drop for BsvScanLease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.handles.remove(&self.id);
        }
    }
}

pub(crate) struct BsvSigningLease {
    pub id: String,
    _lease: funding::PayerSigningLease,
}

impl BsvSigningLease {
    fn new(id: String, lease: funding::PayerSigningLease) -> Self {
        Self { id, _lease: lease }
    }
}

#[derive(uniffi::Object)]
pub struct BsvHomeBsvAddressPool {
    gateway: Arc<BsvHomeGateway>,
    wallets: Arc<dyn BsvHomeWalletStore>,
    store: Arc<dyn BsvHomeBsvAddressStore>,
    keystore: Arc<BsvHomeKeystore>,
    password: Arc<dyn BsvHomeKeystorePassword>,
    mutation: Mutex<()>,
    active_scans: ActiveScans,
    display_refreshes: DisplayRefreshes,
}

#[uniffi::export]
impl BsvHomeBsvAddressPool {
    #[uniffi::constructor]
    pub fn new(
        gateway: Arc<BsvHomeGateway>,
        wallet_store: Arc<dyn BsvHomeWalletStore>,
        store: Arc<dyn BsvHomeBsvAddressStore>,
        keystore: Arc<BsvHomeKeystore>,
        password: Arc<dyn BsvHomeKeystorePassword>,
    ) -> Self {
        Self {
            gateway,
            wallets: wallet_store,
            store,
            keystore,
            password,
            mutation: Mutex::new(()),
            active_scans: Arc::new(StdMutex::new(BsvScans::default())),
            display_refreshes: DisplayRefreshes::default(),
        }
    }

    pub async fn snapshot(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvAddressSnapshot>, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) {
            if wallet.wallet_type == WalletType::PrivateKey {
                self.payer_state(&wallet).await?;
            }
            return Ok(Some(single_address_snapshot(&wallet)?));
        }
        Ok(self.load(&wallet).await?.map(|(_, state)| state.snapshot()))
    }

    pub async fn prepare(&self, wallet_id: WalletId) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) {
            if wallet.wallet_type == WalletType::PrivateKey {
                self.payer_state(&wallet).await?;
            }
            return single_address_snapshot(&wallet);
        }
        let _mutation = self.mutation.lock().await;
        if let Some((_, state)) = self.load(&wallet).await? {
            return Ok(state.snapshot());
        }
        let account = wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?;
        let public = match wallet.wallet_type {
            WalletType::View => account.clone(),
            WalletType::Multicoin | WalletType::Single => {
                let password = decode_password(&self.password.get_password(false)?);
                let xpub = self.keystore.bsv_account_xpub(keystore_id_for_wallet(wallet.id.id()), &account.address, password)?;
                Account {
                    extended_public_key: Some(xpub),
                    ..account.clone()
                }
            }
            WalletType::PrivateKey => return Err(pool_error("A private key wallet is not an HD account")),
        };
        let state = PoolState::from_account(wallet.id.clone(), &public)?;
        self.store_state(&wallet, None, &state).await?;
        Ok(state.snapshot())
    }

    pub async fn sync(&self, wallet_id: WalletId) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.scan(wallet_id, false, None, &self.gateway.bsv_client(), ScanPreparation::Initialize).await
    }

    pub async fn continue_scan(&self, wallet_id: WalletId) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.scan(wallet_id, true, None, &self.gateway.bsv_client(), ScanPreparation::Initialize).await
    }

    pub fn scan_generation(&self, wallet_id: WalletId) -> Result<u64, BsvHomeServiceError> {
        Ok(self.active_scans.lock().map_err(pool_error)?.generations.get(&wallet_id).copied().unwrap_or(0))
    }

    pub async fn sync_at_generation(&self, wallet_id: WalletId, generation: u64, extend: bool) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.scan(wallet_id, extend, Some(generation), &self.gateway.bsv_client(), ScanPreparation::Initialize)
            .await
    }

    pub fn cancel_scan(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        let handles = {
            let mut active = self.active_scans.lock().map_err(pool_error)?;
            let generation = active.generations.entry(wallet_id.clone()).or_insert(0);
            *generation = generation.checked_add(1).ok_or_else(|| pool_error("BSV scan generation exhausted"))?;
            active
                .handles
                .values()
                .filter(|(wallet, _)| *wallet == wallet_id)
                .map(|(_, handle)| handle.clone())
                .collect::<Vec<_>>()
        };
        for handle in handles {
            handle.abort();
        }
        Ok(())
    }
}

impl BsvHomeBsvAddressPool {
    async fn scan(
        &self,
        wallet_id: WalletId,
        extend: bool,
        expected_generation: Option<u64>,
        client: &BsvClient<AlienClient>,
        preparation: ScanPreparation,
    ) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        let (handle, registration) = AbortHandle::new_pair();
        let id = uuid::Uuid::new_v4().to_string();
        let (generation, discovery) = {
            let mut active = self.active_scans.lock().map_err(pool_error)?;
            let generation = active.generations.get(&wallet_id).copied().unwrap_or(0);
            if expected_generation.is_some_and(|expected| expected != generation) {
                return Err(BsvHomeServiceError::Cancelled);
            }
            active.discovery.retain(|_, lock| lock.strong_count() > 0);
            let discovery = active.discovery.get(&wallet_id).and_then(Weak::upgrade).unwrap_or_else(|| Arc::new(Mutex::new(())));
            active.discovery.insert(wallet_id.clone(), Arc::downgrade(&discovery));
            active.handles.insert(id.clone(), (wallet_id.clone(), handle));
            (generation, discovery)
        };
        let _lease = BsvScanLease {
            id,
            active: self.active_scans.clone(),
        };
        Abortable::new(
            async {
                match preparation {
                    ScanPreparation::Initialize => {
                        self.prepare(wallet_id.clone()).await?;
                    }
                    ScanPreparation::RequireExisting => {
                        self.required_state(&self.wallet(&wallet_id).await?).await?;
                    }
                }
                let _discovery = discovery.lock().await;
                let wallet = self.wallet(&wallet_id).await?;
                if !has_hd_account(&wallet) {
                    if wallet.wallet_type == WalletType::PrivateKey {
                        self.refresh_payer_reservations(&wallet).await?;
                    }
                    return single_address_snapshot(&wallet);
                }
                self.check_scan(&wallet_id, generation)?;
                self.refresh_known(&wallet, generation, client).await?;
                let snapshot = self.discover(&wallet, extend, generation, client).await?;
                self.check_scan(&wallet_id, generation)?;
                Ok(snapshot)
            },
            registration,
        )
        .await
        .map_err(|_| BsvHomeServiceError::Cancelled)?
    }

    fn check_scan(&self, wallet_id: &WalletId, generation: u64) -> Result<(), BsvHomeServiceError> {
        if self.scan_generation(wallet_id.clone())? != generation {
            return Err(BsvHomeServiceError::Cancelled);
        }
        Ok(())
    }

    pub(crate) fn authorize_broadcast(&self, wallet: &Wallet) -> Result<(), BsvHomeServiceError> {
        let password = decode_password(&self.password.get_password(false)?);
        self.keystore.verify_wallet_password(keystore_id_for_wallet(wallet.id.id()), password)?;
        Ok(())
    }

    pub(crate) fn initial_record(wallet: &Wallet, words: &str) -> Result<Option<BsvHomeBsvAddressStoredState>, BsvHomeServiceError> {
        if !has_mnemonic(wallet) || wallet.account(Chain::BitcoinSv).is_none() {
            return Ok(None);
        }
        let state = PoolState::new(wallet.id.clone(), bsv_account_xpub(words).map_err(pool_error)?)?;
        let main = wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?;
        state.validate(&wallet.id, &main.address)?;
        Ok(Some(BsvHomeBsvAddressStoredState {
            revision: 0,
            value: serde_json::to_string(&state).map_err(pool_error)?,
        }))
    }

    pub(crate) async fn initialize_import(&self, wallet: &Wallet, words: &str) -> Result<(), BsvHomeServiceError> {
        if wallet.account(Chain::BitcoinSv).is_none() {
            return Ok(());
        }
        let _mutation = self.mutation.lock().await;
        if self.load(wallet).await?.is_some() {
            return Ok(());
        }
        if let Some(record) = Self::initial_record(wallet, words)?
            && !self.store.save(wallet.id.clone(), None, record).await?
        {
            return Err(pool_error("BSV address initialization changed concurrently"));
        }
        Ok(())
    }

    pub(crate) async fn wallet(&self, wallet_id: &WalletId) -> Result<Wallet, BsvHomeServiceError> {
        self.wallets.get_wallet(wallet_id.clone()).await?.ok_or_else(|| pool_error("BSV wallet not found"))
    }

    async fn load(&self, wallet: &Wallet) -> Result<Option<(u64, PoolState)>, BsvHomeServiceError> {
        self.payer_state(wallet)
            .await?
            .map(|(revision, state)| match state {
                ledger::PayerState::Hd(state) => Ok((revision, state)),
                ledger::PayerState::SingleAddress(_) => Err(pool_error("BSV single-address state cannot be read as an HD pool")),
            })
            .transpose()
    }

    async fn required_state(&self, wallet: &Wallet) -> Result<(u64, PoolState), BsvHomeServiceError> {
        self.load(wallet).await?.ok_or_else(|| pool_error("BSV address pool is not initialized"))
    }

    async fn store_state(&self, wallet: &Wallet, revision: Option<u64>, state: &PoolState) -> Result<(), BsvHomeServiceError> {
        let revision_next = match revision {
            Some(revision) => revision.checked_add(1).ok_or_else(|| pool_error("BSV address revision exhausted"))?,
            None => 0,
        };
        let value = serde_json::to_string(state).map_err(pool_error)?;
        if !self
            .store
            .save(wallet.id.clone(), revision, BsvHomeBsvAddressStoredState { revision: revision_next, value })
            .await?
        {
            return Err(pool_error("BSV address state changed concurrently; retry"));
        }
        Ok(())
    }

    async fn refresh_known(&self, wallet: &Wallet, generation: u64, client: &BsvClient<AlienClient>) -> Result<(), BsvHomeServiceError> {
        let (revision, mut state) = self.required_state(wallet).await?;
        let previous = serde_json::to_value(&state).map_err(pool_error)?;
        let addresses = state.records.iter().map(|record| record.address.clone()).collect::<Vec<_>>();
        let activities = client.address_activities(&addresses).await.map_err(map_provider_error)?;
        self.check_scan(&wallet.id, generation)?;
        let observations = state
            .records
            .iter()
            .zip(activities)
            .map(|(record, (history, confirmed))| (record.locator, history, confirmed))
            .collect::<Vec<_>>();
        for reservation in &mut state.reservations {
            if let Some(saved) = &reservation.checkpoint {
                let transaction = self
                    .gateway
                    .transaction_by_hash(TransactionIdRequest::new(Chain::BitcoinSv, saved.transaction_id.clone(), None))
                    .await?;
                self.check_scan(&wallet.id, generation)?;
                reservation.confirmed = transaction.is_some_and(|transaction| transaction.hash() == saved.transaction_id && transaction.state == TransactionState::Confirmed);
            }
        }
        self.refresh_funding_confirmations(&mut state.funding_reservations).await?;
        self.check_scan(&wallet.id, generation)?;
        for (locator, history, confirmed) in observations {
            state.observe(locator, history, confirmed)?;
        }
        if state.discovery == BsvHomeBsvDiscoveryState::Complete {
            state.refill()?;
        }
        let _mutation = self.mutation.lock().await;
        self.check_scan(&wallet.id, generation)?;
        if serde_json::to_value(&state).map_err(pool_error)? == previous {
            if self.store.get(wallet.id.clone()).await?.map(|state| state.revision) != Some(revision) {
                return Err(pool_error("BSV address state changed concurrently; retry"));
            }
            Ok(())
        } else {
            self.store_state(wallet, Some(revision), &state).await
        }
    }

    pub(crate) async fn refresh_reservations(&self, wallet: &Wallet) -> Result<(), BsvHomeServiceError> {
        self.refresh_payer_reservations(wallet).await
    }

    async fn discover(&self, wallet: &Wallet, extend: bool, generation: u64, client: &BsvClient<AlienClient>) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        if extend {
            let _mutation = self.mutation.lock().await;
            let (revision, mut state) = self.required_state(wallet).await?;
            state.receive.stopped = false;
            state.receive.empty_count = 0;
            state.change.stopped = false;
            state.change.empty_count = 0;
            state.discovery = BsvHomeBsvDiscoveryState::Incomplete;
            self.check_scan(&wallet.id, generation)?;
            self.store_state(wallet, Some(revision), &state).await?;
        }
        for _ in 0..MAX_DISCOVERY_STEPS {
            self.check_scan(&wallet.id, generation)?;
            let (_, state) = self.required_state(wallet).await?;
            let branch = if !state.receive.stopped {
                BsvAddressBranch::Receive
            } else if !state.change.stopped {
                BsvAddressBranch::Change
            } else {
                return Ok(state.snapshot());
            };
            let progress = match branch {
                BsvAddressBranch::Receive => &state.receive,
                BsvAddressBranch::Change => &state.change,
            };
            let locator = BsvAddressLocator {
                branch,
                index: progress.next_index,
            };
            let address = state.address(locator)?;
            let (history, confirmed) = client.address_activity(&address).await.map_err(map_provider_error)?;
            let _mutation = self.mutation.lock().await;
            self.check_scan(&wallet.id, generation)?;
            let (revision, mut state) = self.required_state(wallet).await?;
            state.observe(locator, history, confirmed)?;
            let progress = match branch {
                BsvAddressBranch::Receive => &mut state.receive,
                BsvAddressBranch::Change => &mut state.change,
            };
            if progress.next_index != locator.index {
                return Err(pool_error("BSV discovery cursor changed concurrently"));
            }
            progress.next_index = progress
                .next_index
                .checked_add(1)
                .filter(|index| *index < 1 << 31)
                .ok_or_else(|| pool_error("BSV derivation index exhausted"))?;
            progress.empty_count = if history { 0 } else { progress.empty_count + 1 };
            progress.stopped = progress.empty_count >= GAP_LIMIT;
            state.discovery = if state.receive.stopped && state.change.stopped {
                BsvHomeBsvDiscoveryState::Complete
            } else {
                BsvHomeBsvDiscoveryState::Incomplete
            };
            if state.discovery == BsvHomeBsvDiscoveryState::Complete {
                state.refill()?;
            }
            self.store_state(wallet, Some(revision), &state).await?;
        }
        Ok(self.required_state(wallet).await?.1.snapshot())
    }

    pub(crate) async fn owned_addresses(&self, wallet_id: WalletId) -> Result<Vec<String>, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) {
            let state = self.payer_state(&wallet).await?;
            return payer::payer_addresses(&wallet, state.as_ref().map(|(_, state)| state));
        }
        match self.load(&wallet).await? {
            Some((_, state)) => Ok(state.records.into_iter().map(|record| record.address).collect()),
            None => Ok(vec![
                wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?.address.clone(),
            ]),
        }
    }

    pub(crate) async fn balance(&self, wallet_id: WalletId, authenticate: bool) -> Result<AssetBalance, BsvHomeServiceError> {
        if authenticate && self.sync(wallet_id.clone()).await?.discovery != BsvHomeBsvDiscoveryState::Complete {
            return Err(pool_error("BSV automatic discovery is incomplete; continue scanning"));
        }
        self.wallet_balance(wallet_id, None, &self.gateway.bsv_client()).await.map(|snapshot| snapshot.balance)
    }

    pub(crate) async fn receipt_balance(&self, wallet_id: WalletId, hash: &str) -> Result<Option<AssetBalance>, BsvHomeServiceError> {
        let snapshot = self.wallet_balance(wallet_id, Some(hash), &self.gateway.bsv_client()).await?;
        Ok(snapshot.indexed.then_some(snapshot.balance))
    }

    async fn wallet_balance(&self, wallet_id: WalletId, receipt_hash: Option<&str>, client: &BsvClient<AlienClient>) -> Result<BalanceSnapshot, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) {
            let (balance, indexed) = self.single_wallet_balance(&wallet, receipt_hash).await?;
            return Ok(BalanceSnapshot { balance, indexed, revision: None });
        }
        let state = self.load(&wallet).await?;
        if state.as_ref().is_some_and(|(_, state)| state.discovery != BsvHomeBsvDiscoveryState::Complete) {
            return Err(pool_error("BSV automatic discovery is incomplete; continue scanning"));
        }
        let revision = state.as_ref().map(|(revision, _)| *revision);
        let addresses = addresses_from_state(&wallet, state.as_ref().map(|(_, state)| state))?;
        let reserved: HashSet<_> = state
            .as_ref()
            .into_iter()
            .flat_map(|(_, state)| {
                state
                    .reservations
                    .iter()
                    .filter(|record| !record.confirmed)
                    .flat_map(|record| &record.spend.inputs)
                    .chain(state.funding_reservations.iter().filter(|record| !record.confirmed).flat_map(|record| &record.spend.inputs))
            })
            .map(|input| (input.transaction_id.clone(), input.vout))
            .collect();
        let mut available = BigUint::ZERO;
        let mut pending = BigUint::ZERO;
        let mut reserved_value = BigUint::ZERO;
        let mut indexed = receipt_hash.is_none();
        for (_, output) in client.wallet_native_unspent(&addresses).await.map_err(map_provider_error)? {
            indexed |= receipt_hash == Some(output.tx_hash.as_str());
            if reserved.contains(&(output.tx_hash.clone(), output.tx_pos)) {
                reserved_value += output.value;
                continue;
            }
            match output.status.as_str() {
                "confirmed" if output.height.is_some_and(|height| height > 0) => available += output.value,
                "unconfirmed" => pending += output.value,
                _ => return Err(pool_error("invalid BSV UTXO confirmation status")),
            }
        }
        self.check_revision(&wallet.id, revision).await?;
        let mut balance = Balance::with_pending_unconfirmed(available, pending);
        balance.reserved = reserved_value;
        Ok(BalanceSnapshot {
            balance: AssetBalance::new_balance(Chain::BitcoinSv.as_asset_id(), balance),
            indexed,
            revision,
        })
    }

    pub(crate) async fn transactions(&self, wallet_id: WalletId, limit: u32) -> Result<Vec<Transaction>, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) {
            return self.single_transactions(&wallet, limit).await;
        }
        let state = self.load(&wallet).await?;
        if state.as_ref().is_some_and(|(_, state)| state.discovery != BsvHomeBsvDiscoveryState::Complete) {
            return Err(pool_error("BSV automatic discovery is incomplete; continue scanning"));
        }
        let revision = state.as_ref().map(|(revision, _)| *revision);
        let addresses = addresses_from_state(&wallet, state.as_ref().map(|(_, state)| state))?;
        let mut transactions = self
            .gateway
            .bsv_client()
            .wallet_transactions(&addresses, limit as usize)
            .await
            .map_err(map_provider_error)?;
        if let Some((_, state)) = state {
            for transaction in &mut transactions {
                if transaction.direction == primitives::TransactionDirection::SelfTransfer
                    && let Some(reservation) = state
                        .reservations
                        .iter()
                        .find(|reservation| reservation.checkpoint.as_ref().is_some_and(|saved| saved.transaction_id == transaction.hash()))
                {
                    let saved = reservation.checkpoint.as_ref().ok_or_else(|| pool_error("BSV checkpoint missing"))?;
                    transaction.to = saved.recipient_address.clone();
                    transaction.value = reservation.value.into();
                }
            }
        }
        self.check_revision(&wallet.id, revision).await?;
        Ok(transactions)
    }

    async fn check_revision(&self, wallet_id: &WalletId, revision: Option<u64>) -> Result<(), BsvHomeServiceError> {
        if self.store.get(wallet_id.clone()).await?.map(|state| state.revision) != revision {
            return Err(pool_error("BSV address scope changed during synchronization; retry"));
        }
        Ok(())
    }

    pub(crate) async fn preload_spend(&self, wallet_id: WalletId, sender: &str, is_max: bool) -> Result<Option<BsvSpendMetadata>, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) {
            return Ok(None);
        }
        self.preload_payer_spend(wallet_id, sender, is_max).await.map(Some)
    }

    pub(crate) async fn reserve(&self, wallet: &Wallet, input: &SignerInput, original_value: String, is_max: bool) -> Result<BsvSigningLease, BsvHomeServiceError> {
        let request = PayerReservationRequest::transfer(wallet, input, original_value, is_max)?;
        self.reserve_payer_spend(wallet, request).await
    }

    pub(crate) async fn release_unsigned(&self, wallet: &Wallet, id: &str) -> Result<(), BsvHomeServiceError> {
        self.release_payer_unsigned(wallet, id).await
    }

    pub(crate) async fn save_signed(&self, wallet: &Wallet, id: &str, saved: BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeServiceError> {
        self.save_payer_transfer(wallet, id, saved).await
    }

    pub(crate) async fn saved_for_transfer(
        &self,
        wallet: &Wallet,
        recipient: &str,
        original_value: &str,
        is_max: bool,
    ) -> Result<Option<BsvHomeTransactionCheckpointData>, BsvHomeServiceError> {
        self.payer_saved_transfer(wallet, recipient, original_value, is_max).await
    }

    pub(crate) async fn pending_input(&self, wallet: &Wallet, recipient: &str, original_value: &str, is_max: bool) -> Result<Option<SignerInput>, BsvHomeServiceError> {
        self.payer_pending_input(wallet, recipient, original_value, is_max).await
    }

    pub(crate) async fn validate_checkpoint(&self, wallet: &Wallet, saved: &BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeServiceError> {
        if saved.bsv_spend.is_none() && saved.version != 3 {
            return Ok(());
        }
        let (_, state) = self.payer_state(wallet).await?.ok_or_else(|| pool_error("BSV payer ledger is missing"))?;
        payer::validate_checkpoint(&state, wallet, saved)
    }
}

#[cfg(test)]
mod tests;

fn has_mnemonic(wallet: &Wallet) -> bool {
    match wallet.wallet_type {
        WalletType::Multicoin | WalletType::Single => true,
        WalletType::PrivateKey | WalletType::View => false,
    }
}

fn has_hd_account(wallet: &Wallet) -> bool {
    match wallet.wallet_type {
        WalletType::Multicoin | WalletType::Single => true,
        WalletType::View => wallet
            .account(Chain::BitcoinSv)
            .and_then(|account| account.extended_public_key.as_ref())
            .is_some_and(|key| key.starts_with("xpub")),
        WalletType::PrivateKey => false,
    }
}

fn map_provider_error(error: bsv_home_bsv::rpc::ProviderError) -> BsvHomeServiceError {
    crate::gateway::map_network_error(error).into()
}

fn addresses_from_state(wallet: &Wallet, state: Option<&PoolState>) -> Result<Vec<String>, BsvHomeServiceError> {
    match state {
        Some(state) => Ok(state.records.iter().map(|record| record.address.clone()).collect()),
        None => Ok(vec![
            wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?.address.clone(),
        ]),
    }
}

fn single_address_snapshot(wallet: &Wallet) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
    let address = wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?.address.clone();
    Ok(BsvHomeBsvAddressSnapshot {
        records: vec![BsvHomeBsvAddressRecord {
            branch: BsvAddressBranch::Receive,
            index: 0,
            address: address.clone(),
            status: BsvHomeBsvAddressStatus::Unused,
            is_current: true,
            is_main: true,
        }],
        current_address: Some(address),
        receive_count: 1,
        selected_index: Some(0),
        can_add: false,
        receive_scanned: 0,
        change_scanned: 0,
        discovery: BsvHomeBsvDiscoveryState::Complete,
        allocation_paused: false,
    })
}
