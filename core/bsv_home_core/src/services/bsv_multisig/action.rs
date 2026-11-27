use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use bsv_home_bsv::BsvAddress;
use bsv_home_hash::sha2::sha256;
use futures::future::{AbortHandle, Abortable};
use primitives::{Address, Chain, Wallet, WalletId};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use super::funding::PublicFundingPayerEligibility;
use super::model::{RemoteProposal, RemoteWallet};
use super::paymail::bsv_multisig_paymail_alias;
use super::public_funding::BsvHomeBsvMultisigPublicFundingSource;
use super::service::{authentication, identity_address};
use super::{BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigService, rules};
use crate::services::bsv_cold_wallet::{BsvHomeBsvColdWalletStore, bsv_cold_wallet_require_hot_signing, rules::require_hot_wallet_id};
use crate::services::error::BsvHomeServiceError;
use crate::services::portal::BsvHomePortalAuthService;
use crate::services::portal::{BsvHomePortalAuthError, BsvMultisigTarget as Target};

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvMultisigActionIntent {
    PrepareProposal {
        wallet_id: String,
        destination: String,
        amount_bsv: String,
        memo: String,
    },
    SignProposal {
        proposal_id: String,
        expected_version: u64,
        review_id: String,
    },
    BroadcastProposal {
        proposal_id: String,
        expected_version: u64,
        review_id: String,
    },
    SubmitFunding {
        preparation_id: String,
    },
    RetryFunding {
        wallet_id: String,
    },
    RetryPublicFunding {
        operation_id: String,
    },
    WithdrawProposal {
        proposal_id: String,
        expected_version: u64,
        review_id: String,
    },
    UpdatePaymailSettings {
        wallet_id: String,
        alias: String,
        active: bool,
        expected_version: u64,
        configuration_hash: String,
    },
}

