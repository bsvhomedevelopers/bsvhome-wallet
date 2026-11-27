use crate::services::error::BsvHomeServiceError;
use primitives::WalletId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeKeystoreAuthentication {
    Biometrics,
    Passcode,
    None,
}

#[uniffi::export(rust, foreign)]
pub trait BsvHomeKeystorePassword: Send + Sync {
    fn get_password(&self, create_if_missing: bool) -> Result<String, BsvHomeServiceError>;
    fn get_wallet_password(&self, wallet_id: WalletId) -> Result<Option<String>, BsvHomeServiceError>;
    fn delete_wallet_password(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError>;
    fn authentication(&self) -> Result<BsvHomeKeystoreAuthentication, BsvHomeServiceError>;
}
