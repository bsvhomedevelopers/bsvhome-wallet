use std::sync::Arc;

use async_trait::async_trait;
use primitives::{Wallet, WalletId};

use super::action::{BsvHomeBsvMultisigActionIntent as ActionIntent, BsvHomeBsvMultisigActionScope as ActionScope};
use super::funding_model::{BsvHomeBsvMultisigFundingTarget as Target, FundingCheckpoint, PayerFundingBinding, PayerFundingCheckpoint};
use super::funding_operation::{FundingBoundary, FundingOperations, FundingRequest, FundingSource};
use super::model::{
    BsvHomeBsvMultisigFundingPayer, BsvHomeBsvMultisigFundingPreparation, BsvHomeBsvMultisigFundingPreparationTarget, BsvHomeBsvMultisigFundingResult,
    BsvHomeBsvMultisigFundingStatus as Status,
};
use super::public_funding::BsvHomeBsvMultisigPublicFundingSource;
use super::service::{authentication, identity_address};
use super::{BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigService, rules};
use crate::services::BsvHomeServiceError;
use crate::services::bsv_cold_wallet::BsvHomeBsvColdWalletStore;
use crate::services::preferences::BsvHomePreferencesService;

#[derive(Clone)]
pub(super) struct PublicFundingPayerEligibility {
    preferences: Arc<BsvHomePreferencesService>,
    pub(super) cold_store: Arc<dyn BsvHomeBsvColdWalletStore>,
}

impl PublicFundingPayerEligibility {
    pub(super) fn new(preferences: Arc<BsvHomePreferencesService>, cold_store: Arc<dyn BsvHomeBsvColdWalletStore>) -> Self {
        Self { preferences, cold_store }
    }

    pub(super) fn check(&self, wallet_id: &WalletId) -> Result<(), Error> {
        if !self.is_eligible(wallet_id)? {
            return Err(Error::FundingPayerMissing);
        }
        Ok(())
    }

    fn is_eligible(&self, wallet_id: &WalletId) -> Result<bool, Error> {
        if self.preferences.is_multisig_identity(wallet_id) {
            return Ok(false);
        }
        let paired = self.cold_store.has_state(wallet_id.clone()).map_err(metadata_error)?;
        Ok(!paired && !self.preferences.is_multisig_identity(wallet_id))
    }

    pub(super) async fn wallets(&self, wallets: Vec<Wallet>, source: Option<&BsvHomeBsvMultisigPublicFundingSource>) -> Result<Vec<BsvHomeBsvMultisigFundingPayer>, Error> {
        if let Some(source) = source {
            source.check()?;
        }
        let mut eligible = Vec::new();
        for wallet in rules::bsv_multisig_signing_wallets(wallets)? {
            if let Some(source) = source
                && !source.permits_payer(&wallet)?
            {
                continue;
            }
            if self.preferences.is_multisig_identity(&wallet.id) {
                continue;
            }
            let paired = self.cold_store.get(wallet.id.clone()).await.map_err(metadata_error)?.is_some();
            if !paired && self.is_eligible(&wallet.id)? {
                eligible.push(wallet);
            }
        }
        if let Some(source) = source {
            source.check()?;
        }
        funding_payer_views(eligible)
    }
}

#[uniffi::export]
pub async fn bsv_multisig_public_funding_payers(
    wallets: Vec<Wallet>,
    preferences: Arc<BsvHomePreferencesService>,
    cold_store: Arc<dyn BsvHomeBsvColdWalletStore>,
    source: BsvHomeBsvMultisigPublicFundingSource,
) -> Result<Vec<BsvHomeBsvMultisigFundingPayer>, Error> {
    PublicFundingPayerEligibility::new(preferences, cold_store).wallets(wallets, Some(&source)).await
}

fn metadata_error(_: BsvHomeServiceError) -> Error {
    Error::Storage {
        reason: "BSV_PUBLIC_PAYER_METADATA_UNAVAILABLE".into(),
    }
}

#[uniffi::export]
pub fn bsv_multisig_funding_payers(wallets: Vec<Wallet>) -> Result<Vec<BsvHomeBsvMultisigFundingPayer>, Error> {
    funding_payer_views(rules::bsv_multisig_signing_wallets(wallets)?)
}

fn funding_payer_views(wallets: Vec<Wallet>) -> Result<Vec<BsvHomeBsvMultisigFundingPayer>, Error> {
    wallets
        .into_iter()
        .map(|wallet| {
            Ok(BsvHomeBsvMultisigFundingPayer {
                wallet_id: wallet.id.id(),
                name: wallet.name.clone(),
                address: identity_address(&wallet)?,
            })
        })
        .collect()
}

