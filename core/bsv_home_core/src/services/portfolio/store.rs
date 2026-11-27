use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{PortfolioAsset, WalletId};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomePortfolioStore: Send + Sync {
    async fn get_wallet_assets(&self, wallet_id: WalletId) -> Result<Vec<PortfolioAsset>, BsvHomeServiceError>;
}
