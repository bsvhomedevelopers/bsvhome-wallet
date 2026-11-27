pub mod model;
pub mod rules;
#[cfg(test)]
mod scope_tests;
pub mod store;

use crate::services::error::BsvHomeServiceError;
use chrono::Utc;
use std::sync::Arc;

use primitives::contact::ContactAddress;
use primitives::{Chain, Contact};

use crate::address_formatter::{BsvHomeAddressFormatStyle, BsvHomeAddressService};
use crate::models::payment::BsvHomePayment;
use crate::payment::BsvHomePaymentService;
use crate::services::chain::{BsvHomeChainService, BsvHomeRuntimeEnvironment};
use crate::services::file::BsvHomeFileStore;
use crate::services::name::BsvHomeAddressStore;

pub use model::{BsvHomeContactAddressInput, BsvHomeContactAvatar, BsvHomeContactInput, BsvHomeContactScannedAddress};
pub use store::BsvHomeContactStore;

const AVATAR_EXTENSION: &str = "png";

#[derive(uniffi::Object)]
pub struct BsvHomeContactService {
    store: Arc<dyn BsvHomeContactStore>,
    address_store: Arc<dyn BsvHomeAddressStore>,
    files: Arc<dyn BsvHomeFileStore>,
    environment: BsvHomeRuntimeEnvironment,
}

#[uniffi::export]
impl BsvHomeContactService {
    #[uniffi::constructor]
    pub fn new(store: Arc<dyn BsvHomeContactStore>, address_store: Arc<dyn BsvHomeAddressStore>, files: Arc<dyn BsvHomeFileStore>) -> Self {
        Self::new_with_environment(store, address_store, files, BsvHomeRuntimeEnvironment::Production)
    }

    #[uniffi::constructor]
    pub fn new_with_environment(
        store: Arc<dyn BsvHomeContactStore>,
        address_store: Arc<dyn BsvHomeAddressStore>,
        files: Arc<dyn BsvHomeFileStore>,
        environment: BsvHomeRuntimeEnvironment,
    ) -> Self {
        Self {
            store,
            address_store,
            files,
            environment,
        }
    }

    pub async fn save_contact(&self, input: BsvHomeContactInput) -> Result<Contact, BsvHomeServiceError> {
        let existing_addresses = match &input.existing {
            Some(existing) => self.store.get_addresses(existing.id.clone()).await?,
            None => Vec::new(),
        };
        let addresses = retained_addresses(self.environment, input.addresses, &existing_addresses);
        require_address_changes(self.environment, &addresses, &existing_addresses)?;
        let image_url = match input.avatar {
            BsvHomeContactAvatar::Empty => None,
            BsvHomeContactAvatar::Image { image_url } => Some(image_url),
            BsvHomeContactAvatar::Rendered { image } => Some(self.save_avatar(image)?),
        };
        let contact = rules::contact(input.existing.as_ref(), input.id, input.name, input.description, image_url, Utc::now());
        match input.existing {
            Some(existing) => {
                self.update_contact_with_existing(contact.clone(), addresses, existing_addresses).await?;
                if let Some(previous) = existing.image_url.filter(|previous| contact.image_url.as_ref() != Some(previous)) {
                    let _ = self.remove_avatar(previous);
                }
            }
            None => self.add_contact(contact.clone(), addresses).await?,
        }
        Ok(contact)
    }

    pub async fn delete_contact(&self, contact: Contact) -> Result<(), BsvHomeServiceError> {
        let existing = self.store.get_addresses(contact.id.clone()).await?;
        self.store.delete_contact(contact.id.clone()).await?;
        self.address_store.delete_address_names(rules::address_names(&contact, &existing)).await?;
        match contact.image_url {
            Some(file_name) => self.files.remove(file_name),
            None => Ok(()),
        }
    }

    pub async fn update_contact(&self, contact: Contact, addresses: Vec<ContactAddress>) -> Result<(), BsvHomeServiceError> {
        let existing = self.store.get_addresses(contact.id.clone()).await?;
        let addresses = retained_addresses(self.environment, addresses, &existing);
        require_address_changes(self.environment, &addresses, &existing)?;
        self.update_contact_with_existing(contact, addresses, existing).await
    }

