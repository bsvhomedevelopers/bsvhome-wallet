mod client;
mod error;
#[cfg(test)]
mod live_tests;
mod model;
pub mod platform;
pub use platform::*;
mod mpc;
pub use mpc::*;
mod multisig_target;
mod paymail;
mod paymail_destination;
mod paymail_error;
#[cfg(test)]
mod paymail_live_tests;
mod paymail_model;
mod paymail_rules;
#[cfg(test)]
mod paymail_tests;
mod payment;
pub(crate) mod rules;
mod swap;
mod swap_error;
mod swap_model;
mod swap_rules;
mod swap_store;
#[cfg(test)]
mod swap_tests;
#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests;
mod transfer_password;
mod transfer_password_error;
mod transfer_password_model;
#[cfg(test)]
mod transfer_password_testkit;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use futures::lock::Mutex;
use primitives::Chain;
use primitives::{Wallet, WalletId};
use serde::de::{DeserializeOwned, IgnoredAny};

use crate::alien::{AlienHttpMethod, AlienProvider};
use crate::keystore::{BsvHomeKeystore, decode_password, keystore_id_for_wallet};
use crate::message::sign_type::{SignDigestType, SignMessage};
use crate::message::signer::{MessageSigner, recover_bitcoin_message_address};
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomeSecureStore;
use crate::services::wallet::BsvHomeKeystorePassword;
use crate::services::wallet_session::BsvHomeWalletSessionService;

use client::PortalClient;
pub use error::BsvHomePortalAuthError;
use model::{Identity, IdentityRequest, LoginRequest, Session};
pub(crate) use multisig_target::BsvMultisigTarget;
pub use paymail::BsvHomePaymailService;
pub use paymail_destination::BsvHomePaymailDestinationService;
pub(crate) use paymail_destination::{BsvHomePaymailDestinationSnapshot, MAX_SATOSHIS as MAX_BSV_PAYMENT_SATOSHIS};
pub use paymail_error::BsvHomePaymailError;
pub use paymail_model::{
    BsvHomePaymailBinding, BsvHomePaymailBindingAvailability, BsvHomePaymailBindingIntent, BsvHomePaymailDestination, BsvHomePaymailMutation, BsvHomePaymailMutationStatus,
    BsvHomePaymailOutput, BsvHomePaymailResolvedDestination, BsvHomePaymailSnapshot, BsvHomePaymailVerification,
};
pub use swap::BsvHomePortalSwapService;
pub use swap_error::BsvHomePortalSwapError;
pub use swap_model::{
    BsvHomePortalSwapAction, BsvHomePortalSwapActionNavigation, BsvHomePortalSwapActionType, BsvHomePortalSwapAsset, BsvHomePortalSwapDirection, BsvHomePortalSwapEntryMode,
    BsvHomePortalSwapOrder, BsvHomePortalSwapOrderPage, BsvHomePortalSwapOrderStatus, BsvHomePortalSwapPair, BsvHomePortalSwapPaymentOrder, BsvHomePortalSwapPaymentPreparation,
    BsvHomePortalSwapPaymentRecovery, BsvHomePortalSwapQuote, BsvHomePortalSwapResolutionStatus, BsvHomePortalSwapSyncStatus, BsvHomePortalSwapVerificationStatus,
};
pub use swap_store::BsvHomePortalSwapStore;
pub use transfer_password::BsvHomePortalTransferPasswordService;
pub use transfer_password_error::BsvHomeTransferPasswordError;
pub use transfer_password_model::BsvHomeTransferPasswordSnapshot;
use transfer_password_model::TransferPasswordAction;

const SESSION: &str = "session";

enum SessionRetry {
    Authenticate,
    Never,
}

struct WalletRequestCheckpoint<'a> {
    generation: u64,
    check: &'a (dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    before_request: &'a (dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    kind: WalletRequestKind,
}

enum WalletRequestKind {
    Standard,
    MultisigAction(Submission),
    TransferPassword(TransferPasswordAction),
}

#[derive(Clone, Copy)]
enum Submission {
    Read,
    Write,
}

enum WalletScope {
    Current,
    Fixed { wallet_id: WalletId, address: String },
}

#[derive(uniffi::Object)]
pub struct BsvHomePortalAuthService {
    client: PortalClient,
    keystore: Arc<BsvHomeKeystore>,
    password: Arc<dyn BsvHomeKeystorePassword>,
    session: Arc<BsvHomeWalletSessionService>,
    device_key: Arc<BsvHomeDeviceKeyService>,
    store: Arc<dyn BsvHomeSecureStore>,
    device_type: String,
    wallet_scope: WalletScope,
    connection: Mutex<()>,
    connection_version: AtomicU64,
}

