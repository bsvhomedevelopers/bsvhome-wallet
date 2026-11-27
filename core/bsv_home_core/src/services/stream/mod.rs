pub mod connection;
pub mod rules;
pub mod subscription;
#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests;

pub use connection::BsvHomeStreamConnection;
pub use subscription::BsvHomeStreamSubscriptionService;

use crate::services::error::BsvHomeServiceError;
use std::sync::Arc;

use tracing::error;

use primitives::{Chain, StreamEvent, StreamTransactionsUpdate, SupportMessageSender, SupportStreamEvent};

use crate::services::balance::BsvHomeBalanceService;
use crate::services::device::BsvHomeDeviceService;
use crate::services::fiat::BsvHomeFiatService;
use crate::services::nft::BsvHomeNftService;
use crate::services::notification::BsvHomeNotificationStore;
use crate::services::perpetual::BsvHomePerpetualService;
use crate::services::price::BsvHomePriceService;
use crate::services::price_alert::BsvHomePriceAlertService;
use crate::services::support::BsvHomeSupportStore;
use crate::services::transactions::BsvHomeTransactionsService;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomeStreamService {
    price: Arc<BsvHomePriceService>,
    price_alert: Arc<BsvHomePriceAlertService>,
    balance: Arc<BsvHomeBalanceService>,
    transactions: Arc<BsvHomeTransactionsService>,
    nft: Arc<BsvHomeNftService>,
    perpetual: Arc<BsvHomePerpetualService>,
    fiat: Arc<BsvHomeFiatService>,
    notifications: Arc<dyn BsvHomeNotificationStore>,
    support: Arc<dyn BsvHomeSupportStore>,
    subscriptions: Arc<BsvHomeStreamSubscriptionService>,
    session: Arc<BsvHomeWalletSessionService>,
    device: Arc<BsvHomeDeviceService>,
}

#[uniffi::export]
impl BsvHomeStreamService {
    #[uniffi::constructor]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        price: Arc<BsvHomePriceService>,
        price_alert: Arc<BsvHomePriceAlertService>,
        balance: Arc<BsvHomeBalanceService>,
        transactions: Arc<BsvHomeTransactionsService>,
        nft: Arc<BsvHomeNftService>,
        perpetual: Arc<BsvHomePerpetualService>,
        fiat: Arc<BsvHomeFiatService>,
        notifications: Arc<dyn BsvHomeNotificationStore>,
        support: Arc<dyn BsvHomeSupportStore>,
        subscriptions: Arc<BsvHomeStreamSubscriptionService>,
        session: Arc<BsvHomeWalletSessionService>,
        device: Arc<BsvHomeDeviceService>,
    ) -> Self {
        Self {
            price,
            price_alert,
            balance,
            transactions,
            nft,
            perpetual,
            fiat,
            notifications,
            support,
            subscriptions,
            session,
            device,
        }
    }

    pub async fn prepare_connection(&self) -> Result<bool, BsvHomeServiceError> {
        if !self.session.environment.uses_bsv_home_hosted_services() {
            return Ok(false);
        }
        if !self.subscriptions.prepare_session(self.session.get_current_wallet_id()?).await? {
            return Ok(false);
        }
        if let Err(error) = self.device.synchronize_if_needed().await {
            error!(%error, "stream device synchronization failed");
        }
        Ok(true)
    }

    pub async fn connected(&self) -> Result<(), BsvHomeServiceError> {
        self.subscriptions.reconnect().await
    }

    pub async fn disconnected(&self) {
        self.subscriptions.reset().await;
    }

    pub async fn handle(&self, event: String) -> Result<(), BsvHomeServiceError> {
        let event = serde_json::from_str(&event).map_err(|error| BsvHomeServiceError::InvalidInput { msg: error.to_string() })?;
        self.handle_event(event).await
    }
}

impl BsvHomeStreamService {
    async fn handle_event(&self, event: StreamEvent) -> Result<(), BsvHomeServiceError> {
        match event {
            StreamEvent::Prices(payload) => {
                self.price.update_rates(payload.rates).await?;
                self.price.update_prices(payload.prices).await
            }
            StreamEvent::Balances(update) => self.balance.update(update.wallet_id, update.asset_ids).await,
            StreamEvent::Transactions(update) => sync_transaction_event(&self.transactions, &self.balance, update).await,
            StreamEvent::PriceAlerts(_) => self.price_alert.sync(None).await,
            StreamEvent::Nft(update) => self.nft.sync_wallet(update.wallet_id).await.map(|_| ()),
            StreamEvent::Perpetual(update) => {
                let Some(wallet) = self.session.get_wallet(update.wallet_id.clone()).await? else {
                    return Ok(());
                };
                let Some(account) = rules::hyperliquid_account(&wallet.accounts) else {
                    return Ok(());
                };
                self.perpetual.sync_positions(update.wallet_id, Chain::HyperCore, account.address.clone()).await.map(|_| ())
            }
            StreamEvent::InAppNotification(update) => self.notifications.save_notifications(vec![update.notification]).await,
            StreamEvent::FiatTransaction(update) => self.fiat.sync_transactions(update.wallet_id).await,
            StreamEvent::Support(SupportStreamEvent::Message(message)) => {
                let from_agent = matches!(message.sender, SupportMessageSender::Agent(_));
                self.support.save_messages(vec![message]).await?;
                if from_agent {
                    self.support.clear_typing()?;
                }
                Ok(())
            }
            StreamEvent::Support(SupportStreamEvent::Typing(typing)) => self.support.update_typing(typing),
        }
    }
}

async fn sync_transaction_event(transactions: &BsvHomeTransactionsService, balance: &BsvHomeBalanceService, update: StreamTransactionsUpdate) -> Result<(), BsvHomeServiceError> {
    let history = transactions.sync_wallet(update.wallet_id.clone(), None).await;
    let balance = balance.update(update.wallet_id, update.asset_ids).await;
    match (history, balance) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(history), Err(balance)) => Err(BsvHomeServiceError::Core {
            msg: format!("transaction history: {history}; balance update: {balance}"),
        }),
    }
}
