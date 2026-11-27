use std::sync::Arc;

use crate::services::preferences::{BsvHomePreferencesStore, BsvHomeSecureStore};

pub(crate) struct PreferencesWrapper {
    pub(crate) preferences: Arc<dyn BsvHomePreferencesStore>,
}

impl primitives::Preferences for PreferencesWrapper {
    fn get(&self, key: String) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.preferences.get(key))
    }

    fn set(&self, key: String, value: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.preferences.set(key, value).map_err(Into::into)
    }

    fn remove(&self, key: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.preferences.remove(key).map_err(Into::into)
    }
}

pub(crate) struct SecureStoreWrapper {
    pub(crate) store: Arc<dyn BsvHomeSecureStore>,
}

impl primitives::Preferences for SecureStoreWrapper {
    fn get(&self, key: String) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        self.store.get(key).map_err(Into::into)
    }

    fn set(&self, key: String, value: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.store.set(key, value).map_err(Into::into)
    }

    fn remove(&self, key: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.store.remove(key).map_err(Into::into)
    }
}

#[cfg(test)]
use crate::services::error::BsvHomeServiceError;

#[cfg(test)]
#[derive(Debug, Default)]
pub struct EmptyPreferences;

#[cfg(test)]
impl BsvHomeSecureStore for EmptyPreferences {
    fn get(&self, _key: String) -> Result<Option<String>, BsvHomeServiceError> {
        Ok(None)
    }

    fn set(&self, _key: String, _value: String) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }

    fn remove(&self, _key: String) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }
}

#[cfg(test)]
impl BsvHomePreferencesStore for EmptyPreferences {
    fn get(&self, _key: String) -> Option<String> {
        None
    }

    fn set(&self, _key: String, _value: String) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }

    fn remove(&self, _key: String) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }

    fn clear(&self) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }
}
