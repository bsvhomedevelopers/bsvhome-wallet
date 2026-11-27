use std::sync::{
    Arc, Mutex as NativeMutex,
    atomic::{AtomicU64, Ordering},
};

use futures::lock::Mutex;
use primitives::ValueAccess;
use serde_json::{Value, json};

use super::BsvHomeMpcError;
use super::model::{
    BackupProof, BsvHomeMpcBackupMaterial, BsvHomeMpcDeviceProof, BsvHomeMpcParticipant, BsvHomeMpcPendingPreparation, BsvHomeMpcPendingTask, BsvHomeMpcPendingTaskAction,
    BsvHomeMpcPendingTaskState, BsvHomeMpcTransfer, BsvHomeMpcWallet, StoredState,
};
use super::rules::{MAX_DOCUMENT_BYTES, PROTOCOL, balance, hash, hex_text, identifier, number, optional_text, transfer_actions, wallet, wallet_actions};
use crate::services::portal::BsvHomePaymailDestinationService;
use crate::services::portal::platform::{BsvHomePlatformAccountError, BsvHomePlatformAccountService, PlatformSessionLease};
use crate::services::preferences::BsvHomeSecureStore;

#[derive(uniffi::Object)]
pub struct BsvHomeMpcWalletService {
    pub(super) account: Arc<BsvHomePlatformAccountService>,
    pub(super) participant: Arc<dyn BsvHomeMpcParticipant>,
    pub(super) device: Arc<dyn BsvHomeMpcDeviceProof>,
    pub(super) store: Arc<dyn BsvHomeSecureStore>,
    pub(super) paymail: Arc<BsvHomePaymailDestinationService>,
    pub(super) operation: Mutex<()>,
    active_native: NativeMutex<Option<(String, u64)>>,
    native_generation: AtomicU64,
}

#[uniffi::export]
impl BsvHomeMpcWalletService {
    #[uniffi::constructor]
    pub fn new(
        account: Arc<BsvHomePlatformAccountService>,
        participant: Arc<dyn BsvHomeMpcParticipant>,
        device: Arc<dyn BsvHomeMpcDeviceProof>,
        store: Arc<dyn BsvHomeSecureStore>,
        paymail: Arc<BsvHomePaymailDestinationService>,
    ) -> Self {
        Self {
            account,
            participant,
            device,
            store,
            paymail,
            operation: Mutex::new(()),
            active_native: NativeMutex::new(None),
            native_generation: AtomicU64::new(0),
        }
    }

    pub fn local_wallets(&self) -> Result<Vec<BsvHomeMpcWallet>, BsvHomeMpcError> {
        let lease = self.account.session_lease()?;
        let state = self.state(&lease)?;
        Ok(state
            .wallets
            .iter()
            .map(|known| {
                let mut wallet = known.clone();
                if let Some(balance) = wallet.balance.as_mut()
                    && balance.state == "CURRENT"
                {
                    balance.state = "STALE".into();
                }
                wallet.actions = wallet_actions(
                    &wallet,
                    state
                        .recovery
                        .as_ref()
                        .filter(|recovery| recovery.wallet_id == wallet.wallet_id)
                        .map(|recovery| recovery.status.as_str()),
                );
                if state.active_task.as_ref().is_some_and(|task| task.wallet_id == wallet.wallet_id) {
                    wallet.actions.clear();
                }
                wallet
            })
            .collect())
    }

    pub fn pending_task(&self) -> Result<Option<BsvHomeMpcPendingTask>, BsvHomeMpcError> {
        let lease = self.account.session_lease()?;
        let state = self.state(&lease)?;
        if let Some(task) = state.active_task {
            let mut actions = vec![BsvHomeMpcPendingTaskAction::Reconcile];
            if ["PREPARED", "STARTED", "ACTIVE", "PROTOCOL", "SIGNING"].contains(&task.status.as_str()) {
                actions.push(BsvHomeMpcPendingTaskAction::Cancel);
            }
            return Ok(Some(BsvHomeMpcPendingTask {
                kind: task.kind,
                id: Some(task.id),
                wallet_id: Some(task.wallet_id),
                status: Some(task.status),
                state: BsvHomeMpcPendingTaskState::KnownTask,
                actions,
            }));
        }
        state
            .pending_task_request
            .map(|request| {
                Ok(BsvHomeMpcPendingTask {
                    kind: request.get_string("kind")?.into(),
                    id: None,
                    wallet_id: optional_text(&request, "walletId")?,
                    status: None,
                    state: BsvHomeMpcPendingTaskState::PrepareReceiptUnknown,
                    actions: vec![BsvHomeMpcPendingTaskAction::Reconcile],
                })
            })
            .transpose()
    }

