use super::addresses::BsvHomeBsvAddressStoredState;
use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{Wallet, WalletId};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeWalletStore: Send + Sync {
    async fn get_wallets(&self) -> Result<Vec<Wallet>, BsvHomeServiceError>;
    async fn get_wallet(&self, wallet_id: WalletId) -> Result<Option<Wallet>, BsvHomeServiceError>;
    async fn add_wallet(&self, wallet: Wallet) -> Result<(), BsvHomeServiceError>;
    async fn add_wallet_with_bsv_addresses(&self, wallet: Wallet, addresses: Option<BsvHomeBsvAddressStoredState>) -> Result<(), BsvHomeServiceError>;
    async fn delete_wallet(&self, wallet_id: WalletId) -> Result<bool, BsvHomeServiceError>;
    async fn set_pinned(&self, wallet_id: WalletId, pinned: bool) -> Result<(), BsvHomeServiceError>;
    async fn set_name(&self, wallet_id: WalletId, name: String) -> Result<(), BsvHomeServiceError>;
    async fn set_image_url(&self, wallet_id: WalletId, image_url: Option<String>) -> Result<(), BsvHomeServiceError>;
}
