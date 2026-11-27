use super::action::{
    BsvHomeBsvMultisigActionIntent as ActionIntent, BsvHomeBsvMultisigActionScope as ActionScope, BsvHomeBsvMultisigAuthorization,
    BsvHomeBsvMultisigAuthorizationRequest as AuthorizationRequest, ReviewedProposal, review_hash,
};
use super::funding_model::PayerFundingCheckpoint;
use super::funding_operation::FundingPreparation;
use super::paymail::BsvHomeBsvMultisigPaymailSettings;
use super::receipts::ActivityCache;
use super::{BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigSigner, BsvHomeBsvMultisigStore, model::*, rules};
use crate::{
    gateway::BsvHomeGateway,
    services::{
        BsvHomeServiceError,
        bsv_cold_wallet::BsvHomeBsvColdWalletStore,
        portal::{BsvHomePortalAuthService, BsvMultisigTarget as Target},
        wallet::addresses::BsvHomeBsvAddressPool,
        wallet_session::BsvHomeWalletSessionService,
    },
};
use bsv_home_bsv::multisig::{Policy, assemble_proposal, member_address, review_proposal};
use chrono::Utc;
use futures::lock::Mutex as AsyncMutex;
use primitives::{Address, AssetId, Chain, Wallet};
use serde::Serialize;
use serde_json::to_value;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Mutex, Weak},
};
use uuid::Uuid;

#[derive(Default)]
pub(super) struct Cache {
    pub wallets: BTreeMap<String, RemoteWallet>,
    pub identity_address: Option<String>,
    pub identity_wallet: Option<Wallet>,
    pub actions: Vec<Weak<ActionScope>>,
    pub reviews: VecDeque<ReviewedProposal>,
    pub proposals: BTreeMap<String, RemoteProposal>,
    pub balances: BTreeMap<String, (u64, u64, bool)>,
    pub previews: BTreeMap<String, (RemoteWallet, u64, u64)>,
    pub preparations: BTreeMap<String, FundingPreparation>,
    pub funding: BTreeMap<String, PayerFundingCheckpoint>,
    pub ready_creation: Option<ReadyCreation>,
    pub activity: BTreeMap<String, ActivityCache>,
    pub paymail_settings: BTreeMap<String, BsvHomeBsvMultisigPaymailSettings>,
}

#[derive(uniffi::Object)]
pub struct BsvHomeBsvMultisigService {
    pub(super) store: Arc<dyn BsvHomeBsvMultisigStore>,
    pub(super) cold_store: Arc<dyn BsvHomeBsvColdWalletStore>,
    pub(super) session: Arc<BsvHomeWalletSessionService>,
    pub(super) gateway: Arc<BsvHomeGateway>,
    pub(super) signer: Arc<dyn BsvHomeBsvMultisigSigner>,
    pub(super) auth: Arc<BsvHomePortalAuthService>,
    pub(super) addresses: Arc<BsvHomeBsvAddressPool>,
    pub(super) wallet_id: String,
    pub(super) cache: Mutex<Cache>,
    pub(super) mutation: AsyncMutex<()>,
    pub(super) workflow_version: u64,
    pub(super) action_owner: Uuid,
    pub(super) authorization: Option<Arc<dyn BsvHomeBsvMultisigAuthorization>>,
}

#[uniffi::export]
impl BsvHomeBsvMultisigService {
    #[uniffi::constructor]
    pub fn new(
        store: Arc<dyn BsvHomeBsvMultisigStore>,
        session: Arc<BsvHomeWalletSessionService>,
        gateway: Arc<BsvHomeGateway>,
        signer: Arc<dyn BsvHomeBsvMultisigSigner>,
        auth: Arc<BsvHomePortalAuthService>,
        addresses: Arc<BsvHomeBsvAddressPool>,
        wallet_id: String,
        cold_store: Arc<dyn BsvHomeBsvColdWalletStore>,
    ) -> Self {
        let workflow_version = auth.multisig_version();
        Self {
            store,
            cold_store,
            session,
            gateway,
            signer,
            auth,
            addresses,
            wallet_id,
            cache: Mutex::new(Cache::default()),
            mutation: AsyncMutex::new(()),
            workflow_version,
            action_owner: Uuid::new_v4(),
            authorization: None,
        }
    }

    #[uniffi::constructor]
    pub fn new_with_authorization(
        store: Arc<dyn BsvHomeBsvMultisigStore>,
        session: Arc<BsvHomeWalletSessionService>,
        gateway: Arc<BsvHomeGateway>,
        signer: Arc<dyn BsvHomeBsvMultisigSigner>,
        auth: Arc<BsvHomePortalAuthService>,
        addresses: Arc<BsvHomeBsvAddressPool>,
        wallet_id: String,
        authorization: Arc<dyn BsvHomeBsvMultisigAuthorization>,
        cold_store: Arc<dyn BsvHomeBsvColdWalletStore>,
    ) -> Self {
        Self {
            authorization: Some(authorization),
            ..Self::new(store, session, gateway, signer, auth, addresses, wallet_id, cold_store)
        }
    }

    pub fn cancel(&self) {
        if let Ok(mut cache) = self.cache() {
            for action in cache.actions.iter().filter_map(Weak::upgrade) {
                action.cancel();
            }
            cache.actions.clear();
            cache.reviews.clear();
        }
        self.auth.cancel();
    }

    pub fn supports_asset(&self, asset_id: String) -> bool {
        AssetId::new(&asset_id).is_some_and(|a| a.chain == Chain::BitcoinSv && a == Chain::BitcoinSv.as_asset_id() && a.chain.config().supports_multisig())
    }

    pub fn qr_size_limit(&self) -> u64 {
        2000
    }

