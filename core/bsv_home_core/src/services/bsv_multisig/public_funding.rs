use std::collections::BTreeMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use async_trait::async_trait;
use bsv_home_bsv::multisig::Policy;
use futures::future::{AbortHandle, Abortable};
use futures::lock::Mutex as AsyncMutex;
use number_formatter::BigNumberFormatter;
use primitives::Wallet;
use uuid::Uuid;

use super::action::{BsvHomeBsvMultisigActionIntent as ActionIntent, BsvHomeBsvMultisigActionScope as ActionScope, action_portal_error};
use super::funding::PublicFundingPayerEligibility;
use super::funding_model::{BsvHomeBsvMultisigFundingTarget as Target, PayerFundingBinding, PayerFundingCheckpoint};
use super::funding_operation::{FundingBoundary, FundingOperations, FundingPreparation, FundingRequest, FundingSource, funding_action_hash};
use super::{
    BsvHomeBsvMultisigAuthorization, BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigFundingPreparation, BsvHomeBsvMultisigFundingPreparationTarget,
    BsvHomeBsvMultisigFundingResult, BsvHomeBsvMultisigPaymentRequest, BsvHomeBsvMultisigSigner, BsvHomeBsvMultisigUnsubmittedFunding, rules,
};
use crate::gateway::BsvHomeGateway;
use crate::models::custom_types::BsvHomeBigInt;
use crate::services::bsv_cold_wallet::BsvHomeBsvColdWalletStore;
use crate::services::portal::{BsvHomePaymailError, BsvHomePortalAuthError, BsvHomePortalAuthService};
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::send::{BsvHomeSendPublicFundingPreparation, BsvHomeSendService};
use crate::services::wallet::addresses::BsvHomeBsvAddressPool;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(Clone, uniffi::Enum)]
pub enum BsvHomeBsvMultisigPublicFundingSource {
    PaymentRequest {
        request: BsvHomeBsvMultisigPaymentRequest,
    },
    Paymail {
        service: Arc<BsvHomeSendService>,
        preparation: Arc<BsvHomeSendPublicFundingPreparation>,
    },
}

impl BsvHomeBsvMultisigPublicFundingSource {
    pub(super) fn check(&self) -> Result<(), Error> {
        match self {
            Self::PaymentRequest { request } => request.validate(),
            Self::Paymail { service, preparation } => service.check_public_funding_scope(preparation).map_err(paymail_error),
        }
    }

    pub(super) fn permits_payer(&self, payer: &Wallet) -> Result<bool, Error> {
        self.check()?;
        match self {
            Self::PaymentRequest { .. } => Ok(true),
            Self::Paymail { service, preparation } => service.public_funding_payer_matches(preparation, payer).map_err(paymail_error),
        }
    }

    pub(super) fn check_payer(&self, payer: &Wallet) -> Result<(), Error> {
        if !self.permits_payer(payer)? {
            return Err(Error::FundingChanged);
        }
        Ok(())
    }

    pub(super) async fn check_wallet(&self, check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync)) -> Result<(), Error> {
        self.check()?;
        match self {
            Self::PaymentRequest { .. } => Ok(()),
            Self::Paymail { service, preparation } => service.check_public_funding_source(preparation, check).await.map_err(paymail_error),
        }
    }

    pub(super) async fn confirm(
        &self,
        payer: &Wallet,
        amount: u64,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<BsvHomeBsvMultisigPaymentRequest, Error> {
        self.check_payer(payer)?;
        check()?;
        let request = match self {
            Self::PaymentRequest { request } => BsvHomeBsvMultisigPaymentRequest {
                amount: BigNumberFormatter::big_decimal_value(&amount.to_string(), 8)
                    .map_err(|_| Error::FundingChanged)?
                    .normalized()
                    .to_plain_string(),
                ..request.clone()
            },
            Self::Paymail { service, preparation } => service
                .confirm_public_funding_for_payer(preparation.clone(), payer, BsvHomeBigInt::from(amount), check)
                .await
                .map_err(paymail_error)?,
        };
        self.check()?;
        check()?;
        request.validate()?;
        if rules::amount_bsv(&request.amount)? != amount {
            return Err(Error::FundingChanged);
        }
        Ok(request)
    }
}

