pub mod rules;

use std::sync::Arc;

use primitives::{AuthNonce, AuthPayload, Wallet};

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::auth::create_auth_message;
use crate::keystore::decode_password;
use crate::keystore::{BsvHomeKeystore, keystore_id_for_wallet};
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::BsvHomeKeystorePassword;

#[derive(uniffi::Object)]
pub struct BsvHomeAuthService {
    api: Arc<BsvHomeDeviceApiClient>,
    keystore: Arc<BsvHomeKeystore>,
    password: Arc<dyn BsvHomeKeystorePassword>,
    device_key: Arc<BsvHomeDeviceKeyService>,
}

#[uniffi::export]
impl BsvHomeAuthService {
    #[uniffi::constructor]
    pub fn new(api: Arc<BsvHomeDeviceApiClient>, keystore: Arc<BsvHomeKeystore>, password: Arc<dyn BsvHomeKeystorePassword>, device_key: Arc<BsvHomeDeviceKeyService>) -> Self {
        Self {
            api,
            keystore,
            password,
            device_key,
        }
    }
}

impl BsvHomeAuthService {
    pub async fn get_nonce(&self) -> Result<AuthNonce, BsvHomeServiceError> {
        Ok(self.api.client.get_auth_nonce().await.map_err(BsvHomeApiError::from)?)
    }

    pub async fn get_auth_payload(&self, wallet: Wallet) -> Result<AuthPayload, BsvHomeServiceError> {
        let account = rules::auth_account(&wallet).ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: format!("wallet {} has no {} account", wallet.id.id(), rules::AUTH_CHAIN),
        })?;
        let nonce = self.get_nonce().await?;
        let message = create_auth_message(&account.address, nonce.clone());
        let password = decode_password(&self.password.get_password(false)?);
        let signature = self.keystore.sign_auth(keystore_id_for_wallet(wallet.id.id()), rules::AUTH_CHAIN, message.hash, password)?;
        Ok(AuthPayload {
            device_id: self.device_key.device_id()?,
            chain: rules::AUTH_CHAIN,
            address: account.address.clone(),
            nonce: nonce.nonce,
            signature,
        })
    }
}
