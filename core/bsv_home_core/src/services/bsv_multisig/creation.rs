use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::to_value;
use uuid::Uuid;

use primitives::Wallet;

use super::model::{CreationPayload, ReadyCreation, RemoteWallet, StoredCreation};
use super::service::identity_address;
use super::{
    BsvHomeBsvMultisigCreationDraft, BsvHomeBsvMultisigCreationStatus, BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigService, BsvHomeBsvMultisigSignerBinding,
    BsvHomeBsvMultisigWallet, rules,
};
use crate::services::portal::BsvMultisigTarget as Target;

#[cfg(test)]
#[path = "creation_tests.rs"]
mod tests;

#[uniffi::export]
impl BsvHomeBsvMultisigService {
    pub async fn begin_creation(&self, name: String, threshold: u32, member_names: Vec<String>, signer_wallet_id: String) -> Result<BsvHomeBsvMultisigCreationDraft, Error> {
        let name = rules::creation_text(&name, 30)?;
        if !(2..=5).contains(&member_names.len()) || threshold < 2 || threshold as usize > member_names.len() {
            return Err(Error::InvalidRules);
        }
        let member_names = member_names.iter().map(|name| rules::creation_text(name, 20)).collect::<Result<Vec<_>, _>>()?;
        if signer_wallet_id != self.wallet_id {
            return Err(Error::WalletChanged);
        }
        self.workflow_checkpoint()?;
        let _mutation = self.mutation.lock().await;
        let (local, signing_identity) = self.creation_identity().await?;
        let payload = CreationPayload {
            name,
            threshold,
            member_names,
            signing_identity,
        };
        let (_, state) = self.read()?;
        let prefix = self.scope("");
        for (key, creation) in &state.creations {
            if let Some(id) = key.strip_prefix(&prefix)
                && creation.wallet_id.is_none()
            {
                if creation.payload.signing_identity != payload.signing_identity {
                    return Err(Error::WalletChanged);
                }
                if creation.payload == payload {
                    return Ok(creation.payload.draft(id.to_string(), BsvHomeBsvMultisigCreationStatus::Submitted));
                }
            }
        }
        {
            let cache = self.cache()?;
            if let Some(ready) = &cache.ready_creation
                && ready.payload == payload
            {
                return Ok(payload.draft(ready.id.clone(), BsvHomeBsvMultisigCreationStatus::Ready));
            }
        }
        let wallets = self.creation_wallets(&local).await?;
        let id = Uuid::new_v4().to_string();
        let draft = payload.draft(id.clone(), BsvHomeBsvMultisigCreationStatus::Ready);
        self.cache()?.ready_creation = Some(ReadyCreation {
            id,
            payload,
            baseline_wallet_ids: wallets.into_iter().map(|wallet| wallet.id).collect(),
        });
        Ok(draft)
    }

    pub async fn submit_creation(&self, draft_id: String) -> Result<BsvHomeBsvMultisigWallet, Error> {
        self.workflow_checkpoint()?;
        let _mutation = self.mutation.lock().await;
        let (local, identity) = self.creation_identity().await?;
        let (revision, mut state) = self.read()?;
        if let Some(creation) = state.creations.get(&self.scope(&draft_id)) {
            self.check_creation_identity(creation, &identity)?;
            return match &creation.wallet_id {
                Some(wallet_id) => self.creation_wallet(&local, &creation.payload, wallet_id).await,
                None => Err(Error::CreationUncertain { draft_id }),
            };
        }
        let creation = {
            let mut cache = self.cache()?;
            let ready = cache.ready_creation.as_ref().filter(|ready| ready.id == draft_id).ok_or(Error::MissingRecord)?;
            if ready.payload.signing_identity != identity {
                return Err(Error::WalletChanged);
            }
            let creation = StoredCreation {
                payload: ready.payload.clone(),
                baseline_wallet_ids: ready.baseline_wallet_ids.clone(),
                wallet_id: None,
            };
            for (key, pending) in &state.creations {
                if let Some(id) = key.strip_prefix(&self.scope(""))
                    && pending.wallet_id.is_none()
                    && pending.payload == creation.payload
                {
                    cache.ready_creation = None;
                    return Err(Error::CreationUncertain { draft_id: id.to_string() });
                }
            }
            creation
        };
        state.creations.insert(self.scope(&draft_id), creation.clone());
        self.save(revision, state).await?;
        self.check_workflow_wallet(&local).await?;
        self.cache()?.ready_creation = None;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Create<'a> {
            name: &'a str,
            threshold: u32,
            member_count: usize,
            public_key: &'a str,
        }
        let result: Result<RemoteWallet, Error> = self
            .workflow_request(
                &local,
                Target::Create {
                    body: to_value(Create {
                        name: &creation.payload.name,
                        threshold: creation.payload.threshold,
                        member_count: creation.payload.member_names.len(),
                        public_key: &identity.public_key,
                    })?,
                },
            )
            .await;
        self.check_workflow_wallet(&local).await?;
        let remote = result.map_err(|_| Error::CreationUncertain { draft_id: draft_id.clone() })?;
        rules::validate_wallet(&remote)?;
        if !creation.payload.matches(&remote) || remote.members.len() != 1 || creation.baseline_wallet_ids.contains(&remote.id) {
            return Err(Error::MissingSigner);
        }
        self.commit_creation(&local, &draft_id, creation, remote).await
    }

    pub async fn reconcile_creation(&self, draft_id: String) -> Result<Vec<BsvHomeBsvMultisigWallet>, Error> {
        self.workflow_checkpoint()?;
        let _mutation = self.mutation.lock().await;
        let (local, identity) = self.creation_identity().await?;
        let creation = self.stored_creation(&draft_id, &identity)?;
        if let Some(wallet_id) = &creation.wallet_id {
            return Ok(vec![self.creation_wallet(&local, &creation.payload, wallet_id).await?]);
        }
        let wallets = self.creation_wallets(&local).await?;
        let (_, state) = self.read()?;
        let cache = self.cache()?;
        wallets
            .iter()
            .filter(|wallet| !creation.baseline_wallet_ids.contains(&wallet.id) && creation.payload.matches(wallet))
            .map(|wallet| self.wallet_view(wallet, &state, &cache, &identity.address))
            .collect()
    }

    pub async fn accept_creation(&self, draft_id: String, wallet_id: String) -> Result<BsvHomeBsvMultisigWallet, Error> {
        self.workflow_checkpoint()?;
        let _mutation = self.mutation.lock().await;
        let (local, identity) = self.creation_identity().await?;
        let creation = self.stored_creation(&draft_id, &identity)?;
        if let Some(resolved) = &creation.wallet_id {
            if *resolved != wallet_id {
                return Err(Error::ConcurrentUpdate);
            }
            return self.creation_wallet(&local, &creation.payload, resolved).await;
        }
        let remote: RemoteWallet = self.workflow_request(&local, Target::Wallet { wallet_id: wallet_id.clone() }).await?;
        rules::validate_wallet(&remote)?;
        if remote.id != wallet_id || creation.baseline_wallet_ids.contains(&wallet_id) || !creation.payload.matches(&remote) {
            return Err(Error::MissingSigner);
        }
        self.commit_creation(&local, &draft_id, creation, remote).await
    }
}

