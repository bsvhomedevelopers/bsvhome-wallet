use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bsv_home_bsv::multisig::{Funding, FundingReview, Policy, funding_transaction_id, review_funding};
use futures::lock::Mutex as AsyncMutex;
use primitives::{Chain, Wallet};
use uuid::Uuid;

use super::action::{BsvHomeBsvMultisigActionScope as ActionScope, BsvHomeBsvMultisigAuthorizationRequest as AuthorizationRequest, review_hash};
use super::funding_model::{BsvHomeBsvMultisigFundingTarget as Target, PayerFundingBinding, PayerFundingCheckpoint};
use super::model::{
    BsvHomeBsvMultisigFundingPreparation, BsvHomeBsvMultisigFundingPreparationTarget, BsvHomeBsvMultisigFundingResult, BsvHomeBsvMultisigFundingStatus as Status,
    BsvHomeBsvMultisigUnsubmittedFunding,
};
use super::public_funding::BsvHomeBsvMultisigPublicFundingSource;
use super::service::{authentication, identity_address};
use super::{BsvHomeBsvMultisigAuthorization, BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigSigner, rules};
use crate::gateway::{BsvHomeGateway, GatewayError};
use crate::services::wallet::addresses::BsvHomeBsvAddressPool;
use crate::services::wallet_session::BsvHomeWalletSessionService;

const REVIEW_LIFETIME: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub(super) enum FundingSource {
    Member,
    Public(BsvHomeBsvMultisigPublicFundingSource),
}

impl FundingSource {
    pub(super) fn public_source(&self) -> Option<BsvHomeBsvMultisigPublicFundingSource> {
        match self {
            Self::Member => None,
            Self::Public(source) => Some(source.clone()),
        }
    }
}

pub(super) struct FundingRequest {
    pub payer: Wallet,
    pub scope: String,
    pub target: Target,
    pub review_target: BsvHomeBsvMultisigFundingPreparationTarget,
    pub policy: Policy,
    pub amount: u64,
    pub auth_version: u64,
    pub source: FundingSource,
}

pub(super) struct FundingPreparation {
    pub binding: PayerFundingBinding,
    pub payer: Wallet,
    pub funding: Funding,
    pub review: FundingReview,
    pub review_target: BsvHomeBsvMultisigFundingPreparationTarget,
    pub created: Instant,
    pub auth_version: u64,
    pub source: FundingSource,
}

impl FundingPreparation {
    pub(super) fn check_lifetime(&self) -> Result<(), Error> {
        if self.created.elapsed() >= REVIEW_LIFETIME {
            return Err(Error::FundingExpired);
        }
        Ok(())
    }

    pub(super) fn action_hash(&self) -> Result<[u8; 32], Error> {
        self.check_lifetime()?;
        review_hash(&(&self.binding, &self.payer, self.auth_version))
    }

    pub(super) fn view(&self) -> Result<BsvHomeBsvMultisigFundingPreparation, Error> {
        self.check_lifetime()?;
        Ok(BsvHomeBsvMultisigFundingPreparation {
            id: self.binding.operation_id.clone(),
            target: self.review_target.clone(),
            payer_wallet_id: self.payer.id.id(),
            payer_wallet_name: self.payer.name.clone(),
            sender_address: self.review.sender_address.clone(),
            amount_satoshis: self.review.amount.to_string(),
            fee_satoshis: self.review.fee.to_string(),
            total_satoshis: total(&self.review)?,
            change_satoshis: self.review.change.to_string(),
            input_count: self.review.input_count,
            configuration_hash: self.review.configuration_hash.clone(),
            remaining_seconds: REVIEW_LIFETIME.saturating_sub(self.created.elapsed()).as_secs() as u32,
        })
    }
}

#[async_trait]
pub(super) trait FundingBoundary: Sync {
    async fn check(&self, action: Option<&ActionScope>) -> Result<(), Error>;
    async fn validate_target(&self, binding: &PayerFundingBinding, action: Option<&ActionScope>) -> Result<(), Error>;
    fn sync_failure_code(&self) -> &'static str;
    fn target_name(&self, binding: &PayerFundingBinding) -> Result<String, Error>;
    async fn project(&self, checkpoint: &PayerFundingCheckpoint) -> Result<(), Error>;
    async fn refresh(&self, checkpoint: &PayerFundingCheckpoint, action: Option<&ActionScope>) -> Result<(), Error>;
}

