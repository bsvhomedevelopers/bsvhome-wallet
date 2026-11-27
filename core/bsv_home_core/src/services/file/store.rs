use crate::services::error::BsvHomeServiceError;

#[uniffi::export(rust, foreign)]
pub trait BsvHomeFileStore: Send + Sync {
    fn save_file(&self, data: Vec<u8>, extension: String) -> Result<String, BsvHomeServiceError>;
    fn save_named_file(&self, data: Vec<u8>, file_name: String) -> Result<String, BsvHomeServiceError>;
    fn exists(&self, file_name: String) -> bool;
    fn path(&self, file_name: String) -> String;
    fn remove(&self, file_name: String) -> Result<(), BsvHomeServiceError>;
}
