use async_trait::async_trait;
use primitives::Wallet;

use crate::BsvHomeCoreError;
use crate::models::transaction::{BsvHomeSignedTransaction, BsvHomeSignerInput};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeTransactionSigner: Send + Sync {
    async fn sign(&self, wallet: Wallet, input: BsvHomeSignerInput) -> Result<Vec<BsvHomeSignedTransaction>, BsvHomeCoreError>;
}
