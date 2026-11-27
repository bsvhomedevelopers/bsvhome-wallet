use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use primitives::BannerState;

use super::{BsvHomeBannerKey, BsvHomeBannerStore};
use crate::services::error::BsvHomeServiceError;

#[derive(Default)]
pub struct MemoryBannerStore {
    pub states: Mutex<HashMap<String, BannerState>>,
    pub writes: Mutex<Vec<Vec<BsvHomeBannerKey>>>,
}

#[async_trait]
impl BsvHomeBannerStore for MemoryBannerStore {
    async fn get_state(&self, key: BsvHomeBannerKey) -> Result<Option<BannerState>, BsvHomeServiceError> {
        Ok(self.states.lock().unwrap().get(&key.identifier()).copied())
    }

    async fn set_state(&self, key: BsvHomeBannerKey, state: BannerState) -> Result<(), BsvHomeServiceError> {
        self.states.lock().unwrap().insert(key.identifier(), state);
        Ok(())
    }

    async fn add_banners(&self, keys: Vec<BsvHomeBannerKey>, state: BannerState) -> Result<(), BsvHomeServiceError> {
        let mut states = self.states.lock().unwrap();
        for key in &keys {
            states.entry(key.identifier()).or_insert(state);
        }
        self.writes.lock().unwrap().push(keys);
        Ok(())
    }
}
