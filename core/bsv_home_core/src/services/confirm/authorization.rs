use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use primitives::{Chain, Wallet};

use super::BsvHomeConfirmError;
use crate::BsvHomeCoreError;
use crate::services::bsv_cold_wallet::{BsvHomeBsvColdWalletStore, bsv_cold_wallet_require_hot_signing};
use crate::services::transfer::BsvHomeTransferData;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeConfirmAuthorization: Send + Sync {
    async fn authorize(&self, wallet: Wallet, transfer: BsvHomeTransferData) -> Result<(), BsvHomeCoreError>;
}

pub(super) struct Authorization {
    pub port: Arc<dyn BsvHomeConfirmAuthorization>,
    pub session: Arc<BsvHomeWalletSessionService>,
}

pub(super) struct ExecutionScope {
    pub revision: Arc<AtomicU64>,
    pub generation: u64,
    pub wallet: Wallet,
    pub chain: Chain,
    pub session: Option<Arc<BsvHomeWalletSessionService>>,
    pub cold_wallet_store: Arc<dyn BsvHomeBsvColdWalletStore>,
}

impl ExecutionScope {
    pub fn unscoped(wallet: Wallet, chain: Chain, cold_wallet_store: Arc<dyn BsvHomeBsvColdWalletStore>) -> Self {
        Self {
            revision: Arc::new(AtomicU64::new(0)),
            generation: 0,
            wallet,
            chain,
            session: None,
            cold_wallet_store,
        }
    }

    pub fn check_signing(&self) -> Result<(), BsvHomeConfirmError> {
        self.check_revision()?;
        bsv_cold_wallet_require_hot_signing(self.wallet.clone(), self.chain, self.cold_wallet_store.clone())?;
        self.check_revision()
    }

    pub fn check_revision(&self) -> Result<(), BsvHomeConfirmError> {
        if self.revision.load(Ordering::SeqCst) != self.generation {
            return Err(BsvHomeConfirmError::Cancelled);
        }
        Ok(())
    }

    pub async fn check(&self) -> Result<(), BsvHomeConfirmError> {
        self.check_signing()?;
        if let Some(session) = &self.session {
            let current = session.get_current_wallet().await?;
            if current
                .as_ref()
                .is_none_or(|wallet| wallet.id != self.wallet.id || wallet.wallet_type != self.wallet.wallet_type || wallet.account(self.chain) != self.wallet.account(self.chain))
            {
                return Err(BsvHomeConfirmError::Cancelled);
            }
        }
        self.check_signing()
    }
}

pub(super) async fn check(scope: Option<&ExecutionScope>) -> Result<(), BsvHomeConfirmError> {
    if let Some(scope) = scope {
        scope.check().await?;
    }
    Ok(())
}

pub(super) fn authorization_error(error: BsvHomeCoreError) -> BsvHomeConfirmError {
    match error {
        BsvHomeCoreError::Cancelled => BsvHomeConfirmError::Cancelled,
        error => BsvHomeConfirmError::Load { msg: error.to_string() },
    }
}