    pub async fn load(&self) -> Result<(), Error> {
        let Some(_mutation) = self.mutation.try_lock() else {
            return Ok(());
        };
        let version = self.auth.multisig_version();
        let wallet = self.current_wallet().await?;
        let address = identity_address(&wallet)?;
        let remote: Vec<RemoteWallet> = self.request(&wallet, Target::Wallets).await?;
        if remote.len() > 100 {
            return Err(Error::invalid("WALLET_LIST_LIMIT"));
        }
        let mut wallets = BTreeMap::new();
        let mut proposals = BTreeMap::new();
        let mut balance_error = None;
        for item in remote {
            rules::validate_wallet(&item)?;
            if item.members.iter().all(|m| m.address != address) || wallets.contains_key(&item.id) {
                return Err(Error::MissingSigner);
            }
            for proposal in self.fetch_proposals(&wallet, &item).await? {
                if proposals.insert(proposal.id.clone(), proposal).is_some() {
                    return Err(Error::invalid("INVALID_RESPONSE"));
                }
            }
            if item.status == "ACTIVE"
                && let Err(error) = self.fetch_balance(&wallet, &item, version).await
                && balance_error.is_none()
            {
                balance_error = Some(error);
            }
            wallets.insert(item.id.clone(), item);
        }
        self.check(&wallet, version).await?;
        {
            let mut cache = self.cache()?;
            cache.activity.retain(|id, _| wallets.contains_key(id));
            cache.paymail_settings.retain(|id, _| wallets.contains_key(id));
            cache.wallets = wallets;
            cache.proposals = proposals;
        }
        match balance_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub fn get_wallets(&self) -> Result<Vec<BsvHomeBsvMultisigWallet>, Error> {
        self.check_identity()?;
        let (_, state) = self.read()?;
        let address = self.cached_address()?;
        let cache = self.cache()?;
        cache.wallets.values().map(|w| self.wallet_view(w, &state, &cache, &address)).collect()
    }

    pub fn get_wallet(&self, wallet_id: String) -> Result<BsvHomeBsvMultisigWallet, Error> {
        self.get_wallets()?.into_iter().find(|w| w.id == wallet_id).ok_or(Error::MissingRecord)
    }

    pub fn get_proposals(&self, wallet_id: Option<String>) -> Result<Vec<BsvHomeBsvMultisigProposal>, Error> {
        self.check_identity()?;
        let (_, state) = self.read()?;
        let address = self.cached_address()?;
        let cache = self.cache()?;
        cache
            .proposals
            .values()
            .filter(|p| wallet_id.as_ref().is_none_or(|id| p.wallet_id == *id))
            .map(|p| self.proposal_view(p, cache.wallets.get(&p.wallet_id).ok_or(Error::MissingRecord)?, &state, &address))
            .collect()
    }

    pub fn get_proposal(&self, proposal_id: String) -> Result<BsvHomeBsvMultisigProposal, Error> {
        self.get_proposals(None)?.into_iter().find(|p| p.id == proposal_id).ok_or(Error::MissingRecord)
    }

    pub fn pending_count(&self) -> Result<u32, Error> {
        Ok(self.get_wallets()?.iter().map(|w| w.pending_count).sum())
    }

    pub fn related_multisig_count(&self, signer_wallet_id: String) -> Result<u32, Error> {
        if signer_wallet_id != self.wallet_id {
            return Ok(0);
        }
        Ok(self.read()?.1.trusted.len() as u32)
    }

    pub async fn current_signing_identity(&self) -> Result<BsvHomeBsvMultisigSignerBinding, Error> {
        let version = self.auth.multisig_version();
        let wallet = self.current_wallet().await?;
        let public_key = self.signer.public_key(wallet.clone()).await.map_err(authentication)?;
        self.check(&wallet, version).await?;
        let address = identity_address(&wallet)?;
        if member_address(&public_key)? != address {
            return Err(Error::MissingSigner);
        }
        Ok(BsvHomeBsvMultisigSignerBinding {
            wallet_id: wallet.id.id(),
            public_key,
            address,
        })
    }

    pub async fn invitation(&self, wallet_id: String) -> Result<BsvHomeBsvMultisigInvitation, Error> {
        let _mutation = self.mutation.lock().await;
        let version = self.auth.multisig_version();
        let local = self.current_wallet().await?;
        let remote = self.fetch_wallet(&local, &wallet_id).await?;
        let address = identity_address(&local)?;
        if remote.status != "DRAFT" || remote.members.len() >= remote.member_count as usize || remote.members.first().is_none_or(|m| m.address != address) {
            return Err(Error::invalid("CREATOR_REQUIRED"));
        }
        self.signer.authenticate(local.clone(), None).await.map_err(authentication)?;
        self.check(&local, version).await?;
        let invite: RemoteInvite = self
            .request(
                &local,
                Target::Invites {
                    wallet_id: wallet_id.clone(),
                    body: to_value(Version {
                        expected_version: rules::nonnegative(remote.version)?,
                    })?,
                },
            )
            .await?;
        rules::validate_invitation_token(&invite.token)?;
        if invite.wallet_id != wallet_id || rules::nonnegative(invite.expires_at)? <= now() {
            return Err(Error::invalid("INVALID_INVITE"));
        }
        self.check(&local, version).await?;
        Ok(BsvHomeBsvMultisigInvitation {
            wallet_id,
            token: invite.token,
            expires_at: invite.expires_at as u64,
        })
    }

    pub async fn preview_invitation(&self, token: String) -> Result<BsvHomeBsvMultisigInvitePreview, Error> {
        let token = rules::parse_bsv_multisig_invitation(token)?;
        let version = self.auth.multisig_version();
        let local = self.current_wallet().await?;
        let identity = self.current_signing_identity().await?;
        #[derive(Serialize)]
        struct Preview<'a> {
            token: &'a str,
        }
        let remote: RemoteInvitePreview = self
            .request(
                &local,
                Target::PreviewInvite {
                    body: to_value(Preview { token: &token })?,
                },
            )
            .await?;
        rules::validate_wallet(&remote.wallet)?;
        let expiry = rules::nonnegative(remote.expires_at)?;
        if expiry <= now()
            || remote.wallet.status != "DRAFT"
            || remote.wallet.members.iter().any(|m| m.address == identity.address)
            || remote.wallet.members.len() >= remote.wallet.member_count as usize
        {
            return Err(Error::invalid("INVALID_INVITE"));
        }
        self.check(&local, version).await?;
        let view = {
            let cache = self.cache()?;
            self.wallet_view(&remote.wallet, &self.read()?.1, &cache, &identity.address)?
        };
        self.cache()?.previews.insert(token.clone(), (remote.wallet, expiry, version));
        Ok(BsvHomeBsvMultisigInvitePreview {
            wallet: view,
            token,
            expires_at: expiry,
            signing_identity: identity,
        })
    }

