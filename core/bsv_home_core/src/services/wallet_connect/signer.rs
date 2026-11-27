use async_trait::async_trait;

use super::model::{BsvHomeWalletConnectMessageRequest, BsvHomeWalletConnectTransactionRequest};
use crate::services::error::BsvHomeServiceError;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeWalletConnectSigner: Send + Sync {
    async fn sign_message(&self, request: BsvHomeWalletConnectMessageRequest) -> Result<String, BsvHomeServiceError>;
    async fn sign_transaction(&self, request: BsvHomeWalletConnectTransactionRequest) -> Result<String, BsvHomeServiceError>;
}