#[uniffi::export]
impl BsvHomeBsvMultisigService {
    pub async fn funding_payers(&self) -> Result<Vec<BsvHomeBsvMultisigFundingPayer>, Error> {
        self.current_wallet().await?;
        bsv_multisig_funding_payers(self.session.get_wallets().await.map_err(authentication)?)
    }

    pub async fn prepare_funding(&self, wallet_id: String, payer_wallet_id: String, amount_bsv: String) -> Result<BsvHomeBsvMultisigFundingPreparation, Error> {
        let _mutation = self.mutation.lock().await;
        let version = self.auth.multisig_version();
        let local = self.current_wallet().await?;
        let remote = self.fetch_wallet(&local, &wallet_id).await?;
        let policy = self.trusted_policy(&remote, &local)?;
        if self
            .member_funding_checkpoint(&wallet_id)
            .await?
            .is_some_and(|checkpoint| checkpoint.status == Status::Pending)
        {
            return Err(Error::FundingPending);
        }
        let payer = self.payer(&payer_wallet_id).await?;
        let wallet = self.wallet_view(&remote, &self.read()?.1, &*self.cache()?, &identity_address(&local)?)?;
        let boundary = MemberFundingBoundary {
            service: self,
            local: &local,
            wallet_id: &wallet_id,
            version,
        };
        let preparation = self
            .funding_operations()
            .prepare(
                FundingRequest {
                    payer,
                    scope: self.scope(&wallet_id),
                    target: Target::Member { wallet_id: wallet_id.clone() },
                    review_target: BsvHomeBsvMultisigFundingPreparationTarget::Member {
                        wallet_id: wallet_id.clone(),
                        name: wallet.name,
                        members: wallet.members,
                        threshold: wallet.threshold,
                    },
                    policy,
                    amount: rules::amount_bsv(&amount_bsv)?,
                    auth_version: version,
                    source: FundingSource::Member,
                },
                &boundary,
            )
            .await?;
        self.check(&local, version).await?;
        let view = preparation.view()?;
        let mut cache = self.cache()?;
        cache.wallets.insert(wallet_id, remote);
        cache.preparations.clear();
        cache.preparations.insert(view.id.clone(), preparation);
        Ok(view)
    }

    pub async fn submit_funding(&self, preparation_id: String, action: Arc<ActionScope>) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        self.claim_action(
            &action,
            ActionIntent::SubmitFunding {
                preparation_id: preparation_id.clone(),
            },
        )?;
        let preparation = self.cache()?.preparations.remove(&preparation_id).ok_or(Error::FundingExpired)?;
        let wallet_id = match &preparation.binding.target {
            Target::Member { wallet_id } => wallet_id.clone(),
            Target::Public { .. } => return Err(Error::FundingChanged),
        };
        let local = self.cache()?.identity_wallet.clone().ok_or(Error::MissingSigner)?;
        let boundary = MemberFundingBoundary {
            service: self,
            local: &local,
            wallet_id: &wallet_id,
            version: action.generation(),
        };
        self.funding_operations().submit(preparation, action, &self.mutation, &boundary).await
    }

    pub async fn pending_funding(&self, wallet_id: String) -> Result<Option<BsvHomeBsvMultisigFundingResult>, Error> {
        let local = self.current_wallet().await?;
        let Some(checkpoint) = self.member_funding_checkpoint(&wallet_id).await? else {
            return Ok(None);
        };
        if checkpoint.status != Status::Pending {
            return Ok(None);
        }
        let boundary = MemberFundingBoundary {
            service: self,
            local: &local,
            wallet_id: &wallet_id,
            version: self.auth.multisig_version(),
        };
        boundary.validate_target(&checkpoint.binding, None).await?;
        let payer = self.payer(&checkpoint.binding.payer_wallet_id.id()).await?;
        self.funding_operations().result(&checkpoint, &payer).map(Some)
    }

    pub async fn funding_result(&self, wallet_id: String) -> Result<Option<BsvHomeBsvMultisigFundingResult>, Error> {
        self.current_wallet().await?;
        match self.member_funding_checkpoint(&wallet_id).await? {
            Some(checkpoint) => {
                let payer = self.payer(&checkpoint.binding.payer_wallet_id.id()).await?;
                self.funding_operations().result(&checkpoint, &payer).map(Some)
            }
            None => Ok(None),
        }
    }

    pub async fn retry_funding(&self, wallet_id: String, action: Arc<ActionScope>) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        self.claim_action(&action, ActionIntent::RetryFunding { wallet_id: wallet_id.clone() })?;
        let checkpoint = self.funding_snapshot(&wallet_id)?;
        let local = self.cache()?.identity_wallet.clone().ok_or(Error::MissingSigner)?;
        let boundary = MemberFundingBoundary {
            service: self,
            local: &local,
            wallet_id: &wallet_id,
            version: action.generation(),
        };
        self.funding_operations().retry(checkpoint, action, &self.mutation, &boundary).await
    }

    pub async fn retry_funding_sync(&self, wallet_id: String) -> Result<BsvHomeBsvMultisigFundingResult, Error> {
        let _mutation = self.mutation.lock().await;
        let local = self.current_wallet().await?;
        let checkpoint = self.member_funding_checkpoint(&wallet_id).await?.ok_or(Error::MissingRecord)?;
        let boundary = MemberFundingBoundary {
            service: self,
            local: &local,
            wallet_id: &wallet_id,
            version: self.auth.multisig_version(),
        };
        boundary.validate_target(&checkpoint.binding, None).await?;
        self.funding_operations().sync(checkpoint, None, &boundary).await
    }
}

