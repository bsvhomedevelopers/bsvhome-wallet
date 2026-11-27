use primitives::ValueAccess;
use serde_json::{Value, json};

use super::model::{BsvHomeMpcRecovery, BsvHomeMpcRecoveryInformation, BsvHomeMpcWallet, StoredState, Task};
use super::rules::{MAX_DOCUMENT_BYTES, hash, hex_text, identifier, number, optional_text, recovery_actions, wallet, wallet_actions};
use super::{BsvHomeMpcError, BsvHomeMpcWalletService};
use crate::services::portal::platform::PlatformSessionLease;

#[uniffi::export]
impl BsvHomeMpcWalletService {
    pub async fn restore_backup(&self, document: String, recovery_key: String, name: String) -> Result<BsvHomeMpcWallet, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let name = name.trim().to_string();
        if document.len() > MAX_DOCUMENT_BYTES || !hex_text(&recovery_key, 64) || name.is_empty() || name.chars().count() > 30 {
            return Err(BsvHomeMpcError::InvalidInput);
        }
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        if state.active_task.is_some() || state.pending_task_request.is_some() {
            return Err(BsvHomeMpcError::OperationPending);
        }
        let backup: Value = serde_json::from_str(&document)?;
        let context = backup.get_value("context")?;
        let wallet_id = context.get_string("wallet_id")?.to_string();
        identifier(&wallet_id)?;
        let value = self.request(&lease, "portal/mpc/wallets/one", json!({"id":wallet_id})).await?;
        let mut restored = wallet(&value, name, true)?;
        restored.actions = wallet_actions(&restored, None);
        self.backup_context(context, &restored, restored.epoch)?;
        let result = async {
            self.native(&lease, &wallet_id, json!({"op":"restore","backup":backup,"recovery_key":recovery_key,"expected":context}))
                .await?;
            self.local_identity(&lease, &restored, false).await?;
            self.replace_wallet(&mut state, restored.clone());
            state.backup = None;
            self.save(&lease, &state)?;
            Ok(restored)
        }
        .await;
        self.participant.close(wallet_id).await?;
        result
    }

    pub async fn start_recovery(&self, wallet_id: String) -> Result<BsvHomeMpcRecovery, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        let known = self.known_wallet(&state, &wallet_id)?;
        if !known.restored || state.active_task.is_some() {
            return Err(BsvHomeMpcError::OperationPending);
        }
        let target = self.device.public_key().await?;
        let challenge = self
            .request(&lease, "portal/mpc/recovery/challenge", json!({"id":wallet_id,"targetDevicePublicKey":target}))
            .await?;
        let action = challenge.get_string("actionHash")?;
        if !hex_text(action, 64) {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        let grant = self.account.authorize(action).await?;
        let signature = self.device.sign_message(action.into()).await?;
        let value = self
            .request(
                &lease,
                "portal/mpc/recovery/start",
                json!({"id":wallet_id,"targetDevicePublicKey":target,"grant":grant,"deviceSignature":signature}),
            )
            .await?;
        let recovery = self.recovery_record(&value, &wallet_id)?;
        if recovery.source_epoch != known.epoch {
            return Err(BsvHomeMpcError::EpochMismatch);
        }
        state.recovery = Some(recovery.clone());
        self.save(&lease, &state)?;
        Ok(recovery)
    }

    pub async fn recovery_status(&self, id: String) -> Result<BsvHomeMpcRecovery, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        let previous = state
            .recovery
            .as_ref()
            .filter(|recovery| recovery.id == id)
            .ok_or(BsvHomeMpcError::OperationPending)?
            .clone();
        let value = self.request(&lease, "portal/mpc/recovery/status", json!({"id":id})).await?;
        let recovery = self.recovery_record(&value, &previous.wallet_id)?;
        if recovery.id != id || recovery.source_epoch != previous.source_epoch {
            return Err(BsvHomeMpcError::EpochMismatch);
        }
        if recovery.status == "CANCELLED" || recovery.status == "EXPIRED" || recovery.status == "REJECTED" {
            self.discard_recovery_candidate(&lease, &state, &recovery.wallet_id).await?;
            state.backup = None;
        }
        state.recovery = Some(recovery.clone());
        self.save(&lease, &state)?;
        Ok(recovery)
    }

    pub async fn current_recovery(&self) -> Result<Option<BsvHomeMpcRecovery>, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        Ok(self.state(&lease)?.recovery)
    }

    pub async fn refresh_recovery(&self, id: String) -> Result<BsvHomeMpcRecovery, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        self.reconcile_task(&lease, &mut state).await?;
        let previous = state
            .recovery
            .as_ref()
            .filter(|recovery| recovery.id == id)
            .ok_or(BsvHomeMpcError::OperationPending)?
            .clone();
        let status = self.request(&lease, "portal/mpc/recovery/status", json!({"id":id})).await?;
        let recovery = self.recovery_record(&status, &previous.wallet_id)?;
        if recovery.id != id || recovery.source_epoch != previous.source_epoch || recovery.status != "WAITING" {
            return Err(BsvHomeMpcError::OperationPending);
        }
        let known = self.known_wallet(&state, &recovery.wallet_id)?.clone();
        if !known.restored || known.epoch != recovery.source_epoch {
            return Err(BsvHomeMpcError::EpochMismatch);
        }
        let task = self
            .prepare_task(&lease, &mut state, json!({"kind":"RECOVER","walletId":known.wallet_id,"recoveryId":id}), Some(known.epoch))
            .await?;
        let result = async {
            self.local_identity(&lease, &known, false).await?;
            self.grouped(&lease, &task, true).await?;
            let status = self.request(&lease, "portal/mpc/recovery/status", json!({"id":id})).await?;
            let next = self.recovery_record(&status, &known.wallet_id)?;
            if next.id != id || next.status != "AWAIT_BACKUP" || next.source_epoch != known.epoch {
                return Err(BsvHomeMpcError::OperationPending);
            }
            self.local_identity(&lease, &known, false).await?;
            state.recovery = Some(next.clone());
            state.active_task = None;
            state.backup = None;
            self.save(&lease, &state)?;
            Ok(next)
        }
        .await;
        self.participant.close(known.wallet_id).await?;
        result
    }

    pub async fn cancel_recovery(&self, id: String) -> Result<BsvHomeMpcRecovery, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        let previous = state
            .recovery
            .as_ref()
            .filter(|recovery| recovery.id == id)
            .ok_or(BsvHomeMpcError::OperationPending)?
            .clone();
        if state.active_task.is_some() || state.pending_task_request.is_some() {
            return Err(BsvHomeMpcError::OperationPending);
        }
        let status = self.request(&lease, "portal/mpc/recovery/status", json!({"id":id})).await?;
        let current = self.recovery_record(&status, &previous.wallet_id)?;
        let cancelled = match current.status.as_str() {
            "CANCELLED" => current,
            "WAIT_REVIEW" | "NEEDS_INFO" | "WAITING" | "PROTOCOL" | "AWAIT_BACKUP" => {
                let action = hash(format!("WowPay/RecoveryB/cancel/v1\n{id}").as_bytes());
                let grant = self.account.authorize(&action).await?;
                let value = self.request(&lease, "portal/mpc/recovery/cancel", json!({"id":id,"grant":grant})).await?;
                self.recovery_record(&value, &previous.wallet_id)?
            }
            _ => return Err(BsvHomeMpcError::UnsupportedState { state: current.status }),
        };
        if cancelled.id != id || cancelled.source_epoch != previous.source_epoch || cancelled.status != "CANCELLED" {
            return Err(BsvHomeMpcError::OperationPending);
        }
        self.discard_recovery_candidate(&lease, &state, &previous.wallet_id).await?;
        state.recovery = Some(cancelled.clone());
        state.backup = None;
        self.save(&lease, &state)?;
        Ok(cancelled)
    }

    pub async fn recovery_timeline(&self, id: String) -> Result<BsvHomeMpcRecoveryInformation, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        identifier(&id)?;
        let lease = self.account.session_lease()?;
        let state = self.state(&lease)?;
        let known = state.recovery.as_ref().filter(|recovery| recovery.id == id).ok_or(BsvHomeMpcError::OperationPending)?;
        self.recovery_information(&lease, known).await
    }

    pub async fn submit_recovery_information(&self, id: String, reason: String) -> Result<BsvHomeMpcRecovery, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        let recovery = state
            .recovery
            .as_ref()
            .filter(|recovery| recovery.id == id && recovery.status == "NEEDS_INFO")
            .ok_or(BsvHomeMpcError::OperationPending)?
            .clone();
        let reason = reason.trim();
        if !safe_reason(reason) {
            return Err(BsvHomeMpcError::InvalidInput);
        }
        let current = self.recovery_information(&lease, &recovery).await?;
        if !current.can_supply_information {
            return Err(BsvHomeMpcError::OperationPending);
        }
        let challenge = self.request(&lease, "portal/mpc/recovery/information/challenge", json!({"id":id,"reason":reason})).await?;
        let action = challenge.get_string("actionHash")?;
        if !hex_text(action, 64) {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        let grant = self.account.authorize(action).await?;
        let value = self
            .request(&lease, "portal/mpc/recovery/information/submit", json!({"id":id,"reason":reason,"grant":grant}))
            .await?;
        if value.get_string("id")? != id || value.get_string("walletId")? != recovery.wallet_id || value.get_string("status")? != "WAIT_REVIEW" {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        let observed = self.recovery_information(&lease, &recovery).await?;
        let submitted = BsvHomeMpcRecovery {
            id: recovery.id,
            wallet_id: recovery.wallet_id,
            source_epoch: recovery.source_epoch,
            actions: recovery_actions(&observed.status),
            status: observed.status,
            review_note: observed.review_note,
            not_before_at: observed.not_before_at,
        };
        state.recovery = Some(submitted.clone());
        self.save(&lease, &state)?;
        Ok(submitted)
    }
}

