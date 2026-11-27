use std::collections::BTreeMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::Poll;
use std::time::{Duration, Instant};

use bsv_home_bsv::finalize_wallet_transaction;
use futures::future::{AbortHandle, Abortable, poll_fn};
use futures::lock::Mutex as AsyncMutex;
use futures::task::AtomicWaker;
use futures::{FutureExt, pin_mut, select};
use primitives::{Account, Chain, TransactionDirection, TransactionIdRequest, TransactionState, Wallet, WalletId, WalletType};
use uuid::Uuid;

use super::rules::{self, Event, Receipt, ReceiptQueue, Work};
use crate::gateway::BsvHomeGateway;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::clock::sleep;
use crate::services::error::BsvHomeServiceError;
use crate::services::transaction_state::{BsvHomeTransactionStateService, BsvHomeTransactionStatusService};
use crate::services::transactions::BsvHomeTransactionsService;
use crate::services::wallet::addresses::BsvHomeBsvAddressPool;
use crate::services::wallet_session::BsvHomeWalletSessionService;

const CATCH_UP_INTERVAL: Duration = Duration::from_secs(60);
const RECEIPT_TIMEOUT: Duration = Duration::from_secs(18);
const RETRY_DELAYS: [u64; 4] = [0, 800, 2_000, 5_000];

#[derive(Clone, PartialEq)]
struct Identity {
    wallet_id: WalletId,
    account: Account,
    wallet_type: WalletType,
}

#[derive(Default)]
struct State {
    identity: Option<Identity>,
    scripts: BTreeMap<String, String>,
    request_id: Option<String>,
    queue: ReceiptQueue,
    abort: Option<AbortHandle>,
}

#[derive(uniffi::Object)]
pub struct BsvHomeBsvReceiptService {
    gateway: Arc<BsvHomeGateway>,
    addresses: Arc<BsvHomeBsvAddressPool>,
    session: Arc<BsvHomeWalletSessionService>,
    balances: Arc<BsvHomeBalanceService>,
    transactions: Arc<BsvHomeTransactionsService>,
    transaction_state: Arc<BsvHomeTransactionStateService>,
    transaction_status: Arc<dyn BsvHomeTransactionStatusService>,
    url: String,
    generation: AtomicU64,
    state: Mutex<State>,
    lifecycle: AsyncMutex<()>,
    worker: AsyncMutex<()>,
    wake: AtomicWaker,
}

