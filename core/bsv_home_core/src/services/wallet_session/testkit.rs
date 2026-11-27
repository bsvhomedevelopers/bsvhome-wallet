use std::sync::Mutex;

use primitives::WalletId;

use super::BsvHomeWalletSessionStore;
use crate::services::error::BsvHomeServiceError;

#[derive(Default)]
pub struct MemoryWalletSessionStore {
    pub current: Mutex<Option<WalletId>>,
}

impl BsvHomeWalletSessionStore for MemoryWalletSessionStore {
    fn get_current_wallet_id(&self) -> Result<Option<WalletId>, BsvHomeServiceError> {
        Ok(self.current.lock().unwrap().clone())
    }
    fn set_current_wallet_id(&self, wallet_id: Option<WalletId>) -> Result<(), BsvHomeServiceError> {
        *self.current.lock().unwrap() = wallet_id;
        Ok(())
    }
}