#[derive(Default)]
struct PublicFundingCache {
    preparations: BTreeMap<String, FundingPreparation>,
    results: BTreeMap<String, PayerFundingCheckpoint>,
    actions: Vec<Weak<ActionScope>>,
    reads: BTreeMap<Uuid, AbortHandle>,
}

#[derive(uniffi::Object)]
pub struct BsvHomeBsvMultisigPublicFundingService {
    operations: FundingOperations,
    auth: Arc<BsvHomePortalAuthService>,
    payer: Wallet,
    eligibility: PublicFundingPayerEligibility,
    owner: Uuid,
    version: u64,
    cancelled: AtomicBool,
    mutation: AsyncMutex<()>,
    cache: Mutex<PublicFundingCache>,
}

#[uniffi::export]
impl BsvHomeBsvMultisigPublicFundingService {
    #[uniffi::constructor]
    pub fn new(
        session: Arc<BsvHomeWalletSessionService>,
        gateway: Arc<BsvHomeGateway>,
        addresses: Arc<BsvHomeBsvAddressPool>,
        signer: Arc<dyn BsvHomeBsvMultisigSigner>,
        authorization: Arc<dyn BsvHomeBsvMultisigAuthorization>,
        payer_auth: Arc<BsvHomePortalAuthService>,
        payer: Wallet,
        preferences: Arc<BsvHomePreferencesService>,
        cold_store: Arc<dyn BsvHomeBsvColdWalletStore>,
    ) -> Result<Self, Error> {
        payer_auth.require_fixed_wallet(&payer)?;
        rules::bsv_multisig_signing_wallets(vec![payer.clone()])?.first().ok_or(Error::FundingPayerMissing)?;
        Ok(Self {
            operations: FundingOperations::new(session, gateway, addresses, signer, Some(authorization)),
            version: payer_auth.multisig_version(),
            auth: payer_auth,
            payer,
            eligibility: PublicFundingPayerEligibility::new(preferences, cold_store),
            owner: Uuid::new_v4(),
            cancelled: AtomicBool::new(false),
            mutation: AsyncMutex::new(()),
            cache: Mutex::new(PublicFundingCache::default()),
        })
    }

    pub async fn prepare_funding(&self, source: BsvHomeBsvMultisigPublicFundingSource, amount_bsv: String) -> Result<BsvHomeBsvMultisigFundingPreparation, Error> {
        self.read_operation(async {
            let _mutation = self.mutation.lock().await;
            self.check_scope()?;
            let payer = self.eligible_payer().await?;
            let amount = rules::amount_bsv(&amount_bsv)?;
            let check = || self.check_hot_scope().map_err(action_portal_error);
            let request = source.confirm(&payer, amount, &check).await?;
            self.eligible_payer().await?;
            let target = Target::Public { request: request.clone() };
            let boundary = PublicFundingBoundary {
                service: self,
                source: Some(&source),
            };
            let preparation = self
                .operations
                .prepare(
                    FundingRequest {
                        payer,
                        scope: self.scope(),
                        target,
                        review_target: BsvHomeBsvMultisigFundingPreparationTarget::Public { request: request.clone() },
                        policy: Policy {
                            threshold: request.threshold,
                            public_keys: request.public_keys,
                        },
                        amount,
                        auth_version: self.version,
                        source: FundingSource::Public(source.clone()),
                    },
                    &boundary,
                )
                .await?;
            self.check_scope()?;
            let view = preparation.view()?;
            let mut cache = self.cache()?;
            self.check_scope()?;
            cache.preparations.clear();
            cache.preparations.insert(view.id.clone(), preparation);
            Ok(view)
        })
        .await
    }

