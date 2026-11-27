use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{FiatTransactionData, WalletId};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeFiatStore: Send + Sync {
    async fn set_transactions(&self, wallet_id: WalletId, transactions: Vec<FiatTransactionData>) -> Result<(), BsvHomeServiceError>;
}
