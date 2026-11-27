use std::{
    collections::{HashMap, HashSet},
    error::Error,
    future::Future,
    str::FromStr,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use bitcoin::Txid;
use bsv_home_client::{Client, ClientError, Target, sleep};
use futures::lock::Mutex;
#[cfg(any(test, feature = "testkit"))]
use futures::lock::MutexGuard;
use primitives::Address;
use serde::de::DeserializeOwned;

use super::{
    models::{AddressHistory, AddressResult, History, TransactionInfo, Unspent},
    target::{BsvTarget, Confirmation},
};
use crate::BsvAddress;

const MAX_ADDRESS_PAGES: usize = 100;
const MAX_HISTORY_ADDRESSES: usize = 20;
const MAX_UNCONFIRMED_RESULTS: usize = 100_000;

pub type ProviderError = Box<dyn Error + Send + Sync>;
#[derive(Debug)]
pub struct BsvRequestScheduler {
    next_request: Mutex<Instant>,
}
impl Default for BsvRequestScheduler {
    fn default() -> Self {
        Self {
            next_request: Mutex::new(Instant::now()),
        }
    }
}
impl BsvRequestScheduler {
    pub fn shared() -> Arc<Self> {
        static SCHEDULER: OnceLock<Arc<BsvRequestScheduler>> = OnceLock::new();
        SCHEDULER.get_or_init(|| Arc::new(Self::default())).clone()
    }
    #[cfg(any(test, feature = "testkit"))]
    pub async fn hold_request_queue(&self) -> MutexGuard<'_, Instant> {
        self.next_request.lock().await
    }

    pub(crate) async fn acquire(&self) {
        let scheduled = {
            let mut next = self.next_request.lock().await;
            let scheduled = (*next).max(Instant::now());
            *next = scheduled + Duration::from_millis(400);
            scheduled
        };
        sleep(scheduled.saturating_duration_since(Instant::now())).await;
    }
}

