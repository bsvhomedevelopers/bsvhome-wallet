use crate::services::error::BsvHomeServiceError;
use primitives::WalletId;

#[uniffi::export(rust, foreign)]
pub trait BsvHomeWalletSessionStore: Send + Sync {
    fn get_current_wallet_id(&self) -> Result<Option<WalletId>, BsvHomeServiceError>;
    fn set_current_wallet_id(&self, wallet_id: Option<WalletId>) -> Result<(), BsvHomeServiceError>;
}