pub(super) struct FundingOperations {
    session: Arc<BsvHomeWalletSessionService>,
    gateway: Arc<BsvHomeGateway>,
    addresses: Arc<BsvHomeBsvAddressPool>,
    signer: Arc<dyn BsvHomeBsvMultisigSigner>,
    authorization: Option<Arc<dyn BsvHomeBsvMultisigAuthorization>>,
}

impl FundingOperations {
    pub(super) fn new(
        session: Arc<BsvHomeWalletSessionService>,
        gateway: Arc<BsvHomeGateway>,
        addresses: Arc<BsvHomeBsvAddressPool>,
        signer: Arc<dyn BsvHomeBsvMultisigSigner>,
        authorization: Option<Arc<dyn BsvHomeBsvMultisigAuthorization>>,
    ) -> Self {
        Self {
            session,
            gateway,
            addresses,
            signer,
            authorization,
        }
    }

    pub(super) async fn check_payer(&self, expected: &Wallet) -> Result<(), Error> {
        let current = self
            .session
            .get_wallet(expected.id.clone())
            .await
            .map_err(authentication)?
            .ok_or(Error::FundingPayerMissing)?;
        rules::bsv_multisig_signing_wallets(vec![current.clone()])?.first().ok_or(Error::WatchOnly)?;
        if current.id != expected.id || current.wallet_type != expected.wallet_type || current.account(Chain::BitcoinSv) != expected.account(Chain::BitcoinSv) {
            return Err(Error::FundingChanged);
        }
        Ok(())
    }

    pub(super) async fn prepare(&self, request: FundingRequest, boundary: &dyn FundingBoundary) -> Result<FundingPreparation, Error> {
        boundary.check(None).await?;
        self.check_payer(&request.payer).await?;
        let checkpoints = self.checkpoints(&request.payer).await?;
        if checkpoints
            .iter()
            .any(|checkpoint| checkpoint.binding.scope == request.scope && checkpoint.status == Status::Pending)
        {
            return Err(Error::FundingPending);
        }
        let sender = identity_address(&request.payer)?;
        let spend = self.addresses.preload_payer_spend(request.payer.id.clone(), &sender, false).await.map_err(authentication)?;
        boundary.check(None).await?;
        self.check_payer(&request.payer).await?;
        let funding_json = self
            .gateway
            .bsv_multisig_funding_prepare_with_spend(sender, request.policy, request.amount, spend)
            .await
            .map_err(gateway_error)?;
        boundary.check(None).await?;
        self.check_payer(&request.payer).await?;
        let funding: Funding = serde_json::from_str(&funding_json)?;
        let review = review_funding(&funding)?;
        let binding = PayerFundingBinding {
            scope: request.scope,
            operation_id: Uuid::new_v4().to_string(),
            payer_wallet_id: request.payer.id.clone(),
            target: request.target,
            funding_json,
            review_json: serde_json::to_string(&review)?,
        };
        binding.validate()?;
        if review.amount != request.amount || review.sender_address != identity_address(&request.payer)? {
            return Err(Error::FundingChanged);
        }
        boundary.validate_target(&binding, None).await?;
        Ok(FundingPreparation {
            binding,
            payer: request.payer,
            funding,
            review,
            review_target: request.review_target,
            created: Instant::now(),
            auth_version: request.auth_version,
            source: request.source,
        })
    }

    pub(super) async fn checkpoints(&self, payer: &Wallet) -> Result<Vec<PayerFundingCheckpoint>, Error> {
        self.check_payer(payer).await?;
        let checkpoints = self.addresses.funding_checkpoints(payer).await.map_err(authentication)?;
        self.check_payer(payer).await?;
        Ok(checkpoints)
    }

    pub(super) async fn unsigned(&self, payer: &Wallet) -> Result<Vec<PayerFundingBinding>, Error> {
        self.check_payer(payer).await?;
        let bindings = self.addresses.unsubmitted_funding(payer).await.map_err(authentication)?;
        self.check_payer(payer).await?;
        Ok(bindings)
    }

