use super::{BsvHomeBsvMultisigActionScope, BsvHomeBsvMultisigStoredState};
use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::Wallet;
use std::sync::Arc;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeBsvMultisigStore: Send + Sync {
    fn get_state(&self) -> Result<Option<BsvHomeBsvMultisigStoredState>, BsvHomeServiceError>;
    async fn compare_and_swap(&self, expected_revision: u64, json: String) -> Result<bool, BsvHomeServiceError>;
}

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeBsvMultisigSigner: Send + Sync {
    async fn public_key(&self, wallet: Wallet) -> Result<String, BsvHomeServiceError>;
    async fn authenticate(&self, wallet: Wallet, scope: Option<Arc<BsvHomeBsvMultisigActionScope>>) -> Result<(), BsvHomeServiceError>;
    async fn sign_proposal(
        &self,
        wallet: Wallet,
        policy_json: String,
        proposal_json: String,
        recipient: String,
        amount: u64,
        maximum_fee: u64,
        scope: Arc<BsvHomeBsvMultisigActionScope>,
    ) -> Result<String, BsvHomeServiceError>;
    async fn sign_funding(&self, wallet: Wallet, funding_json: String, review_json: String, scope: Arc<BsvHomeBsvMultisigActionScope>) -> Result<String, BsvHomeServiceError>;
}
