use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::channel::oneshot;
use primitives::currency::Currency;
use primitives::{AssetId, Chain, PriceAlert, StreamMessage, WalletId};

use super::{BsvHomeStreamConnection, BsvHomeStreamSubscriptionService};
use crate::services::balance::{BsvHomeAssetBalance, BsvHomeBalanceRecord, BsvHomeBalanceStore};
use crate::services::error::BsvHomeServiceError;
use crate::services::price_alert::BsvHomePriceAlertStore;

pub struct SubscriptionTestkit {
    pub service: BsvHomeStreamSubscriptionService,
    pub balances: Arc<MemoryBalanceStore>,
    pub connection: Arc<MemoryStreamConnection>,
    pub wallet_id: WalletId,
}

impl SubscriptionTestkit {
    pub fn new(enabled: &[Chain], alerted: &[Chain]) -> Self {
        let wallet_id = WalletId::Multicoin("0x1".into());
        let balances = Arc::new(MemoryBalanceStore {
            enabled: Mutex::new(HashMap::from([(wallet_id.clone(), asset_ids(enabled))])),
            ..Default::default()
        });
        let connection = Arc::new(MemoryStreamConnection::default());
        connection.connected.store(true, Ordering::SeqCst);
        let service = BsvHomeStreamSubscriptionService::new(balances.clone(), Arc::new(MemoryPriceAlertStore(asset_ids(alerted))), connection.clone());
        Self {
            service,
            balances,
            connection,
            wallet_id,
        }
    }
}

pub fn asset_ids(chains: &[Chain]) -> Vec<AssetId> {
    chains.iter().copied().map(AssetId::from_chain).collect()
}

#[derive(Default)]
pub struct MemoryBalanceStore {
    enabled: Mutex<HashMap<WalletId, Vec<AssetId>>>,
    pub rows: Mutex<Vec<BsvHomeAssetBalance>>,
    pub saved: Mutex<Vec<(WalletId, Vec<BsvHomeBalanceRecord>)>>,
    pub failure: Mutex<Option<BsvHomeServiceError>>,
}

#[async_trait]
impl BsvHomeBalanceStore for MemoryBalanceStore {
    async fn get_available_balances(&self, _wallet_id: WalletId, asset_ids: Vec<AssetId>) -> Result<Vec<BsvHomeAssetBalance>, BsvHomeServiceError> {
        Ok(self.rows.lock().unwrap().iter().filter(|balance| asset_ids.contains(&balance.asset_id)).cloned().collect())
    }

    async fn update_balances(&self, wallet_id: WalletId, balances: Vec<BsvHomeBalanceRecord>) -> Result<(), BsvHomeServiceError> {
        if let Some(error) = self.failure.lock().unwrap().clone() {
            return Err(error);
        }
        self.saved.lock().unwrap().push((wallet_id, balances));
        Ok(())
    }

    async fn get_enabled_asset_ids(&self, wallet_id: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        Ok(self.enabled.lock().unwrap().get(&wallet_id).cloned().unwrap_or_default())
    }

    async fn set_assets_enabled(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>, enabled: bool) -> Result<(), BsvHomeServiceError> {
        let mut wallets = self.enabled.lock().unwrap();
        let stored = wallets.entry(wallet_id).or_default();
        if enabled {
            stored.extend(asset_ids);
        } else {
            stored.retain(|asset_id| !asset_ids.contains(asset_id));
        }
        Ok(())
    }

    async fn set_asset_pinned(&self, _wallet_id: WalletId, _asset_id: AssetId, _pinned: bool) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }
}

struct MemoryPriceAlertStore(Vec<AssetId>);

#[async_trait]
impl BsvHomePriceAlertStore for MemoryPriceAlertStore {
    async fn get_price_alerts(&self, _asset_id: Option<AssetId>) -> Result<Vec<PriceAlert>, BsvHomeServiceError> {
        Ok(self
            .0
            .iter()
            .map(|asset_id| PriceAlert {
                asset_id: asset_id.clone(),
                currency: Currency::USD,
                price: None,
                price_percent_change: None,
                price_direction: None,
                identifier: String::new(),
                last_notified_at: None,
            })
            .collect())
    }

    async fn update_price_alerts(&self, _alerts: Vec<PriceAlert>, _delete_ids: Vec<String>) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryStreamConnection {
    pub connected: AtomicBool,
    pub fail_next_send: AtomicBool,
    pause: Mutex<Option<oneshot::Receiver<()>>>,
    sent: Mutex<Vec<String>>,
}

impl MemoryStreamConnection {
    pub fn pause_next_send(&self) -> oneshot::Sender<()> {
        let (sender, receiver) = oneshot::channel();
        *self.pause.lock().unwrap() = Some(receiver);
        sender
    }

    pub fn messages(&self) -> Vec<(&'static str, Vec<AssetId>)> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .filter_map(|message| serde_json::from_str::<StreamMessage>(message).ok())
            .map(|message| match message {
                StreamMessage::SubscribePrices(prices) => ("subscribe", prices.assets),
                StreamMessage::AddPrices(prices) => ("add", prices.assets),
                StreamMessage::GetPrices(prices) => ("get", prices.assets),
                StreamMessage::UnsubscribePrices(prices) => ("unsubscribe", prices.assets),
                StreamMessage::SubscribeRealtimePrices(prices) => ("subscribeRealtime", prices.assets),
                StreamMessage::UnsubscribeRealtimePrices(prices) => ("unsubscribeRealtime", prices.assets),
            })
            .collect()
    }
}

#[async_trait]
impl BsvHomeStreamConnection for MemoryStreamConnection {
    async fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn send(&self, message: String) -> Result<(), BsvHomeServiceError> {
        let pause = self.pause.lock().unwrap().take();
        if let Some(pause) = pause {
            pause.await.unwrap();
        }
        if self.fail_next_send.swap(false, Ordering::SeqCst) {
            return Err(BsvHomeServiceError::Platform { msg: "send failed".into() });
        }
        self.sent.lock().unwrap().push(message);
        Ok(())
    }
}
