use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures::lock::Mutex as AsyncMutex;
use primitives::Wallet;
use serde::Serialize;
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::services::transfer_password::{validate_current_password, validate_new_password, validate_verification_password};

use super::model::Identity;
use super::transfer_password_model::{
    TransferPasswordAction, TransferPasswordSetRequest, TransferPasswordStatusRequest, TransferPasswordStatusResponse, TransferPasswordVerifyRequest,
};
use super::{BsvHomePortalAuthService, BsvHomeTransferPasswordError, BsvHomeTransferPasswordSnapshot, SessionRetry, WalletScope, rules};

#[cfg(test)]
#[path = "transfer_password_tests.rs"]
mod tests;

struct PasswordState {
    identity: Option<Identity>,
    snapshot: Option<BsvHomeTransferPasswordSnapshot>,
}

#[derive(uniffi::Object)]
pub struct BsvHomePortalTransferPasswordService {
    auth: Arc<BsvHomePortalAuthService>,
    wallet_id: String,
    cancelled: AtomicBool,
    operation: AsyncMutex<()>,
    state: Mutex<PasswordState>,
}

#[uniffi::export]
impl BsvHomePortalTransferPasswordService {
    #[uniffi::constructor]
    pub fn new(auth: Arc<BsvHomePortalAuthService>, wallet_id: String) -> Result<Self, BsvHomeTransferPasswordError> {
        match &auth.wallet_scope {
            WalletScope::Current => {}
            WalletScope::Fixed { .. } => {
                return Err(BsvHomeTransferPasswordError::InvalidConfiguration);
            }
        }
        Self::with_identity(auth, wallet_id, None)
    }

    #[uniffi::constructor]
    pub fn new_for_wallet(auth: Arc<BsvHomePortalAuthService>, wallet: Wallet) -> Result<Self, BsvHomeTransferPasswordError> {
        let identity = rules::identity(&wallet)?;
        match &auth.wallet_scope {
            WalletScope::Fixed { wallet_id, address } if *wallet_id == wallet.id && *address == identity.address => {}
            WalletScope::Current | WalletScope::Fixed { .. } => {
                return Err(BsvHomeTransferPasswordError::InvalidConfiguration);
            }
        }
        auth.multisig_check_scope(&identity.wallet_id)?;
        Self::with_identity(auth, identity.wallet_id.clone(), Some(identity))
    }

    pub async fn status(&self) -> Result<BsvHomeTransferPasswordSnapshot, BsvHomeTransferPasswordError> {
        let generation = self.auth.connection_version.load(Ordering::SeqCst);
        self.check_generation(generation)?;
        let _operation = self.operation.lock().await;
        self.check_generation(generation)?;
        self.state.lock().map_err(|_| BsvHomeTransferPasswordError::Unavailable)?.snapshot = None;
        let wallet = self.current_wallet(generation).await?;
        let identity = rules::identity(&wallet)?;
        let response = self
            .request(
                &wallet,
                TransferPasswordAction::Status,
                &TransferPasswordStatusRequest { address: &identity.address },
                generation,
            )
            .await?;
        let configured = serde_json::from_value::<TransferPasswordStatusResponse>(Value::Object(response))
            .map_err(|_| BsvHomeTransferPasswordError::InvalidResponse)?
            .configured;
        let snapshot = BsvHomeTransferPasswordSnapshot {
            wallet_id: identity.wallet_id,
            address: identity.address,
            configured,
            generation,
        };
        self.state.lock().map_err(|_| BsvHomeTransferPasswordError::Unavailable)?.snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }

    pub async fn set(
        &self,
        snapshot: BsvHomeTransferPasswordSnapshot,
        current_password: Option<String>,
        new_password: String,
        confirmation: String,
    ) -> Result<BsvHomeTransferPasswordSnapshot, BsvHomeTransferPasswordError> {
        let current_password = current_password.map(Zeroizing::new);
        let new_password = Zeroizing::new(new_password);
        let confirmation = Zeroizing::new(confirmation);
        self.require_snapshot(&snapshot)?;
        validate_new_password(&new_password, &confirmation)?;
        validate_current_password(snapshot.configured, current_password.as_deref().map(String::as_str))?;
        let _operation = self.operation.lock().await;
        self.require_snapshot(&snapshot)?;
        let wallet = self.current_wallet(snapshot.generation).await?;
        self.require_snapshot(&snapshot)?;
        self.request(
            &wallet,
            TransferPasswordAction::Set,
            &TransferPasswordSetRequest {
                address: &snapshot.address,
                current_password: current_password.as_deref().map(String::as_str),
                new_password: &new_password,
            },
            snapshot.generation,
        )
        .await?;
        let updated = BsvHomeTransferPasswordSnapshot { configured: true, ..snapshot };
        self.state.lock().map_err(|_| BsvHomeTransferPasswordError::Unavailable)?.snapshot = Some(updated.clone());
        Ok(updated)
    }

