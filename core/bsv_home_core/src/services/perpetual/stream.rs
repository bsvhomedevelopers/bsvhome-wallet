use std::sync::Arc;

use async_trait::async_trait;
use bsv_home_hypercore::{
    models::websocket::HyperliquidRequest,
    provider::{websocket_mapper::account_subscriptions, websocket_subscriptions::WebSocketSubscriptions},
};
use futures::lock::Mutex;
use primitives::chart::ChartCandleUpdate;
use primitives::{PerpetualAccountMode, WalletId};

use super::BsvHomePerpetualService;
use super::model::BsvHomePerpetualSocketUpdate;
use crate::models::perpetual::BsvHomePerpetualSubscription;
use crate::services::error::BsvHomeServiceError;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomePerpetualStreamConnection: Send + Sync {
    async fn send(&self, message: String) -> Result<(), BsvHomeServiceError>;
}

#[derive(uniffi::Object)]
pub struct BsvHomePerpetualStreamService {
    perpetual: Arc<BsvHomePerpetualService>,
    connection: Arc<dyn BsvHomePerpetualStreamConnection>,
    subscriptions: Mutex<WebSocketSubscriptions>,
}

#[uniffi::export]
impl BsvHomePerpetualStreamService {
    #[uniffi::constructor]
    pub fn new(perpetual: Arc<BsvHomePerpetualService>, connection: Arc<dyn BsvHomePerpetualStreamConnection>) -> Self {
        Self {
            perpetual,
            connection,
            subscriptions: Mutex::new(WebSocketSubscriptions::new()),
        }
    }

    pub async fn connected(&self, address: String, mode: PerpetualAccountMode) -> Result<(), BsvHomeServiceError> {
        let requests = self.subscriptions.lock().await.connected(account_subscriptions(address, mode));
        self.send(requests).await
    }

    pub async fn disconnected(&self) {
        self.subscriptions.lock().await.disconnected();
    }

    pub async fn subscribe(&self, subscription: BsvHomePerpetualSubscription) -> Result<(), BsvHomeServiceError> {
        let requests = self.subscriptions.lock().await.subscribe(subscription.map());
        self.send(requests).await
    }

    pub async fn unsubscribe(&self, subscription: BsvHomePerpetualSubscription) -> Result<(), BsvHomeServiceError> {
        let requests = self.subscriptions.lock().await.unsubscribe(&subscription.map());
        self.send(requests).await
    }

    pub async fn handle(&self, wallet_id: WalletId, mode: PerpetualAccountMode, data: Vec<u8>) -> Result<Option<ChartCandleUpdate>, BsvHomeServiceError> {
        match self.perpetual.apply_socket_message(wallet_id, mode, data).await? {
            BsvHomePerpetualSocketUpdate::Candle { candle } => Ok(Some(candle)),
            BsvHomePerpetualSocketUpdate::Applied | BsvHomePerpetualSocketUpdate::SubscriptionResponse { .. } | BsvHomePerpetualSocketUpdate::Unknown => Ok(None),
            BsvHomePerpetualSocketUpdate::Error { message } => Err(BsvHomeServiceError::Core { msg: message }),
        }
    }
}

impl BsvHomePerpetualStreamService {
    async fn send(&self, requests: Vec<HyperliquidRequest>) -> Result<(), BsvHomeServiceError> {
        for request in requests {
            let message = serde_json::to_string(&request).map_err(|error| BsvHomeServiceError::Core { msg: error.to_string() })?;
            self.connection.send(message).await?;
        }
        Ok(())
    }
}
