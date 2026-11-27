use primitives::ValueAccess;
use serde_json::{Value, json};

use super::model::{StoredState, Task};
use super::rules::{hash, hex_text, task_matches, wallet};
use super::{BsvHomeMpcError, BsvHomeMpcWalletService};
use crate::services::portal::platform::{BsvHomePlatformAccountError, PlatformSessionLease};

#[uniffi::export]
impl BsvHomeMpcWalletService {
    pub async fn cancel_task(&self) -> Result<(), BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        let task = state.active_task.as_ref().ok_or(BsvHomeMpcError::OperationPending)?.clone();
        let current = self.task_status(&lease, &task).await?;
        let status = current.get_string("status")?;
        if let Some(active) = state.active_task.as_mut() {
            active.status = status.into();
        }
        self.save(&lease, &state)?;
        if !["PREPARED", "STARTED", "ACTIVE", "PROTOCOL", "SIGNING"].contains(&status) {
            return Err(BsvHomeMpcError::UnsupportedState { state: status.into() });
        }
        self.participant.close(task.wallet_id.clone()).await?;
        let signature = self.device.sign_message(format!("WowPay/MPC/cancel/v1\n{}", task.id)).await?;
        let response = self.request(&lease, "portal/mpc/tasks/cancel", json!({"id":task.id,"deviceSignature":signature})).await?;
        if response.get_string("status")? != "CANCELLED" {
            return Err(BsvHomeMpcError::OperationPending);
        }
        state.active_task = None;
        self.save(&lease, &state)
    }

    pub async fn reconcile(&self) -> Result<(), BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        self.reconcile_task(&lease, &mut state).await
    }
}

