pub mod rules;
pub mod store;
#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use primitives::{Wallet, WalletId};

use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::BsvHomeWalletStore;

pub use store::BsvHomeWalletSessionStore;

#[derive(uniffi::Object)]
pub struct BsvHomeWalletSessionService {
    store: Arc<dyn BsvHomeWalletSessionStore>,
    wallets: Arc<dyn BsvHomeWalletStore>,
    pub(crate) environment: BsvHomeRuntimeEnvironment,
}

#[uniffi::export]
impl BsvHomeWalletSessionService {
    #[uniffi::constructor]
    pub fn new(store: Arc<dyn BsvHomeWalletSessionStore>, wallets: Arc<dyn BsvHomeWalletStore>) -> Self {
        Self::new_with_environment(store, wallets, BsvHomeRuntimeEnvironment::Production)
    }

    #[uniffi::constructor]
    pub fn new_with_environment(store: Arc<dyn BsvHomeWalletSessionStore>, wallets: Arc<dyn BsvHomeWalletStore>, environment: BsvHomeRuntimeEnvironment) -> Self {
        Self { store, wallets, environment }
    }

    pub fn get_current_wallet_id(&self) -> Result<Option<WalletId>, BsvHomeServiceError> {
        self.store.get_current_wallet_id()
    }

    pub fn set_current_wallet_id(&self, wallet_id: Option<WalletId>) -> Result<(), BsvHomeServiceError> {
        if self.store.get_current_wallet_id()? == wallet_id {
            return Ok(());
        }
        self.store.set_current_wallet_id(wallet_id)
    }

    pub async fn get_current_wallet(&self) -> Result<Option<Wallet>, BsvHomeServiceError> {
        match self.store.get_current_wallet_id()? {
            Some(wallet_id) => self.get_wallet(wallet_id).await,
            None => Ok(None),
        }
    }

    pub fn shows_rewards(&self, wallets: Vec<Wallet>) -> bool {
        rules::shows_rewards(&wallets)
    }

    pub async fn get_wallets(&self) -> Result<Vec<Wallet>, BsvHomeServiceError> {
        Ok(self
            .wallets
            .get_wallets()
            .await?
            .into_iter()
            .filter(|wallet| self.environment.allows_wallet(wallet))
            .map(|wallet| self.environment.visible_wallet(wallet))
            .collect())
    }

    pub async fn get_wallet(&self, wallet_id: WalletId) -> Result<Option<Wallet>, BsvHomeServiceError> {
        Ok(self
            .wallets
            .get_wallet(wallet_id)
            .await?
            .filter(|wallet| self.environment.allows_wallet(wallet))
            .map(|wallet| self.environment.visible_wallet(wallet)))
    }
}

impl BsvHomeWalletSessionService {
    pub fn current_wallet_id(&self) -> Result<WalletId, BsvHomeServiceError> {
        self.store.get_current_wallet_id()?.ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: "no current wallet".to_string(),
        })
    }

    pub async fn current_wallet(&self) -> Result<Wallet, BsvHomeServiceError> {
        let wallet_id = self.current_wallet_id()?;
        self.get_wallet(wallet_id.clone()).await?.ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: format!("wallet {} not found", wallet_id.id()),
        })
    }
}