    pub async fn verify(&self, snapshot: BsvHomeTransferPasswordSnapshot, password: String) -> Result<(), BsvHomeTransferPasswordError> {
        let password = Zeroizing::new(password);
        self.require_snapshot(&snapshot)?;
        if !snapshot.configured {
            return Err(BsvHomeTransferPasswordError::NotConfigured);
        }
        validate_verification_password(&password)?;
        let _operation = self.operation.lock().await;
        self.require_snapshot(&snapshot)?;
        let wallet = self.current_wallet(snapshot.generation).await?;
        self.require_snapshot(&snapshot)?;
        self.request(
            &wallet,
            TransferPasswordAction::Verify,
            &TransferPasswordVerifyRequest {
                address: &snapshot.address,
                password: &password,
            },
            snapshot.generation,
        )
        .await?;
        Ok(())
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.auth.cancel();
    }
}

impl BsvHomePortalTransferPasswordService {
    fn with_identity(auth: Arc<BsvHomePortalAuthService>, wallet_id: String, identity: Option<Identity>) -> Result<Self, BsvHomeTransferPasswordError> {
        if wallet_id.is_empty() || auth.client.base_url.ends_with("/v2/api/portal") {
            return Err(BsvHomeTransferPasswordError::InvalidConfiguration);
        }
        Ok(Self {
            auth,
            wallet_id,
            cancelled: AtomicBool::new(false),
            operation: AsyncMutex::new(()),
            state: Mutex::new(PasswordState { identity, snapshot: None }),
        })
    }

    fn check_generation(&self, generation: u64) -> Result<(), BsvHomeTransferPasswordError> {
        if self.cancelled.load(Ordering::SeqCst) || self.auth.connection_version.load(Ordering::SeqCst) != generation {
            return Err(BsvHomeTransferPasswordError::Cancelled);
        }
        Ok(())
    }

    fn require_snapshot(&self, snapshot: &BsvHomeTransferPasswordSnapshot) -> Result<(), BsvHomeTransferPasswordError> {
        self.check_generation(snapshot.generation)?;
        if self.state.lock().map_err(|_| BsvHomeTransferPasswordError::Unavailable)?.snapshot.as_ref() != Some(snapshot) {
            return Err(BsvHomeTransferPasswordError::InvalidSnapshot);
        }
        Ok(())
    }

    async fn current_wallet(&self, generation: u64) -> Result<Wallet, BsvHomeTransferPasswordError> {
        self.check_generation(generation)?;
        let wallet = match &self.auth.wallet_scope {
            WalletScope::Current => self.auth.session.current_wallet().await.map_err(BsvHomeTransferPasswordError::from),
            WalletScope::Fixed { wallet_id, .. } => self
                .auth
                .session
                .get_wallet(wallet_id.clone())
                .await
                .map_err(BsvHomeTransferPasswordError::from)
                .and_then(|wallet| wallet.ok_or(BsvHomeTransferPasswordError::WalletChanged)),
        };
        self.check_generation(generation)?;
        let wallet = wallet?;
        let identity = rules::identity(&wallet)?;
        if identity.wallet_id != self.wallet_id {
            return Err(BsvHomeTransferPasswordError::WalletChanged);
        }
        let checked = self.auth.check_connection(&identity, generation).await;
        self.check_generation(generation)?;
        checked?;
        let mut state = self.state.lock().map_err(|_| BsvHomeTransferPasswordError::Unavailable)?;
        if state.identity.as_ref().is_some_and(|bound| bound != &identity) {
            return Err(BsvHomeTransferPasswordError::WalletChanged);
        }
        state.identity = Some(identity);
        Ok(wallet)
    }

    async fn request<T: Serialize>(&self, wallet: &Wallet, action: TransferPasswordAction, body: &T, generation: u64) -> Result<Map<String, Value>, BsvHomeTransferPasswordError> {
        self.check_generation(generation)?;
        let retry = match action {
            TransferPasswordAction::Status => SessionRetry::Authenticate,
            TransferPasswordAction::Set | TransferPasswordAction::Verify => SessionRetry::Never,
        };
        let body = serde_json::to_vec(body).map_err(|_| BsvHomeTransferPasswordError::Unavailable)?;
        let response = self.auth.transfer_password_request_at_generation(wallet, action, body, retry, generation).await;
        self.check_generation(generation)?;
        self.current_wallet(generation).await?;
        let response = response.map_err(BsvHomeTransferPasswordError::from);
        if response
            .as_ref()
            .is_err_and(|error| *error == BsvHomeTransferPasswordError::Unavailable || *error == BsvHomeTransferPasswordError::InvalidResponse)
        {
            self.state.lock().map_err(|_| BsvHomeTransferPasswordError::Unavailable)?.snapshot = None;
        }
        response
    }
}
