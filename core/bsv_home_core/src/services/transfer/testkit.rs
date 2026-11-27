use std::sync::Mutex;

use async_trait::async_trait;
use primitives::{RecentActivityType, WalletId};

use super::{BsvHomeRecentActivity, BsvHomeRecentActivityStore};
use crate::services::error::BsvHomeServiceError;

#[derive(Default)]
pub struct MemoryRecentActivityStore {
    pub added: Mutex<Vec<(BsvHomeRecentActivity, WalletId)>>,
}

#[async_trait]
impl BsvHomeRecentActivityStore for MemoryRecentActivityStore {
    async fn add(&self, activity: BsvHomeRecentActivity, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        self.added.lock().unwrap().push((activity, wallet_id));
        Ok(())
    }
    async fn clear(&self, _wallet_id: WalletId, _types: Vec<RecentActivityType>) -> Result<(), BsvHomeServiceError> {
        self.added.lock().unwrap().clear();
        Ok(())
    }
}