    pub fn begin_action(&self, intent: ActionIntent) -> Result<Arc<ActionScope>, Error> {
        self.check_hot_scope()?;
        let (hash, source) = match &intent {
            ActionIntent::SubmitFunding { preparation_id } => {
                let cache = self.cache()?;
                let preparation = cache.preparations.get(preparation_id).ok_or(Error::FundingExpired)?;
                preparation.check_lifetime()?;
                (preparation.action_hash()?, preparation.source.public_source())
            }
            ActionIntent::RetryPublicFunding { operation_id } => {
                let cache = self.cache()?;
                let checkpoint = cache.results.get(operation_id).ok_or(Error::MissingRecord)?;
                (funding_action_hash(checkpoint)?, None)
            }
            ActionIntent::PrepareProposal { .. }
            | ActionIntent::SignProposal { .. }
            | ActionIntent::BroadcastProposal { .. }
            | ActionIntent::RetryFunding { .. }
            | ActionIntent::WithdrawProposal { .. }
            | ActionIntent::UpdatePaymailSettings { .. } => return Err(Error::InvalidRules),
        };
        let action = ActionScope::new_with_source(
            self.owner,
            self.auth.clone(),
            self.version,
            self.payer.clone(),
            intent,
            hash,
            source,
            Some(self.eligibility.clone()),
            self.eligibility.cold_store.clone(),
            None,
        );
        let mut cache = self.cache()?;
        self.check_hot_scope()?;
        cache.actions.retain(|action| action.strong_count() > 0);
        cache.actions.push(Arc::downgrade(&action));
        Ok(action)
    }

    pub async fn submit_funding(&self, preparation_id: String, action: Arc<ActionScope>) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        self.check_hot_scope()?;
        let preparation = {
            let mut cache = self.cache()?;
            let preparation = cache.preparations.get(&preparation_id).ok_or(Error::FundingExpired)?;
            action.claim(
                self.owner,
                &self.payer,
                &ActionIntent::SubmitFunding {
                    preparation_id: preparation_id.clone(),
                },
                &preparation.action_hash()?,
            )?;
            cache.preparations.remove(&preparation_id).ok_or(Error::FundingExpired)?
        };
        let source = preparation.source.public_source().ok_or(Error::FundingChanged)?;
        let boundary = PublicFundingBoundary {
            service: self,
            source: Some(&source),
        };
        self.operations.submit(preparation, action, &self.mutation, &boundary).await
    }

    pub async fn funding_results(&self) -> Result<Vec<BsvHomeBsvMultisigFundingResult>, Error> {
        self.read_operation(async {
            let payer = self.bound_payer().await?;
            let checkpoints = self.operations.checkpoints(&payer).await?;
            self.bound_payer().await?;
            let checkpoints = checkpoints.into_iter().filter(|checkpoint| self.owns_checkpoint(checkpoint)).collect::<Vec<_>>();
            let results = checkpoints
                .iter()
                .map(|checkpoint| self.operations.result(checkpoint, &payer))
                .collect::<Result<Vec<_>, _>>()?;
            let mut cache = self.cache()?;
            self.check_scope()?;
            cache.results = checkpoints.into_iter().map(|checkpoint| (checkpoint.binding.operation_id.clone(), checkpoint)).collect();
            Ok(results)
        })
        .await
    }

    pub async fn funding_result(&self, operation_id: String) -> Result<Option<BsvHomeBsvMultisigFundingResult>, Error> {
        Ok(self.funding_results().await?.into_iter().find(|result| result.operation_id == operation_id))
    }

    pub async fn retry_funding(&self, operation_id: String, action: Arc<ActionScope>) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        self.check_hot_scope()?;
        let checkpoint = self.cache()?.results.get(&operation_id).cloned().ok_or(Error::MissingRecord)?;
        action.claim(
            self.owner,
            &self.payer,
            &ActionIntent::RetryPublicFunding { operation_id },
            &funding_action_hash(&checkpoint)?,
        )?;
        let boundary = PublicFundingBoundary { service: self, source: None };
        self.operations.retry(checkpoint, action, &self.mutation, &boundary).await
    }

    pub async fn retry_funding_sync(&self, operation_id: String) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        self.read_operation(async {
            let payer = self.bound_payer().await?;
            let checkpoint = self
                .operations
                .checkpoints(&payer)
                .await?
                .into_iter()
                .find(|checkpoint| checkpoint.binding.operation_id == operation_id)
                .ok_or(Error::MissingRecord)?;
            if !self.owns_checkpoint(&checkpoint) {
                return Err(Error::FundingChanged);
            }
            let boundary = PublicFundingBoundary { service: self, source: None };
            self.operations.sync(checkpoint, None, &boundary).await
        })
        .await
    }

    pub async fn unsubmitted_funding(&self) -> Result<Vec<BsvHomeBsvMultisigUnsubmittedFunding>, Error> {
        self.read_operation(async {
            let payer = self.bound_payer().await?;
            let bindings = self.operations.unsigned(&payer).await?;
            self.bound_payer().await?;
            bindings
                .into_iter()
                .filter(|binding| self.owns_unsigned(binding))
                .map(|binding| self.operations.unsigned_view(&binding, &payer))
                .collect()
        })
        .await
    }

    pub async fn discard_unsubmitted_funding(&self, operation_id: String) -> Result<(), Error> {
        self.read_operation(async {
            let payer = self.bound_payer().await?;
            let binding = self
                .operations
                .unsigned(&payer)
                .await?
                .into_iter()
                .find(|binding| binding.operation_id == operation_id)
                .ok_or(Error::MissingRecord)?;
            if !self.owns_unsigned(&binding) {
                return Err(Error::FundingChanged);
            }
            self.bound_payer().await?;
            self.operations.discard_unsigned(&payer, &binding).await?;
            self.bound_payer().await?;
            Ok(())
        })
        .await
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut cache) = self.cache.lock() {
            for action in cache.actions.iter().filter_map(Weak::upgrade) {
                action.cancel();
            }
            for abort in cache.reads.values() {
                abort.abort();
            }
            cache.actions.clear();
            cache.preparations.clear();
            cache.results.clear();
        }
    }
}

