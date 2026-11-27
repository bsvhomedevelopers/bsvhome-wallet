use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::BsvHomePreferencesStore;
use crate::services::error::BsvHomeServiceError;

#[derive(Default)]
pub struct MemoryPreferencesStore {
    pub values: Mutex<HashMap<String, String>>,
    pub fail_set: AtomicBool,
}

impl BsvHomePreferencesStore for MemoryPreferencesStore {
    fn get(&self, key: String) -> Option<String> {
        self.values.lock().unwrap().get(&key).cloned()
    }

    fn set(&self, key: String, value: String) -> Result<(), BsvHomeServiceError> {
        if self.fail_set.load(Ordering::SeqCst) {
            return Err(BsvHomeServiceError::Store {
                msg: "preference write failed".into(),
            });
        }
        self.values.lock().unwrap().insert(key, value);
        Ok(())
    }

    fn remove(&self, key: String) -> Result<(), BsvHomeServiceError> {
        self.values.lock().unwrap().remove(&key);
        Ok(())
    }

    fn clear(&self) -> Result<(), BsvHomeServiceError> {
        self.values.lock().unwrap().clear();
        Ok(())
    }
}