#[uniffi::export]
impl BsvHomePortalAuthService {
    #[uniffi::constructor]
    pub fn new(
        provider: Arc<dyn AlienProvider>,
        keystore: Arc<BsvHomeKeystore>,
        password: Arc<dyn BsvHomeKeystorePassword>,
        session: Arc<BsvHomeWalletSessionService>,
        device_key: Arc<BsvHomeDeviceKeyService>,
        store: Arc<dyn BsvHomeSecureStore>,
        base_url: String,
        language: String,
        device_type: String,
    ) -> Result<Self, BsvHomePortalAuthError> {
        if !matches!(device_type.as_str(), "3" | "4") {
            return Err(BsvHomePortalAuthError::InvalidSession);
        }
        Ok(Self {
            client: PortalClient::new(provider, base_url, language)?,
            keystore,
            password,
            session,
            device_key,
            store,
            device_type,
            wallet_scope: WalletScope::Current,
            connection: Mutex::new(()),
            connection_version: AtomicU64::new(0),
        })
    }

    #[uniffi::constructor]
    pub fn new_for_wallet(
        provider: Arc<dyn AlienProvider>,
        keystore: Arc<BsvHomeKeystore>,
        password: Arc<dyn BsvHomeKeystorePassword>,
        session: Arc<BsvHomeWalletSessionService>,
        device_key: Arc<BsvHomeDeviceKeyService>,
        store: Arc<dyn BsvHomeSecureStore>,
        base_url: String,
        language: String,
        device_type: String,
        wallet: Wallet,
    ) -> Result<Self, BsvHomePortalAuthError> {
        let identity = rules::identity(&wallet)?;
        let mut service = Self::new(provider, keystore, password, session, device_key, store, base_url, language, device_type)?;
        service.wallet_scope = WalletScope::Fixed {
            wallet_id: wallet.id,
            address: identity.address,
        };
        Ok(service)
    }

    pub async fn connect(&self) -> Result<(), BsvHomePortalAuthError> {
        self.swap_request::<Vec<IgnoredAny>>("/auth/swaps/pairs", AlienHttpMethod::Get, None).await.map(|_| ())
    }

    pub fn cancel(&self) {
        self.connection_version.fetch_add(1, Ordering::SeqCst);
    }
}

impl BsvHomePortalAuthService {
    pub(crate) fn require_fixed_wallet(&self, wallet: &Wallet) -> Result<(), BsvHomePortalAuthError> {
        let identity = rules::identity(wallet)?;
        self.check_version(0)?;
        match &self.wallet_scope {
            WalletScope::Fixed { wallet_id, address } if *wallet_id == wallet.id && *address == identity.address => Ok(()),
            WalletScope::Current | WalletScope::Fixed { .. } => Err(BsvHomePortalAuthError::WalletChanged),
        }
    }

    pub(crate) async fn multisig_wallet(&self) -> Result<Wallet, BsvHomePortalAuthError> {
        match &self.wallet_scope {
            WalletScope::Current => self.swap_wallet().await,
            WalletScope::Fixed { wallet_id, address } => {
                self.check_version(0)?;
                let wallet = self.session.get_wallet(wallet_id.clone()).await?.ok_or(BsvHomePortalAuthError::WalletChanged)?;
                let identity = rules::identity(&wallet)?;
                if wallet.id != *wallet_id || identity.address != *address {
                    return Err(BsvHomePortalAuthError::WalletChanged);
                }
                self.check_version(0)?;
                Ok(wallet)
            }
        }
    }

    pub(crate) fn multisig_check_scope(&self, wallet_id: &str) -> Result<(), BsvHomePortalAuthError> {
        let expected = match &self.wallet_scope {
            WalletScope::Current => self.session.current_wallet_id()?.id(),
            WalletScope::Fixed { wallet_id, .. } => {
                self.check_version(0)?;
                wallet_id.id()
            }
        };
        if expected != wallet_id {
            return Err(BsvHomePortalAuthError::WalletChanged);
        }
        Ok(())
    }

    pub(crate) fn multisig_origin(&self) -> &str {
        self.client.base_url.trim_end_matches("/v2/api/portal").trim_end_matches("/api/portal")
    }

    pub(crate) fn multisig_version(&self) -> u64 {
        self.connection_version.load(Ordering::SeqCst)
    }