    pub fn default_chain(&self) -> Chain {
        let default = rules::default_contact_chain();
        let chains = BsvHomeChainService::new_with_environment(self.environment).get_chains(String::new());
        if chains.contains(&default) {
            default
        } else {
            chains.first().copied().unwrap_or(default)
        }
    }
}

impl BsvHomeContactService {
    async fn update_contact_with_existing(&self, contact: Contact, addresses: Vec<ContactAddress>, existing: Vec<ContactAddress>) -> Result<(), BsvHomeServiceError> {
        let stale = rules::stale_addresses(existing, &addresses);
        self.store
            .update_contact(contact.clone(), addresses.clone(), stale.iter().map(|address| address.id.clone()).collect())
            .await?;
        self.address_store.delete_address_names(rules::address_names(&contact, &stale)).await?;
        self.save_address_names(&contact, &addresses).await
    }

    async fn add_contact(&self, contact: Contact, addresses: Vec<ContactAddress>) -> Result<(), BsvHomeServiceError> {
        self.store.save_contact(contact.clone(), addresses.clone()).await?;
        self.save_address_names(&contact, &addresses).await
    }

    fn save_avatar(&self, image: Vec<u8>) -> Result<String, BsvHomeServiceError> {
        self.files.save_file(image, AVATAR_EXTENSION.to_string())
    }

    fn remove_avatar(&self, file_name: String) -> Result<(), BsvHomeServiceError> {
        self.files.remove(file_name)
    }

    async fn save_address_names(&self, contact: &Contact, addresses: &[ContactAddress]) -> Result<(), BsvHomeServiceError> {
        self.address_store.save_address_names(rules::address_names(contact, addresses)).await
    }
}

fn require_address_changes(environment: BsvHomeRuntimeEnvironment, addresses: &[ContactAddress], existing: &[ContactAddress]) -> Result<(), BsvHomeServiceError> {
    for address in addresses {
        let previous = existing.iter().find(|previous| previous.id == address.id);
        if previous.is_some_and(|previous| {
            previous.id == address.id
                && previous.contact_id == address.contact_id
                && previous.chain == address.chain
                && previous.address == address.address
                && previous.memo == address.memo
        }) {
            continue;
        }
        if let Some(previous) = previous
            && !environment.allows_chain(previous.chain)
        {
            environment.require_chain(previous.chain)?;
        }
        environment.require_chain(address.chain)?;
    }
    Ok(())
}

fn retained_addresses(environment: BsvHomeRuntimeEnvironment, mut addresses: Vec<ContactAddress>, existing: &[ContactAddress]) -> Vec<ContactAddress> {
    addresses.extend(
        existing
            .iter()
            .filter(|address| !environment.allows_chain(address.chain) && !addresses.iter().any(|candidate| candidate.id == address.id))
            .cloned()
            .collect::<Vec<_>>(),
    );
    addresses
}

#[derive(uniffi::Object)]
pub struct BsvHomeManageContactService {
    contacts: Arc<BsvHomeContactService>,
    addresses: Arc<BsvHomeAddressService>,
    payments: Arc<BsvHomePaymentService>,
}

#[uniffi::export]
impl BsvHomeManageContactService {
    #[uniffi::constructor]
    pub fn new(contacts: Arc<BsvHomeContactService>, addresses: Arc<BsvHomeAddressService>, payments: Arc<BsvHomePaymentService>) -> Self {
        Self { contacts, addresses, payments }
    }

    pub fn scanned_address(&self, input: String) -> BsvHomeContactScannedAddress {
        let request = match self.payments.decode_url(input.clone()) {
            Ok(BsvHomePayment::Request { request }) => Some(request),
            Ok(BsvHomePayment::Link { link: _ }) | Err(_) => None,
        };
        rules::scanned_address(&input, request.as_ref())
    }

    pub fn default_chain(&self) -> Chain {
        self.contacts.default_chain()
    }

    pub async fn save_contact(&self, input: BsvHomeContactInput) -> Result<Contact, BsvHomeServiceError> {
        self.contacts.save_contact(input).await
    }

    pub fn format_address(&self, address: String, chain: Chain, style: BsvHomeAddressFormatStyle) -> String {
        self.addresses.format(address, Some(chain), style)
    }
}