impl BsvHomeMpcWalletService {
    async fn recovery_information(&self, lease: &PlatformSessionLease, known: &BsvHomeMpcRecovery) -> Result<BsvHomeMpcRecoveryInformation, BsvHomeMpcError> {
        let response = self.request(lease, "portal/mpc/recovery/timeline", json!({"id":known.id})).await?;
        recovery_information(&response, &known.id, &known.wallet_id, &lease.user_id)
    }
    async fn discard_recovery_candidate(&self, lease: &PlatformSessionLease, state: &StoredState, wallet_id: &str) -> Result<(), BsvHomeMpcError> {
        let known = self.known_wallet(state, wallet_id)?;
        self.participant.close(wallet_id.into()).await?;
        let result = async {
            let local = self.native(lease, wallet_id, json!({"op":"status"})).await?;
            self.backup_context(local.get_value("context")?, known, known.epoch)?;
            if local.get_string("status")? == "REFRESH_PENDING" {
                self.native(lease, wallet_id, json!({"op":"abort_refresh","epoch":known.epoch})).await?;
            }
            Ok(())
        }
        .await;
        self.participant.close(wallet_id.into()).await?;
        result
    }
    pub(super) fn recovery_record(&self, value: &Value, wallet_id: &str) -> Result<BsvHomeMpcRecovery, BsvHomeMpcError> {
        if value.get_string("walletId")? != wallet_id {
            return Err(BsvHomeMpcError::IdentityMismatch);
        }
        let id = value.get_string("id")?;
        identifier(id)?;
        Ok(BsvHomeMpcRecovery {
            id: id.into(),
            wallet_id: wallet_id.into(),
            status: value.get_string("status")?.into(),
            source_epoch: number(value.get_value("sourceEpoch")?)?,
            not_before_at: optional_text(value, "notBeforeAt")?,
            review_note: optional_text(value, "reviewNote")?,
            actions: recovery_actions(value.get_string("status")?),
        })
    }
    pub(super) async fn commit_recovery(&self, lease: &PlatformSessionLease, state: &mut StoredState, task: &Task) -> Result<(), BsvHomeMpcError> {
        let previous = self.known_wallet(state, &task.wallet_id)?.clone();
        let remote = self.public_wallet(lease, &previous).await?;
        if remote.epoch != task.epoch || remote.status != "ACTIVE" {
            return Err(BsvHomeMpcError::EpochMismatch);
        }
        let local = self.native(lease, &task.wallet_id, json!({"op":"status"})).await?;
        let local_epoch = number(local.get_value("context")?.get_value("epoch")?)?;
        if local.get_string("status")? == "REFRESH_PENDING" && local_epoch.checked_add(1) == Some(task.epoch) {
            self.native(lease, &task.wallet_id, json!({"op":"commit","epoch":task.epoch,"backup_confirmed":true}))
                .await?;
        } else if local_epoch != task.epoch || local.get_string("status")? != "ACTIVE" {
            return Err(BsvHomeMpcError::EpochMismatch);
        }
        let mut active = remote;
        active.restored = false;
        self.local_identity(lease, &active, true).await?;
        let recovery_id = task.context.get_string("recoveryId")?;
        let response = self.request(lease, "portal/mpc/recovery/status", json!({"id":recovery_id})).await?;
        let recovery = self.recovery_record(&response, &task.wallet_id)?;
        if recovery.id != recovery_id || recovery.status != "COMPLETE" {
            return Err(BsvHomeMpcError::OperationPending);
        }
        self.replace_wallet(state, active);
        state.recovery = Some(recovery);
        state.backup = None;
        self.save(lease, state)
    }
}

