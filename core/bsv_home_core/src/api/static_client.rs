use std::sync::Arc;

use bsv_home_api::BsvHomeStaticApiClient as StaticApiClient;

use crate::alien::{AlienClient, AlienProvider, new_alien_client};
use crate::config::public::ASSETS_URL;

#[derive(Debug, uniffi::Object)]
pub struct BsvHomeStaticApiClient {
    pub(crate) client: StaticApiClient<AlienClient>,
}

#[uniffi::export]
impl BsvHomeStaticApiClient {
    #[uniffi::constructor]
    pub fn new(provider: Arc<dyn AlienProvider>) -> Self {
        Self {
            client: StaticApiClient::new(new_alien_client(ASSETS_URL.to_string(), provider)),
        }
    }
}
