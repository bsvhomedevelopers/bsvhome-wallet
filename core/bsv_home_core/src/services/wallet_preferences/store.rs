use crate::services::error::BsvHomeServiceError;
use primitives::WalletId;

#[uniffi::export(rust, foreign)]
pub trait BsvHomeWalletPreferencesStore: Send + Sync {
    fn get(&self, wallet_id: WalletId, key: String) -> Option<String>;
    fn set(&self, wallet_id: WalletId, key: String, value: String) -> Result<(), BsvHomeServiceError>;
    fn delete_preferences(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError>;
}