impl BsvHomeBsvMultisigService {
    fn funding_operations(&self) -> FundingOperations {
        FundingOperations::new(
            self.session.clone(),
            self.gateway.clone(),
            self.addresses.clone(),
            self.signer.clone(),
            self.authorization.clone(),
        )
    }

    async fn payer(&self, id: &str) -> Result<Wallet, Error> {
        let wallet = self
            .session
            .get_wallet(WalletId::from_id(id).ok_or(Error::FundingPayerMissing)?)
            .await
            .map_err(authentication)?
            .ok_or(Error::FundingPayerMissing)?;
        rules::bsv_multisig_signing_wallets(vec![wallet.clone()])?.first().ok_or(Error::WatchOnly)?;
        Ok(wallet)
    }

    pub(super) fn funding_snapshot(&self, wallet_id: &str) -> Result<PayerFundingCheckpoint, Error> {
        let cached = self.cache()?.funding.get(wallet_id).cloned();
        let checkpoint = match cached {
            Some(checkpoint) => checkpoint,
            None => self.read()?.1.funding.get(&self.scope(wallet_id)).ok_or(Error::MissingRecord)?.validated_checkpoint()?,
        };
        if checkpoint.binding.scope != self.scope(wallet_id) || checkpoint.binding.target != (Target::Member { wallet_id: wallet_id.into() }) {
            return Err(Error::FundingChanged);
        }
        Ok(checkpoint)
    }

    async fn member_funding_checkpoint(&self, wallet_id: &str) -> Result<Option<PayerFundingCheckpoint>, Error> {
        let local = self.current_wallet().await?;
        let version = self.auth.multisig_version();
        let scope = self.scope(wallet_id);
        let indexed = self.read()?.1.funding.get(&scope).map(FundingCheckpoint::validated_checkpoint).transpose()?;
        if let Some(indexed) = &indexed {
            self.payer(&indexed.binding.payer_wallet_id.id()).await?;
            self.check(&local, version).await?;
        }
        let mut candidates = Vec::new();
        let operations = self.funding_operations();
        for payer in rules::bsv_multisig_signing_wallets(self.session.get_wallets().await.map_err(authentication)?)? {
            let checkpoints = operations.checkpoints(&payer).await?;
            self.check(&local, version).await?;
            for checkpoint in checkpoints {
                if checkpoint.binding.scope == scope && checkpoint.binding.target == (Target::Member { wallet_id: wallet_id.into() }) {
                    candidates.push(checkpoint);
                }
            }
        }
        let pending = candidates.iter().filter(|checkpoint| checkpoint.status == Status::Pending).cloned().collect::<Vec<_>>();
        let selected = match pending.as_slice() {
            [] => match &indexed {
                Some(indexed) => Some(
                    candidates
                        .iter()
                        .find(|checkpoint| checkpoint.same_signed_payment(indexed))
                        .cloned()
                        .ok_or(Error::FundingChanged)?,
                ),
                None => match candidates.as_slice() {
                    [] => None,
                    [checkpoint] => Some(checkpoint.clone()),
                    _ => return Err(Error::FundingPending),
                },
            },
            [checkpoint] => Some(checkpoint.clone()),
            _ => return Err(Error::FundingPending),
        };
        if let Some(checkpoint) = &selected {
            self.check(&local, version).await?;
            if indexed.as_ref() != Some(checkpoint) {
                let (revision, mut state) = self.read()?;
                state.funding.insert(scope, FundingCheckpoint::Current(checkpoint.clone()));
                self.save(revision, state).await?;
                self.check(&local, version).await?;
            }
            self.cache()?.funding.insert(wallet_id.into(), checkpoint.clone());
        } else {
            self.cache()?.funding.remove(wallet_id);
        }
        Ok(selected)
    }

