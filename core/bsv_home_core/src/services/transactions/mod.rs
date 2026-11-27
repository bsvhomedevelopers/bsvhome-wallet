pub mod details;
pub mod model;
pub mod rules;
pub mod store;
#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests;

use crate::services::error::BsvHomeServiceError;
use std::sync::Arc;

use chrono::Utc;
use primitives::{AssetId, Chain, Currency, Transaction, Wallet, WalletId};

pub use details::BsvHomeTransactionDetailsService;
pub use model::{
    BsvHomeAmountSign, BsvHomeSwapAgain, BsvHomeSwapProgress, BsvHomeSwapProgressStep, BsvHomeTransactionAmount, BsvHomeTransactionDetailRow, BsvHomeTransactionDetailRows,
    BsvHomeTransactionDetailSection, BsvHomeTransactionFilter, BsvHomeTransactionHeader, BsvHomeTransactionHeaderAction, BsvHomeTransactionHeaderKind,
    BsvHomeTransactionParticipant, BsvHomeTransactionParticipantRole, BsvHomeTransactionRow, BsvHomeTransactionRowSubtitle, BsvHomeTransactionRowValue,
    BsvHomeTransactionStateTone, BsvHomeTransactionStatus, BsvHomeTransactionTitle,
};
pub use store::BsvHomeTransactionStore;

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::gateway::BsvHomeGateway;
use crate::services::assets::BsvHomeAssetsService;
use crate::services::chain::rules as chain_rules;
use crate::services::name::BsvHomeAddressStore;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::transaction_state::BsvHomeTransactionStatusService;
use crate::services::wallet::addresses::BsvHomeBsvAddressPool;
use crate::services::wallet_preferences::BsvHomeWalletPreferencesService;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomeTransactionsService {
    gateway: Arc<BsvHomeGateway>,
    api: Arc<BsvHomeDeviceApiClient>,
    assets: Arc<BsvHomeAssetsService>,
    store: Arc<dyn BsvHomeTransactionStore>,
    address_store: Arc<dyn BsvHomeAddressStore>,
    wallet_preferences: Arc<BsvHomeWalletPreferencesService>,
    preferences: Arc<BsvHomePreferencesService>,
    session: Arc<BsvHomeWalletSessionService>,
    transaction_status: Arc<dyn BsvHomeTransactionStatusService>,
    addresses: Arc<BsvHomeBsvAddressPool>,
}

#[uniffi::export]
impl BsvHomeTransactionsService {
    #[uniffi::constructor]
    pub fn new(
        api: Arc<BsvHomeDeviceApiClient>,
        gateway: Arc<BsvHomeGateway>,
        assets: Arc<BsvHomeAssetsService>,
        store: Arc<dyn BsvHomeTransactionStore>,
        address_store: Arc<dyn BsvHomeAddressStore>,
        wallet_preferences: Arc<BsvHomeWalletPreferencesService>,
        preferences: Arc<BsvHomePreferencesService>,
        session: Arc<BsvHomeWalletSessionService>,
        transaction_status: Arc<dyn BsvHomeTransactionStatusService>,
        addresses: Arc<BsvHomeBsvAddressPool>,
    ) -> Self {
        Self {
            api,
            gateway,
            assets,
            store,
            address_store,
            wallet_preferences,
            preferences,
            session,
            transaction_status,
            addresses,
        }
    }

    pub fn filter_chains(&self, wallet: Wallet) -> Vec<Chain> {
        chain_rules::wallet_chains_by_rank(&self.session.environment.visible_wallet(wallet))
    }