    pub fn pending_preparation(&self) -> Result<Option<BsvHomeMpcPendingPreparation>, BsvHomeMpcError> {
        let lease = self.account.session_lease()?;
        Ok(self.state(&lease)?.pending_preparation)
    }

    pub fn pending_transfers(&self, wallet_id: String) -> Result<Vec<BsvHomeMpcTransfer>, BsvHomeMpcError> {
        let lease = self.account.session_lease()?;
        let state = self.state(&lease)?;
        self.known_wallet(&state, &wallet_id)?;
        Ok(state
            .transfers
            .into_iter()
            .filter(|saved| saved.transfer.wallet_id == wallet_id)
            .map(|mut saved| {
                saved.transfer.actions = transfer_actions(&saved.transfer.status, saved.signed_transaction.is_some());
                saved.transfer
            })
            .collect())
    }

    pub async fn suspend(&self) -> Result<(), BsvHomeMpcError> {
        let activity = self.active_native.lock().map_err(|_| BsvHomeMpcError::NativeUnavailable)?.clone();
        if let Some((wallet_id, generation)) = activity {
            self.participant.close(wallet_id.clone()).await?;
            let mut current = self.active_native.lock().map_err(|_| BsvHomeMpcError::NativeUnavailable)?;
            if current.as_ref() == Some(&(wallet_id, generation)) {
                *current = None;
            }
        }
        Ok(())
    }