    pub async fn join_invitation(&self, preview: BsvHomeBsvMultisigInvitePreview) -> Result<BsvHomeBsvMultisigWallet, Error> {
        let _mutation = self.mutation.lock().await;
        let (reviewed, expiry, version) = self.cache()?.previews.get(&preview.token).cloned().ok_or_else(|| Error::invalid("INVALID_INVITE_REVIEW"))?;
        if expiry <= now() || preview.expires_at != expiry || preview.wallet.id != reviewed.id || preview.wallet.version != rules::nonnegative(reviewed.version)? {
            return Err(Error::invalid("INVALID_INVITE_REVIEW"));
        }
        let local = self.current_wallet().await?;
        let identity = self.current_signing_identity().await?;
        if identity != preview.signing_identity {
            return Err(Error::WalletChanged);
        }
        self.check(&local, version).await?;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Join<'a> {
            token: &'a str,
            public_key: &'a str,
            wallet_id: &'a str,
            expected_version: u64,
        }
        let response: Result<RemoteWallet, Error> = self
            .request(
                &local,
                Target::Join {
                    body: to_value(Join {
                        token: &preview.token,
                        public_key: &identity.public_key,
                        wallet_id: &reviewed.id,
                        expected_version: preview.wallet.version,
                    })?,
                },
            )
            .await;
        if let Err(Error::Api { code }) = &response
            && [
                "STALE_VERSION",
                "INVALID_INVITE",
                "INVITE_EXPIRED",
                "INVITE_USED",
                "INVALID_INVITE_REVIEW",
                "INVITATION_WALLET_MISMATCH",
            ]
            .contains(&code.as_str())
        {
            self.cache()?.previews.remove(&preview.token);
        }
        let remote = response?;
        rules::validate_wallet(&remote)?;
        if remote.id != reviewed.id
            || remote.threshold != reviewed.threshold
            || remote.member_count != reviewed.member_count
            || remote.members.len() != reviewed.members.len() + 1
            || reviewed.members.iter().any(|member| remote.members.iter().all(|next| next != member))
            || remote.members.iter().all(|m| m.address != identity.address || m.public_key != identity.public_key)
        {
            return Err(Error::MissingSigner);
        }
        self.check(&local, version).await?;
        self.cache()?.wallets.insert(remote.id.clone(), remote.clone());
        self.cache()?.previews.remove(&preview.token);
        self.get_wallet(remote.id)
    }

    pub async fn approve_configuration(&self, wallet_id: String, expected_version: u64) -> Result<BsvHomeBsvMultisigWallet, Error> {
        let _mutation = self.mutation.lock().await;
        let version = self.auth.multisig_version();
        let local = self.current_wallet().await?;
        let reviewed_policy = rules::wallet_policy(&self.remote_wallet(&wallet_id)?)?;
        let remote = self.fetch_wallet(&local, &wallet_id).await?;
        let policy = rules::wallet_policy(&remote)?;
        if remote.version != expected_version as i64 || policy != reviewed_policy {
            return Err(Error::ConcurrentUpdate);
        }
        let (_, previous) = self.read()?;
        let address = identity_address(&local)?;
        match rules::configuration_status(&remote, &address, previous.trusted.get(&self.scope(&wallet_id)))? {
            BsvHomeBsvMultisigConfigurationStatus::Changed => return Err(Error::ConfigurationConflict),
            BsvHomeBsvMultisigConfigurationStatus::Unavailable => return Err(Error::ConfigurationNotConfirmed),
            BsvHomeBsvMultisigConfigurationStatus::NeedsConfirmation
            | BsvHomeBsvMultisigConfigurationStatus::NeedsLocalReview
            | BsvHomeBsvMultisigConfigurationStatus::WaitingForMembers
            | BsvHomeBsvMultisigConfigurationStatus::Confirmed => {}
        }
        self.signer.authenticate(local.clone(), None).await.map_err(authentication)?;
        self.check(&local, version).await?;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Confirm<'a> {
            expected_version: u64,
            configuration_hash: &'a str,
        }
        let hash = remote.configuration_hash.as_ref().ok_or(Error::ConfigurationNotConfirmed)?;
        let confirmed: RemoteWallet = self
            .request(
                &local,
                Target::Confirm {
                    wallet_id: wallet_id.clone(),
                    body: to_value(Confirm {
                        expected_version,
                        configuration_hash: hash,
                    })?,
                },
            )
            .await?;
        rules::validate_wallet(&confirmed)?;
        if confirmed.id != wallet_id || rules::wallet_policy(&confirmed)? != policy || confirmed.members.iter().all(|m| m.address != address || !m.confirmed) {
            return Err(Error::ConfigurationConflict);
        }
        self.check(&local, version).await?;
        let (revision, mut state) = self.read()?;
        state.trusted.insert(self.scope(&wallet_id), policy);
        self.save(revision, state).await?;
        self.check(&local, version).await?;
        self.cache()?.wallets.insert(wallet_id.clone(), confirmed);
        self.get_wallet(wallet_id)
    }

    pub async fn prepare_proposal(
        &self,
        wallet_id: String,
        destination: String,
        amount_bsv: String,
        memo: String,
        action: Arc<ActionScope>,
    ) -> Result<BsvHomeBsvMultisigProposal, Error> {
        self.claim_action(
            &action,
            ActionIntent::PrepareProposal {
                wallet_id: wallet_id.clone(),
                destination: destination.clone(),
                amount_bsv: amount_bsv.clone(),
                memo: memo.clone(),
            },
        )?;
        action
            .run(async {
                let _mutation = self.mutation.lock().await;
                action.check()?;
                let amount = rules::amount_bsv(&amount_bsv)?;
                let destination = destination.trim().to_string();
                bsv_home_bsv::BsvAddress::from_str(&destination).map_err(|_| Error::invalid("INVALID_ADDRESS"))?;
                let memo = rules::text(&memo, 100, true)?;
                let local = self.action_current_wallet(&action).await?;
                let remote = self.action_wallet(&local, &wallet_id, &action).await?;
                self.trusted_policy(&remote, &local)?;
                self.check_action_wallet_review(&action, &remote)?;
                let (revision, mut state) = self.read()?;
                let scope = self.scope(&wallet_id);
                let is_new_intention = !state.intentions.contains_key(&scope);
                let intention = match state.intentions.get(&scope) {
                    Some(intent) if intent.recipient == destination && intent.amount == amount && intent.note == memo => intent.clone(),
                    Some(_) => return Err(Error::invalid("IDEMPOTENCY_CONFLICT")),
                    None => PrepareIntent {
                        key: Uuid::new_v4().to_string(),
                        recipient: destination.clone(),
                        amount,
                        note: memo.clone(),
                    },
                };
                self.authorize_action(
                    &local,
                    &local,
                    AuthorizationRequest::PrepareProposal {
                        wallet_id: wallet_id.clone(),
                        expected_version: rules::nonnegative(remote.version)?,
                        configuration_hash: remote.configuration_hash.clone().ok_or(Error::ConfigurationNotConfirmed)?,
                        destination: destination.clone(),
                        amount_satoshis: amount.to_string(),
                        memo: memo.clone(),
                    },
                    action.clone(),
                )
                .await?;
                self.signer.authenticate(local.clone(), Some(action.clone())).await.map_err(authentication)?;
                self.check_action(&local, &action).await?;
                state.intentions.insert(scope.clone(), intention.clone());
                self.save(revision, state).await?;
                #[derive(Serialize)]
                #[serde(rename_all = "camelCase")]
                struct Prepare<'a> {
                    expected_version: u64,
                    idempotency_key: &'a str,
                    expires_in_seconds: u64,
                    recipient: &'a str,
                    amount: u64,
                    note: &'a str,
                }
                let response: Result<RemoteProposal, Error> = self
                    .action_request(
                        &local,
                        Target::Prepare {
                            wallet_id: wallet_id.clone(),
                            body: to_value(Prepare {
                                expected_version: rules::nonnegative(remote.version)?,
                                idempotency_key: &intention.key,
                                expires_in_seconds: 86400,
                                recipient: &destination,
                                amount,
                                note: &memo,
                            })?,
                        },
                        &action,
                    )
                    .await;
                let proposal = match response {
                    Ok(proposal) => proposal,
                    Err(error) => {
                        if is_new_intention && prepare_rejected(&error) {
                            self.check_action(&local, &action).await?;
                            let (revision, mut state) = self.read()?;
                            if state.intentions.get(&scope).is_none_or(|saved| saved.key != intention.key) {
                                return Err(Error::ConcurrentUpdate);
                            }
                            state.intentions.remove(&scope);
                            self.save(revision, state).await?;
                        }
                        return Err(error);
                    }
                };
                rules::validate_proposal(&proposal, &remote)?;
                if proposal.proposal.recipient != destination || proposal.proposal.amount != amount || proposal.note.as_deref().unwrap_or("") != memo {
                    return Err(Error::invalid("INVALID_BSV_TRANSACTION"));
                }
                action.submitted(Some(proposal.id.clone()), proposal.txid.clone())?;
                action.finish()?;
                self.check_action(&local, &action).await?;
                let (revision, mut state) = self.read()?;
                state.intentions.remove(&scope);
                if let Err(error) = self.save(revision, state).await {
                    action.sync_failed()?;
                    return Err(error);
                }
                self.check_action(&local, &action).await?;
                self.cache()?.wallets.insert(remote.id.clone(), remote);
                self.cache()?.proposals.insert(proposal.id.clone(), proposal.clone());
                self.get_proposal(proposal.id)
            })
            .await
    }

    pub fn proposal_review(&self, proposal_id: String) -> Result<Vec<BsvHomeBsvMultisigField>, Error> {
        self.check_identity()?;
        let proposal = self.remote_proposal(&proposal_id)?;
        proposal_review_fields(&self.remote_wallet(&proposal.wallet_id)?, &proposal)
    }

    pub fn proposal_review_snapshot(&self, proposal_id: String) -> Result<BsvHomeBsvMultisigProposalReview, Error> {
        self.check_identity()?;
        let (wallet, proposal) = {
            let cache = self.cache()?;
            let proposal = cache.proposals.get(&proposal_id).cloned().ok_or(Error::MissingRecord)?;
            (cache.wallets.get(&proposal.wallet_id).cloned().ok_or(Error::MissingRecord)?, proposal)
        };
        let fields = proposal_review_fields(&wallet, &proposal)?;
        let view = self.proposal_view(&proposal, &wallet, &self.read()?.1, &self.cached_address()?)?;
        let review_id = Uuid::new_v4().to_string();
        let reviewed = ReviewedProposal {
            id: review_id.clone(),
            proposal_id,
            version: view.version,
            hash: review_hash(&(&wallet, &proposal))?,
        };
        let mut cache = self.cache()?;
        cache.reviews.push_back(reviewed);
        if cache.reviews.len() > MAX_PROPOSAL_REVIEWS {
            cache.reviews.pop_front();
        }
        Ok(BsvHomeBsvMultisigProposalReview {
            review_id,
            proposal: view,
            fields,
        })
    }

    pub async fn sign_proposal(&self, proposal_id: String, expected_version: u64, action: Arc<ActionScope>) -> Result<BsvHomeBsvMultisigProposal, Error> {
        self.claim_action(
            &action,
            ActionIntent::SignProposal {
                proposal_id: proposal_id.clone(),
                expected_version,
                review_id: action.review_id()?.to_string(),
            },
        )?;
        action
            .run(async {
                let _mutation = self.mutation.lock().await;
                action.check()?;
                let version = action.generation();
                let local = self.action_current_wallet(&action).await?;
                let previous = self.remote_proposal(&proposal_id)?;
                let wallet = self.action_wallet(&local, &previous.wallet_id, &action).await?;
                let policy = self.trusted_policy(&wallet, &local)?;
                let proposal = self.fetch_proposal_with_action(&local, &wallet, &proposal_id, Some(&action)).await?;
                if proposal.version != expected_version as i64 || proposal.proposal != previous.proposal {
                    return Err(Error::ConcurrentUpdate);
                }
                self.check_action_proposal_review(&action, &wallet, &proposal)?;
                let view = self.proposal_view(&proposal, &wallet, &self.read()?.1, &identity_address(&local)?)?;
                if !view.can_sign {
                    return Err(Error::invalid("PROPOSAL_CLOSED"));
                }
                self.authorize_action(
                    &local,
                    &local,
                    AuthorizationRequest::SignProposal {
                        wallet_id: wallet.id.clone(),
                        proposal_id: proposal.id.clone(),
                        expected_version,
                        configuration_hash: wallet.configuration_hash.clone().ok_or(Error::ConfigurationNotConfirmed)?,
                        destination: proposal.proposal.recipient.clone(),
                        amount_satoshis: proposal.proposal.amount.to_string(),
                        fee_satoshis: proposal.proposal.fee.to_string(),
                    },
                    action.clone(),
                )
                .await?;
                let signature_json = self
                    .signer
                    .sign_proposal(
                        local.clone(),
                        serde_json::to_string(&policy)?,
                        serde_json::to_string(&proposal.proposal)?,
                        proposal.proposal.recipient.clone(),
                        proposal.proposal.amount,
                        proposal.proposal.fee,
                        action.clone(),
                    )
                    .await
                    .map_err(authentication)?;
                let signatures: bsv_home_bsv::multisig::SignatureSet = serde_json::from_str(&signature_json)?;
                if member_address(&signatures.public_key)? != identity_address(&local)? || signatures.signatures.len() != proposal.proposal.inputs.len() {
                    return Err(Error::MissingSigner);
                }
                self.check_action(&local, &action).await?;
                #[derive(Serialize)]
                #[serde(rename_all = "camelCase")]
                struct Sign<'a> {
                    expected_version: u64,
                    public_key: &'a str,
                    signatures: &'a [String],
                }
                let result: RemoteProposal = self
                    .action_request(
                        &local,
                        Target::Signatures {
                            proposal_id,
                            body: to_value(Sign {
                                expected_version,
                                public_key: &signatures.public_key,
                                signatures: &signatures.signatures,
                            })?,
                        },
                        &action,
                    )
                    .await?;
                self.save_proposal_result(&local, version, &wallet, &proposal, result, Some(&action)).await
            })
            .await
    }

    pub async fn broadcast_proposal(&self, proposal_id: String, expected_version: u64, action: Arc<ActionScope>) -> Result<BsvHomeBsvMultisigProposal, Error> {
        self.claim_action(
            &action,
            ActionIntent::BroadcastProposal {
                proposal_id: proposal_id.clone(),
                expected_version,
                review_id: action.review_id()?.to_string(),
            },
        )?;
        action
            .run(async {
                let _mutation = self.mutation.lock().await;
                action.check()?;
                let version = action.generation();
                let local = self.action_current_wallet(&action).await?;
                let previous = self.remote_proposal(&proposal_id)?;
                let wallet = self.action_wallet(&local, &previous.wallet_id, &action).await?;
                let policy = self.trusted_policy(&wallet, &local)?;
                let proposal = self.fetch_proposal_with_action(&local, &wallet, &proposal_id, Some(&action)).await?;
                if proposal.version != expected_version as i64 || proposal.proposal != previous.proposal || proposal.signatures != previous.signatures {
                    return Err(Error::ConcurrentUpdate);
                }
                self.check_action_proposal_review(&action, &wallet, &proposal)?;
                if !self.proposal_view(&proposal, &wallet, &self.read()?.1, &identity_address(&local)?)?.can_broadcast {
                    return Err(Error::invalid("PROPOSAL_NOT_READY"));
                }
                let raw = assemble_proposal(&policy, &proposal.proposal, &proposal.signatures)?;
                let expected_transaction_id = bsv_home_bsv::multisig::proposal_transaction_id(&policy, &proposal.proposal, &raw)?;
                self.authorize_action(
                    &local,
                    &local,
                    AuthorizationRequest::BroadcastProposal {
                        wallet_id: wallet.id.clone(),
                        proposal_id: proposal.id.clone(),
                        expected_version,
                        configuration_hash: wallet.configuration_hash.clone().ok_or(Error::ConfigurationNotConfirmed)?,
                        destination: proposal.proposal.recipient.clone(),
                        amount_satoshis: proposal.proposal.amount.to_string(),
                        fee_satoshis: proposal.proposal.fee.to_string(),
                        transaction_id: expected_transaction_id.clone(),
                    },
                    action.clone(),
                )
                .await?;
                self.signer.authenticate(local.clone(), Some(action.clone())).await.map_err(authentication)?;
                self.check_action(&local, &action).await?;
                #[derive(Serialize)]
                #[serde(rename_all = "camelCase")]
                struct Broadcast<'a> {
                    expected_version: u64,
                    raw_transaction: &'a str,
                }
                let result: RemoteProposal = self
                    .action_request(
                        &local,
                        Target::Broadcast {
                            proposal_id,
                            body: to_value(Broadcast {
                                expected_version,
                                raw_transaction: &raw,
                            })?,
                        },
                        &action,
                    )
                    .await?;
                if result.txid.as_deref() != Some(expected_transaction_id.as_str()) || (result.status != "BROADCAST" && result.status != "CONFIRMED") {
                    return Err(Error::invalid("BROADCAST_CONFLICT"));
                }
                self.save_proposal_result(&local, version, &wallet, &proposal, result, Some(&action)).await
            })
            .await
    }

    pub async fn withdraw_proposal(&self, proposal_id: String, expected_version: u64, action: Arc<ActionScope>) -> Result<BsvHomeBsvMultisigProposal, Error> {
        self.claim_action(
            &action,
            ActionIntent::WithdrawProposal {
                proposal_id: proposal_id.clone(),
                expected_version,
                review_id: action.review_id()?.to_string(),
            },
        )?;
        action
            .run(async {
                let _mutation = self.mutation.lock().await;
                action.check()?;
                let version = action.generation();
                let local = self.action_current_wallet(&action).await?;
                let previous = self.remote_proposal(&proposal_id)?;
                let wallet = self.action_wallet(&local, &previous.wallet_id, &action).await?;
                let proposal = self.fetch_proposal_with_action(&local, &wallet, &proposal_id, Some(&action)).await?;
                if proposal.version != expected_version as i64 || proposal.proposal != previous.proposal {
                    return Err(Error::ConcurrentUpdate);
                }
                self.check_action_proposal_review(&action, &wallet, &proposal)?;
                if !self.proposal_view(&proposal, &wallet, &self.read()?.1, &identity_address(&local)?)?.can_withdraw {
                    return Err(Error::invalid("CREATOR_REQUIRED"));
                }
                self.signer.authenticate(local.clone(), Some(action.clone())).await.map_err(authentication)?;
                self.check_action(&local, &action).await?;
                let result: RemoteProposal = self
                    .action_request(
                        &local,
                        Target::Withdraw {
                            proposal_id,
                            body: to_value(Version { expected_version })?,
                        },
                        &action,
                    )
                    .await?;
                if result.status != "WITHDRAWN" {
                    return Err(Error::invalid("PROPOSAL_CLOSED"));
                }
                self.save_proposal_result(&local, version, &wallet, &proposal, result, Some(&action)).await
            })
            .await
    }

    pub async fn refresh_wallet(&self, wallet_id: String) -> Result<BsvHomeBsvMultisigWallet, Error> {
        let version = self.auth.multisig_version();
        let local = self.current_wallet().await?;
        let wallet = self.fetch_wallet(&local, &wallet_id).await?;
        self.check(&local, version).await?;
        self.cache()?.wallets.insert(wallet_id.clone(), wallet.clone());
        if wallet.status == "ACTIVE" {
            self.fetch_balance(&local, &wallet, version).await?;
        }
        self.get_wallet(wallet_id)
    }

    pub async fn refresh_proposal(&self, proposal_id: String) -> Result<BsvHomeBsvMultisigProposal, Error> {
        let version = self.auth.multisig_version();
        let local = self.current_wallet().await?;
        let previous = self.remote_proposal(&proposal_id)?;
        let wallet = self.fetch_wallet(&local, &previous.wallet_id).await?;
        let result: RemoteProposal = self.request(&local, Target::Refresh { proposal_id }).await?;
        self.save_proposal_result(&local, version, &wallet, &previous, result, None).await
    }

    pub fn export_configuration(&self, wallet_id: String) -> Result<String, Error> {
        let view = self.get_wallet(wallet_id.clone())?;
        if !view.can_transact {
            return Err(Error::ConfigurationNotConfirmed);
        }
        rules::public_configuration(&self.remote_wallet(&wallet_id)?)
    }
    pub fn receive_request(&self, wallet_id: String) -> Result<String, Error> {
        self.export_configuration(wallet_id)
    }
}