impl BsvHomeBsvMultisigActionIntent {
    fn proposal_id(&self) -> Option<String> {
        match self {
            Self::SignProposal { proposal_id, .. } | Self::BroadcastProposal { proposal_id, .. } | Self::WithdrawProposal { proposal_id, .. } => Some(proposal_id.clone()),
            Self::PrepareProposal { .. } | Self::SubmitFunding { .. } | Self::RetryFunding { .. } | Self::RetryPublicFunding { .. } | Self::UpdatePaymailSettings { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvMultisigActionSubmission {
    NotSubmitted,
    CancelledBeforeSubmission,
    Unknown,
    Submitted,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvMultisigActionOutcome {
    pub submission: BsvHomeBsvMultisigActionSubmission,
    pub proposal_id: Option<String>,
    pub transaction_id: Option<String>,
    pub sync_failed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvMultisigAuthorizationRequest {
    PrepareProposal {
        wallet_id: String,
        expected_version: u64,
        configuration_hash: String,
        destination: String,
        amount_satoshis: String,
        memo: String,
    },
    SignProposal {
        wallet_id: String,
        proposal_id: String,
        expected_version: u64,
        configuration_hash: String,
        destination: String,
        amount_satoshis: String,
        fee_satoshis: String,
    },
    BroadcastProposal {
        wallet_id: String,
        proposal_id: String,
        expected_version: u64,
        configuration_hash: String,
        destination: String,
        amount_satoshis: String,
        fee_satoshis: String,
        transaction_id: String,
    },
    Funding {
        target_name: String,
        commitment: String,
        configuration_hash: String,
        sender_address: String,
        amount_satoshis: String,
        fee_satoshis: String,
        transaction_id: Option<String>,
    },
}

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeBsvMultisigAuthorization: Send + Sync {
    async fn authorize(&self, wallet: Wallet, request: BsvHomeBsvMultisigAuthorizationRequest, scope: Arc<BsvHomeBsvMultisigActionScope>) -> Result<(), BsvHomeServiceError>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Running,
    Finished,
}

struct ActionState {
    phase: Phase,
    abort: Option<AbortHandle>,
    outcome: BsvHomeBsvMultisigActionOutcome,
}

#[derive(uniffi::Object)]
pub struct BsvHomeBsvMultisigActionScope {
    id: Uuid,
    owner: Uuid,
    auth: Arc<BsvHomePortalAuthService>,
    cold_store: Arc<dyn BsvHomeBsvColdWalletStore>,
    generation: u64,
    wallet: Wallet,
    intent: BsvHomeBsvMultisigActionIntent,
    review_hash: [u8; 32],
    public_source: Option<BsvHomeBsvMultisigPublicFundingSource>,
    public_payer: Option<PublicFundingPayerEligibility>,
    funding_payer: Option<WalletId>,
    cancelled: AtomicBool,
    state: Mutex<ActionState>,
}

#[uniffi::export]
impl BsvHomeBsvMultisigActionScope {
    pub fn id(&self) -> String {
        self.id.to_string()
    }

    pub fn check(&self) -> Result<(), Error> {
        if self.auth.multisig_version() != self.generation {
            self.cancel();
            return Err(Error::Cancelled);
        }
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(Error::Cancelled);
        }
        if let Err(error) = self.auth.multisig_check_scope(&self.wallet.id.id()) {
            self.cancel();
            return Err(error.into());
        }
        match &self.public_payer {
            Some(payer) => {
                if let Err(error) = payer.check(&self.wallet.id) {
                    self.cancel();
                    return Err(error);
                }
            }
            None => {
                if let Err(error) = bsv_cold_wallet_require_hot_signing(self.wallet.clone(), Chain::BitcoinSv, self.cold_store.clone()) {
                    self.cancel();
                    return Err(authentication(error));
                }
            }
        }
        if let Some(payer) = self.funding_payer.clone()
            && let Err(error) = require_hot_wallet_id(payer, Chain::BitcoinSv, self.cold_store.as_ref())
        {
            self.cancel();
            return Err(authentication(error));
        }
        if let Some(source) = &self.public_source
            && let Err(error) = source.check_payer(&self.wallet)
        {
            self.cancel();
            return Err(error);
        }
        drop(self.state()?);
        Ok(())
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Ok(state) = self.state.lock()
            && let Some(abort) = &state.abort
        {
            abort.abort();
        }
    }

    pub fn outcome(&self) -> Result<BsvHomeBsvMultisigActionOutcome, Error> {
        let invalidated = self.check().is_err();
        let mut outcome = self.state()?.outcome.clone();
        if invalidated && outcome.submission == BsvHomeBsvMultisigActionSubmission::NotSubmitted {
            outcome.submission = BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission;
        }
        Ok(outcome)
    }
}

impl BsvHomeBsvMultisigActionScope {
    pub(super) fn new(
        owner: Uuid,
        auth: Arc<BsvHomePortalAuthService>,
        generation: u64,
        wallet: Wallet,
        intent: BsvHomeBsvMultisigActionIntent,
        review_hash: [u8; 32],
        cold_store: Arc<dyn BsvHomeBsvColdWalletStore>,
        funding_payer: Option<WalletId>,
    ) -> Arc<Self> {
        Self::new_with_source(owner, auth, generation, wallet, intent, review_hash, None, None, cold_store, funding_payer)
    }

    pub(super) fn new_with_source(
        owner: Uuid,
        auth: Arc<BsvHomePortalAuthService>,
        generation: u64,
        wallet: Wallet,
        intent: BsvHomeBsvMultisigActionIntent,
        review_hash: [u8; 32],
        public_source: Option<BsvHomeBsvMultisigPublicFundingSource>,
        public_payer: Option<PublicFundingPayerEligibility>,
        cold_store: Arc<dyn BsvHomeBsvColdWalletStore>,
        funding_payer: Option<WalletId>,
    ) -> Arc<Self> {
        let proposal_id = intent.proposal_id();
        Arc::new(Self {
            id: Uuid::new_v4(),
            owner,
            auth,
            cold_store,
            generation,
            wallet,
            intent,
            review_hash,
            public_source,
            public_payer,
            funding_payer,
            cancelled: AtomicBool::new(false),
            state: Mutex::new(ActionState {
                phase: Phase::Ready,
                abort: None,
                outcome: BsvHomeBsvMultisigActionOutcome {
                    submission: BsvHomeBsvMultisigActionSubmission::NotSubmitted,
                    proposal_id,
                    transaction_id: None,
                    sync_failed: false,
                },
            }),
        })
    }

    pub(super) async fn run<T>(&self, future: impl Future<Output = Result<T, Error>>) -> Result<T, Error> {
        let (abort, registration) = AbortHandle::new_pair();
        {
            let mut state = self.state()?;
            if self.cancelled.load(Ordering::SeqCst) {
                return Err(Error::Cancelled);
            }
            state.abort = Some(abort);
        }
        let result = Abortable::new(future, registration).await.map_err(|_| Error::Cancelled).and_then(|result| result);
        let mut state = self.state()?;
        state.abort = None;
        if result.is_err() && state.outcome.submission == BsvHomeBsvMultisigActionSubmission::Submitted {
            state.outcome.sync_failed = true;
        }
        result
    }

    pub(super) fn review_id(&self) -> Result<&str, Error> {
        match &self.intent {
            BsvHomeBsvMultisigActionIntent::SignProposal { review_id, .. }
            | BsvHomeBsvMultisigActionIntent::BroadcastProposal { review_id, .. }
            | BsvHomeBsvMultisigActionIntent::WithdrawProposal { review_id, .. } => Ok(review_id),
            BsvHomeBsvMultisigActionIntent::PrepareProposal { .. }
            | BsvHomeBsvMultisigActionIntent::SubmitFunding { .. }
            | BsvHomeBsvMultisigActionIntent::RetryFunding { .. }
            | BsvHomeBsvMultisigActionIntent::RetryPublicFunding { .. }
            | BsvHomeBsvMultisigActionIntent::UpdatePaymailSettings { .. } => Err(Error::ConcurrentUpdate),
        }
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn claim(&self, owner: Uuid, wallet: &Wallet, intent: &BsvHomeBsvMultisigActionIntent, review_hash: &[u8; 32]) -> Result<(), Error> {
        self.check_binding(owner, wallet, intent, review_hash)?;
        let mut state = self.state()?;
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(Error::Cancelled);
        }
        if state.phase != Phase::Ready {
            return Err(Error::ConcurrentUpdate);
        }
        state.phase = Phase::Running;
        Ok(())
    }

    pub(super) fn check_binding(&self, owner: Uuid, wallet: &Wallet, intent: &BsvHomeBsvMultisigActionIntent, review_hash: &[u8; 32]) -> Result<(), Error> {
        self.check()?;
        if self.owner != owner
            || self.wallet.id != wallet.id
            || self.wallet.wallet_type != wallet.wallet_type
            || self.wallet.account(Chain::BitcoinSv) != wallet.account(Chain::BitcoinSv)
        {
            return Err(Error::WalletChanged);
        }
        if self.intent != *intent || self.review_hash != *review_hash {
            return Err(Error::ConcurrentUpdate);
        }
        Ok(())
    }

    pub(super) fn before_submission(&self, proposal_id: Option<String>, transaction_id: Option<String>) -> Result<(), Error> {
        self.check()?;
        let mut state = self.state()?;
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(Error::Cancelled);
        }
        if state.phase != Phase::Running || state.outcome.submission != BsvHomeBsvMultisigActionSubmission::NotSubmitted {
            return Err(Error::ConcurrentUpdate);
        }
        let proposal_id = merge_identity(&state.outcome.proposal_id, proposal_id)?;
        let transaction_id = merge_identity(&state.outcome.transaction_id, transaction_id)?;
        state.outcome.proposal_id = proposal_id;
        state.outcome.transaction_id = transaction_id;
        state.outcome.submission = BsvHomeBsvMultisigActionSubmission::Unknown;
        Ok(())
    }

    pub(super) fn submitted(&self, proposal_id: Option<String>, transaction_id: Option<String>) -> Result<(), Error> {
        let mut state = self.state()?;
        if state.phase != Phase::Running {
            return Err(Error::ConcurrentUpdate);
        }
        match state.outcome.submission {
            BsvHomeBsvMultisigActionSubmission::Unknown | BsvHomeBsvMultisigActionSubmission::Submitted => {}
            BsvHomeBsvMultisigActionSubmission::NotSubmitted | BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission => return Err(Error::ConcurrentUpdate),
        }
        let proposal_id = merge_identity(&state.outcome.proposal_id, proposal_id)?;
        let transaction_id = merge_identity(&state.outcome.transaction_id, transaction_id)?;
        state.outcome.proposal_id = proposal_id;
        state.outcome.transaction_id = transaction_id;
        state.outcome.submission = BsvHomeBsvMultisigActionSubmission::Submitted;
        Ok(())
    }

    pub(super) fn observed_submission(&self, transaction_id: String) -> Result<(), Error> {
        let mut state = self.state()?;
        if state.phase != Phase::Running || state.outcome.submission == BsvHomeBsvMultisigActionSubmission::CancelledBeforeSubmission {
            return Err(Error::ConcurrentUpdate);
        }
        state.outcome.transaction_id = merge_identity(&state.outcome.transaction_id, Some(transaction_id))?;
        state.outcome.submission = BsvHomeBsvMultisigActionSubmission::Submitted;
        Ok(())
    }

    pub(super) fn sync_failed(&self) -> Result<(), Error> {
        let mut state = self.state()?;
        if state.outcome.submission != BsvHomeBsvMultisigActionSubmission::Submitted {
            return Err(Error::ConcurrentUpdate);
        }
        state.outcome.sync_failed = true;
        Ok(())
    }

    pub(super) fn finish(&self) -> Result<(), Error> {
        let mut state = self.state()?;
        if state.phase != Phase::Running {
            return Err(Error::ConcurrentUpdate);
        }
        state.phase = Phase::Finished;
        Ok(())
    }

    fn state(&self) -> Result<MutexGuard<'_, ActionState>, Error> {
        self.state.lock().map_err(|_| Error::Storage {
            reason: "multisig action state unavailable".into(),
        })
    }
}

fn merge_identity(current: &Option<String>, next: Option<String>) -> Result<Option<String>, Error> {
    if let Some(next) = next {
        if current.as_ref().is_some_and(|current| *current != next) {
            return Err(Error::ConcurrentUpdate);
        }
        return Ok(Some(next));
    }
    Ok(current.clone())
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;

pub(super) struct ReviewedProposal {
    pub id: String,
    pub proposal_id: String,
    pub version: u64,
    pub hash: [u8; 32],
}

#[uniffi::export]
impl BsvHomeBsvMultisigService {
    pub fn begin_action(&self, intent: BsvHomeBsvMultisigActionIntent) -> Result<Arc<BsvHomeBsvMultisigActionScope>, Error> {
        self.check_identity()?;
        let wallet = self.cache()?.identity_wallet.clone().ok_or(Error::MissingSigner)?;
        let intent = self.action_intent(intent)?;
        let review_hash = self.action_review_hash(&intent)?;
        let payer = match &intent {
            BsvHomeBsvMultisigActionIntent::SubmitFunding { preparation_id } => Some(self.cache()?.preparations.get(preparation_id).ok_or(Error::FundingExpired)?.payer.id.clone()),
            BsvHomeBsvMultisigActionIntent::RetryFunding { wallet_id } => Some(self.funding_snapshot(wallet_id)?.binding.payer_wallet_id),
            BsvHomeBsvMultisigActionIntent::PrepareProposal { .. }
            | BsvHomeBsvMultisigActionIntent::SignProposal { .. }
            | BsvHomeBsvMultisigActionIntent::BroadcastProposal { .. }
            | BsvHomeBsvMultisigActionIntent::RetryPublicFunding { .. }
            | BsvHomeBsvMultisigActionIntent::WithdrawProposal { .. }
            | BsvHomeBsvMultisigActionIntent::UpdatePaymailSettings { .. } => None,
        };
        let action = BsvHomeBsvMultisigActionScope::new(
            self.action_owner,
            self.auth.clone(),
            self.auth.multisig_version(),
            wallet,
            intent,
            review_hash,
            self.cold_store.clone(),
            payer,
        );
        action.check()?;
        let mut cache = self.cache()?;
        cache.actions.retain(|action| action.strong_count() > 0);
        cache.actions.push(Arc::downgrade(&action));
        Ok(action)
    }
}

impl BsvHomeBsvMultisigService {
    pub(super) fn action_intent(&self, intent: BsvHomeBsvMultisigActionIntent) -> Result<BsvHomeBsvMultisigActionIntent, Error> {
        Ok(match intent {
            BsvHomeBsvMultisigActionIntent::RetryPublicFunding { .. } => return Err(Error::InvalidRules),
            BsvHomeBsvMultisigActionIntent::PrepareProposal {
                wallet_id,
                destination,
                amount_bsv,
                memo,
            } => {
                let amount = rules::amount_bsv(&amount_bsv)?;
                let destination = destination.trim().to_string();
                BsvAddress::from_str(&destination).map_err(|_| Error::invalid("INVALID_ADDRESS"))?;
                BsvHomeBsvMultisigActionIntent::PrepareProposal {
                    wallet_id,
                    destination,
                    amount_bsv: amount.to_string(),
                    memo: rules::text(&memo, 100, true)?,
                }
            }
            BsvHomeBsvMultisigActionIntent::UpdatePaymailSettings {
                wallet_id,
                alias,
                active,
                expected_version,
                configuration_hash,
            } => BsvHomeBsvMultisigActionIntent::UpdatePaymailSettings {
                wallet_id,
                alias: bsv_multisig_paymail_alias(alias)?,
                active,
                expected_version,
                configuration_hash,
            },
            other @ (BsvHomeBsvMultisigActionIntent::SignProposal { .. }
            | BsvHomeBsvMultisigActionIntent::BroadcastProposal { .. }
            | BsvHomeBsvMultisigActionIntent::WithdrawProposal { .. }
            | BsvHomeBsvMultisigActionIntent::SubmitFunding { .. }
            | BsvHomeBsvMultisigActionIntent::RetryFunding { .. }) => other,
        })
    }

    pub(super) fn action_review_hash(&self, intent: &BsvHomeBsvMultisigActionIntent) -> Result<[u8; 32], Error> {
        match intent {
            BsvHomeBsvMultisigActionIntent::PrepareProposal { wallet_id, .. } => review_hash(&self.remote_wallet(wallet_id)?),
            BsvHomeBsvMultisigActionIntent::SignProposal {
                proposal_id,
                expected_version,
                review_id,
            }
            | BsvHomeBsvMultisigActionIntent::BroadcastProposal {
                proposal_id,
                expected_version,
                review_id,
            }
            | BsvHomeBsvMultisigActionIntent::WithdrawProposal {
                proposal_id,
                expected_version,
                review_id,
            } => {
                let proposal = self.remote_proposal(proposal_id)?;
                if rules::nonnegative(proposal.version)? != *expected_version {
                    return Err(Error::ConcurrentUpdate);
                }
                let hash = review_hash(&(self.remote_wallet(&proposal.wallet_id)?, proposal))?;
                let cache = self.cache()?;
                let reviewed = cache.reviews.iter().find(|reviewed| reviewed.id == *review_id).ok_or(Error::ConcurrentUpdate)?;
                if reviewed.proposal_id != *proposal_id || reviewed.version != *expected_version || reviewed.hash != hash {
                    return Err(Error::ConcurrentUpdate);
                }
                Ok(hash)
            }
            BsvHomeBsvMultisigActionIntent::SubmitFunding { preparation_id } => {
                let cache = self.cache()?;
                let preparation = cache.preparations.get(preparation_id).ok_or(Error::FundingExpired)?;
                if preparation.auth_version != self.auth.multisig_version() {
                    return Err(Error::Cancelled);
                }
                preparation.action_hash()
            }
            BsvHomeBsvMultisigActionIntent::RetryFunding { wallet_id } => {
                let checkpoint = self.funding_snapshot(wallet_id)?;
                super::funding_operation::funding_action_hash(&checkpoint)
            }
            BsvHomeBsvMultisigActionIntent::RetryPublicFunding { .. } => Err(Error::InvalidRules),
            BsvHomeBsvMultisigActionIntent::UpdatePaymailSettings {
                wallet_id,
                expected_version,
                configuration_hash,
                ..
            } => {
                let cache = self.cache()?;
                let settings = cache.paymail_settings.get(wallet_id).cloned().ok_or(Error::MissingRecord)?;
                drop(cache);
                if settings.version != *expected_version || settings.configuration_hash != *configuration_hash {
                    return Err(Error::ConcurrentUpdate);
                }
                review_hash(&(self.remote_wallet(wallet_id)?, settings))
            }
        }
    }

    pub(super) fn check_action_wallet_review(&self, action: &BsvHomeBsvMultisigActionScope, wallet: &RemoteWallet) -> Result<(), Error> {
        if action.review_hash != review_hash(wallet)? {
            return Err(Error::ConcurrentUpdate);
        }
        Ok(())
    }

    pub(super) fn check_action_proposal_review(&self, action: &BsvHomeBsvMultisigActionScope, wallet: &RemoteWallet, proposal: &RemoteProposal) -> Result<(), Error> {
        if action.review_hash != review_hash(&(wallet, proposal))? {
            return Err(Error::ConcurrentUpdate);
        }
        Ok(())
    }

    pub(super) fn claim_action(&self, action: &BsvHomeBsvMultisigActionScope, intent: BsvHomeBsvMultisigActionIntent) -> Result<(), Error> {
        action.check()?;
        let intent = self.action_intent(intent)?;
        let wallet = self.cache()?.identity_wallet.clone().ok_or(Error::MissingSigner)?;
        action.claim(self.action_owner, &wallet, &intent, &self.action_review_hash(&intent)?)
    }

    pub(super) async fn action_current_wallet(&self, action: &BsvHomeBsvMultisigActionScope) -> Result<Wallet, Error> {
        action.check()?;
        self.check_identity()?;
        let wallet = self.auth.multisig_wallet().await?;
        self.check_action(&wallet, action).await?;
        Ok(wallet)
    }

    pub(super) async fn check_action(&self, wallet: &Wallet, action: &BsvHomeBsvMultisigActionScope) -> Result<(), Error> {
        action.check()?;
        self.check(wallet, action.generation).await?;
        action.check()?;
        if action.wallet.id != wallet.id || action.wallet.wallet_type != wallet.wallet_type || action.wallet.account(Chain::BitcoinSv) != wallet.account(Chain::BitcoinSv) {
            return Err(Error::WalletChanged);
        }
        Ok(())
    }

    pub(super) async fn authorize_action(
        &self,
        wallet: &Wallet,
        local: &Wallet,
        request: BsvHomeBsvMultisigAuthorizationRequest,
        action: Arc<BsvHomeBsvMultisigActionScope>,
    ) -> Result<(), Error> {
        self.check_action(local, &action).await?;
        if let Some(authorization) = &self.authorization {
            authorization.authorize(wallet.clone(), request, action.clone()).await.map_err(authentication)?;
        }
        self.check_action(local, &action).await
    }

    pub(super) async fn action_request<R: DeserializeOwned>(&self, wallet: &Wallet, target: Target, action: &BsvHomeBsvMultisigActionScope) -> Result<R, Error> {
        let check = || action.check().map_err(action_portal_error);
        let before_request = || {
            action.check().map_err(action_portal_error)?;
            action.before_submission(None, None).map_err(action_portal_error)?;
            Ok(())
        };
        Ok(self
            .auth
            .multisig_checkpoint_request_at_generation(wallet, target, action.generation, &check, &before_request)
            .await?)
    }

    pub(super) async fn action_wallet(&self, local: &Wallet, id: &str, action: &BsvHomeBsvMultisigActionScope) -> Result<RemoteWallet, Error> {
        let wallet: RemoteWallet = self.action_request(local, Target::Wallet { wallet_id: id.into() }, action).await?;
        rules::validate_wallet(&wallet)?;
        let address = identity_address(local)?;
        if wallet.id != id || wallet.members.iter().all(|member| member.address != address) {
            return Err(Error::MissingSigner);
        }
        self.check_action(local, action).await?;
        Ok(wallet)
    }
}

pub(super) fn review_hash(input: &impl Serialize) -> Result<[u8; 32], Error> {
    Ok(sha256(&serde_json::to_vec(input)?))
}

pub(super) fn action_portal_error(error: Error) -> BsvHomePortalAuthError {
    match error {
        Error::Cancelled => BsvHomePortalAuthError::Service {
            error: BsvHomeServiceError::Cancelled,
        },
        Error::InvalidPackage { .. }
        | Error::InvalidRules
        | Error::ConfigurationConflict
        | Error::ConcurrentUpdate
        | Error::MissingSigner
        | Error::NotActivated
        | Error::MissingRecord
        | Error::Provider { .. }
        | Error::Storage { .. }
        | Error::Authentication { .. }
        | Error::Api { .. }
        | Error::ConfigurationNotConfirmed
        | Error::WalletChanged
        | Error::WatchOnly
        | Error::FundingExpired
        | Error::FundingPending
        | Error::FundingChanged
        | Error::FundingPayerMissing
        | Error::CreationUncertain { .. } => BsvHomePortalAuthError::WalletChanged,
    }
}