pub(super) fn recovery_information(value: &Value, id: &str, wallet_id: &str, owner: &str) -> Result<BsvHomeMpcRecoveryInformation, BsvHomeMpcError> {
    let returned_owner = match value.get_value("userId")? {
        Value::String(owner) => owner.clone(),
        Value::Number(owner) => owner.as_u64().ok_or(BsvHomeMpcError::InvalidResponse)?.to_string(),
        _ => return Err(BsvHomeMpcError::InvalidResponse),
    };
    if returned_owner != owner {
        return Err(BsvHomeMpcError::IdentityMismatch);
    }
    let mut information: BsvHomeMpcRecoveryInformation = serde_json::from_value(value.clone())?;
    if information.id != id || information.wallet_id != wallet_id {
        return Err(BsvHomeMpcError::IdentityMismatch);
    }
    if information.status.is_empty() || information.notification_status.is_empty() || information.events.iter().any(|event| event.event_type.is_empty() || event.time.is_empty()) {
        return Err(BsvHomeMpcError::InvalidResponse);
    }
    information.can_supply_information = information.status == "NEEDS_INFO";
    Ok(information)
}

pub(super) fn safe_reason(reason: &str) -> bool {
    if reason.trim().is_empty() || reason.encode_utf16().count() > 500 {
        return false;
    }
    let lowercase = reason.to_ascii_lowercase();
    if lowercase.as_bytes().windows(64).any(|window| window.iter().all(u8::is_ascii_hexdigit))
        || lowercase
            .match_indices("xprv")
            .any(|(index, _)| lowercase.as_bytes().get(index + 4).is_some_and(u8::is_ascii_alphanumeric))
        || lowercase
            .lines()
            .any(|line| line.find("-----begin ").is_some_and(|index| line[index + 11..].contains("private key")))
    {
        return false;
    }
    for field in ["\"recovery_key\"", "\"ciphertext\"", "\"party\"", "\"mnemonic\"", "\"seed\""] {
        if lowercase
            .match_indices(field)
            .any(|(index, _)| lowercase[index + field.len()..].trim_start().starts_with(':'))
        {
            return false;
        }
    }
    let words = lowercase.split_whitespace().collect::<Vec<_>>();
    if [12, 15, 18, 21, 24].contains(&words.len()) && words.iter().all(|word| !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_lowercase())) {
        return false;
    }
    true
}
