use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use futures::lock::Mutex;
use primitives::{Chain, Wallet};
use serde::Serialize;
use zeroize::Zeroizing;

use crate::alien::AlienHttpMethod;
use crate::keystore::{decode_password, keystore_id_for_wallet};
use crate::services::error::BsvHomeServiceError;

use super::paymail_error::BsvHomePaymailError;
use super::paymail_model::{
    Binding, BindingChallenge, BindingRequest, BsvHomePaymailBinding, BsvHomePaymailBindingIntent, BsvHomePaymailMutation, BsvHomePaymailMutationStatus, BsvHomePaymailSnapshot,
    BsvHomePaymailVerification, ChallengeRequest, Configuration, PendingBinding,
};
use super::{BsvHomePortalAuthError, BsvHomePortalAuthService, SessionRetry, paymail_rules, rules};

#[derive(uniffi::Object)]
pub struct BsvHomePaymailService {
    auth: Arc<BsvHomePortalAuthService>,
    wallet_id: String,
    trusted_public_origins: Vec<String>,
    mutation: Mutex<()>,
}

#[uniffi::export]
impl BsvHomePaymailService {
    #[uniffi::constructor]
    pub fn new(auth: Arc<BsvHomePortalAuthService>, trusted_public_origins: Vec<String>, wallet_id: String) -> Result<Self, BsvHomePaymailError> {
        let origins = trusted_public_origins.iter().map(|value| paymail_rules::origin(value)).collect::<Result<Vec<_>, _>>()?;
        if origins.is_empty() || wallet_id.is_empty() {
            return Err(BsvHomePaymailError::InvalidConfiguration);
        }
        Ok(Self {
            auth,
            wallet_id,
            trusted_public_origins: origins,
            mutation: Mutex::new(()),
        })
    }

    pub async fn load(&self) -> Result<BsvHomePaymailSnapshot, BsvHomePaymailError> {
        self.load_with_checkpoint(&|| Ok(())).await
    }

    pub async fn binding_intent(&self, snapshot: BsvHomePaymailSnapshot, alias: String, domain: String, active: bool) -> Result<BsvHomePaymailBindingIntent, BsvHomePaymailError> {
        self.require_verified(&snapshot)?;
        let version = self.workflow_version();
        let wallet = self.current_wallet().await?;
        let alias = paymail_rules::alias(&alias)?;
        let domain = paymail_rules::domain(&domain)?;
        let previous = paymail_rules::previous_binding(&snapshot, &alias, &domain, active)?;
        let (_, current) = self.fetch_state(&wallet, version).await?;
        self.require_verified(&current)?;
        let current_previous = paymail_rules::previous_binding(&current, &alias, &domain, active)?;
        if snapshot.wallet_id != current.wallet_id || snapshot.address != current.address || snapshot.public_origin != current.public_origin || previous != current_previous {
            return Err(BsvHomePaymailError::BindingChanged);
        }
        let previous_version = current_previous.map_or(0, |previous| previous.version);
        Ok(BsvHomePaymailBindingIntent {
            wallet_id: current.wallet_id,
            address: current.address,
            public_origin: current.public_origin,
            handle: format!("{alias}@{domain}"),
            active,
            previous_version,
        })
    }