    pub async fn list_wallets(&self) -> Result<Vec<BsvHomeMpcWallet>, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        self.reconcile_task(&lease, &mut state).await?;
        for index in 0..state.wallets.len() {
            let known = state.wallets[index].clone();
            let mut current = self.public_wallet(&lease, &known).await?;
            if current.epoch != known.epoch {
                return Err(BsvHomeMpcError::EpochMismatch);
            }
            let result = self.request(&lease, "portal/chain/wallets/balance", json!({"id":known.wallet_id})).await;
            match result {
                Ok(value) => current.balance = Some(balance(&value)?),
                Err(BsvHomeMpcError::Platform {
                    error: BsvHomePlatformAccountError::Api { code },
                }) if ["CHAIN_DISABLED", "CHAIN_UNAVAILABLE", "CHAIN_HISTORY_NOT_INDEXED"].contains(&code.as_str()) => {
                    current.balance = known.balance.map(|mut balance| {
                        balance.state = "UNAVAILABLE".into();
                        balance
                    });
                }
                Err(error) => return Err(error),
            }
            current.actions = wallet_actions(
                &current,
                state
                    .recovery
                    .as_ref()
                    .filter(|recovery| recovery.wallet_id == current.wallet_id)
                    .map(|recovery| recovery.status.as_str()),
            );
            state.wallets[index] = current;
        }
        self.save(&lease, &state)?;
        Ok(state.wallets)
    }

    pub async fn create_wallet(&self, name: String) -> Result<BsvHomeMpcWallet, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let name = name.trim().to_string();
        if name.is_empty() || name.chars().count() > 30 {
            return Err(BsvHomeMpcError::InvalidInput);
        }
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        self.reconcile_task(&lease, &mut state).await?;
        let public_key = self.device.public_key().await?;
        let task = self
            .prepare_task(&lease, &mut state, json!({"kind":"CREATE","devicePublicKey":public_key}), Some(0))
            .await?;
        let result = async {
            self.grouped(&lease, &task, false).await?;
            let value = self.request(&lease, "portal/mpc/wallets/one", json!({"id":task.wallet_id})).await?;
            let mut created = wallet(&value, name, false)?;
            created.actions = wallet_actions(&created, None);
            self.local_identity(&lease, &created, false).await?;
            if created.wallet_id != task.wallet_id || created.epoch != task.epoch || created.status != "AWAIT_BACKUP" {
                return Err(BsvHomeMpcError::InvalidResponse);
            }
            state.wallets.push(created.clone());
            state.active_task = None;
            self.save(&lease, &state)?;
            Ok(created)
        }
        .await;
        self.participant.close(task.wallet_id).await?;
        result
    }

    pub async fn export_backup(&self, wallet_id: String) -> Result<BsvHomeMpcBackupMaterial, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        let known = self.known_wallet(&state, &wallet_id)?.clone();
        let result = async {
            let value = self.native(&lease, &wallet_id, json!({"op":"backup"})).await?;
            if value.get_value("backup_verified")? != &Value::Bool(true) {
                return Err(BsvHomeMpcError::BackupMismatch);
            }
            let context = value.get_value("context")?;
            let epoch = number(context.get_value("epoch")?)?;
            let allowed_epoch = if known.restored
                && state
                    .recovery
                    .as_ref()
                    .is_some_and(|recovery| recovery.wallet_id == wallet_id && recovery.status == "AWAIT_BACKUP")
            {
                known.epoch.checked_add(1).ok_or(BsvHomeMpcError::EpochMismatch)?
            } else {
                known.epoch
            };
            self.backup_context(context, &known, allowed_epoch)?;
            if value.get_value("backup")?.get_value("context")? != context {
                return Err(BsvHomeMpcError::BackupMismatch);
            }
            let document = serde_json::to_string(value.get_value("backup")?)?;
            let recovery_key = value.get_string("recovery_key")?.to_string();
            if !hex_text(&recovery_key, 64) || document.len() > MAX_DOCUMENT_BYTES {
                return Err(BsvHomeMpcError::BackupMismatch);
            }
            let commitment = hash(document.as_bytes());
            state.backup = Some(BackupProof {
                wallet_id: wallet_id.clone(),
                epoch,
                commitment: commitment.clone(),
            });
            self.save(&lease, &state)?;
            Ok(BsvHomeMpcBackupMaterial {
                wallet_id: wallet_id.clone(),
                owner_id: lease.user_id.clone(),
                environment: self.account.scope().into(),
                epoch,
                document,
                recovery_key,
                commitment,
            })
        }
        .await;
        self.participant.close(wallet_id).await?;
        result
    }

    pub async fn confirm_backup(&self, material: BsvHomeMpcBackupMaterial, recovery_key_confirmation: String, file_saved: bool) -> Result<BsvHomeMpcWallet, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        let known = self.known_wallet(&state, &material.wallet_id)?.clone();
        let backup = state.backup.as_ref().ok_or(BsvHomeMpcError::BackupMismatch)?;
        if !file_saved
            || material.environment != self.account.scope()
            || material.owner_id != lease.user_id
            || material.wallet_id != backup.wallet_id
            || material.epoch != backup.epoch
            || material.commitment != backup.commitment
            || hash(material.document.as_bytes()) != material.commitment
            || !hex_text(&material.recovery_key, 64)
            || recovery_key_confirmation != material.recovery_key
        {
            return Err(BsvHomeMpcError::BackupMismatch);
        }
        let document: Value = serde_json::from_str(&material.document)?;
        self.backup_context(document.get_value("context")?, &known, material.epoch)?;
        let recovery = state
            .recovery
            .as_ref()
            .filter(|recovery| recovery.wallet_id == known.wallet_id && recovery.status == "AWAIT_BACKUP")
            .cloned();
        let request = match recovery {
            Some(ref recovery) => json!({"kind":"RECOVERY_BACKUP","walletId":known.wallet_id,"recoveryId":recovery.id,"backupCommitment":material.commitment}),
            None => json!({"kind":"BACKUP","walletId":known.wallet_id,"backupCommitment":material.commitment}),
        };
        let result = async {
            let verified = self
                .native(
                    &lease,
                    &known.wallet_id,
                    json!({"op":"verify_backup","backup":document,"recovery_key":recovery_key_confirmation,"expected":document.get_value("context")?}),
                )
                .await?;
            if verified.get_value("verified")? != &Value::Bool(true) || verified.get_value("context")? != document.get_value("context")? {
                return Err(BsvHomeMpcError::BackupMismatch);
            }
            let task = self.prepare_task(&lease, &mut state, request, Some(material.epoch)).await?;
            let digest = hash(format!("WowPay/MPC/backup-confirm/v1\n{}", task.action_hash).as_bytes());
            self.signing(&lease, &task, &[digest]).await?;
            let complete = self.task_status(&lease, &task).await?;
            if complete.get_string("status")? != "COMPLETE" {
                return Err(BsvHomeMpcError::OperationPending);
            }
            if recovery.is_some() {
                self.commit_recovery(&lease, &mut state, &task).await?;
            }
            let mut active = self.public_wallet(&lease, &known).await?;
            if active.status != "ACTIVE" || active.epoch != material.epoch {
                return Err(BsvHomeMpcError::EpochMismatch);
            }
            active.restored = false;
            self.local_identity(&lease, &active, true).await?;
            self.replace_wallet(&mut state, active.clone());
            state.active_task = None;
            state.backup = None;
            self.save(&lease, &state)?;
            Ok(active)
        }
        .await;
        self.participant.close(known.wallet_id).await?;
        result
    }

    pub async fn refresh_wallet(&self, wallet_id: String) -> Result<BsvHomeMpcWallet, BsvHomeMpcError> {
        let lease = self.account.session_lease()?;
        identifier(&wallet_id)?;
        self.request(&lease, "portal/chain/wallets/sync", json!({"id":wallet_id})).await?;
        self.list_wallets()
            .await?
            .into_iter()
            .find(|wallet| wallet.wallet_id == wallet_id)
            .ok_or(BsvHomeMpcError::WalletNotFound)
    }
}

