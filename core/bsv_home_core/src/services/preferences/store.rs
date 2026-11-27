use crate::services::error::BsvHomeServiceError;

#[uniffi::export(rust, foreign)]
pub trait BsvHomePreferencesStore: Send + Sync {
    fn get(&self, key: String) -> Option<String>;
    fn set(&self, key: String, value: String) -> Result<(), BsvHomeServiceError>;
    fn remove(&self, key: String) -> Result<(), BsvHomeServiceError>;
    fn clear(&self) -> Result<(), BsvHomeServiceError>;
}

#[uniffi::export(rust, foreign)]
pub trait BsvHomeSecureStore: Send + Sync {
    fn get(&self, key: String) -> Result<Option<String>, BsvHomeServiceError>;
    fn set(&self, key: String, value: String) -> Result<(), BsvHomeServiceError>;
    fn remove(&self, key: String) -> Result<(), BsvHomeServiceError>;
}