    pub fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }

    pub async fn sync(&self, asset_id: Option<AssetId>) -> Result<(), BsvHomeServiceError> {
        self.sync_wallet(self.session.current_wallet_id()?, asset_id).await
    }

    pub async fn sync_wallet(&self, wallet_id: WalletId, asset_id: Option<AssetId>) -> Result<(), BsvHomeServiceError> {
        if let Some(asset_id) = &asset_id {
            self.session.environment.require_asset(asset_id)?;
        }
        let wallet = self.session.get_wallet(wallet_id.clone()).await?.ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: "wallet not found".to_string(),
        })?;
        if asset_id.as_ref().is_some_and(|asset| asset.chain == Chain::BitcoinSv && !asset.is_native()) {
            return Err(BsvHomeServiceError::Unsupported {
                msg: "BSV_API_UNSUPPORTED: token history".to_string(),
            });
        }
        if let Some(asset_id) = asset_id {
            if wallet.account(asset_id.chain).is_none() {
                return Err(BsvHomeServiceError::InvalidInput {
                    msg: format!("wallet has no {} account for transaction history", asset_id.chain),
                });
            }
            return match asset_id.chain {
                Chain::TronShasta => self.sync_address_history(wallet_id, &wallet, Chain::TronShasta, 100).await,
                Chain::BitcoinSv => self.sync_address_history(wallet_id, &wallet, Chain::BitcoinSv, 1000).await,
                _ => self.sync_bsv_home_history(wallet_id, Some(asset_id)).await,
            };
        }

        let shasta = self.sync_address_history(wallet_id.clone(), &wallet, Chain::TronShasta, 100).await;
        let bsv = self.sync_address_history(wallet_id.clone(), &wallet, Chain::BitcoinSv, 1000).await;
        let hosted = if wallet.accounts.iter().any(|account| account.chain.supports_bsv_home_backend()) {
            self.sync_bsv_home_history(wallet_id, None).await
        } else {
            Ok(())
        };
        let failures = [("Shasta", shasta), ("BSV", bsv), ("Hosted", hosted)]
            .into_iter()
            .filter_map(|(source, result)| result.err().map(|error| format!("{source}: {error}")))
            .collect::<Vec<_>>();
        if failures.is_empty() {
            Ok(())
        } else {
            Err(BsvHomeServiceError::Core { msg: failures.join("; ") })
        }
    }
}

impl BsvHomeTransactionsService {
    async fn sync_address_history(&self, wallet_id: WalletId, wallet: &Wallet, chain: Chain, limit: u32) -> Result<(), BsvHomeServiceError> {
        let Some(account) = wallet.account(chain) else {
            return Ok(());
        };
        let mut transactions = if chain == Chain::BitcoinSv {
            self.addresses.transactions(wallet_id.clone(), limit).await?
        } else {
            self.gateway.get_address_transactions(chain, account.address.clone(), limit).await?
        };
        if chain.is_testnet() {
            let supported = self.session.environment.development_asset_ids(wallet);
            transactions.retain(|transaction| supported.contains(&transaction.asset_id));
        }
        self.save_address_history(wallet_id, transactions).await
    }

    pub(crate) async fn save_address_history(&self, wallet_id: WalletId, transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError> {
        let pending = rules::pending_transactions(&transactions);
        self.store.save_transactions(wallet_id.clone(), transactions).await?;
        if !pending.is_empty() {
            self.transaction_status.track(wallet_id, pending);
        }
        Ok(())
    }

    async fn sync_bsv_home_history(&self, wallet_id: WalletId, asset_id: Option<AssetId>) -> Result<(), BsvHomeServiceError> {
        let from_timestamp = self.wallet_preferences.get_transactions_timestamp(wallet_id.clone(), asset_id.clone());
        let timestamp = Utc::now().timestamp() as u64;
        let response = self
            .api
            .client
            .get_transactions(wallet_id.id(), asset_id.as_ref().map(|asset_id| asset_id.to_string()), from_timestamp)
            .await
            .map_err(BsvHomeApiError::from)?;

        let new_asset_ids = self.assets.sync_missing_assets(rules::transaction_asset_ids(&response.transactions)).await?;
        if !new_asset_ids.is_empty() {
            self.assets.add_missing_balances(wallet_id.clone(), new_asset_ids).await?;
        }
        let pending = rules::pending_transactions(&response.transactions)
            .into_iter()
            .filter(|transaction| self.session.environment.allows_asset(&transaction.asset_id))
            .collect::<Vec<_>>();
        self.store.save_transactions(wallet_id.clone(), response.transactions).await?;
        self.address_store.save_address_names(response.address_names).await?;
        self.wallet_preferences.set_transactions_timestamp(wallet_id.clone(), asset_id, timestamp)?;
        if !pending.is_empty() {
            self.transaction_status.track(wallet_id, pending);
        }
        Ok(())
    }
}