impl BsvHomeMpcWalletService {
    pub(super) fn state(&self, lease: &PlatformSessionLease) -> Result<StoredState, BsvHomeMpcError> {
        self.account.require_lease(lease)?;
        let saved = self.store.get(self.state_key(lease))?;
        match saved {
            None => Ok(StoredState::default()),
            Some(value) if value.len() <= MAX_DOCUMENT_BYTES => Ok(serde_json::from_str(&value)?),
            Some(_) => Err(BsvHomeMpcError::SecureStorageUnavailable),
        }
    }
    pub(super) fn save(&self, lease: &PlatformSessionLease, state: &StoredState) -> Result<(), BsvHomeMpcError> {
        self.account.require_lease(lease)?;
        let raw = serde_json::to_string(state)?;
        if raw.len() > MAX_DOCUMENT_BYTES {
            return Err(BsvHomeMpcError::SecureStorageUnavailable);
        }
        Ok(self.store.set(self.state_key(lease), raw)?)
    }
    fn state_key(&self, lease: &PlatformSessionLease) -> String {
        format!("mpc.{}", hash(format!("{}\n{}", self.account.scope(), lease.user_id).as_bytes()))
    }
    pub(super) fn known_wallet<'a>(&self, state: &'a StoredState, wallet_id: &str) -> Result<&'a BsvHomeMpcWallet, BsvHomeMpcError> {
        state.wallets.iter().find(|wallet| wallet.wallet_id == wallet_id).ok_or(BsvHomeMpcError::WalletNotFound)
    }
    pub(super) fn replace_wallet(&self, state: &mut StoredState, wallet: BsvHomeMpcWallet) {
        state.wallets.retain(|known| known.wallet_id != wallet.wallet_id);
        let mut wallet = wallet;
        wallet.actions = wallet_actions(
            &wallet,
            state
                .recovery
                .as_ref()
                .filter(|recovery| recovery.wallet_id == wallet.wallet_id)
                .map(|recovery| recovery.status.as_str()),
        );
        state.wallets.push(wallet);
    }
    pub(super) async fn request(&self, lease: &PlatformSessionLease, path: &str, body: Value) -> Result<Value, BsvHomeMpcError> {
        self.account.require_lease(lease)?;
        let result = self.account.authenticated_json(path, body).await?;
        self.account.require_lease(lease)?;
        Ok(result)
    }
    pub(super) async fn native(&self, lease: &PlatformSessionLease, wallet_id: &str, request: Value) -> Result<Value, BsvHomeMpcError> {
        self.account.require_lease(lease)?;
        let generation = self.native_generation.fetch_add(1, Ordering::Relaxed);
        *self.active_native.lock().map_err(|_| BsvHomeMpcError::NativeUnavailable)? = Some((wallet_id.into(), generation));
        let response = self.participant.exchange(wallet_id.into(), serde_json::to_string(&request)?).await?;
        self.account.require_lease(lease)?;
        if response.len() > MAX_DOCUMENT_BYTES {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        Ok(serde_json::from_str(&response)?)
    }
    pub(super) async fn public_wallet(&self, lease: &PlatformSessionLease, known: &BsvHomeMpcWallet) -> Result<BsvHomeMpcWallet, BsvHomeMpcError> {
        let value = self.request(lease, "portal/mpc/wallets/one", json!({"id":known.wallet_id})).await?;
        let current = wallet(&value, known.name.clone(), known.restored)?;
        if current.wallet_id != known.wallet_id || current.public_key != known.public_key || current.address != known.address {
            return Err(BsvHomeMpcError::IdentityMismatch);
        }
        Ok(current)
    }
    pub(super) async fn local_identity(&self, lease: &PlatformSessionLease, known: &BsvHomeMpcWallet, require_active: bool) -> Result<(), BsvHomeMpcError> {
        let value = self.native(lease, &known.wallet_id, json!({"op":"status"})).await?;
        self.backup_context(value.get_value("context")?, known, known.epoch)?;
        if value.get_string("address")? != known.address {
            return Err(BsvHomeMpcError::IdentityMismatch);
        }
        if require_active && (known.restored || known.status != "ACTIVE" || value.get_string("status")? != "ACTIVE") {
            return Err(BsvHomeMpcError::OperationPending);
        }
        Ok(())
    }
    pub(super) fn backup_context(&self, value: &Value, known: &BsvHomeMpcWallet, epoch: u64) -> Result<(), BsvHomeMpcError> {
        if value.get_string("wallet_id")? != known.wallet_id
            || value.get_string("public_key")? != known.public_key
            || value.get_string("protocol")? != PROTOCOL
            || number(value.get_value("format")?)? != 1
        {
            return Err(BsvHomeMpcError::BackupMismatch);
        }
        if number(value.get_value("epoch")?)? != epoch {
            return Err(BsvHomeMpcError::EpochMismatch);
        }
        Ok(())
    }
}
