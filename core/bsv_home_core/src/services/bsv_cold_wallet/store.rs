use std::sync::Arc;

use async_trait::async_trait;
use primitives::{Wallet, WalletId};

use super::{BsvHomeBsvColdWalletActionScope, BsvHomeBsvColdWalletStateStamp, BsvHomeBsvColdWalletStoredState};
use crate::services::error::BsvHomeServiceError;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeBsvColdWalletStore: Send + Sync {
    fn state_stamp(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvColdWalletStateStamp>, BsvHomeServiceError>;
    fn has_state(&self, wallet_id: WalletId) -> Result<bool, BsvHomeServiceError>;
    async fn get(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvColdWalletStoredState>, BsvHomeServiceError>;
    async fn compare_and_swap(&self, wallet_id: WalletId, expected_revision: Option<u64>, state: BsvHomeBsvColdWalletStoredState) -> Result<bool, BsvHomeServiceError>;
}

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeBsvColdWalletSigner: Send + Sync {
    async fn authenticate_import(&self) -> Result<(), BsvHomeServiceError>;
    async fn authorize(&self, wallet: Wallet, scope: Arc<BsvHomeBsvColdWalletActionScope>) -> Result<(), BsvHomeServiceError>;
    async fn authenticate(&self, wallet: Wallet, scope: Arc<BsvHomeBsvColdWalletActionScope>) -> Result<(), BsvHomeServiceError>;
    async fn sign_request(&self, wallet: Wallet, request_json: String, review_json: String, scope: Arc<BsvHomeBsvColdWalletActionScope>) -> Result<String, BsvHomeServiceError>;
}
