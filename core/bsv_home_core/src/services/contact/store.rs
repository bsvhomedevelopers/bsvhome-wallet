use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::Contact;
use primitives::contact::ContactAddress;

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeContactStore: Send + Sync {
    async fn get_addresses(&self, contact_id: String) -> Result<Vec<ContactAddress>, BsvHomeServiceError>;
    async fn save_contact(&self, contact: Contact, addresses: Vec<ContactAddress>) -> Result<(), BsvHomeServiceError>;
    async fn update_contact(&self, contact: Contact, addresses: Vec<ContactAddress>, delete_address_ids: Vec<String>) -> Result<(), BsvHomeServiceError>;
    async fn delete_contact(&self, contact_id: String) -> Result<(), BsvHomeServiceError>;
}
