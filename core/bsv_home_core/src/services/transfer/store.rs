use async_trait::async_trait;
use primitives::{RecentActivityType, WalletId};

use crate::services::error::BsvHomeServiceError;
use crate::services::transfer::BsvHomeRecentActivity;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeRecentActivityStore: Send + Sync {
    async fn add(&self, activity: BsvHomeRecentActivity, wallet_id: WalletId) -> Result<(), BsvHomeServiceError>;
    async fn clear(&self, wallet_id: WalletId, types: Vec<RecentActivityType>) -> Result<(), BsvHomeServiceError>;
}