    pub async fn submit_binding(&self, intent: BsvHomePaymailBindingIntent) -> Result<BsvHomePaymailMutation, BsvHomePaymailError> {
        let version = self.workflow_version();
        let _mutation = self.mutation.lock().await;
        let wallet = self.current_wallet().await?;
        self.check_workflow(&wallet, version).await?;
        let (configuration, snapshot) = self.fetch_state(&wallet, version).await?;
        self.require_verified(&snapshot)?;
        let (alias, domain) = paymail_rules::handle(&intent.handle)?;
        let previous = paymail_rules::previous_binding(&snapshot, &alias, &domain, intent.active)?;
        if intent.wallet_id != snapshot.wallet_id
            || intent.address != snapshot.address
            || intent.public_origin != snapshot.public_origin
            || intent.handle != format!("{alias}@{domain}")
            || intent.previous_version != previous.map_or(0, |binding| binding.version)
        {
            return Err(BsvHomePaymailError::BindingChanged);
        }
        let password = Zeroizing::new(decode_password(&self.auth.password.get_password(false)?));
        self.check_workflow(&wallet, version).await?;
        let identity_public_key = self
            .auth
            .keystore
            .paymail_identity_public_key(keystore_id_for_wallet(intent.wallet_id.clone()), &intent.address, password.to_vec())?;
        if previous.is_some_and(|previous| previous.identity_public_key != identity_public_key) {
            return Err(BsvHomePaymailError::IdentityMismatch);
        }
        let challenge = self
            .auth
            .wallet_request::<BindingChallenge>(
                &wallet,
                "/auth/paymail/challenges",
                AlienHttpMethod::Post,
                Some(encode(&ChallengeRequest {
                    alias: &alias,
                    domain: &domain,
                    address: &intent.address,
                    identity_public_key: &identity_public_key,
                    active: intent.active,
                })?),
                SessionRetry::Authenticate,
            )
            .await?;
        self.check_workflow(&wallet, version).await?;
        paymail_rules::validate_challenge(&challenge, &intent, &identity_public_key, Utc::now())?;
        let signatures = self.auth.keystore.sign_paymail_binding(
            keystore_id_for_wallet(intent.wallet_id.clone()),
            &intent.address,
            &identity_public_key,
            &challenge.message,
            password.to_vec(),
        )?;
        drop(password);
        self.check_workflow(&wallet, version).await?;
        paymail_rules::validate_challenge(&challenge, &intent, &identity_public_key, Utc::now())?;
        let pending = PendingBinding {
            intent: intent.clone(),
            identity_public_key: identity_public_key.clone(),
        };
        let checkpointed = AtomicBool::new(false);
        let submission = self
            .auth
            .wallet_checkpoint_request::<Binding>(
                &wallet,
                "/auth/paymail/bindings",
                AlienHttpMethod::Post,
                Some(encode(&BindingRequest {
                    challenge_id: &challenge.challenge_id,
                    address_signature: &signatures.address_signature,
                    identity_signature: &signatures.identity_signature,
                })?),
                SessionRetry::Never,
                &|| {
                    self.save_pending(&pending)?;
                    checkpointed.store(true, Ordering::SeqCst);
                    Ok(())
                },
            )
            .await;
        self.check_workflow(&wallet, version).await?;
        let binding = match submission {
            Ok(binding) => binding,
            Err(error) if !checkpointed.load(Ordering::SeqCst) => return Err(error.into()),
            Err(error) => return self.reconcile_submission(&wallet, &intent, &identity_public_key, version, error.into()).await,
        };
        let mut bindings = snapshot.bindings.into_iter().map(binding_from_snapshot).collect::<Vec<_>>();
        bindings.retain(|previous| previous.handle != intent.handle);
        bindings.push(binding);
        let confirmed = paymail_rules::snapshot(&wallet, &configuration, bindings);
        match confirmed {
            Ok(snapshot)
                if snapshot
                    .bindings
                    .iter()
                    .any(|binding| paymail_rules::binding_matches(binding, &intent, &identity_public_key)) =>
            {
                self.clear_pending(&intent)?;
                Ok(BsvHomePaymailMutation {
                    status: BsvHomePaymailMutationStatus::Confirmed,
                    snapshot: Some(snapshot),
                })
            }
            Ok(_) | Err(_) => {
                self.reconcile_submission(&wallet, &intent, &identity_public_key, version, BsvHomePaymailError::InvalidResponse)
                    .await
            }
        }
    }

    pub fn cancel(&self) {
        self.auth.cancel();
    }
}

impl BsvHomePaymailService {
    pub(super) async fn current_wallet(&self) -> Result<Wallet, BsvHomePaymailError> {
        let wallet = self.auth.session.current_wallet().await?;
        if wallet.id.id() != self.wallet_id {
            return Err(BsvHomePortalAuthError::WalletChanged.into());
        }
        rules::identity(&wallet)?;
        Ok(wallet)
    }

    pub(super) fn workflow_version(&self) -> u64 {
        self.auth.connection_version.load(Ordering::SeqCst)
    }

    pub(super) fn check_workflow_scope(&self, wallet: &Wallet, version: u64) -> Result<(), BsvHomePaymailError> {
        self.auth.check_version(version)?;
        if wallet.id.id() != self.wallet_id || self.auth.session.current_wallet_id()? != wallet.id {
            return Err(BsvHomePortalAuthError::WalletChanged.into());
        }
        Ok(())
    }

    pub(super) async fn check_workflow(&self, wallet: &Wallet, version: u64) -> Result<(), BsvHomePaymailError> {
        self.check_workflow_scope(wallet, version)?;
        let current = self.current_wallet().await?;
        if current.id != wallet.id || current.wallet_type != wallet.wallet_type || current.account(Chain::BitcoinSv) != wallet.account(Chain::BitcoinSv) {
            return Err(BsvHomePaymailError::BindingChanged);
        }
        self.auth.check_connection(&rules::identity(wallet)?, version).await?;
        self.check_workflow_scope(wallet, version)?;
        Ok(())
    }

