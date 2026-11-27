use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeStreamConnection: Send + Sync {
    async fn is_connected(&self) -> bool;
    async fn send(&self, message: String) -> Result<(), BsvHomeServiceError>;
}
