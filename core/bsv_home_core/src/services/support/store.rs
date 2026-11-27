use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{SupportMessage, SupportTyping};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeSupportStore: Send + Sync {
    async fn save_messages(&self, messages: Vec<SupportMessage>) -> Result<(), BsvHomeServiceError>;
    async fn save_message(&self, id: String, message: SupportMessage) -> Result<(), BsvHomeServiceError>;
    fn update_typing(&self, typing: SupportTyping) -> Result<(), BsvHomeServiceError>;
    fn clear_typing(&self) -> Result<(), BsvHomeServiceError>;
}
