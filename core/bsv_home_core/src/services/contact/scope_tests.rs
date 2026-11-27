use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use futures::executor::block_on;
use primitives::{Chain, Contact, contact::ContactAddress};

use super::{BsvHomeContactAvatar, BsvHomeContactInput, BsvHomeContactService, BsvHomeContactStore, rules};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::error::BsvHomeServiceError;
use crate::services::file::BsvHomeFileStore;
use crate::services::wallet::testkit::MemoryAddressStore;

#[derive(Default)]
struct Store {
    addresses: Mutex<Vec<ContactAddress>>,
    writes: Mutex<usize>,
}

#[async_trait]
impl BsvHomeContactStore for Store {
    async fn get_addresses(&self, _: String) -> Result<Vec<ContactAddress>, BsvHomeServiceError> {
        Ok(self.addresses.lock().unwrap().clone())
    }
    async fn save_contact(&self, _: Contact, addresses: Vec<ContactAddress>) -> Result<(), BsvHomeServiceError> {
        *self.writes.lock().unwrap() += 1;
        *self.addresses.lock().unwrap() = addresses;
        Ok(())
    }
    async fn update_contact(&self, contact: Contact, addresses: Vec<ContactAddress>, _: Vec<String>) -> Result<(), BsvHomeServiceError> {
        self.save_contact(contact, addresses).await
    }
    async fn delete_contact(&self, _: String) -> Result<(), BsvHomeServiceError> {
        panic!("scope changes must preserve contacts")
    }
}

struct UnusedFiles;
impl BsvHomeFileStore for UnusedFiles {
    fn save_file(&self, _: Vec<u8>, _: String) -> Result<String, BsvHomeServiceError> {
        panic!("invalid address must be rejected before saving avatar")
    }
    fn save_named_file(&self, _: Vec<u8>, _: String) -> Result<String, BsvHomeServiceError> {
        panic!("unused")
    }
    fn exists(&self, _: String) -> bool {
        false
    }
    fn path(&self, name: String) -> String {
        name
    }
    fn remove(&self, _: String) -> Result<(), BsvHomeServiceError> {
        panic!("unused")
    }
}

fn service(store: Arc<Store>, environment: BsvHomeRuntimeEnvironment) -> BsvHomeContactService {
    BsvHomeContactService::new_with_environment(store, Arc::new(MemoryAddressStore::default()), Arc::new(UnusedFiles), environment)
}

#[test]
fn test_wowpay_contact_creation_rejects_other_chains_before_avatar_and_store() {
    block_on(async {
        let store = Arc::new(Store::default());
        let scoped = service(store.clone(), BsvHomeRuntimeEnvironment::WowPay);
        for chain in [Chain::Bitcoin, Chain::Ethereum, Chain::Tron, Chain::TronShasta] {
            assert!(
                scoped
                    .save_contact(BsvHomeContactInput {
                        id: "contact".into(),
                        existing: None,
                        name: "Name".into(),
                        description: String::new(),
                        avatar: BsvHomeContactAvatar::Rendered { image: vec![1] },
                        addresses: vec![rules::contact_address("contact".into(), chain, "address".into(), None)]
                    })
                    .await
                    .is_err()
            );
        }
        assert_eq!(*store.writes.lock().unwrap(), 0);
        assert_eq!(
            scoped.default_chain(),
            crate::services::chain::BsvHomeChainService::new_with_environment(BsvHomeRuntimeEnvironment::WowPay).get_chains(String::new())[0]
        );
        assert_eq!(service(store, BsvHomeRuntimeEnvironment::Production).default_chain(), Chain::Bitcoin);
    });
}

#[test]
fn test_wowpay_contact_edit_retains_hidden_addresses_and_rejects_changed_hidden_values() {
    block_on(async {
        let hidden = rules::contact_address("contact".into(), Chain::Ethereum, "legacy".into(), Some("memo".into()));
        let hidden_tron = rules::contact_address("contact".into(), Chain::Tron, "legacy-tron".into(), None);
        let store = Arc::new(Store {
            addresses: Mutex::new(vec![hidden.clone(), hidden_tron.clone()]),
            ..Default::default()
        });
        let service = service(store.clone(), BsvHomeRuntimeEnvironment::WowPay);
        let contact = rules::contact(None, "contact".into(), "Name".into(), String::new(), None, Utc::now());
        let allowed = rules::contact_address("contact".into(), Chain::BitcoinSv, "current".into(), None);
        service.update_contact(contact.clone(), vec![allowed.clone()]).await.unwrap();
        assert_eq!(
            serde_json::to_value(&*store.addresses.lock().unwrap()).unwrap(),
            serde_json::to_value(vec![allowed, hidden.clone(), hidden_tron.clone()]).unwrap()
        );
        for changed in [
            ContactAddress {
                memo: Some("changed".into()),
                ..hidden.clone()
            },
            ContactAddress { chain: Chain::Tron, ..hidden },
            ContactAddress {
                address: "changed-tron".into(),
                ..hidden_tron
            },
        ] {
            assert!(service.update_contact(contact.clone(), vec![changed]).await.is_err());
        }
        assert_eq!(*store.writes.lock().unwrap(), 1);
    });
}
