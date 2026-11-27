use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomeSecureStore;

#[derive(Default)]
pub(crate) struct MemoryOfflinePasswordStore {
    pub(crate) values: Mutex<HashMap<String, String>>,
    pub(crate) fail_get: AtomicBool,
    pub(crate) fail_set: AtomicBool,
    pub(crate) reads: AtomicUsize,
    pub(crate) writes: AtomicUsize,
    pub(crate) removes: AtomicUsize,
}

impl BsvHomeSecureStore for MemoryOfflinePasswordStore {
    fn get(&self, key: String) -> Result<Option<String>, BsvHomeServiceError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.fail_get.load(Ordering::SeqCst) {
            return Err(store_error());
        }
        Ok(self.values.lock().map_err(|_| store_error())?.get(&key).cloned())
    }

    fn set(&self, key: String, value: String) -> Result<(), BsvHomeServiceError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.fail_set.load(Ordering::SeqCst) {
            return Err(store_error());
        }
        self.values.lock().map_err(|_| store_error())?.insert(key, value);
        Ok(())
    }

    fn remove(&self, key: String) -> Result<(), BsvHomeServiceError> {
        self.removes.fetch_add(1, Ordering::SeqCst);
        self.values.lock().map_err(|_| store_error())?.remove(&key);
        Ok(())
    }
}

fn store_error() -> BsvHomeServiceError {
    BsvHomeServiceError::Store {
        msg: "offline password store unavailable".into(),
    }
}
