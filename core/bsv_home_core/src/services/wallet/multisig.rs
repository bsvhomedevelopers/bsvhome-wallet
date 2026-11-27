use primitives::{Wallet, WalletId, WalletSource};

use super::{BsvHomeWalletImportResult, BsvHomeWalletService};
use crate::services::{bsv_multisig::bsv_multisig_signing_wallets, chain::BsvHomeRuntimeEnvironment, error::BsvHomeServiceError};

#[uniffi::export]
impl BsvHomeWalletService {
    pub fn is_multisig_identity(&self, wallet_id: WalletId) -> bool {
        match self.session.environment {
            BsvHomeRuntimeEnvironment::WowPay => self.app_preferences.is_multisig_identity(&wallet_id),
            BsvHomeRuntimeEnvironment::Production | BsvHomeRuntimeEnvironment::LocalDevelopment => false,
        }
    }

    pub fn complete_multisig_identity_import(&self, result: BsvHomeWalletImportResult, source: WalletSource) -> Result<Wallet, BsvHomeServiceError> {
        if self.session.environment != BsvHomeRuntimeEnvironment::WowPay {
            return Err(BsvHomeServiceError::Unsupported {
                msg: "dedicated multisig identities are unavailable in this environment".into(),
            });
        }
        let wallet = match &result {
            BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
        };
        let eligible = bsv_multisig_signing_wallets(vec![wallet.clone()]).map_err(|_| BsvHomeServiceError::InvalidInput {
            msg: "invalid BSV signing identity".into(),
        })?;
        if eligible.is_empty() {
            return Err(BsvHomeServiceError::InvalidInput {
                msg: "a signing identity requires a local BSV wallet".into(),
            });
        }
        if let BsvHomeWalletImportResult::New { .. } = &result {
            self.app_preferences.mark_multisig_identity(&wallet.id)?;
        }
        let select = match source {
            WalletSource::Create => self.session.get_current_wallet_id()?.is_none(),
            WalletSource::Import => true,
        };
        if select {
            self.session.set_current_wallet_id(Some(wallet.id.clone()))?;
        }
        Ok(wallet.clone())
    }
}
