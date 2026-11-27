pub mod model;
pub mod rules;
pub mod store;

use crate::services::error::BsvHomeServiceError;
use std::sync::Arc;

use primitives::{AddressName, Chain, ChainAddress};

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::services::recipient::{BsvHomeRecipientError, BsvHomeRecipientValidation, rules as recipient_rules};
use crate::services::transfer::BsvHomeRecipient;

pub use model::BsvHomeNameRecordState;
pub use store::BsvHomeAddressStore;

#[derive(uniffi::Object)]
pub struct BsvHomeNameService {
    api: Arc<BsvHomeDeviceApiClient>,
    store: Arc<dyn BsvHomeAddressStore>,
}

#[uniffi::export]
impl BsvHomeNameService {
    #[uniffi::constructor]
    pub fn new(api: Arc<BsvHomeDeviceApiClient>, store: Arc<dyn BsvHomeAddressStore>) -> Self {
        Self { api, store }
    }

    pub fn validate_recipient(&self, chain: Chain, input: String, state: BsvHomeNameRecordState) -> BsvHomeRecipientValidation {
        recipient_rules::validation(chain, &input, &state)
    }

    pub fn recipient(
        &self,
        chain: Chain,
        input: String,
        state: BsvHomeNameRecordState,
        memo: Option<String>,
        references: Vec<String>,
    ) -> Result<BsvHomeRecipient, BsvHomeRecipientError> {
        recipient_rules::recipient(chain, &input, &state, memo, references)
    }

    pub fn is_name_supported(&self, name: String) -> bool {
        rules::is_name_supported(&name)
    }

    pub fn name_record_debounce_milliseconds(&self) -> u64 {
        rules::name_record_debounce_milliseconds()
    }

    pub async fn get_name_record(&self, name: String, chain: Chain) -> Result<BsvHomeNameRecordState, BsvHomeServiceError> {
        let record = self.api.client.get_name_record(name, chain.to_string()).await.map_err(BsvHomeApiError::from)?;
        Ok(rules::resolved(record))
    }
}

impl BsvHomeNameService {
    pub async fn address_name(&self, chain: Chain, address: String) -> Result<Option<AddressName>, BsvHomeServiceError> {
        self.store.get_address_name(chain, address).await
    }

    pub async fn get_address_names(&self, requests: Vec<ChainAddress>) -> Result<Vec<AddressName>, BsvHomeServiceError> {
        let requests = rules::unique_requests(requests);
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let mut cached = Vec::new();
        let mut missing = Vec::new();
        for request in requests {
            match self.store.get_address_name(request.chain, request.address.clone()).await? {
                Some(name) => cached.push(name),
                None => missing.push(request),
            }
        }
        if missing.is_empty() {
            return Ok(cached);
        }

        let remote = match self.api.client.get_address_names(missing).await {
            Ok(names) => names,
            Err(_) => return Ok(cached),
        };
        self.store.save_address_names(remote.clone()).await?;
        cached.extend(remote);
        Ok(cached)
    }
}
