pub mod rules;

use std::sync::Arc;

use primitives::{PlatformStore, Release};

use crate::config::APP_UPDATE_ENABLED;
use crate::services::config::BsvHomeConfigService;
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomePreferencesService;

#[derive(uniffi::Object)]
pub struct BsvHomeAppUpdateService {
    config: Arc<BsvHomeConfigService>,
    preferences: Arc<BsvHomePreferencesService>,
}

#[uniffi::export]
impl BsvHomeAppUpdateService {
    #[uniffi::constructor]
    pub fn new(config: Arc<BsvHomeConfigService>, preferences: Arc<BsvHomePreferencesService>) -> Self {
        Self { config, preferences }
    }

    pub fn is_enabled(&self) -> bool {
        APP_UPDATE_ENABLED
    }

    pub async fn newest(&self, store: PlatformStore, current_version: String) -> Result<Option<Release>, BsvHomeServiceError> {
        if !self.is_enabled() || store == PlatformStore::Local {
            return Ok(None);
        }
        let config = self.config.get_config().await?;
        Ok(rules::newest_release(&config.releases, store, &current_version))
    }

    pub async fn check(&self, store: PlatformStore, current_version: String) -> Result<Option<Release>, BsvHomeServiceError> {
        if !self.is_enabled() || store == PlatformStore::Local {
            return Ok(None);
        }
        let config = self.config.get_config().await?;
        let skipped_version = self.preferences.get_skipped_app_version();
        Ok(rules::available_update(&config.releases, store, &current_version, skipped_version.as_deref()))
    }

    pub fn skip(&self, version: String) -> Result<(), BsvHomeServiceError> {
        self.preferences.set_skipped_app_version(version)
    }

    pub fn is_version_higher(&self, new: String, current: String) -> bool {
        rules::is_version_higher(new, current)
    }
}

#[cfg(test)]
mod tests {
    use primitives::{ConfigResponse, ConfigVersions, SwapConfig};

    use super::*;
    use crate::api::BsvHomeApiClient;
    use crate::services::preferences::testkit::MemoryPreferencesStore;
    use crate::testkit::TestAlienProvider;

    #[test]
    fn test_disabled_updates_skip_requests_and_cached_releases() {
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
        let config = Arc::new(BsvHomeConfigService::new(Arc::new(BsvHomeApiClient::new(provider.clone())), preferences.clone()));
        let service = BsvHomeAppUpdateService::new(config, preferences.clone());

        futures::executor::block_on(async {
            assert_eq!(service.check(PlatformStore::AppStore, "1.0.0".into()).await.unwrap(), None);
            assert_eq!(service.newest(PlatformStore::AppStore, "1.0.0".into()).await.unwrap(), None);

            preferences
                .set_config(&ConfigResponse {
                    releases: vec![Release::new(PlatformStore::AppStore, "99.0.0".into(), true)],
                    versions: ConfigVersions {
                        fiat_on_ramp_assets: 0,
                        fiat_off_ramp_assets: 0,
                        swap_assets: 0,
                    },
                    swap: SwapConfig { enabled_providers: vec![] },
                })
                .unwrap();

            assert_eq!(service.check(PlatformStore::AppStore, "1.0.0".into()).await.unwrap(), None);
            assert_eq!(service.newest(PlatformStore::AppStore, "1.0.0".into()).await.unwrap(), None);
        });

        assert_eq!(provider.requested_paths(), Vec::<String>::new());
    }
}