#[uniffi::export]
impl BsvHomeBsvReceiptService {
    #[uniffi::constructor]
    pub fn new(
        gateway: Arc<BsvHomeGateway>,
        addresses: Arc<BsvHomeBsvAddressPool>,
        session: Arc<BsvHomeWalletSessionService>,
        balances: Arc<BsvHomeBalanceService>,
        transactions: Arc<BsvHomeTransactionsService>,
        transaction_state: Arc<BsvHomeTransactionStateService>,
        transaction_status: Arc<dyn BsvHomeTransactionStatusService>,
        base_url: String,
    ) -> Result<Self, BsvHomeServiceError> {
        Ok(Self {
            gateway,
            addresses,
            session,
            balances,
            transactions,
            transaction_state,
            transaction_status,
            url: rules::socket_url(&base_url)?,
            generation: AtomicU64::new(0),
            state: Mutex::new(State::default()),
            lifecycle: AsyncMutex::new(()),
            worker: AsyncMutex::new(()),
            wake: AtomicWaker::new(),
        })
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub async fn prepare_connection(&self) -> Result<bool, BsvHomeServiceError> {
        let _lifecycle = self.lifecycle.lock().await;
        let generation = self.generation.load(Ordering::SeqCst);
        let prepared = self.prepared_identity().await?;
        self.check_generation(generation)?;
        let Some((identity, scripts)) = prepared else {
            self.cancel();
            return Ok(false);
        };
        let mut state = self.state()?;
        self.check_generation(generation)?;
        if state.identity.as_ref() != Some(&identity) {
            let next_generation = generation.wrapping_add(1);
            self.generation
                .compare_exchange(generation, next_generation, Ordering::SeqCst, Ordering::SeqCst)
                .map_err(|_| BsvHomeServiceError::Cancelled)?;
            if let Some(abort) = state.abort.take() {
                abort.abort();
            }
            *state = State {
                identity: Some(identity),
                scripts,
                ..Default::default()
            };
            self.wake.wake();
            drop(state);
            self.check_generation(next_generation)?;
        }
        Ok(true)
    }

    pub async fn connected(&self) -> Result<String, BsvHomeServiceError> {
        let _lifecycle = self.lifecycle.lock().await;
        let generation = self.generation.load(Ordering::SeqCst);
        let identity = self.identity(generation).await?;
        let addresses = self.addresses.receipt_addresses(identity.wallet_id.clone()).await?.ok_or(BsvHomeServiceError::Cancelled)?;
        self.check_generation(generation)?;
        let mut state = self.state()?;
        self.check_generation(generation)?;
        state.scripts = rules::receive_scripts(addresses)?;
        state.request_id = Some(Uuid::new_v4().to_string());
        self.addresses.invalidate_receipt_refresh(identity.wallet_id)?;
        state.queue.push(Event::Resync);
        let message = rules::subscription(state.request_id.as_deref().ok_or(BsvHomeServiceError::Cancelled)?, &state.scripts)?;
        self.wake.wake();
        drop(state);
        self.check_generation(generation)?;
        Ok(message)
    }

    pub async fn refresh_subscription(&self) -> Result<Option<String>, BsvHomeServiceError> {
        let _lifecycle = self.lifecycle.lock().await;
        let generation = self.generation.load(Ordering::SeqCst);
        let identity = self.identity(generation).await?;
        let addresses = self.addresses.receipt_addresses(identity.wallet_id.clone()).await?.ok_or(BsvHomeServiceError::Cancelled)?;
        let scripts = rules::receive_scripts(addresses)?;
        self.check_generation(generation)?;
        let mut state = self.state()?;
        self.check_generation(generation)?;
        if state.scripts == scripts {
            return Ok(None);
        }
        state.scripts = scripts;
        state.request_id = Some(Uuid::new_v4().to_string());
        state.queue = ReceiptQueue::default();
        self.addresses.invalidate_receipt_refresh(identity.wallet_id)?;
        state.queue.push(Event::Resync);
        let message = rules::subscription(state.request_id.as_deref().ok_or(BsvHomeServiceError::Cancelled)?, &state.scripts)?;
        self.wake.wake();
        drop(state);
        self.check_generation(generation)?;
        Ok(Some(message))
    }

    pub fn receive_event(&self, event: String) -> Result<(), BsvHomeServiceError> {
        let mut state = self.state()?;
        if let Some(request_id) = &state.request_id
            && let Some(event) = rules::parse_event(&event, request_id, &state.scripts)
            && state.queue.push(event)
        {
            let identity = state.identity.as_ref().ok_or(BsvHomeServiceError::Cancelled)?;
            self.addresses.invalidate_receipt_refresh(identity.wallet_id.clone())?;
            self.wake.wake();
        }
        Ok(())
    }

    pub async fn run(&self) -> Result<(), BsvHomeServiceError> {
        let generation = self.generation.load(Ordering::SeqCst);
        let _worker = self.worker.lock().await;
        self.identity(generation).await?;
        let (abort, registration) = AbortHandle::new_pair();
        {
            let mut state = self.state()?;
            self.check_generation(generation)?;
            state.abort = Some(abort);
        }
        let result = Abortable::new(self.run_worker(generation, CATCH_UP_INTERVAL), registration).await;
        if self.generation.load(Ordering::SeqCst) == generation {
            self.state()?.abort = None;
        }
        result.map_err(|_| BsvHomeServiceError::Cancelled)?
    }

    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut state) = self.state.lock() {
            if let Some(abort) = state.abort.take() {
                abort.abort();
            }
            *state = State::default();
        }
        self.wake.wake();
    }
}

