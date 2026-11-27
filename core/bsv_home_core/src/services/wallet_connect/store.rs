use async_trait::async_trait;
use primitives::{WalletConnection, WalletConnectionSession};

use crate::services::error::BsvHomeServiceError;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeConnectionStore: Send + Sync {
    async fn get_connection(&self, session_id: String) -> Result<Option<WalletConnection>, BsvHomeServiceError>;
    async fn get_sessions(&self) -> Result<Vec<WalletConnectionSession>, BsvHomeServiceError>;
    async fn add_connection(&self, connection: WalletConnection) -> Result<(), BsvHomeServiceError>;
    async fn update_session(&self, session: WalletConnectionSession) -> Result<(), BsvHomeServiceError>;
    async fn delete_sessions(&self, session_ids: Vec<String>) -> Result<(), BsvHomeServiceError>;
}
