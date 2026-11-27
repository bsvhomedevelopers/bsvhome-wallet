use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{AddressName, Chain};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeAddressStore: Send + Sync {
    async fn get_address_name(&self, chain: Chain, address: String) -> Result<Option<AddressName>, BsvHomeServiceError>;
    async fn save_address_names(&self, names: Vec<AddressName>) -> Result<(), BsvHomeServiceError>;
    async fn delete_address_names(&self, names: Vec<AddressName>) -> Result<(), BsvHomeServiceError>;
}
