use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use primitives::{AddressName, Chain, Wallet, WalletId};

use super::BsvHomeWalletStore;
use super::addresses::BsvHomeBsvAddressStoredState;
use super::addresses::testkit::MemoryBsvAddressStore;
use super::password::{BsvHomeKeystoreAuthentication, BsvHomeKeystorePassword};
use crate::services::error::BsvHomeServiceError;
use crate::services::name::BsvHomeAddressStore;

pub const TEST_PASSWORD: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

#[derive(Default)]
pub struct MemoryWalletStore {
    pub wallets: Mutex<Vec<Wallet>>,
    pub add_wallet_error: Mutex<Option<BsvHomeServiceError>>,
    pub bsv_addresses: Arc<MemoryBsvAddressStore>,
}

#[async_trait::async_trait]
impl BsvHomeWalletStore for MemoryWalletStore {
    async fn get_wallets(&self) -> Result<Vec<Wallet>, BsvHomeServiceError> {
        Ok(self.wallets.lock().unwrap().clone())
    }
    async fn get_wallet(&self, wallet_id: WalletId) -> Result<Option<Wallet>, BsvHomeServiceError> {
        Ok(self.wallets.lock().unwrap().iter().find(|wallet| wallet.id == wallet_id).cloned())
    }
    async fn add_wallet(&self, wallet: Wallet) -> Result<(), BsvHomeServiceError> {
        if let Some(error) = self.add_wallet_error.lock().unwrap().clone() {
            return Err(error);
        }
        let mut wallets = self.wallets.lock().unwrap();
        wallets.retain(|stored| stored.id != wallet.id);
        wallets.push(wallet);
        Ok(())
    }
    async fn add_wallet_with_bsv_addresses(&self, wallet: Wallet, addresses: Option<BsvHomeBsvAddressStoredState>) -> Result<(), BsvHomeServiceError> {
        if let Some(error) = self.add_wallet_error.lock().unwrap().clone() {
            return Err(error);
        }
        if addresses.is_some()
            && let Some(error) = self.bsv_addresses.save_error.lock().unwrap().clone()
        {
            return Err(error);
        }
        let mut wallets = self.wallets.lock().unwrap();
        let mut states = self.bsv_addresses.states.lock().unwrap();
        if let Some(addresses) = addresses {
            if states.contains_key(&wallet.id) {
                return Err(BsvHomeServiceError::Store {
                    msg: "BSV payer journal already exists".into(),
                });
            }
            states.insert(wallet.id.clone(), addresses);
        }
        wallets.retain(|stored| stored.id != wallet.id);
        wallets.push(wallet);
        Ok(())
    }
    async fn delete_wallet(&self, wallet_id: WalletId) -> Result<bool, BsvHomeServiceError> {
        let mut wallets = self.wallets.lock().unwrap();
        let before = wallets.len();
        wallets.retain(|wallet| wallet.id != wallet_id);
        self.bsv_addresses.pending.lock().unwrap().remove(&wallet_id);
        Ok(before != wallets.len())
    }
    async fn set_pinned(&self, _wallet_id: WalletId, _pinned: bool) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }
    async fn set_name(&self, _wallet_id: WalletId, _name: String) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }
    async fn set_image_url(&self, _wallet_id: WalletId, _image_url: Option<String>) -> Result<(), BsvHomeServiceError> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryKeystorePassword {
    pub wallet_passwords: Mutex<HashMap<String, String>>,
    pub create_requests: Mutex<Vec<bool>>,
    pub password_error: Mutex<Option<BsvHomeServiceError>>,
}

impl BsvHomeKeystorePassword for MemoryKeystorePassword {
    fn get_password(&self, create_if_missing: bool) -> Result<String, BsvHomeServiceError> {
        self.create_requests.lock().unwrap().push(create_if_missing);
        if let Some(error) = self.password_error.lock().unwrap().clone() {
            return Err(error);
        }
        Ok(TEST_PASSWORD.to_string())
    }
    fn get_wallet_password(&self, wallet_id: WalletId) -> Result<Option<String>, BsvHomeServiceError> {
        Ok(self.wallet_passwords.lock().unwrap().get(&wallet_id.id()).cloned())
    }
    fn delete_wallet_password(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        self.wallet_passwords.lock().unwrap().remove(&wallet_id.id());
        Ok(())
    }
    fn authentication(&self) -> Result<BsvHomeKeystoreAuthentication, BsvHomeServiceError> {
        Ok(BsvHomeKeystoreAuthentication::None)
    }
}

#[derive(Default)]
pub struct MemoryAddressStore {
    pub names: Mutex<HashMap<(Chain, String), AddressName>>,
}

#[async_trait::async_trait]
impl BsvHomeAddressStore for MemoryAddressStore {
    async fn get_address_name(&self, chain: Chain, address: String) -> Result<Option<AddressName>, BsvHomeServiceError> {
        Ok(self.names.lock().unwrap().get(&(chain, address)).cloned())
    }
    async fn save_address_names(&self, names: Vec<AddressName>) -> Result<(), BsvHomeServiceError> {
        let mut stored = self.names.lock().unwrap();
        for name in names {
            stored.insert((name.chain, name.address.clone()), name);
        }
        Ok(())
    }
    async fn delete_address_names(&self, names: Vec<AddressName>) -> Result<(), BsvHomeServiceError> {
        let mut stored = self.names.lock().unwrap();
        for name in names {
            stored.remove(&(name.chain, name.address));
        }
        Ok(())
    }
}