    pub(crate) async fn multisig_check_wallet(&self, wallet: &Wallet, version: u64) -> Result<(), BsvHomePortalAuthError> {
        self.check_connection(&rules::identity(wallet)?, version).await
    }

    pub(crate) async fn multisig_wallet_request<R: DeserializeOwned>(&self, wallet: &Wallet, target: BsvMultisigTarget) -> Result<R, BsvHomePortalAuthError> {
        self.multisig_wallet_request_at_generation(wallet, target, self.multisig_version()).await
    }

    pub(crate) async fn multisig_wallet_request_at_generation<R: DeserializeOwned>(
        &self,
        wallet: &Wallet,
        target: BsvMultisigTarget,
        generation: u64,
    ) -> Result<R, BsvHomePortalAuthError> {
        let (path, method, body) = target.request()?;
        let retry = match method {
            AlienHttpMethod::Get => SessionRetry::Authenticate,
            AlienHttpMethod::Post | AlienHttpMethod::Put | AlienHttpMethod::Delete | AlienHttpMethod::Head | AlienHttpMethod::Patch | AlienHttpMethod::Options => {
                SessionRetry::Never
            }
        };
        self.wallet_request_at_generation(wallet, &path, method, body, retry, generation).await
    }

    pub(crate) async fn multisig_checkpoint_request_at_generation<R: DeserializeOwned>(
        &self,
        wallet: &Wallet,
        target: BsvMultisigTarget,
        generation: u64,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
        before_request: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<R, BsvHomePortalAuthError> {
        let (path, method, body) = target.request()?;
        let (retry, submission) = match method {
            AlienHttpMethod::Get => (SessionRetry::Authenticate, Submission::Read),
            AlienHttpMethod::Post | AlienHttpMethod::Put | AlienHttpMethod::Delete | AlienHttpMethod::Head | AlienHttpMethod::Patch | AlienHttpMethod::Options => {
                (SessionRetry::Never, Submission::Write)
            }
        };
        self.wallet_scoped_request(
            wallet,
            &path,
            method,
            body,
            retry,
            WalletRequestCheckpoint {
                generation,
                check,
                before_request,
                kind: WalletRequestKind::MultisigAction(submission),
            },
        )
        .await
    }

    async fn swap_request<R: DeserializeOwned>(&self, path: &str, method: AlienHttpMethod, body: Option<Vec<u8>>) -> Result<R, BsvHomePortalAuthError> {
        let wallet = self.swap_wallet().await?;
        self.swap_wallet_request(&wallet, path, method, body).await
    }

    async fn swap_wallet_request<R: DeserializeOwned>(&self, wallet: &Wallet, path: &str, method: AlienHttpMethod, body: Option<Vec<u8>>) -> Result<R, BsvHomePortalAuthError> {
        self.wallet_request(wallet, path, method, body, SessionRetry::Authenticate).await
    }

    async fn wallet_request<R: DeserializeOwned>(
        &self,
        wallet: &Wallet,
        path: &str,
        method: AlienHttpMethod,
        body: Option<Vec<u8>>,
        retry: SessionRetry,
    ) -> Result<R, BsvHomePortalAuthError> {
        self.wallet_checkpoint_request(wallet, path, method, body, retry, &|| Ok(())).await
    }

    async fn wallet_checkpoint_request<R: DeserializeOwned>(
        &self,
        wallet: &Wallet,
        path: &str,
        method: AlienHttpMethod,
        body: Option<Vec<u8>>,
        retry: SessionRetry,
        before_request: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<R, BsvHomePortalAuthError> {
        self.wallet_scoped_request(
            wallet,
            path,
            method,
            body,
            retry,
            WalletRequestCheckpoint {
                generation: self.connection_version.load(Ordering::SeqCst),
                check: &|| Ok(()),
                before_request,
                kind: WalletRequestKind::Standard,
            },
        )
        .await
    }

    async fn wallet_request_at_generation<R: DeserializeOwned>(
        &self,
        wallet: &Wallet,
        path: &str,
        method: AlienHttpMethod,
        body: Option<Vec<u8>>,
        retry: SessionRetry,
        generation: u64,
    ) -> Result<R, BsvHomePortalAuthError> {
        self.wallet_scoped_request(
            wallet,
            path,
            method,
            body,
            retry,
            WalletRequestCheckpoint {
                generation,
                check: &|| Ok(()),
                before_request: &|| Ok(()),
                kind: WalletRequestKind::Standard,
            },
        )
        .await
    }

    async fn wallet_read_checkpoint_request_at_generation<R: DeserializeOwned>(
        &self,
        wallet: &Wallet,
        path: &str,
        generation: u64,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<R, BsvHomePortalAuthError> {
        self.wallet_scoped_request(
            wallet,
            path,
            AlienHttpMethod::Get,
            None,
            SessionRetry::Authenticate,
            WalletRequestCheckpoint {
                generation,
                check,
                before_request: &|| Ok(()),
                kind: WalletRequestKind::Standard,
            },
        )
        .await
    }

    async fn transfer_password_request_at_generation<R: DeserializeOwned>(
        &self,
        wallet: &Wallet,
        action: TransferPasswordAction,
        body: Vec<u8>,
        retry: SessionRetry,
        generation: u64,
    ) -> Result<R, BsvHomePortalAuthError> {
        self.wallet_scoped_request(
            wallet,
            action.path(),
            AlienHttpMethod::Post,
            Some(body),
            retry,
            WalletRequestCheckpoint {
                generation,
                check: &|| Ok(()),
                before_request: &|| Ok(()),
                kind: WalletRequestKind::TransferPassword(action),
            },
        )
        .await
    }

    async fn wallet_scoped_request<R: DeserializeOwned>(
        &self,
        wallet: &Wallet,
        path: &str,
        method: AlienHttpMethod,
        body: Option<Vec<u8>>,
        retry: SessionRetry,
        checkpoint: WalletRequestCheckpoint<'_>,
    ) -> Result<R, BsvHomePortalAuthError> {
        match &self.wallet_scope {
            WalletScope::Current => {}
            WalletScope::Fixed { .. } => {
                let permitted = match checkpoint.kind {
                    WalletRequestKind::Standard | WalletRequestKind::MultisigAction(_) => path.starts_with("/auth/multisig/"),
                    WalletRequestKind::TransferPassword(action) => action.path() == path && TransferPasswordAction::permits(&self.client.base_url, path, method),
                };
                if !permitted {
                    return Err(BsvHomePortalAuthError::WalletChanged);
                }
            }
        }
        let version = checkpoint.generation;
        (checkpoint.check)()?;
        let identity = rules::identity(wallet)?;
        let _connection = self.connection.lock().await;
        (checkpoint.check)()?;
        self.check_checkpoint(&identity, version, checkpoint.check).await?;
        if path.starts_with("/auth/multisig/") {
            self.client.multisig_request_ready()?;
        }
        let mut session = match self.cached_session(&identity)? {
            Some(session) => session,
            None => self.login(&identity, version, checkpoint.check).await?,
        };
        for attempt in 0..=1 {
            self.check_checkpoint(&identity, version, checkpoint.check).await?;
            match checkpoint.kind {
                WalletRequestKind::MultisigAction(Submission::Read) => {}
                WalletRequestKind::MultisigAction(Submission::Write) | WalletRequestKind::Standard | WalletRequestKind::TransferPassword(_) => (checkpoint.before_request)()?,
            }
            let response = self.client.authenticated(&session.token, path, method, body.clone()).await;
            let preserve_submission = match checkpoint.kind {
                WalletRequestKind::MultisigAction(Submission::Write) => response.is_ok(),
                WalletRequestKind::MultisigAction(Submission::Read) | WalletRequestKind::Standard | WalletRequestKind::TransferPassword(_) => false,
            };
            if !preserve_submission {
                self.check_checkpoint(&identity, version, checkpoint.check).await?;
            }
            match response {
                Ok(response) => return Ok(response),
                Err(error) if error.requires_authentication => {
                    self.store.remove(SESSION.into())?;
                    if path.starts_with("/auth/multisig/") && self.client.multisig_request_ready().is_err() {
                        return Err(error.into());
                    }
                    let can_retry = match retry {
                        SessionRetry::Authenticate => attempt == 0,
                        SessionRetry::Never => false,
                    };
                    if !can_retry {
                        return Err(error.into());
                    }
                    session = self.login(&identity, version, checkpoint.check).await?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(BsvHomePortalAuthError::InvalidSession)
    }

    async fn swap_wallet(&self) -> Result<Wallet, BsvHomePortalAuthError> {
        let wallet = self.session.current_wallet().await?;
        rules::identity(&wallet)?;
        Ok(wallet)
    }

    async fn check_connection(&self, identity: &Identity, version: u64) -> Result<(), BsvHomePortalAuthError> {
        self.check_version(version)?;
        let current = match &self.wallet_scope {
            WalletScope::Current => self.session.get_current_wallet().await?,
            WalletScope::Fixed { wallet_id, address } => {
                self.check_version(0)?;
                if identity.wallet_id != wallet_id.id() || identity.address != *address {
                    return Err(BsvHomePortalAuthError::WalletChanged);
                }
                self.session.get_wallet(wallet_id.clone()).await?
            }
        };
        let unchanged = match current {
            Some(wallet) => match rules::identity(&wallet) {
                Ok(current) => current == *identity,
                Err(BsvHomePortalAuthError::MissingBsvAccount | BsvHomePortalAuthError::WatchOnly) => false,
                Err(error) => return Err(error),
            },
            None => false,
        };
        if !unchanged {
            self.clear_session(identity)?;
            return Err(BsvHomePortalAuthError::WalletChanged);
        }
        self.check_version(version)?;
        Ok(())
    }

    fn check_version(&self, version: u64) -> Result<(), BsvHomePortalAuthError> {
        if self.connection_version.load(Ordering::SeqCst) != version {
            return Err(BsvHomeServiceError::Cancelled.into());
        }
        Ok(())
    }

    fn clear_session(&self, identity: &Identity) -> Result<(), BsvHomePortalAuthError> {
        if let Some(value) = self.store.get(SESSION.into())?
            && let Ok(session) = serde_json::from_str::<Session>(&value)
            && session.wallet_id == identity.wallet_id
            && session.address == identity.address
            && session.base_url == self.client.base_url
        {
            self.store.remove(SESSION.into())?;
        }
        Ok(())
    }

    fn cached_session(&self, identity: &Identity) -> Result<Option<Session>, BsvHomePortalAuthError> {
        let Some(value) = self.store.get(SESSION.into())? else {
            return Ok(None);
        };
        if let Ok(session) = serde_json::from_str::<Session>(&value)
            && rules::session_matches(&session, identity, &self.client.base_url, Utc::now())
        {
            return Ok(Some(session));
        }
        self.store.remove(SESSION.into())?;
        Ok(None)
    }

    async fn check_checkpoint(&self, identity: &Identity, version: u64, check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync)) -> Result<(), BsvHomePortalAuthError> {
        check()?;
        self.check_connection(identity, version).await?;
        check()
    }

    async fn login(&self, identity: &Identity, version: u64, check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync)) -> Result<Session, BsvHomePortalAuthError> {
        self.check_checkpoint(identity, version, check).await?;
        let device_id = self.device_key.device_id()?;
        let request = || IdentityRequest {
            chain: rules::AUTH_CHAIN,
            address: &identity.address,
            device_id: &device_id,
            device_type: &self.device_type,
        };
        let challenge = self.client.challenge(&request()).await?;
        self.check_checkpoint(identity, version, check).await?;
        rules::validate_challenge(&challenge, &request(), &self.client.base_url, Utc::now())?;
        let signer = MessageSigner::new(SignMessage {
            chain: Chain::BitcoinSv,
            sign_type: SignDigestType::BitcoinMessage,
            data: challenge.message.as_bytes().to_vec(),
        });
        check()?;
        let password = decode_password(&self.password.get_password(false)?);
        self.check_checkpoint(identity, version, check).await?;
        let signature = signer.sign_with_keystore(self.keystore.clone(), keystore_id_for_wallet(identity.wallet_id.clone()), password)?;
        if recover_bitcoin_message_address(challenge.message.as_bytes(), &signature)? != identity.address {
            return Err(BsvHomePortalAuthError::InvalidChallenge);
        }
        rules::validate_challenge(&challenge, &request(), &self.client.base_url, Utc::now())?;
        self.check_checkpoint(identity, version, check).await?;
        let login = self
            .client
            .login(&LoginRequest {
                identity: request(),
                challenge_id: &challenge.challenge_id,
                signature: &signature,
            })
            .await?;
        self.check_checkpoint(identity, version, check).await?;
        rules::validate_login(&login, identity, Utc::now())?;
        let session = Session {
            base_url: self.client.base_url.clone(),
            wallet_id: identity.wallet_id.clone(),
            address: identity.address.clone(),
            token: login.token,
            expires_at: login.expires_at,
        };
        let value = serde_json::to_string(&session).map_err(|_| BsvHomePortalAuthError::InvalidSession)?;
        check()?;
        self.store.set(SESSION.into(), value)?;
        if let Err(error) = self.check_checkpoint(identity, version, check).await {
            self.clear_session(identity)?;
            return Err(error);
        }
        Ok(session)
    }
}
