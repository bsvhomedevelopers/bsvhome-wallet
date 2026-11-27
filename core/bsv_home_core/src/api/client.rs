use std::sync::Arc;

use bsv_home_api::BsvHomeApiClient as ApiClient;

use crate::alien::{AlienClient, AlienProvider, coalescing_provider, new_alien_client};
use crate::config::public::API_URL;
use crate::services::price::client::{DEFILLAMA_URL, DefiLlamaClient};

#[derive(Debug, uniffi::Object)]
pub struct BsvHomeApiClient {
    pub(crate) client: ApiClient<AlienClient>,
    pub(crate) defillama: DefiLlamaClient<AlienClient>,
}

#[uniffi::export]
impl BsvHomeApiClient {
    #[uniffi::constructor]
    pub fn new(provider: Arc<dyn AlienProvider>) -> Self {
        let provider = coalescing_provider(provider);
        Self {
            client: ApiClient::new(new_alien_client(API_URL.to_string(), provider.clone())),
            defillama: DefiLlamaClient::new(new_alien_client(DEFILLAMA_URL.to_string(), provider)),
        }
    }
}
