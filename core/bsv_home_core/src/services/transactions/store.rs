use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{Transaction, WalletId};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeTransactionStore: Send + Sync {
    async fn save_transactions(&self, wallet_id: WalletId, transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError>;
}