pub struct BsvClient<C> {
    pub(crate) client: C,
    pub(crate) scheduler: Arc<BsvRequestScheduler>,
}
impl<C: Client> BsvClient<C> {
    pub fn new(client: C) -> Self {
        Self::with_scheduler(client, BsvRequestScheduler::shared())
    }
    pub fn with_scheduler(client: C, scheduler: Arc<BsvRequestScheduler>) -> Self {
        Self { client, scheduler }
    }
    pub(crate) async fn read<T, F, Fut>(&self, operation: &'static str, request: F) -> Result<T, ClientError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, ClientError>>,
    {
        self.read_with_checkpoint(operation, request, &|| Ok(())).await
    }
    pub(crate) async fn read_with_checkpoint<T, F, Fut>(&self, operation: &'static str, request: F, check: &(dyn Fn() -> Result<(), ClientError> + Sync)) -> Result<T, ClientError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, ClientError>>,
    {
        for attempt in 0..3 {
            check()?;
            self.scheduler.acquire().await;
            check()?;
            let started = Instant::now();
            let result = request().await;
            bsv_home_tracing::info_with_fields!(
                "BSV provider request",
                operation = operation,
                attempt = attempt,
                elapsed_ms = started.elapsed().as_millis(),
                success = result.is_ok(),
                limited = rate_limited(&result)
            );
            let retry = match &result {
                Ok(_) => false,
                Err(ClientError::Timeout | ClientError::Network(_)) => true,
                Err(ClientError::Http { status, .. }) => *status == 429 || (500..=599).contains(status),
                Err(ClientError::Serialization(_)) => false,
            };
            if retry && attempt < 2 {
                check()?;
                sleep(Duration::from_secs(1 << attempt)).await;
            } else {
                return result;
            }
        }
        Err(ClientError::Network("BSV read retry budget exhausted".to_string()))
    }
    pub(crate) async fn get<R: DeserializeOwned + Send>(&self, target: BsvTarget) -> Result<R, ClientError> {
        let path = target.path();
        self.read("query", || async {
            let body = self.client.get_text(&path).await?;
            serde_json::from_str(&body).map_err(ClientError::from)
        })
        .await
    }
    pub(crate) async fn transaction_info(&self, txid: &str) -> Result<TransactionInfo, ProviderError> {
        Txid::from_str(txid)?;
        let value: TransactionInfo = self.get(BsvTarget::Transaction { txid: txid.to_string() }).await?;
        if value.txid != txid {
            return Err("BSV transaction id mismatch".into());
        }
        Ok(value)
    }
    pub(crate) async fn address_pages<T: DeserializeOwned + Send>(
        &self,
        address: &str,
        first: Option<AddressResult<T>>,
        target: impl Fn(Option<String>) -> BsvTarget,
    ) -> Result<Vec<T>, ProviderError> {
        let mut results = Vec::new();
        let mut tokens = HashSet::new();
        let mut token: Option<String> = None;
        let mut first = first;
        for _ in 0..MAX_ADDRESS_PAGES {
            let page = match first.take() {
                Some(page) => page,
                None => self.get(target(token.clone())).await?,
            };
            page.validate(address)?;
            results.extend(page.result);
            match page.next_page.filter(|token| !token.is_empty()) {
                Some(next) if tokens.insert(next.clone()) => token = Some(next),
                Some(_) => return Err("BSV pagination token repeated".into()),
                None => return Ok(results),
            }
        }
        Err("BSV pagination exceeded 100 pages".into())
    }
    pub(crate) async fn unspent(&self, address: &str) -> Result<Vec<Unspent>, ProviderError> {
        let outputs: Vec<Unspent> = self
            .address_pages(address, None, |token| BsvTarget::Unspent {
                address: address.to_string(),
                token,
            })
            .await?;
        if outputs.iter().filter(|output| output.status == "unconfirmed").count() >= MAX_UNCONFIRMED_RESULTS {
            return Err("BSV unconfirmed output response reached supplier limit".into());
        }
        Ok(outputs)
    }
    pub(crate) async fn history(&self, address: &str) -> Result<Vec<History>, ProviderError> {
        let (_, history) = self.histories(&[address.to_string()]).await?.pop().ok_or("WhatsOnChain history response missing")?;
        Ok(history)
    }
    pub(crate) async fn histories(&self, addresses: &[String]) -> Result<Vec<(String, Vec<History>)>, ProviderError> {
        if addresses.is_empty() {
            return Err("BSV wallet requires at least one owned address".into());
        }
        let mut unique = HashSet::new();
        for address in addresses {
            BsvAddress::from_str(address)?;
            if !unique.insert(address) {
                return Err("duplicate BSV wallet address".into());
            }
        }
        let path = BsvTarget::HistoryAll.path();
        let mut histories = Vec::with_capacity(addresses.len());
        for batch in addresses.chunks(MAX_HISTORY_ADDRESSES) {
            let body = serde_json::json!({"addresses": batch});
            let response: Vec<AddressHistory> = self
                .read("history", || async {
                    let response = self.client.post_text(&path, &body).await?;
                    serde_json::from_str(&response).map_err(ClientError::from)
                })
                .await?;
            if response.len() != batch.len() {
                return Err("invalid WhatsOnChain history response count".into());
            }
            let mut by_address = HashMap::with_capacity(response.len());
            for response in response {
                if !batch.contains(&response.address) {
                    return Err("unexpected WhatsOnChain history address".into());
                }
                if by_address.insert(response.address.clone(), response).is_some() {
                    return Err("duplicate WhatsOnChain history address".into());
                }
            }
            for address in batch {
                let response = by_address.remove(address).ok_or("WhatsOnChain history response missing")?;
                response.validate(address)?;
                let mut history = Vec::new();
                for (confirmation, section) in [(Confirmation::Unconfirmed, response.unconfirmed), (Confirmation::Confirmed, response.confirmed)] {
                    let first = AddressResult {
                        address: response.address.clone(),
                        result: section.result,
                        error: section.error,
                        next_page: section.next_page,
                    };
                    let results = self
                        .address_pages(address, Some(first), |token| BsvTarget::History {
                            address: address.to_string(),
                            confirmation,
                            token,
                        })
                        .await?;
                    let unconfirmed = match confirmation {
                        Confirmation::Confirmed => false,
                        Confirmation::Unconfirmed => true,
                    };
                    if unconfirmed && results.len() >= MAX_UNCONFIRMED_RESULTS {
                        return Err("BSV unconfirmed history response reached supplier limit".into());
                    }
                    history.extend(results);
                }
                histories.push((address.clone(), history));
            }
        }
        Ok(histories)
    }
    pub async fn native_unspent(&self, address: &str) -> Result<Vec<Unspent>, ProviderError> {
        let owner = BsvAddress::from_str(address)?;
        let mut transactions = HashMap::new();
        let mut outpoints = HashSet::new();
        let mut outputs = Vec::new();
        for output in self.unspent(address).await? {
            if !outpoints.insert((output.tx_hash.clone(), output.tx_pos)) {
                return Err("duplicate BSV UTXO".into());
            }
            match output.status.as_str() {
                "confirmed" if output.height.is_some_and(|height| height > 0) => {}
                "unconfirmed" => {}
                _ => return Err("invalid BSV confirmation status".into()),
            }
            if output.spent_in_mempool || output.value == 1 {
                continue;
            }
            if !transactions.contains_key(&output.tx_hash) {
                transactions.insert(output.tx_hash.clone(), self.transaction_info(&output.tx_hash).await?);
            }
            let transaction = transactions.get(&output.tx_hash).ok_or("missing BSV previous transaction")?;
            if output.status == "confirmed" && !transaction.confirmations.is_some_and(|confirmations| confirmations > 0) {
                return Err("BSV source transaction is no longer confirmed".into());
            }
            let previous = transaction.output(output.tx_pos)?;
            if previous.satoshis()? != output.value {
                return Err("BSV UTXO source value mismatch".into());
            }
            if previous.native_value(&owner)?.is_none() {
                continue;
            }
            if transaction.is_coinbase() && !transaction.confirmations.is_some_and(|confirmations| confirmations >= 100) {
                continue;
            }
            outputs.push(output);
        }
        Ok(outputs)
    }
}

pub(crate) fn rate_limited<T>(result: &Result<T, ClientError>) -> bool {
    match result {
        Ok(_) => false,
        Err(ClientError::Http { status, .. }) => *status == 429,
        Err(ClientError::Network(_) | ClientError::Timeout | ClientError::Serialization(_)) => false,
    }
}