impl CreationPayload {
    fn draft(&self, id: String, status: BsvHomeBsvMultisigCreationStatus) -> BsvHomeBsvMultisigCreationDraft {
        BsvHomeBsvMultisigCreationDraft {
            id,
            name: self.name.clone(),
            threshold: self.threshold,
            member_names: self.member_names.clone(),
            signing_identity: self.signing_identity.clone(),
            status,
        }
    }

    fn matches(&self, wallet: &RemoteWallet) -> bool {
        wallet.name == self.name
            && wallet.threshold == self.threshold
            && wallet.member_count as usize == self.member_names.len()
            && wallet
                .members
                .first()
                .is_some_and(|member| member.public_key == self.signing_identity.public_key && member.address == self.signing_identity.address)
    }
}

impl BsvHomeBsvMultisigService {
    async fn creation_identity(&self) -> Result<(Wallet, BsvHomeBsvMultisigSignerBinding), Error> {
        self.workflow_checkpoint()?;
        let wallet = self.current_wallet().await?;
        self.workflow_checkpoint()?;
        let identity = self.current_signing_identity().await?;
        self.check_workflow_wallet(&wallet).await?;
        Ok((wallet, identity))
    }

    async fn creation_wallets(&self, local: &Wallet) -> Result<Vec<RemoteWallet>, Error> {
        let wallets: Vec<RemoteWallet> = self.workflow_request(local, Target::Wallets).await?;
        if wallets.len() > 100 {
            return Err(Error::invalid("WALLET_LIST_LIMIT"));
        }
        let mut ids = BTreeSet::new();
        let address = identity_address(local)?;
        for wallet in &wallets {
            rules::validate_wallet(wallet)?;
            if !ids.insert(&wallet.id) || wallet.members.iter().all(|member| member.address != address) {
                return Err(Error::MissingSigner);
            }
        }
        Ok(wallets)
    }

    fn check_creation_identity(&self, creation: &StoredCreation, identity: &BsvHomeBsvMultisigSignerBinding) -> Result<(), Error> {
        if creation.payload.signing_identity != *identity {
            return Err(Error::WalletChanged);
        }
        Ok(())
    }

    fn stored_creation(&self, draft_id: &str, identity: &BsvHomeBsvMultisigSignerBinding) -> Result<StoredCreation, Error> {
        let creation = self.read()?.1.creations.get(&self.scope(draft_id)).cloned().ok_or(Error::MissingRecord)?;
        self.check_creation_identity(&creation, identity)?;
        Ok(creation)
    }

    async fn creation_wallet(&self, local: &Wallet, payload: &CreationPayload, wallet_id: &str) -> Result<BsvHomeBsvMultisigWallet, Error> {
        let remote: RemoteWallet = self.workflow_request(local, Target::Wallet { wallet_id: wallet_id.into() }).await?;
        rules::validate_wallet(&remote)?;
        if remote.id != wallet_id || !payload.matches(&remote) {
            return Err(Error::MissingSigner);
        }
        self.cache()?.wallets.insert(wallet_id.into(), remote);
        self.get_wallet(wallet_id.into())
    }

    async fn commit_creation(&self, local: &Wallet, draft_id: &str, mut creation: StoredCreation, remote: RemoteWallet) -> Result<BsvHomeBsvMultisigWallet, Error> {
        self.check_workflow_wallet(local).await?;
        let (revision, mut state) = self.read()?;
        let stored = state.creations.get(&self.scope(draft_id)).ok_or(Error::MissingRecord)?;
        if stored.payload != creation.payload || stored.baseline_wallet_ids != creation.baseline_wallet_ids || stored.wallet_id.as_ref().is_some_and(|id| *id != remote.id) {
            return Err(Error::ConcurrentUpdate);
        }
        creation.wallet_id = Some(remote.id.clone());
        state.member_names.insert(self.scope(&remote.id), creation.payload.member_names.clone());
        state.creations.insert(self.scope(draft_id), creation);
        self.save(revision, state).await?;
        self.check_workflow_wallet(local).await?;
        let id = remote.id.clone();
        self.cache()?.wallets.insert(id.clone(), remote);
        self.get_wallet(id)
    }
}