    pub(super) async fn load_with_checkpoint(&self, check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync)) -> Result<BsvHomePaymailSnapshot, BsvHomePaymailError> {
        check()?;
        let version = self.workflow_version();
        let wallet = self.current_wallet().await?;
        check()?;
        self.fetch_state_with_checkpoint(&wallet, version, check).await.map(|(_, snapshot)| snapshot)
    }

    async fn fetch_state(&self, wallet: &Wallet, version: u64) -> Result<(Configuration, BsvHomePaymailSnapshot), BsvHomePaymailError> {
        self.fetch_state_with_checkpoint(wallet, version, &|| Ok(())).await
    }

    async fn fetch_state_with_checkpoint(
        &self,
        wallet: &Wallet,
        version: u64,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<(Configuration, BsvHomePaymailSnapshot), BsvHomePaymailError> {
        let configuration = self.checked_configuration(wallet, version, check).await?;
        let bindings = self.checked_bindings(wallet, version, check).await?;
        check()?;
        let snapshot = self.apply_verification(paymail_rules::snapshot(wallet, &configuration, bindings)?)?;
        check()?;
        Ok((configuration, snapshot))
    }

    async fn checked_configuration(
        &self,
        wallet: &Wallet,
        version: u64,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<Configuration, BsvHomePaymailError> {
        let configuration = self.checked_read(wallet, "/auth/paymail/config", version, check).await?;
        paymail_rules::configuration(configuration, &self.trusted_public_origins)
    }

    async fn fetch_configuration(&self, wallet: &Wallet, version: u64) -> Result<Configuration, BsvHomePaymailError> {
        self.checked_configuration(wallet, version, &|| Ok(())).await
    }

    async fn checked_bindings(&self, wallet: &Wallet, version: u64, check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync)) -> Result<Vec<Binding>, BsvHomePaymailError> {
        self.checked_read(wallet, "/auth/paymail/me", version, check).await
    }

    async fn fetch_bindings(&self, wallet: &Wallet, version: u64) -> Result<Vec<Binding>, BsvHomePaymailError> {
        self.checked_bindings(wallet, version, &|| Ok(())).await
    }

    async fn checked_read<R: DeserializeOwned>(
        &self,
        wallet: &Wallet,
        path: &str,
        version: u64,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<R, BsvHomePaymailError> {
        check()?;
        self.check_workflow(wallet, version).await?;
        check()?;
        let response = self.auth.wallet_read_checkpoint_request_at_generation(wallet, path, version, check).await?;
        self.check_workflow(wallet, version).await?;
        check()?;
        Ok(response)
    }

    async fn fetch_reconciliation(&self, wallet: &Wallet, version: u64) -> Result<BsvHomePaymailSnapshot, BsvHomePaymailError> {
        let bindings = self.fetch_bindings(wallet, version).await?;
        let configuration = self.fetch_configuration(wallet, version).await?;
        self.apply_verification(paymail_rules::snapshot(wallet, &configuration, bindings)?)
    }

    async fn reconcile_submission(
        &self,
        wallet: &Wallet,
        intent: &BsvHomePaymailBindingIntent,
        identity_public_key: &str,
        version: u64,
        error: BsvHomePaymailError,
    ) -> Result<BsvHomePaymailMutation, BsvHomePaymailError> {
        self.check_workflow(wallet, version).await?;
        let snapshot = match self.fetch_reconciliation(wallet, version).await {
            Ok(snapshot) => Some(snapshot),
            Err(_) => {
                self.check_workflow(wallet, version).await?;
                None
            }
        };
        let (_, domain) = paymail_rules::handle(&intent.handle)?;
        let status = match &snapshot {
            Some(snapshot)
                if snapshot.public_origin == intent.public_origin
                    && snapshot.bindings.iter().any(|binding| paymail_rules::binding_matches(binding, intent, identity_public_key)) =>
            {
                BsvHomePaymailMutationStatus::Reconciled
            }
            Some(snapshot)
                if snapshot.public_origin == intent.public_origin
                    && snapshot
                        .bindings
                        .iter()
                        .any(|binding| binding.handle.ends_with(&format!("@{domain}")) && binding.version > intent.previous_version) =>
            {
                BsvHomePaymailMutationStatus::Changed
            }
            Some(_) | None => match &error {
                BsvHomePaymailError::Api { code, .. } if paymail_rules::binding_rejection(code) => {
                    self.clear_pending(intent)?;
                    return Err(error);
                }
                BsvHomePaymailError::Auth { .. }
                | BsvHomePaymailError::InvalidAlias
                | BsvHomePaymailError::InvalidDomain
                | BsvHomePaymailError::InvalidHandle
                | BsvHomePaymailError::InvalidConfiguration
                | BsvHomePaymailError::InvalidChallenge
                | BsvHomePaymailError::InvalidResponse
                | BsvHomePaymailError::IdentityMismatch
                | BsvHomePaymailError::BindingChanged
                | BsvHomePaymailError::PendingVerification
                | BsvHomePaymailError::UnsupportedCapability
                | BsvHomePaymailError::UnsupportedOutput
                | BsvHomePaymailError::InvalidAmount
                | BsvHomePaymailError::SenderRequired
                | BsvHomePaymailError::Api { .. } => BsvHomePaymailMutationStatus::PendingVerification,
            },
        };
        Ok(BsvHomePaymailMutation { status, snapshot })
    }

    pub(super) fn require_verified(&self, snapshot: &BsvHomePaymailSnapshot) -> Result<(), BsvHomePaymailError> {
        if snapshot.verification == BsvHomePaymailVerification::Pending {
            return Err(BsvHomePaymailError::PendingVerification);
        }
        Ok(())
    }

    fn pending_key(&self, wallet_id: &str, address: &str) -> String {
        format!("paymail_pending|{}|{wallet_id}|{address}", self.auth.client.base_url)
    }

    fn save_pending(&self, pending: &PendingBinding) -> Result<(), BsvHomePortalAuthError> {
        let key = self.pending_key(&pending.intent.wallet_id, &pending.intent.address);
        let value = serde_json::to_string(pending).map_err(|_| BsvHomePortalAuthError::InvalidResponse)?;
        self.auth.store.set(key, value)?;
        Ok(())
    }

    fn clear_pending(&self, intent: &BsvHomePaymailBindingIntent) -> Result<(), BsvHomePaymailError> {
        self.auth.store.remove(self.pending_key(&intent.wallet_id, &intent.address))?;
        Ok(())
    }

    fn apply_verification(&self, mut snapshot: BsvHomePaymailSnapshot) -> Result<BsvHomePaymailSnapshot, BsvHomePaymailError> {
        let Some(value) = self.auth.store.get(self.pending_key(&snapshot.wallet_id, &snapshot.address))? else {
            return Ok(snapshot);
        };
        let pending: PendingBinding = serde_json::from_str(&value).map_err(|_| BsvHomePaymailError::InvalidResponse)?;
        let (_, domain) = paymail_rules::handle(&pending.intent.handle)?;
        paymail_rules::identity_public_key(&pending.identity_public_key)?;
        if pending.intent.wallet_id != snapshot.wallet_id || pending.intent.address != snapshot.address || pending.intent.previous_version >= i64::MAX as u64 {
            return Err(BsvHomePaymailError::InvalidResponse);
        }
        snapshot.verification = if pending.intent.public_origin != snapshot.public_origin {
            BsvHomePaymailVerification::Pending
        } else if snapshot
            .bindings
            .iter()
            .any(|binding| paymail_rules::binding_matches(binding, &pending.intent, &pending.identity_public_key))
        {
            BsvHomePaymailVerification::Reconciled
        } else if snapshot
            .bindings
            .iter()
            .any(|binding| binding.handle.ends_with(&format!("@{domain}")) && binding.version > pending.intent.previous_version)
        {
            BsvHomePaymailVerification::Changed
        } else {
            BsvHomePaymailVerification::Pending
        };
        match snapshot.verification {
            BsvHomePaymailVerification::Reconciled | BsvHomePaymailVerification::Changed => self.clear_pending(&pending.intent)?,
            BsvHomePaymailVerification::Clear | BsvHomePaymailVerification::Pending => {}
        }
        if snapshot.verification == BsvHomePaymailVerification::Pending {
            snapshot.active_handle = None;
        }
        Ok(snapshot)
    }
}

fn binding_from_snapshot(binding: BsvHomePaymailBinding) -> Binding {
    Binding {
        handle: binding.handle,
        address: binding.address,
        identity_public_key: binding.identity_public_key,
        active: binding.active,
        version: binding.version,
    }
}

fn encode(value: &impl Serialize) -> Result<Vec<u8>, BsvHomePaymailError> {
    serde_json::to_vec(value).map_err(|_| {
        BsvHomePaymailError::from(BsvHomeServiceError::Core {
            msg: "Paymail request encoding failed".into(),
        })
    })
}