impl BsvHomeBsvReceiptService {
    fn state(&self) -> Result<MutexGuard<'_, State>, BsvHomeServiceError> {
        self.state.lock().map_err(|_| BsvHomeServiceError::Core {
            msg: "BSV receipt state unavailable".into(),
        })
    }

    fn check_generation(&self, generation: u64) -> Result<(), BsvHomeServiceError> {
        if self.generation.load(Ordering::SeqCst) != generation {
            return Err(BsvHomeServiceError::Cancelled);
        }
        Ok(())
    }

    async fn prepared_identity(&self) -> Result<Option<(Identity, BTreeMap<String, String>)>, BsvHomeServiceError> {
        if !self.session.environment.uses_bsv_receipt_events() {
            return Ok(None);
        }
        let Some(wallet) = self.session.get_current_wallet().await? else { return Ok(None) };
        let Some(identity) = wallet_identity(&wallet) else { return Ok(None) };
        let Some(addresses) = self.addresses.receipt_addresses(identity.wallet_id.clone()).await? else {
            return Ok(None);
        };
        let scripts = rules::receive_scripts(addresses)?;
        if scripts.is_empty() {
            return Err(rules::invalid("BSV receipt subscription has no owned scripts"));
        }
        Ok(Some((identity, scripts)))
    }

    async fn identity(&self, generation: u64) -> Result<Identity, BsvHomeServiceError> {
        self.check_generation(generation)?;
        let identity = self.state()?.identity.clone().ok_or(BsvHomeServiceError::Cancelled)?;
        let current = self.session.get_current_wallet().await?;
        if current.as_ref().and_then(wallet_identity).as_ref() != Some(&identity) {
            return Err(BsvHomeServiceError::Cancelled);
        }
        self.check_generation(generation)?;
        Ok(identity)
    }

    async fn next_work(&self, generation: u64) -> Result<Work, BsvHomeServiceError> {
        poll_fn(|context| {
            self.wake.register(context.waker());
            if let Err(error) = self.check_generation(generation) {
                return Poll::Ready(Err(error));
            }
            match self.state() {
                Ok(mut state) => match state.queue.next() {
                    Some(work) => Poll::Ready(Ok(work)),
                    None => Poll::Pending,
                },
                Err(error) => Poll::Ready(Err(error)),
            }
        })
        .await
    }

    async fn run_worker(&self, generation: u64, interval: Duration) -> Result<(), BsvHomeServiceError> {
        let mut catch_up_at = Instant::now() + interval;
        loop {
            let work = self.next_work(generation).fuse();
            let tick = sleep(catch_up_at.saturating_duration_since(Instant::now())).fuse();
            pin_mut!(work, tick);
            let work = select! {
                work = work => work?,
                _ = tick => { catch_up_at = Instant::now() + interval; Work::Resync },
            };
            match work {
                Work::Resync => self.catch_up(generation).await?,
                Work::Receipt(receipt) => {
                    let verified = self.retry_receipt(generation, &receipt, &RETRY_DELAYS, sleep(RECEIPT_TIMEOUT)).await?;
                    self.check_generation(generation)?;
                    if !verified {
                        let identity = self.identity(generation).await?;
                        self.addresses.invalidate_receipt_refresh(identity.wallet_id)?;
                    }
                    self.state()?.queue.finish(&receipt, verified);
                }
            }
        }
    }

    async fn catch_up(&self, generation: u64) -> Result<(), BsvHomeServiceError> {
        let identity = self.identity(generation).await?;
        let balance = self.addresses.refresh_receipt_balance(identity.wallet_id.clone()).await;
        self.identity(generation).await?;
        let balances = match balance {
            Ok(balance) => self.balances.publish_bsv_receipt_balance(identity.wallet_id.clone(), balance).await,
            Err(error) => Err(error),
        };
        self.identity(generation).await?;
        let history = self.addresses.transactions(identity.wallet_id.clone(), 1000).await;
        self.identity(generation).await?;
        let history = match history {
            Ok(history) => self.transactions.save_address_history(identity.wallet_id, history).await,
            Err(error) => Err(error),
        };
        self.identity(generation).await?;
        match (balances, history) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(balances), Err(history)) => Err(BsvHomeServiceError::Core {
                msg: format!("receipt balances: {balances}; history: {history}"),
            }),
        }
    }

    async fn retry_receipt(&self, generation: u64, receipt: &Receipt, delays: &[u64], deadline: impl Future<Output = ()>) -> Result<bool, BsvHomeServiceError> {
        let attempts = async {
            for delay in delays {
                sleep(Duration::from_millis(*delay)).await;
                self.identity(generation).await?;
                match self.receipt(generation, receipt).await {
                    Ok(true) => return Ok(true),
                    Err(BsvHomeServiceError::Cancelled) => return Err(BsvHomeServiceError::Cancelled),
                    Ok(false) | Err(_) => {}
                }
            }
            Ok(false)
        }
        .fuse();
        let deadline = deadline.fuse();
        pin_mut!(attempts, deadline);
        select! { verified = attempts => verified, _ = deadline => Ok(false) }
    }

    async fn receipt(&self, generation: u64, receipt: &Receipt) -> Result<bool, BsvHomeServiceError> {
        let identity = self.identity(generation).await?;
        let owned = self.addresses.owned_addresses(identity.wallet_id.clone()).await?;
        if !owned.contains(&receipt.address) {
            return Err(rules::invalid("BSV receipt address is not owned"));
        }
        let request = TransactionIdRequest::new(Chain::BitcoinSv, receipt.hash.clone(), None);
        let Some(transaction) = self.gateway.transaction_by_hash(request).await? else {
            return Ok(false);
        };
        self.identity(generation).await?;
        if transaction.asset_id != Chain::BitcoinSv.as_asset_id()
            || transaction.hash() != receipt.hash
            || !transaction
                .utxo_outputs
                .as_ref()
                .is_some_and(|outputs| outputs.iter().any(|output| output.address == receipt.address && output.value > 0u32.into()))
        {
            return Err(rules::invalid("BSV receipt transaction does not pay the subscribed address"));
        }
        let transaction = finalize_wallet_transaction(transaction, &owned).map_err(rules::invalid)?;
        if transaction.direction != TransactionDirection::Incoming {
            self.addresses.invalidate_receipt_refresh(identity.wallet_id)?;
            self.state()?.queue.push(Event::Resync);
            return Ok(true);
        }
        let Some(balance) = self.addresses.receipt_balance(identity.wallet_id.clone(), &receipt.hash).await? else {
            return Ok(false);
        };
        self.identity(generation).await?;
        self.balances.publish_bsv_receipt_balance(identity.wallet_id.clone(), balance).await?;
        self.identity(generation).await?;
        let saved = self.transaction_state.publish_bsv_receipt(identity.wallet_id.clone(), transaction).await?;
        self.identity(generation).await?;
        if let Some(transaction) = saved
            && transaction.state == TransactionState::Pending
        {
            self.transaction_status.track(identity.wallet_id, vec![transaction]);
        }
        Ok(true)
    }
}

fn wallet_identity(wallet: &Wallet) -> Option<Identity> {
    wallet.account(Chain::BitcoinSv).map(|account| Identity {
        wallet_id: wallet.id.clone(),
        account: account.clone(),
        wallet_type: wallet.wallet_type,
    })
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