    pub(super) async fn discard_unsigned(&self, payer: &Wallet, binding: &PayerFundingBinding) -> Result<(), Error> {
        self.check_payer(payer).await?;
        binding.validate()?;
        if binding.payer_wallet_id != payer.id {
            return Err(Error::FundingChanged);
        }
        self.addresses.release_funding_unsigned(payer, binding).await.map_err(authentication)?;
        self.check_payer(payer).await
    }

    pub(super) fn unsigned_view(&self, binding: &PayerFundingBinding, payer: &Wallet) -> Result<BsvHomeBsvMultisigUnsubmittedFunding, Error> {
        let (_, review) = binding.validate()?;
        self.validate_binding_payer(binding, payer)?;
        Ok(BsvHomeBsvMultisigUnsubmittedFunding {
            operation_id: binding.operation_id.clone(),
            target: binding.target.clone(),
            payer_wallet_id: payer.id.id(),
            payer_wallet_name: payer.name.clone(),
            sender_address: review.sender_address.clone(),
            amount_satoshis: review.amount.to_string(),
            fee_satoshis: review.fee.to_string(),
            total_satoshis: total(&review)?,
        })
    }

    pub(super) fn result(&self, checkpoint: &PayerFundingCheckpoint, payer: &Wallet) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        let (_, review) = checkpoint.validate()?;
        self.validate_binding_payer(&checkpoint.binding, payer)?;
        Ok(BsvHomeBsvMultisigFundingResult {
            target: checkpoint.binding.target.clone(),
            operation_id: checkpoint.binding.operation_id.clone(),
            payer_wallet_id: payer.id.id(),
            payer_wallet_name: payer.name.clone(),
            transaction_id: checkpoint.transaction_id.clone(),
            amount_satoshis: review.amount.to_string(),
            fee_satoshis: review.fee.to_string(),
            total_satoshis: total(&review)?,
            status: checkpoint.status.clone(),
            error_code: checkpoint.error_code.clone(),
        })
    }

    pub(super) async fn submit(
        &self,
        preparation: FundingPreparation,
        action: Arc<ActionScope>,
        mutation: &AsyncMutex<()>,
        boundary: &dyn FundingBoundary,
    ) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        let payer = preparation.payer.clone();
        let binding = preparation.binding.clone();
        let result = action
            .run(async {
                action.check()?;
                let _mutation = mutation.lock().await;
                preparation.check_lifetime()?;
                boundary.check(Some(&action)).await?;
                self.check_payer(&payer).await?;
                boundary.validate_target(&binding, Some(&action)).await?;
                let lease = self.addresses.reserve_funding(&payer, binding.clone()).await.map_err(authentication)?;
                action.check()?;
                self.authorize(&payer, &binding, None, action.clone(), boundary).await?;
                preparation.check_lifetime()?;
                boundary.validate_target(&binding, Some(&action)).await?;
                self.check_payer(&payer).await?;
                action.check()?;
                let raw = self
                    .signer
                    .sign_funding(payer.clone(), binding.funding_json.clone(), binding.review_json.clone(), action.clone())
                    .await
                    .map_err(authentication)?;
                let transaction_id = funding_transaction_id(&preparation.funding, &preparation.review, &raw)?;
                let checkpoint = PayerFundingCheckpoint {
                    binding: binding.clone(),
                    raw,
                    transaction_id,
                    status: Status::Pending,
                    error_code: None,
                };
                self.addresses.save_funding_signed(&payer, &lease.id, checkpoint.clone()).await.map_err(authentication)?;
                drop(lease);
                boundary.project(&checkpoint).await?;
                boundary.check(Some(&action)).await?;
                self.broadcast(checkpoint, &payer, action.clone(), boundary).await
            })
            .await;
        if result.is_err() {
            self.addresses.release_funding_unsigned(&payer, &binding).await.map_err(|_| Error::Storage {
                reason: "FUNDING_RESERVATION_RELEASE_FAILED".into(),
            })?;
        }
        result
    }

    pub(super) async fn retry(
        &self,
        checkpoint: PayerFundingCheckpoint,
        action: Arc<ActionScope>,
        mutation: &AsyncMutex<()>,
        boundary: &dyn FundingBoundary,
    ) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        action
            .run(async {
                action.check()?;
                let _mutation = mutation.lock().await;
                boundary.check(Some(&action)).await?;
                let payer = self.payer(&checkpoint.binding).await?;
                self.addresses.require_funding_checkpoint(&payer, &checkpoint).await.map_err(authentication)?;
                boundary.validate_target(&checkpoint.binding, Some(&action)).await?;
                let latest = self
                    .addresses
                    .funding_checkpoints(&payer)
                    .await
                    .map_err(authentication)?
                    .into_iter()
                    .find(|latest| latest.binding.operation_id == checkpoint.binding.operation_id)
                    .ok_or(Error::MissingRecord)?;
                if !latest.same_signed_payment(&checkpoint) {
                    return Err(Error::FundingChanged);
                }
                match latest.status {
                    Status::Broadcast | Status::SyncFailed => {
                        action.observed_submission(latest.transaction_id.clone())?;
                        let result = self.sync(latest, Some(&action), boundary).await;
                        action.finish()?;
                        result
                    }
                    Status::Pending => {
                        self.authorize(&payer, &latest.binding, Some(latest.transaction_id.clone()), action.clone(), boundary)
                            .await?;
                        boundary.validate_target(&latest.binding, Some(&action)).await?;
                        self.check_payer(&payer).await?;
                        self.signer.authenticate(payer.clone(), Some(action.clone())).await.map_err(authentication)?;
                        boundary.check(Some(&action)).await?;
                        self.broadcast(latest, &payer, action.clone(), boundary).await
                    }
                }
            })
            .await
    }

    async fn broadcast(
        &self,
        checkpoint: PayerFundingCheckpoint,
        payer: &Wallet,
        action: Arc<ActionScope>,
        boundary: &dyn FundingBoundary,
    ) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        boundary.validate_target(&checkpoint.binding, Some(&action)).await?;
        self.check_payer(payer).await?;
        self.addresses.require_funding_checkpoint(payer, &checkpoint).await.map_err(authentication)?;
        boundary.check(Some(&action)).await?;
        let check = || action.check().map_err(provider);
        let boundary_error = Mutex::new(None);
        let before_send = || async {
            let checked = async {
                self.check_payer(payer).await?;
                boundary.check(Some(&action)).await
            }
            .await;
            if let Err(error) = &checked {
                *boundary_error.lock().map_err(|_| provider("BSV funding boundary lock failed"))? = Some(error.clone());
            }
            checked.map_err(provider)
        };
        let before_submission = || {
            action.check().map_err(provider)?;
            action.before_submission(None, Some(checkpoint.transaction_id.clone())).map_err(provider)
        };
        let broadcast = self
            .gateway
            .bsv_multisig_funding_broadcast_checked(
                checkpoint.binding.funding_json.clone(),
                checkpoint.binding.review_json.clone(),
                checkpoint.raw.clone(),
                &check,
                before_send,
                &before_submission,
            )
            .await;
        if let Some(error) = boundary_error
            .lock()
            .map_err(|_| Error::Storage {
                reason: "BSV funding boundary lock failed".into(),
            })?
            .take()
        {
            return Err(error);
        }
        match broadcast {
            Ok(transaction_id) => {
                if transaction_id != checkpoint.transaction_id {
                    return Err(Error::FundingChanged);
                }
                action.submitted(None, Some(transaction_id))?;
                let mut accepted = checkpoint;
                accepted.status = Status::Broadcast;
                accepted.error_code = None;
                let accepted = self.addresses.record_funding_outcome(payer, &accepted).await.map_err(authentication)?;
                boundary.project(&accepted).await?;
                let result = self.sync(accepted, Some(&action), boundary).await;
                action.finish()?;
                result
            }
            Err(GatewayError::Offline | GatewayError::NetworkError { .. }) => {
                let mut uncertain = checkpoint;
                uncertain.error_code = Some("BSV_BROADCAST_UNCONFIRMED".into());
                let latest = self.addresses.record_funding_outcome(payer, &uncertain).await.map_err(authentication)?;
                boundary.project(&latest).await?;
                match latest.status {
                    Status::Pending => {
                        action.check()?;
                        action.finish()?;
                        self.result(&latest, payer)
                    }
                    Status::Broadcast | Status::SyncFailed => {
                        action.observed_submission(latest.transaction_id.clone())?;
                        let result = self.sync(latest, Some(&action), boundary).await;
                        action.finish()?;
                        result
                    }
                }
            }
            Err(GatewayError::PlatformError { .. } | GatewayError::NetworkIdMismatch { .. }) => {
                action.check()?;
                Err(Error::FundingChanged)
            }
        }
    }

    pub(super) async fn sync(
        &self,
        mut checkpoint: PayerFundingCheckpoint,
        action: Option<&ActionScope>,
        boundary: &dyn FundingBoundary,
    ) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        if checkpoint.status == Status::Pending {
            return Err(Error::FundingPending);
        }
        boundary.check(action).await?;
        let payer = self.payer(&checkpoint.binding).await?;
        self.addresses.require_funding_checkpoint(&payer, &checkpoint).await.map_err(authentication)?;
        match boundary.refresh(&checkpoint, action).await {
            Ok(()) => {
                checkpoint.status = Status::Broadcast;
                checkpoint.error_code = None;
            }
            Err(error) => {
                checkpoint.status = Status::SyncFailed;
                checkpoint.error_code = Some(boundary.sync_failure_code().into());
                if let Some(action) = action {
                    action.sync_failed()?;
                }
                let saved = self.addresses.record_funding_outcome(&payer, &checkpoint).await.map_err(authentication)?;
                boundary.project(&saved).await?;
                if error == Error::Cancelled {
                    return Err(error);
                }
                return self.result(&saved, &payer);
            }
        }
        boundary.check(action).await?;
        checkpoint = self.addresses.record_funding_outcome(&payer, &checkpoint).await.map_err(authentication)?;
        boundary.project(&checkpoint).await?;
        self.result(&checkpoint, &payer)
    }

    pub(super) async fn refresh_payer(&self, payer: &Wallet) -> Result<(), Error> {
        self.check_payer(payer).await?;
        self.addresses.balance(payer.id.clone(), false).await.map_err(authentication)?;
        self.check_payer(payer).await
    }

    async fn payer(&self, binding: &PayerFundingBinding) -> Result<Wallet, Error> {
        let payer = self
            .session
            .get_wallet(binding.payer_wallet_id.clone())
            .await
            .map_err(authentication)?
            .ok_or(Error::FundingPayerMissing)?;
        rules::bsv_multisig_signing_wallets(vec![payer.clone()])?.first().ok_or(Error::WatchOnly)?;
        self.validate_binding_payer(binding, &payer)?;
        Ok(payer)
    }

    fn validate_binding_payer(&self, binding: &PayerFundingBinding, payer: &Wallet) -> Result<(), Error> {
        let (_, review) = binding.validate()?;
        if binding.payer_wallet_id != payer.id || review.sender_address != identity_address(payer)? {
            return Err(Error::FundingChanged);
        }
        Ok(())
    }

    async fn authorize(
        &self,
        payer: &Wallet,
        binding: &PayerFundingBinding,
        transaction_id: Option<String>,
        action: Arc<ActionScope>,
        boundary: &dyn FundingBoundary,
    ) -> Result<(), Error> {
        boundary.check(Some(&action)).await?;
        self.check_payer(payer).await?;
        let (_, review) = binding.validate()?;
        let target_name = boundary.target_name(binding)?;
        if let Some(authorization) = &self.authorization {
            authorization
                .authorize(
                    payer.clone(),
                    AuthorizationRequest::Funding {
                        target_name,
                        commitment: review.commitment,
                        configuration_hash: review.configuration_hash,
                        sender_address: review.sender_address,
                        amount_satoshis: review.amount.to_string(),
                        fee_satoshis: review.fee.to_string(),
                        transaction_id,
                    },
                    action.clone(),
                )
                .await
                .map_err(authentication)?;
        }
        boundary.check(Some(&action)).await?;
        self.check_payer(payer).await?;
        action.check()
    }
}

pub(super) fn funding_action_hash(checkpoint: &PayerFundingCheckpoint) -> Result<[u8; 32], Error> {
    review_hash(&(&checkpoint.binding, &checkpoint.raw, &checkpoint.transaction_id))
}

fn total(review: &FundingReview) -> Result<String, Error> {
    Ok(review.amount.checked_add(review.fee).ok_or(Error::FundingChanged)?.to_string())
}

fn provider(error: impl std::fmt::Display) -> GatewayError {
    GatewayError::PlatformError { msg: error.to_string() }
}

fn gateway_error(_: GatewayError) -> Error {
    Error::Provider {
        reason: "BSV_FUNDING_PROVIDER_UNAVAILABLE".into(),
    }
}