impl BsvHomeBsvMultisigPublicFundingService {
    fn check_scope(&self) -> Result<(), Error> {
        if self.cancelled.load(Ordering::SeqCst) || self.auth.multisig_version() != self.version {
            return Err(Error::Cancelled);
        }
        self.auth.require_fixed_wallet(&self.payer)?;
        Ok(())
    }

    fn check_hot_scope(&self) -> Result<(), Error> {
        self.check_scope()?;
        self.eligibility.check(&self.payer.id)?;
        Ok(())
    }

    async fn bound_payer(&self) -> Result<Wallet, Error> {
        self.check_scope()?;
        let payer = self.auth.multisig_wallet().await?;
        self.check_scope()?;
        self.operations.check_payer(&self.payer).await?;
        self.check_scope()?;
        Ok(payer)
    }

    async fn eligible_payer(&self) -> Result<Wallet, Error> {
        self.check_hot_scope()?;
        let payer = self.bound_payer().await?;
        self.eligibility.wallets(vec![payer.clone()], None).await?.first().ok_or(Error::FundingPayerMissing)?;
        self.check_hot_scope()?;
        Ok(payer)
    }

    fn scope(&self) -> String {
        format!("{}|{}", self.auth.multisig_origin(), self.payer.id.id())
    }

    fn owns_checkpoint(&self, checkpoint: &PayerFundingCheckpoint) -> bool {
        checkpoint.binding.payer_wallet_id == self.payer.id
            && checkpoint.binding.scope == self.scope()
            && match &checkpoint.binding.target {
                Target::Public { .. } => true,
                Target::Member { .. } => false,
            }
    }

    fn owns_unsigned(&self, binding: &PayerFundingBinding) -> bool {
        binding.payer_wallet_id == self.payer.id && binding.scope.split('|').next() == Some(self.auth.multisig_origin())
    }

    fn cache(&self) -> Result<MutexGuard<'_, PublicFundingCache>, Error> {
        self.cache.lock().map_err(|_| Error::Storage {
            reason: "public funding state lock failed".into(),
        })
    }

    async fn read_operation<T>(&self, future: impl Future<Output = Result<T, Error>>) -> Result<T, Error> {
        self.check_scope()?;
        let id = Uuid::new_v4();
        let (abort, registration) = AbortHandle::new_pair();
        {
            let mut cache = self.cache()?;
            self.check_scope()?;
            cache.reads.insert(id, abort);
        }
        let _lease = PublicFundingReadLease { service: self, id };
        Abortable::new(future, registration).await.map_err(|_| Error::Cancelled)?
    }
}

struct PublicFundingReadLease<'a> {
    service: &'a BsvHomeBsvMultisigPublicFundingService,
    id: Uuid,
}

impl Drop for PublicFundingReadLease<'_> {
    fn drop(&mut self) {
        if let Ok(mut cache) = self.service.cache.lock() {
            cache.reads.remove(&self.id);
        }
    }
}

struct PublicFundingBoundary<'a> {
    service: &'a BsvHomeBsvMultisigPublicFundingService,
    source: Option<&'a BsvHomeBsvMultisigPublicFundingSource>,
}

