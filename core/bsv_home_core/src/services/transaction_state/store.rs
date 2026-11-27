use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{Transaction, TransactionId, TransactionState, WalletId};

use super::model::{BsvHomePendingTransaction, BsvHomeTransactionStateUpdate};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeTransactionStateStore: Send + Sync {
    async fn get_pending_transactions(&self) -> Result<Vec<BsvHomePendingTransaction>, BsvHomeServiceError>;
    async fn get_transaction(&self, wallet_id: WalletId, transaction_id: TransactionId) -> Result<Option<BsvHomePendingTransaction>, BsvHomeServiceError>;
    async fn add_transactions(&self, wallet_id: WalletId, transactions: Vec<Transaction>) -> Result<(), BsvHomeServiceError>;
    async fn get_state(&self, wallet_id: WalletId, transaction_id: TransactionId) -> Result<Option<TransactionState>, BsvHomeServiceError>;
    async fn update_transaction_hash(&self, wallet_id: WalletId, transaction_id: TransactionId, hash: String) -> Result<(), BsvHomeServiceError>;
    async fn delete_transaction(&self, wallet_id: WalletId, transaction_id: TransactionId) -> Result<(), BsvHomeServiceError>;
    async fn update_transaction(&self, wallet_id: WalletId, transaction_id: TransactionId, update: BsvHomeTransactionStateUpdate) -> Result<bool, BsvHomeServiceError>;
}
