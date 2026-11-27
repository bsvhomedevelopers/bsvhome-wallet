use super::BsvHomeFileStore;
use crate::services::error::BsvHomeServiceError;

#[derive(Default)]
pub struct NoopFileStore;

impl BsvHomeFileStore for NoopFileStore {
    fn save_file(&self, _data: Vec<u8>, _extension: String) -> Result<String, BsvHomeServiceError> {
        Ok(String::new())
    }
    fn save_named_file(&self, _data: Vec<u8>, _file_name: String) -> Result<String, BsvHomeServiceError> {
        Ok(String::new())
    }
    fn exists(&self, _file_name: String) -> bool {
        false
    }
    fn path(&self, file_name: String) -> String {
        file_name
    }
    fn remove(&self, _file_name: String) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }
}