impl BsvHomeMpcWalletService {
    pub(super) async fn prepare_task(&self, lease: &PlatformSessionLease, state: &mut StoredState, body: Value, epoch: Option<u64>) -> Result<Task, BsvHomeMpcError> {
        if state.active_task.is_some() || state.pending_task_request.is_some() {
            return Err(BsvHomeMpcError::OperationPending);
        }
        state.pending_task_request = Some(body.clone());
        self.save(lease, state)?;
        let response = self.request(lease, "portal/mpc/tasks/prepare", body.clone()).await;
        let value = match response {
            Ok(value) => value,
            Err(error) => {
                if let BsvHomeMpcError::Platform {
                    error: BsvHomePlatformAccountError::Api { code },
                } = &error
                    && [
                        "MPC_UNAVAILABLE",
                        "PORTAL_AUTH_UNAVAILABLE",
                        "INVALID_REQUEST",
                        "MPC_WALLET_BUSY",
                        "MPC_CAPACITY_RETRY_LATER",
                        "RECOVERY_NOT_READY",
                        "RECOVERY_SAFETY_CONDITIONS_PENDING",
                    ]
                    .contains(&code.as_str())
                {
                    state.pending_task_request = None;
                    self.save(lease, state)?;
                }
                return Err(error);
            }
        };
        let task: Task = serde_json::from_value(value)?;
        let public_key = self.device.public_key().await?;
        task_matches(&task, &body, &public_key, epoch)?;
        state.active_task = Some(task.clone());
        state.pending_task_request = None;
        self.save(lease, state)?;
        Ok(task)
    }
    async fn start_task(&self, lease: &PlatformSessionLease, task: &Task) -> Result<Value, BsvHomeMpcError> {
        let grant = self.account.authorize(&task.action_hash).await?;
        self.account.require_lease(lease)?;
        let signature = self.device.sign_message(task.action_hash.clone()).await?;
        let response = self
            .request(lease, "portal/mpc/tasks/start", json!({"id":task.id,"grant":grant,"deviceSignature":signature}))
            .await?;
        self.frame(&response)
    }
    async fn exchange_frame(&self, lease: &PlatformSessionLease, task: &Task, phase: usize, message: Value) -> Result<Value, BsvHomeMpcError> {
        let raw = serde_json::to_string(&message)?;
        let signature = self
            .device
            .sign_message(format!("WowPay/MPC/exchange/v1\n{}\n{}\n{}", task.id, phase, hash(raw.as_bytes())))
            .await?;
        let response = self
            .request(
                lease,
                "portal/mpc/tasks/exchange",
                json!({"id":task.id,"phase":phase,"messageJson":raw,"deviceSignature":signature}),
            )
            .await?;
        self.frame(&response)
    }
    fn frame(&self, value: &Value) -> Result<Value, BsvHomeMpcError> {
        Ok(serde_json::from_str(value.get_string("messageJson")?)?)
    }
    pub(super) async fn grouped(&self, lease: &PlatformSessionLease, task: &Task, refresh: bool) -> Result<(), BsvHomeMpcError> {
        let b1 = self.start_task(lease, task).await?;
        let session_id = hash(format!("WowPay/MPC/native/v1\n{}\n0", task.id).as_bytes());
        let a1 = self
            .native(
                lease,
                &task.wallet_id,
                json!({"op":if refresh {"refresh"} else {"dkg"},"wallet_id":task.wallet_id,"epoch":task.epoch,"session_id":session_id}),
            )
            .await?;
        let a2 = self.native(lease, &task.wallet_id, json!([a1.at(0)?, b1])).await?;
        let b2 = self.exchange_frame(lease, task, 0, a1.at(1)?.clone()).await?;
        let a3 = self.native(lease, &task.wallet_id, Value::Null).await?;
        let b3 = self.exchange_frame(lease, task, 1, a2.clone()).await?;
        let mut final_message = vec![json!([a2.at(0)?, b2.at(0)?]), b2.at(1)?.clone(), b3.at(0)?.clone(), b3.at(1)?.clone()];
        if !refresh {
            final_message.push(json!({"1":a2.at(2)?,"2":b2.at(2)?}));
            final_message.push(json!({"1":a3.at(2)?,"2":b3.at(2)?}));
        }
        let local = self.native(lease, &task.wallet_id, Value::Array(final_message)).await?;
        let remote = self.exchange_frame(lease, task, 2, a3).await?;
        if local.get_string("public_key")? != remote.get_string("public_key")? {
            return Err(BsvHomeMpcError::IdentityMismatch);
        }
        Ok(())
    }
    pub(super) async fn signing(&self, lease: &PlatformSessionLease, task: &Task, digests: &[String]) -> Result<(), BsvHomeMpcError> {
        if digests.is_empty() || digests.len() > 100 || digests.iter().any(|digest| !hex_text(digest, 64)) {
            return Err(BsvHomeMpcError::InvalidInput);
        }
        let mut b1 = self.start_task(lease, task).await?;
        for (index, digest) in digests.iter().enumerate() {
            let session_id = hash(format!("WowPay/MPC/native/v1\n{}\n{}", task.id, index).as_bytes());
            let a1 = self
                .native(
                    lease,
                    &task.wallet_id,
                    json!({"op":if task.kind=="RECOVERY_BACKUP" {"proof"} else {"sign"},"epoch":task.epoch,"session_id":session_id,"digest":digest}),
                )
                .await?;
            let a2 = self.native(lease, &task.wallet_id, b1).await?;
            let b2 = self.exchange_frame(lease, task, index * 3, a1).await?;
            let a3 = self.native(lease, &task.wallet_id, b2).await?;
            let b3 = self.exchange_frame(lease, task, index * 3 + 1, a2).await?;
            self.native(lease, &task.wallet_id, json!([a3, b3])).await?;
            b1 = self.exchange_frame(lease, task, index * 3 + 2, a3).await?;
        }
        Ok(())
    }
    pub(super) async fn task_status(&self, lease: &PlatformSessionLease, task: &Task) -> Result<Value, BsvHomeMpcError> {
        let value = self.request(lease, "portal/mpc/tasks/status", json!({"id":task.id})).await?;
        if value.get_string("id")? != task.id || value.get_string("walletId")? != task.wallet_id || value.get_string("kind")? != task.kind {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        Ok(value)
    }
    pub(super) async fn reconcile_task(&self, lease: &PlatformSessionLease, state: &mut StoredState) -> Result<(), BsvHomeMpcError> {
        let Some(task) = state.active_task.clone() else {
            if state.pending_task_request.is_some() {
                return Err(BsvHomeMpcError::OperationPending);
            }
            return Ok(());
        };
        let status = self.task_status(lease, &task).await?;
        if let Some(active) = state.active_task.as_mut() {
            active.status = status.get_string("status")?.into();
        }
        self.save(lease, state)?;
        match status.get_string("status")? {
            "COMPLETE" => {}
            "CANCELLED" | "EXPIRED" | "FAILED" => {
                state.active_task = None;
                return self.save(lease, state);
            }
            _ => return Err(BsvHomeMpcError::OperationPending),
        }
        let result = async {
            match task.kind.as_str() {
                "SPEND" => self.retain_signed(lease, state, &task, &status).await?,
                "RECOVERY_BACKUP" => self.commit_recovery(lease, state, &task).await?,
                "CREATE" => {
                    if state.wallets.iter().all(|wallet| wallet.wallet_id != task.wallet_id) {
                        let value = self.request(lease, "portal/mpc/wallets/one", json!({"id":task.wallet_id})).await?;
                        let created = wallet(&value, "BSV".into(), false)?;
                        self.local_identity(lease, &created, false).await?;
                        state.wallets.push(created);
                    }
                }
                "RECOVER" => {
                    let previous = state.recovery.as_ref().ok_or(BsvHomeMpcError::InvalidResponse)?.clone();
                    let response = self.request(lease, "portal/mpc/recovery/status", json!({"id":previous.id})).await?;
                    let recovery = self.recovery_record(&response, &task.wallet_id)?;
                    if recovery.id != previous.id || recovery.status != "AWAIT_BACKUP" || recovery.source_epoch != task.epoch {
                        return Err(BsvHomeMpcError::OperationPending);
                    }
                    state.recovery = Some(recovery);
                }
                "BACKUP" | "PAYMAIL" => {}
                _ => return Err(BsvHomeMpcError::UnsupportedState { state: task.kind.clone() }),
            }
            state.active_task = None;
            self.save(lease, state)
        }
        .await;
        self.participant.close(task.wallet_id).await?;
        result
    }
}