#[async_trait]
impl FundingBoundary for PublicFundingBoundary<'_> {
    async fn check(&self, action: Option<&ActionScope>) -> Result<(), Error> {
        self.service.check_scope()?;
        if let Some(action) = action {
            action.check()?;
        }
        match (action, self.source) {
            (Some(_), Some(_) | None) | (None, Some(_)) => {
                self.service.eligible_payer().await?;
            }
            (None, None) => {
                self.service.bound_payer().await?;
            }
        }
        if let Some(source) = self.source {
            source.check_payer(&self.service.payer)?;
            let check = || match action {
                Some(action) => action.check().map_err(action_portal_error),
                None => self.service.check_hot_scope().map_err(action_portal_error),
            };
            source.check_wallet(&check).await?;
        }
        if let Some(action) = action {
            action.check()?;
        }
        self.service.check_scope()
    }

    async fn validate_target(&self, binding: &PayerFundingBinding, action: Option<&ActionScope>) -> Result<(), Error> {
        self.check(action).await?;
        if binding.scope != self.service.scope() || binding.payer_wallet_id != self.service.payer.id {
            return Err(Error::FundingChanged);
        }
        let (funding, review) = binding.validate()?;
        let Target::Public { request } = &binding.target else {
            return Err(Error::FundingChanged);
        };
        if let Some(source) = self.source {
            source.check_payer(&self.service.payer)?;
            let check = || match action {
                Some(action) => action.check().map_err(action_portal_error),
                None => self.service.check_hot_scope().map_err(action_portal_error),
            };
            if source.confirm(&self.service.payer, review.amount, &check).await? != *request {
                return Err(Error::FundingChanged);
            }
        }
        binding.target.validate_funding(&funding, &review)?;
        self.check(action).await
    }

    fn sync_failure_code(&self) -> &'static str {
        "FUNDING_BALANCE_SYNC_FAILED"
    }

    fn target_name(&self, binding: &PayerFundingBinding) -> Result<String, Error> {
        match &binding.target {
            Target::Public { request } => Ok(request.name.clone()),
            Target::Member { .. } => Err(Error::FundingChanged),
        }
    }

    async fn project(&self, checkpoint: &PayerFundingCheckpoint) -> Result<(), Error> {
        if !self.service.owns_checkpoint(checkpoint) {
            return Err(Error::FundingChanged);
        }
        self.service.cache()?.results.insert(checkpoint.binding.operation_id.clone(), checkpoint.clone());
        Ok(())
    }

    async fn refresh(&self, checkpoint: &PayerFundingCheckpoint, action: Option<&ActionScope>) -> Result<(), Error> {
        self.check(action).await?;
        self.service.operations.refresh_payer(&self.service.payer).await?;
        if !self.service.owns_checkpoint(checkpoint) {
            return Err(Error::FundingChanged);
        }
        self.check(action).await
    }
}

fn paymail_error(error: BsvHomePaymailError) -> Error {
    match error {
        BsvHomePaymailError::Auth { error } => error.into(),
        BsvHomePaymailError::InvalidAmount => Error::invalid("INVALID_AMOUNT"),
        BsvHomePaymailError::SenderRequired => Error::invalid("PAYMAIL_SENDER_REQUIRED"),
        BsvHomePaymailError::Api { code, .. } if code == "RATE_LIMITED" => Error::Api { code },
        BsvHomePaymailError::Api { .. } => Error::Provider {
            reason: "PUBLIC_PAYMAIL_UNAVAILABLE".into(),
        },
        BsvHomePaymailError::InvalidAlias
        | BsvHomePaymailError::InvalidDomain
        | BsvHomePaymailError::InvalidHandle
        | BsvHomePaymailError::InvalidConfiguration
        | BsvHomePaymailError::InvalidChallenge
        | BsvHomePaymailError::InvalidResponse
        | BsvHomePaymailError::IdentityMismatch
        | BsvHomePaymailError::BindingChanged
        | BsvHomePaymailError::PendingVerification
        | BsvHomePaymailError::UnsupportedCapability
        | BsvHomePaymailError::UnsupportedOutput => Error::FundingChanged,
    }
}

#[cfg(test)]
#[path = "public_funding_tests.rs"]
mod tests;
