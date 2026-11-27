use std::sync::Arc;

use crate::services::offline_identity::BsvHomeOfflineIdentityService;
use crate::services::portal::mpc::{BsvHomeMpcDeviceProof, BsvHomeMpcError};
use futures::try_join;

use super::rules;
use super::{
    BsvHomePlatformAccountError, BsvHomePlatformAccountService, BsvHomePlatformAlternativeChallenge, BsvHomePlatformAlternativeKey, BsvHomePlatformAlternativeRequest,
    BsvHomePlatformAlternativeSnapshot,
};

#[derive(uniffi::Object)]
pub struct BsvHomePlatformAlternativeService {
    account: Arc<BsvHomePlatformAccountService>,
    device: Arc<dyn BsvHomeMpcDeviceProof>,
}

#[uniffi::export]
impl BsvHomePlatformAlternativeService {
    #[uniffi::constructor]
    pub fn new(account: Arc<BsvHomePlatformAccountService>, device: Arc<dyn BsvHomeMpcDeviceProof>) -> Self {
        Self { account, device }
    }

    pub async fn snapshot(&self) -> Result<BsvHomePlatformAlternativeSnapshot, BsvHomePlatformAccountError> {
        let lease = self.account.session_lease()?;
        let (keys, request) = try_join!(self.keys(), self.pending())?;
        self.account.require_lease(&lease)?;
        Ok(BsvHomePlatformAlternativeSnapshot { keys, request })
    }

    pub fn public_key_fingerprint(&self, public_key: String) -> Result<String, BsvHomePlatformAccountError> {
        BsvHomeOfflineIdentityService::new()
            .public_key_fingerprint(public_key)
            .map_err(|_| BsvHomePlatformAccountError::InvalidInput)
    }

    pub async fn keys(&self) -> Result<Vec<BsvHomePlatformAlternativeKey>, BsvHomePlatformAccountError> {
        self.account.alternative_keys().await
    }

    pub async fn enrollment(&self, public_key: String, label: String) -> Result<BsvHomePlatformAlternativeChallenge, BsvHomePlatformAccountError> {
        self.account.alternative_enrollment(public_key, label).await
    }

    pub async fn enroll(&self, id: String, proof_json: String) -> Result<BsvHomePlatformAlternativeKey, BsvHomePlatformAccountError> {
        self.account.alternative_enroll(id, proof_json).await
    }

    pub async fn revoke(&self, id: String) -> Result<(), BsvHomePlatformAccountError> {
        self.account.alternative_revoke(id).await
    }

    pub async fn recovery(&self, key_id: String, public_key: String) -> Result<BsvHomePlatformAlternativeChallenge, BsvHomePlatformAccountError> {
        let lease = self.account.session_lease()?;
        let device_public_key = self.device.public_key().await.map_err(device_error)?;
        self.account.require_lease(&lease)?;
        self.account.alternative_recovery(key_id, public_key, device_public_key).await
    }

    pub async fn request(&self, id: String, proof_json: String) -> Result<BsvHomePlatformAlternativeRequest, BsvHomePlatformAccountError> {
        let signature = self.device_signature(&id).await?;
        self.account.alternative_request(id, proof_json, signature).await
    }

    pub async fn pending(&self) -> Result<Option<BsvHomePlatformAlternativeRequest>, BsvHomePlatformAccountError> {
        self.account.alternative_pending().await
    }
    pub async fn status(&self, id: String) -> Result<BsvHomePlatformAlternativeRequest, BsvHomePlatformAccountError> {
        self.account.alternative_status(id).await
    }
    pub async fn cancel(&self, id: String) -> Result<BsvHomePlatformAlternativeRequest, BsvHomePlatformAccountError> {
        self.account.alternative_cancel(id).await
    }
    pub async fn registration(&self, id: String) -> Result<BsvHomePlatformAlternativeChallenge, BsvHomePlatformAccountError> {
        self.account.alternative_registration(id).await
    }

    pub async fn register(&self, id: String, proof_json: String) -> Result<BsvHomePlatformAlternativeRequest, BsvHomePlatformAccountError> {
        let signature = self.device_signature(&id).await?;
        self.account.alternative_register(id, proof_json, signature).await
    }

    pub fn challenge_json(&self, id: String) -> Result<String, BsvHomePlatformAccountError> {
        self.account.alternative_challenge_json(id)
    }
    pub fn review_proof(&self, id: String, proof_json: String) -> Result<(), BsvHomePlatformAccountError> {
        self.account.validate_proof(&id, &proof_json)
    }
    pub fn cancel_workflow(&self) -> Result<(), BsvHomePlatformAccountError> {
        self.account.cancel()
    }
}

impl BsvHomePlatformAlternativeService {
    async fn device_signature(&self, id: &str) -> Result<String, BsvHomePlatformAccountError> {
        let (lease, pending) = self.account.pending_operation(&format!("alternative.{id}"))?;
        let expected_public_key = rules::string(&pending, "devicePublicKey")?;
        let public_key = self.device.public_key().await.map_err(device_error)?;
        if public_key != expected_public_key {
            return Err(BsvHomePlatformAccountError::SessionChanged);
        }
        self.account.require_lease(&lease)?;
        let challenge = pending.get("challenge").ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        let signature = self.device.sign_message(rules::string(challenge, "actionHash")?.into()).await.map_err(device_error)?;
        self.account.require_lease(&lease)?;
        Ok(signature)
    }
}

fn device_error(error: BsvHomeMpcError) -> BsvHomePlatformAccountError {
    match error {
        BsvHomeMpcError::Platform { error } => error,
        BsvHomeMpcError::Cancelled => BsvHomePlatformAccountError::Cancelled,
        BsvHomeMpcError::SecureStorageUnavailable | BsvHomeMpcError::NativeUnavailable => BsvHomePlatformAccountError::SecureStorageUnavailable,
        BsvHomeMpcError::InvalidInput
        | BsvHomeMpcError::PaymailSenderRequired
        | BsvHomeMpcError::PaymailUnavailable
        | BsvHomeMpcError::InvalidResponse
        | BsvHomeMpcError::WalletNotFound
        | BsvHomeMpcError::IdentityMismatch
        | BsvHomeMpcError::EpochMismatch
        | BsvHomeMpcError::BackupMismatch
        | BsvHomeMpcError::TransferChanged
        | BsvHomeMpcError::OperationPending
        | BsvHomeMpcError::UnsupportedState { .. } => BsvHomePlatformAccountError::InvalidInput,
    }
}
