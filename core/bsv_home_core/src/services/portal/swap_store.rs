use crate::services::error::BsvHomeServiceError;

#[uniffi::export(rust, foreign)]
pub trait BsvHomePortalSwapStore: Send + Sync {
    fn get(&self, scope: String) -> Result<Option<String>, BsvHomeServiceError>;
    fn set(&self, scope: String, value: String) -> Result<(), BsvHomeServiceError>;
}