    async fn project_funding_checkpoint(&self, checkpoint: &PayerFundingCheckpoint) -> Result<(), Error> {
        checkpoint.validate()?;
        let Target::Member { wallet_id } = &checkpoint.binding.target else {
            return Err(Error::FundingChanged);
        };
        if checkpoint.binding.scope != self.scope(wallet_id) {
            return Err(Error::FundingChanged);
        }
        let (revision, mut state) = self.read()?;
        if let Some(indexed) = state.funding.get(&checkpoint.binding.scope) {
            let indexed = indexed.validated_checkpoint()?;
            if !checkpoint.same_signed_payment(&indexed) && indexed.status == Status::Pending {
                let payer = self.payer(&indexed.binding.payer_wallet_id.id()).await?;
                let current = self
                    .funding_operations()
                    .checkpoints(&payer)
                    .await?
                    .into_iter()
                    .find(|current| current.same_signed_payment(&indexed))
                    .ok_or(Error::FundingChanged)?;
                if current.status == Status::Pending {
                    return Err(Error::FundingPending);
                }
            }
        }
        state.funding.insert(checkpoint.binding.scope.clone(), FundingCheckpoint::Current(checkpoint.clone()));
        self.save(revision, state).await?;
        self.cache()?.funding.insert(wallet_id.clone(), checkpoint.clone());
        Ok(())
    }
}

struct MemberFundingBoundary<'a> {
    service: &'a BsvHomeBsvMultisigService,
    local: &'a Wallet,
    wallet_id: &'a str,
    version: u64,
}

#[async_trait]
impl FundingBoundary for MemberFundingBoundary<'_> {
    async fn check(&self, action: Option<&ActionScope>) -> Result<(), Error> {
        match action {
            Some(action) => self.service.check_action(self.local, action).await,
            None => self.service.check(self.local, self.version).await,
        }
    }

    async fn validate_target(&self, binding: &PayerFundingBinding, action: Option<&ActionScope>) -> Result<(), Error> {
        self.check(action).await?;
        if binding.scope != self.service.scope(self.wallet_id) || binding.target != (Target::Member { wallet_id: self.wallet_id.into() }) {
            return Err(Error::FundingChanged);
        }
        let remote = match action {
            Some(action) => self.service.action_wallet(self.local, self.wallet_id, action).await?,
            None => self.service.fetch_wallet(self.local, self.wallet_id).await?,
        };
        let (funding, _) = binding.validate()?;
        if let Some(current) = self.service.member_funding_checkpoint(self.wallet_id).await?
            && current.status == Status::Pending
            && current.binding != *binding
        {
            return Err(Error::FundingPending);
        }
        if self.service.trusted_policy(&remote, self.local)? != funding.policy {
            return Err(Error::FundingChanged);
        }
        self.service.cache()?.wallets.insert(remote.id.clone(), remote);
        self.check(action).await
    }

    fn sync_failure_code(&self) -> &'static str {
        "MULTISIG_BALANCE_SYNC_FAILED"
    }

    fn target_name(&self, binding: &PayerFundingBinding) -> Result<String, Error> {
        if binding.target != (Target::Member { wallet_id: self.wallet_id.into() }) {
            return Err(Error::FundingChanged);
        }
        Ok(self.service.remote_wallet(self.wallet_id)?.name)
    }

    async fn project(&self, checkpoint: &PayerFundingCheckpoint) -> Result<(), Error> {
        self.service.project_funding_checkpoint(checkpoint).await
    }

    async fn refresh(&self, checkpoint: &PayerFundingCheckpoint, action: Option<&ActionScope>) -> Result<(), Error> {
        self.validate_target(&checkpoint.binding, action).await?;
        let remote = self.service.remote_wallet(self.wallet_id)?;
        self.service.fetch_balance(self.local, &remote, self.version).await?;
        let payer = self.service.payer(&checkpoint.binding.payer_wallet_id.id()).await?;
        self.service.funding_operations().refresh_payer(&payer).await?;
        self.check(action).await
    }
}