impl BsvHomeBsvMultisigService {
    pub(super) fn cache(&self) -> Result<std::sync::MutexGuard<'_, Cache>, Error> {
        self.cache.lock().map_err(|_| Error::Storage {
            reason: "multisig state lock failed".into(),
        })
    }
    pub(super) fn read(&self) -> Result<(u64, State), Error> {
        match self.store.get_state().map_err(storage)? {
            Some(state) => Ok((state.revision, serde_json::from_str(&state.json)?)),
            None => Ok((0, State::default())),
        }
    }
    pub(super) async fn save(&self, revision: u64, state: State) -> Result<(), Error> {
        if !self.store.compare_and_swap(revision, serde_json::to_string(&state)?).await.map_err(storage)? {
            return Err(Error::ConcurrentUpdate);
        }
        Ok(())
    }
    pub(super) fn scope(&self, wallet_id: &str) -> String {
        format!("{}|{}|{}", self.auth.multisig_origin(), self.wallet_id, wallet_id)
    }
    pub(super) fn check_identity(&self) -> Result<(), Error> {
        self.auth.multisig_check_scope(&self.wallet_id).map_err(Error::from)
    }
    pub(super) async fn current_wallet(&self) -> Result<Wallet, Error> {
        self.check_identity()?;
        let wallet = self.auth.multisig_wallet().await?;
        if wallet.id.id() != self.wallet_id {
            return Err(Error::WalletChanged);
        }
        {
            let mut cache = self.cache()?;
            cache.identity_address = Some(identity_address(&wallet)?);
            cache.identity_wallet = Some(wallet.clone());
        }
        Ok(wallet)
    }
    pub(super) async fn check(&self, wallet: &Wallet, version: u64) -> Result<(), Error> {
        self.check_identity()?;
        self.auth.multisig_check_wallet(wallet, version).await?;
        Ok(())
    }
    pub(super) async fn request<R: serde::de::DeserializeOwned>(&self, wallet: &Wallet, target: Target) -> Result<R, Error> {
        Ok(self.auth.multisig_wallet_request(wallet, target).await?)
    }
    pub(super) fn workflow_checkpoint(&self) -> Result<(), Error> {
        if self.auth.multisig_version() != self.workflow_version {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    pub(super) async fn check_workflow_wallet(&self, wallet: &Wallet) -> Result<(), Error> {
        self.workflow_checkpoint()?;
        self.check(wallet, self.workflow_version).await?;
        self.workflow_checkpoint()
    }
    pub(super) async fn workflow_request<R: serde::de::DeserializeOwned>(&self, wallet: &Wallet, target: Target) -> Result<R, Error> {
        self.workflow_checkpoint()?;
        let result = self.auth.multisig_wallet_request_at_generation(wallet, target, self.workflow_version).await;
        self.check_workflow_wallet(wallet).await?;
        Ok(result?)
    }
    fn cached_address(&self) -> Result<String, Error> {
        self.check_identity()?;
        self.cache()?.identity_address.clone().ok_or(Error::MissingSigner)
    }
    pub(super) fn remote_wallet(&self, id: &str) -> Result<RemoteWallet, Error> {
        self.cache()?.wallets.get(id).cloned().ok_or(Error::MissingRecord)
    }
    pub(super) fn remote_proposal(&self, id: &str) -> Result<RemoteProposal, Error> {
        self.cache()?.proposals.get(id).cloned().ok_or(Error::MissingRecord)
    }
    pub(super) async fn fetch_wallet(&self, local: &Wallet, id: &str) -> Result<RemoteWallet, Error> {
        let wallet: RemoteWallet = self.request(local, Target::Wallet { wallet_id: id.into() }).await?;
        rules::validate_wallet(&wallet)?;
        let address = identity_address(local)?;
        if wallet.id != id || wallet.members.iter().all(|m| m.address != address) {
            return Err(Error::MissingSigner);
        }
        Ok(wallet)
    }
    async fn fetch_proposals(&self, local: &Wallet, wallet: &RemoteWallet) -> Result<Vec<RemoteProposal>, Error> {
        self.fetch_proposals_with_action(local, wallet, None).await
    }
    async fn fetch_proposals_with_action(&self, local: &Wallet, wallet: &RemoteWallet, action: Option<&ActionScope>) -> Result<Vec<RemoteProposal>, Error> {
        let mut output = Vec::new();
        let mut ids = BTreeSet::new();
        let mut offset = 0;
        let mut total = None;
        loop {
            let target = Target::ProposalPage {
                wallet_id: wallet.id.clone(),
                offset,
                limit: 200,
            };
            let page: ProposalPage = match action {
                Some(action) => self.action_request(local, target, action).await?,
                None => self.request(local, target).await?,
            };
            if page.offset != offset
                || page.limit != 200
                || page.total < 0
                || page.total > 10000
                || page.items.len() > 200
                || (page.has_more && page.items.is_empty())
                || total.is_some_and(|v| v != page.total)
            {
                return Err(Error::invalid("PROPOSAL_HISTORY_LIMIT"));
            }
            total = Some(page.total);
            for proposal in page.items {
                rules::validate_proposal(&proposal, wallet)?;
                if !ids.insert(proposal.id.clone()) {
                    return Err(Error::ConcurrentUpdate);
                }
                output.push(proposal);
            }
            if output.len() > 10000 {
                return Err(Error::invalid("PROPOSAL_HISTORY_LIMIT"));
            }
            if !page.has_more {
                if output.len() as i64 != page.total {
                    return Err(Error::ConcurrentUpdate);
                }
                return Ok(output);
            }
            offset = u32::try_from(output.len()).map_err(|_| Error::invalid("PROPOSAL_HISTORY_LIMIT"))?;
        }
    }
    async fn fetch_proposal_with_action(&self, local: &Wallet, wallet: &RemoteWallet, id: &str, action: Option<&ActionScope>) -> Result<RemoteProposal, Error> {
        self.fetch_proposals_with_action(local, wallet, action)
            .await?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or(Error::MissingRecord)
    }
    pub(super) async fn fetch_balance(&self, local: &Wallet, wallet: &RemoteWallet, version: u64) -> Result<(), Error> {
        let result = async {
            let utxos: Vec<RemoteUtxo> = self.request(local, Target::Utxos { wallet_id: wallet.id.clone() }).await?;
            let policy = rules::wallet_policy(wallet)?;
            let mut balance = 0u64;
            let mut outpoints = BTreeSet::new();
            for output in utxos {
                let value = rules::nonnegative(output.satoshis)?;
                let index = u32::try_from(output.output_index).map_err(|_| Error::invalid("INVALID_RESPONSE"))?;
                if !outpoints.insert((output.txid.clone(), index)) {
                    return Err(Error::invalid("INVALID_RESPONSE"));
                }
                bsv_home_bsv::multisig::validate_portal_utxo(&policy, &output.txid, index, value, &output.transaction_hex)?;
                balance = balance
                    .checked_add(value)
                    .filter(|v| *v <= 2_100_000_000_000_000)
                    .ok_or_else(|| Error::invalid("INVALID_AMOUNT"))?;
            }
            self.check(local, version).await?;
            self.cache()?.balances.insert(wallet.id.clone(), (balance, now(), false));
            Ok(())
        }
        .await;
        if result.is_err() {
            self.check(local, version).await?;
            if let Some(balance) = self.cache()?.balances.get_mut(&wallet.id) {
                balance.2 = true;
            }
        }
        result
    }

    pub(super) fn trusted_policy(&self, wallet: &RemoteWallet, local: &Wallet) -> Result<Policy, Error> {
        let state = self.read()?.1;
        match rules::configuration_status(wallet, &identity_address(local)?, state.trusted.get(&self.scope(&wallet.id)))? {
            BsvHomeBsvMultisigConfigurationStatus::Confirmed => rules::wallet_policy(wallet),
            BsvHomeBsvMultisigConfigurationStatus::Changed => Err(Error::ConfigurationConflict),
            BsvHomeBsvMultisigConfigurationStatus::Unavailable
            | BsvHomeBsvMultisigConfigurationStatus::NeedsConfirmation
            | BsvHomeBsvMultisigConfigurationStatus::NeedsLocalReview
            | BsvHomeBsvMultisigConfigurationStatus::WaitingForMembers => Err(Error::ConfigurationNotConfirmed),
        }
    }
    pub(super) fn wallet_view(&self, wallet: &RemoteWallet, state: &State, cache: &Cache, address: &str) -> Result<BsvHomeBsvMultisigWallet, Error> {
        let configuration_status = rules::configuration_status(wallet, address, state.trusted.get(&self.scope(&wallet.id)))?;
        let status = match wallet.status.as_str() {
            "ACTIVE" => BsvHomeBsvMultisigWalletStatus::Active,
            "DRAFT" if wallet.members.len() < wallet.member_count as usize => BsvHomeBsvMultisigWalletStatus::WaitingForMembers,
            "DRAFT" => BsvHomeBsvMultisigWalletStatus::WaitingForConfirmations,
            _ => BsvHomeBsvMultisigWalletStatus::Unknown,
        };
        let names = state.member_names.get(&self.scope(&wallet.id));
        let members = (0..wallet.member_count as usize)
            .map(|slot| {
                let member = wallet.members.get(slot);
                BsvHomeBsvMultisigMember {
                    slot: slot as u32,
                    name: names.and_then(|n| n.get(slot)).cloned().unwrap_or_default(),
                    public_key: member.map(|m| m.public_key.clone()),
                    address: member.map(|m| m.address.clone()),
                    status: match member {
                        None => BsvHomeBsvMultisigMemberStatus::Invited,
                        Some(m) if m.confirmed => BsvHomeBsvMultisigMemberStatus::Confirmed,
                        Some(_) => BsvHomeBsvMultisigMemberStatus::AwaitingApproval,
                    },
                }
            })
            .collect();
        let member = wallet.members.iter().find(|m| m.address == address);
        let balance = cache.balances.get(&wallet.id);
        let proposals = cache.proposals.values().cloned().collect::<Vec<_>>();
        let can_transact = configuration_status == BsvHomeBsvMultisigConfigurationStatus::Confirmed;
        let personal_configuration_pending = rules::needs_configuration_review(&configuration_status);
        let pending_count = rules::personal_pending(wallet, &proposals, address, &configuration_status, now());
        Ok(BsvHomeBsvMultisigWallet {
            id: wallet.id.clone(),
            name: wallet.name.clone(),
            threshold: wallet.threshold,
            member_count: wallet.member_count,
            version: rules::nonnegative(wallet.version)?,
            status,
            configuration_status,
            members,
            configuration_id: wallet.configuration_hash.clone(),
            locking_script: wallet.locking_script.clone(),
            local_signer_wallet_id: member.map(|_| self.wallet_id.clone()),
            local_signer_public_key: member.map(|m| m.public_key.clone()),
            balance_satoshis: balance.map(|v| v.0.to_string()),
            balance_updated_at: balance.map(|v| v.1),
            balance_is_stale: balance.is_none_or(|v| v.2),
            pending_count,
            can_invite: wallet.status == "DRAFT" && wallet.members.len() < wallet.member_count as usize && wallet.members.first().is_some_and(|m| m.address == address),
            can_transact,
            personal_configuration_pending,
        })
    }
    fn proposal_view(&self, proposal: &RemoteProposal, wallet: &RemoteWallet, state: &State, address: &str) -> Result<BsvHomeBsvMultisigProposal, Error> {
        let member = wallet.members.iter().find(|m| m.address == address).ok_or(Error::MissingSigner)?;
        let trusted = rules::configuration_status(wallet, address, state.trusted.get(&self.scope(&wallet.id)))? == BsvHomeBsvMultisigConfigurationStatus::Confirmed;
        let now = now();
        let expires_at = rules::nonnegative(proposal.expires_at)?;
        let unexpired = expires_at > now;
        let state = match proposal.status.as_str() {
            "COLLECTING" if !unexpired => BsvHomeBsvMultisigProposalState::Expired,
            "COLLECTING" => BsvHomeBsvMultisigProposalState::CollectingSignatures,
            "READY" if !unexpired => BsvHomeBsvMultisigProposalState::Expired,
            "READY" | "BROADCAST_FAILED" => BsvHomeBsvMultisigProposalState::ReadyToBroadcast,
            "BROADCAST" => BsvHomeBsvMultisigProposalState::PendingConfirmation,
            "CONFIRMED" => BsvHomeBsvMultisigProposalState::Confirmed,
            "WITHDRAWN" => BsvHomeBsvMultisigProposalState::Stopped,
            "EXPIRED" => BsvHomeBsvMultisigProposalState::Expired,
            _ => BsvHomeBsvMultisigProposalState::Unknown,
        };
        let signed = proposal.signatures.iter().any(|s| s.public_key == member.public_key);
        let collecting = proposal.status == "COLLECTING" && unexpired;
        Ok(BsvHomeBsvMultisigProposal {
            id: proposal.id.clone(),
            wallet_id: wallet.id.clone(),
            version: rules::nonnegative(proposal.version)?,
            destination: proposal.proposal.recipient.clone(),
            value_satoshis: proposal.proposal.amount.to_string(),
            fee_satoshis: proposal.proposal.fee.to_string(),
            memo: proposal.note.clone().unwrap_or_default(),
            state,
            remote_status: proposal.status.clone(),
            input_count: proposal.proposal.inputs.len() as u32,
            signed_members: proposal.signatures.iter().map(|s| s.public_key.clone()).collect(),
            required_signatures: wallet.threshold,
            transaction_id: proposal.txid.clone(),
            created_at: rules::nonnegative(proposal.created_at)?,
            expires_at,
            confirmations: proposal.confirmations.map(rules::nonnegative).transpose()?,
            can_sign: trusted && collecting && !signed,
            can_broadcast: trusted && (proposal.status == "READY" || proposal.status == "BROADCAST_FAILED") && unexpired,
            can_withdraw: collecting && rules::proposal_creator(proposal, wallet)?.is_some_and(|creator| creator.user_id == member.user_id && creator.address == member.address),
            personal_pending: rules::proposal_pending(&proposal.status, expires_at, signed, now),
        })
    }
    async fn save_proposal_result(
        &self,
        local: &Wallet,
        version: u64,
        wallet: &RemoteWallet,
        previous: &RemoteProposal,
        result: RemoteProposal,
        action: Option<&ActionScope>,
    ) -> Result<BsvHomeBsvMultisigProposal, Error> {
        rules::validate_proposal(&result, wallet)?;
        if result.id != previous.id || result.proposal != previous.proposal {
            return Err(Error::invalid("INVALID_BSV_TRANSACTION"));
        }
        if let Some(action) = action {
            action.submitted(Some(result.id.clone()), result.txid.clone())?;
            action.finish()?;
            self.check_action(local, action).await?;
        } else {
            self.check(local, version).await?;
        }
        self.cache()?.wallets.insert(wallet.id.clone(), wallet.clone());
        self.cache()?.proposals.insert(result.id.clone(), result.clone());
        self.get_proposal(result.id)
    }
}

const MAX_PROPOSAL_REVIEWS: usize = 32;

fn proposal_review_fields(wallet: &RemoteWallet, proposal: &RemoteProposal) -> Result<Vec<BsvHomeBsvMultisigField>, Error> {
    let policy = rules::wallet_policy(wallet)?;
    let review = review_proposal(&policy, &proposal.proposal)?;
    Ok(vec![
        field("configurationId", wallet.configuration_hash.clone().ok_or(Error::ConfigurationNotConfirmed)?),
        field("payment", review.recipient),
        field("amount", review.amount.to_string()),
        field("fee", review.fee.to_string()),
        field("change", review.change.to_string()),
        field("inputCount", review.input_count.to_string()),
    ])
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Version {
    expected_version: u64,
}
fn field(title: &str, value: String) -> BsvHomeBsvMultisigField {
    BsvHomeBsvMultisigField { title: title.into(), value }
}
pub(super) fn now() -> u64 {
    Utc::now().timestamp().max(0) as u64
}
pub(super) fn identity_address(wallet: &Wallet) -> Result<String, Error> {
    wallet.account(Chain::BitcoinSv).map(|a| a.address.clone()).ok_or(Error::MissingSigner)
}
pub(super) fn authentication(error: BsvHomeServiceError) -> Error {
    match error {
        BsvHomeServiceError::Cancelled => Error::Cancelled,
        other => Error::Authentication { reason: other.to_string() },
    }
}
fn storage(error: BsvHomeServiceError) -> Error {
    Error::Storage { reason: error.to_string() }
}
fn prepare_rejected(error: &Error) -> bool {
    matches!(error, Error::Api { code } if matches!(code.as_str(),
        "INVALID_AMOUNT" | "INVALID_NOTE" | "INVALID_EXPIRY" | "INVALID_IDEMPOTENCY_KEY" | "INVALID_REQUEST" | "INVALID_POLICY" | "INVALID_PUBLIC_KEY" |
        "INSUFFICIENT_FUNDS" | "TOO_MANY_UTXOS" | "WALLET_NOT_ACTIVE" | "NOT_A_MEMBER" | "BSV_IDENTITY_REQUIRED" | "STALE_VERSION"))
}
