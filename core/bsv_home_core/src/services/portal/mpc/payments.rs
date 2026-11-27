use bsv_home_bsv::{BsvAddress, mpc};
use primitives::{Address, ValueAccess};
use serde_json::{Value, json};
use uuid::Uuid;

use super::model::{BsvHomeMpcHistoryPage, BsvHomeMpcPendingPreparation, BsvHomeMpcTransfer, StoredState, StoredTransfer, Task};
use super::rules::{MAX_MONEY, bsv_amount, hex_text, history, identifier, number, optional_number, optional_text, review, transfer_actions, transfer_unchanged};
use super::{BsvHomeMpcError, BsvHomeMpcWalletService};
use crate::services::portal::platform::{BsvHomePlatformAccountError, PlatformSessionLease, public_code};

#[uniffi::export]
impl BsvHomeMpcWalletService {
    pub async fn prepare_transfer_bsv(&self, wallet_id: String, recipient: String, amount_bsv: String, idempotency_key: String) -> Result<BsvHomeMpcTransfer, BsvHomeMpcError> {
        self.prepare_transfer(wallet_id, recipient, bsv_amount(&amount_bsv)?, idempotency_key).await
    }

    pub async fn prepare_transfer(&self, wallet_id: String, recipient: String, amount: u64, idempotency_key: String) -> Result<BsvHomeMpcTransfer, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        if (!recipient.contains('@') && !BsvAddress::is_valid(&recipient)) || amount == 0 || amount > MAX_MONEY || Uuid::parse_str(&idempotency_key).is_err() {
            return Err(BsvHomeMpcError::InvalidInput);
        }
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        self.reconcile_task(&lease, &mut state).await?;
        let known = self.public_wallet(&lease, self.known_wallet(&state, &wallet_id)?).await?;
        let result = async {
            self.local_identity(&lease, &known, true).await?;
            let continuing_preparation = state.pending_preparation.is_some();
            let pending = match &state.pending_preparation {
                Some(pending) if pending.wallet_id == wallet_id && pending.recipient == recipient && pending.amount == amount => pending.clone(),
                Some(_) => return Err(BsvHomeMpcError::OperationPending),
                None => {
                    let resolved = self.resolved_recipient(&lease, &known, &recipient, amount).await?;
                    BsvHomeMpcPendingPreparation {
                        wallet_id: wallet_id.clone(),
                        recipient: recipient.clone(),
                        target: resolved.address,
                        recipient_handle: (!resolved.handle.is_empty()).then_some(resolved.handle),
                        amount,
                        key: idempotency_key,
                    }
                }
            };
            state.pending_preparation = Some(pending.clone());
            self.save(&lease, &state)?;
            let response = self
                .request(
                    &lease,
                    "portal/chain/transfers/prepare",
                    json!({"walletId":wallet_id,"recipient":pending.target,"amountSatoshis":amount,"idempotencyKey":pending.key}),
                )
                .await;
            let value = match response {
                Ok(value) => value,
                Err(error) => {
                    if let BsvHomeMpcError::Platform {
                        error: BsvHomePlatformAccountError::Api { code },
                    } = &error
                        && !continuing_preparation
                        && [
                            "INVALID_REQUEST",
                            "INSUFFICIENT_AVAILABLE_CONFIRMED_FUNDS",
                            "INSUFFICIENT_BALANCE",
                            "CHAIN_DISABLED",
                            "BROADCAST_DISABLED",
                        ]
                        .contains(&code.as_str())
                    {
                        state.pending_preparation = None;
                        self.save(&lease, &state)?;
                    }
                    return Err(error);
                }
            };
            if value.get_string("walletId")? != wallet_id || value.get_string("recipient")? != pending.target || number(value.get_value("amountSatoshis")?)? != amount {
                return Err(BsvHomeMpcError::TransferChanged);
            }
            let fee = number(value.get_value("feeSatoshis")?)?;
            let proposal = value.get_value("proposal")?.clone();
            let mut reviewed = review(&proposal, &known.public_key, &pending.target, amount, fee)?;
            reviewed.recipient_handle = pending.recipient_handle.clone();
            let transfer = BsvHomeMpcTransfer {
                id: value.get_string("id")?.into(),
                wallet_id: wallet_id.clone(),
                recipient: pending.target.clone(),
                recipient_handle: pending.recipient_handle.clone(),
                amount,
                fee,
                status: value.get_string("status")?.into(),
                transaction_id: optional_text(&value, "txid")?,
                confirmations: optional_number(&value, "confirmations")?
                    .map(u32::try_from)
                    .transpose()
                    .map_err(|_| BsvHomeMpcError::InvalidResponse)?,
                review: reviewed,
                actions: transfer_actions(value.get_string("status")?, false),
                error_code: optional_text(&value, "errorCode")?.map(|code| public_code(&code)),
            };
            let stored = StoredTransfer {
                transfer: transfer.clone(),
                public_key: known.public_key.clone(),
                proposal,
                signed_transaction: None,
            };
            identifier(&transfer.id)?;
            if let Some(previous) = state.transfers.iter().find(|saved| saved.transfer.id == transfer.id) {
                transfer_unchanged(&value, previous)?;
            } else {
                state.transfers.push(stored);
            }
            state.pending_preparation = None;
            self.save(&lease, &state)?;
            Ok(transfer)
        }
        .await;
        self.participant.close(wallet_id).await?;
        result
    }

    pub async fn refresh_transfer(&self, id: String) -> Result<BsvHomeMpcTransfer, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        if state.active_task.as_ref().is_some_and(|task| task.kind == "SPEND") {
            self.reconcile_task(&lease, &mut state).await?;
        }
        let value = self.request(&lease, "portal/chain/transfers/one", json!({"id":id})).await?;
        let index = self.transfer_index(&state, &id)?;
        self.update_transfer(&mut state.transfers[index], &value)?;
        self.save(&lease, &state)?;
        Ok(state.transfers[index].transfer.clone())
    }

    pub async fn sign_transfer(&self, id: String, confirmed_commitment: String) -> Result<BsvHomeMpcTransfer, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        let index = self.transfer_index(&state, &id)?;
        self.confirmed_transfer(&state.transfers[index], &confirmed_commitment)?;
        if state.active_task.is_some() {
            self.reconcile_task(&lease, &mut state).await?;
        }
        let value = self.request(&lease, "portal/chain/transfers/one", json!({"id":id})).await?;
        self.update_transfer(&mut state.transfers[index], &value)?;
        let stored = state.transfers[index].clone();
        let known = self.public_wallet(&lease, self.known_wallet(&state, &stored.transfer.wallet_id)?).await?;
        let result = async {
            match stored.transfer.status.as_str() {
                "SIGNED" | "SUBMITTING" | "SUBMITTED" | "CONFIRMED" | "UNKNOWN" | "CONFLICTED" | "FAILED" | "REJECTED" | "CANCELLED" | "EXPIRED" => {
                    return Ok(stored.transfer.clone());
                }
                "SIGNING" => {
                    let challenge = self.request(&lease, "portal/chain/transfers/resume/challenge", json!({"id":id})).await?;
                    let action = challenge.get_string("actionHash")?;
                    let grant = self.account.authorize(action).await?;
                    let signature = self.device.sign_message(action.into()).await?;
                    let resumed = self
                        .request(&lease, "portal/chain/transfers/resume", json!({"id":id,"grant":grant,"deviceSignature":signature}))
                        .await?;
                    transfer_unchanged(&resumed, &stored)?;
                }
                "PREPARED" => {}
                _ => {
                    return Err(BsvHomeMpcError::UnsupportedState {
                        state: stored.transfer.status.clone(),
                    });
                }
            }
            self.local_identity(&lease, &known, true).await?;
            let task = self
                .prepare_task(&lease, &mut state, json!({"kind":"SPEND","walletId":known.wallet_id,"transferId":id}), Some(known.epoch))
                .await?;
            let reviewed = review(
                task.context.get_value("proposal")?,
                &stored.public_key,
                &stored.transfer.recipient,
                stored.transfer.amount,
                stored.transfer.fee,
            )?;
            if reviewed.commitment != confirmed_commitment {
                return Err(BsvHomeMpcError::TransferChanged);
            }
            self.signing(&lease, &task, &reviewed.digests).await?;
            let complete = self.task_status(&lease, &task).await?;
            if complete.get_string("status")? != "COMPLETE" {
                return Err(BsvHomeMpcError::OperationPending);
            }
            self.retain_signed(&lease, &mut state, &task, &complete).await?;
            state.active_task = None;
            self.save(&lease, &state)?;
            Ok(state.transfers[index].transfer.clone())
        }
        .await;
        self.participant.close(known.wallet_id).await?;
        result
    }

    pub async fn broadcast_transfer(&self, id: String, confirmed_commitment: String) -> Result<BsvHomeMpcTransfer, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        let index = self.transfer_index(&state, &id)?;
        self.confirmed_transfer(&state.transfers[index], &confirmed_commitment)?;
        let value = self.request(&lease, "portal/chain/transfers/one", json!({"id":id})).await?;
        self.update_transfer(&mut state.transfers[index], &value)?;
        let stored = state.transfers[index].clone();
        let raw = stored.signed_transaction.as_ref().ok_or(BsvHomeMpcError::OperationPending)?;
        let txid = self.verify_signed(&stored, raw)?;
        if value.get_string("txid")? != txid {
            return Err(BsvHomeMpcError::TransferChanged);
        }
        let (path, action_key) = match stored.transfer.status.as_str() {
            "SIGNED" => ("portal/chain/transfers/broadcast", "broadcastActionHash"),
            "UNKNOWN" => ("portal/chain/transfers/retry-broadcast", "retryBroadcastActionHash"),
            "SUBMITTING" | "SUBMITTED" | "CONFIRMED" | "CONFLICTED" | "FAILED" | "REJECTED" | "CANCELLED" | "EXPIRED" => return Ok(stored.transfer),
            _ => return Err(BsvHomeMpcError::UnsupportedState { state: stored.transfer.status }),
        };
        let action = value.get_string(action_key)?;
        if !hex_text(action, 64) {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        let grant = self.account.authorize(action).await?;
        self.account.require_lease(&lease)?;
        let signature = self.device.sign_message(action.into()).await?;
        state.transfers[index].set_status("SUBMITTING");
        self.save(&lease, &state)?;
        match self.request(&lease, path, json!({"id":id,"grant":grant,"deviceSignature":signature})).await {
            Ok(response) => self.update_transfer(&mut state.transfers[index], &response)?,
            Err(error) => {
                let uncertain = match &error {
                    BsvHomeMpcError::Platform {
                        error: BsvHomePlatformAccountError::NetworkUnavailable | BsvHomePlatformAccountError::InvalidResponse,
                    } => true,
                    BsvHomeMpcError::Platform {
                        error: BsvHomePlatformAccountError::Api { code },
                    } => code == "BROADCAST_RESULT_UNKNOWN",
                    _ => false,
                };
                if !uncertain {
                    return Err(error);
                }
                state.transfers[index].set_status("UNKNOWN");
                state.transfers[index].transfer.error_code = Some("BROADCAST_RESULT_UNKNOWN".into());
            }
        }
        self.save(&lease, &state)?;
        Ok(state.transfers[index].transfer.clone())
    }

    pub async fn cancel_transfer(&self, id: String) -> Result<BsvHomeMpcTransfer, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        let index = self.transfer_index(&state, &id)?;
        let current = self.request(&lease, "portal/chain/transfers/one", json!({"id":id})).await?;
        self.update_transfer(&mut state.transfers[index], &current)?;
        if state.transfers[index].transfer.status != "PREPARED" {
            return Err(BsvHomeMpcError::OperationPending);
        }
        let response = self.request(&lease, "portal/chain/transfers/cancel", json!({"id":id})).await?;
        self.update_transfer(&mut state.transfers[index], &response)?;
        self.save(&lease, &state)?;
        Ok(state.transfers[index].transfer.clone())
    }

    pub async fn transaction_history(&self, wallet_id: String, page: u32) -> Result<BsvHomeMpcHistoryPage, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        if !(1..=10000).contains(&page) {
            return Err(BsvHomeMpcError::InvalidInput);
        }
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        self.known_wallet(&state, &wallet_id)?;
        let value = self
            .request(&lease, "portal/chain/transactions/page", json!({"objectId":wallet_id,"current":page,"pageSize":20}))
            .await?;
        let source = value.get_value("rows")?.as_array().ok_or(BsvHomeMpcError::InvalidResponse)?;
        let total = number(value.get_value("total")?)?;
        if source.len() > 20 || total < source.len() as u64 {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        let rows = source.iter().map(|row| history(row, &wallet_id)).collect::<Result<Vec<_>, _>>()?;
        let indices = state
            .transfers
            .iter()
            .enumerate()
            .filter(|(_, stored)| stored.transfer.wallet_id == wallet_id)
            .map(|(index, _)| index)
            .rev()
            .skip((page as usize - 1) * 20)
            .take(20)
            .collect::<Vec<_>>();
        let mut pending_transfers = Vec::new();
        for index in indices {
            let id = state.transfers[index].transfer.id.clone();
            let current = self.request(&lease, "portal/chain/transfers/one", json!({"id":id})).await?;
            self.update_transfer(&mut state.transfers[index], &current)?;
            let transfer = state.transfers[index].transfer.clone();
            if transfer.transaction_id.as_ref().is_none_or(|id| rows.iter().all(|row| &row.transaction_id != id)) {
                pending_transfers.push(transfer);
            }
        }
        self.save(&lease, &state)?;
        Ok(BsvHomeMpcHistoryPage {
            rows,
            total,
            page,
            pending_transfers,
        })
    }
}

impl BsvHomeMpcWalletService {
    fn transfer_index(&self, state: &StoredState, id: &str) -> Result<usize, BsvHomeMpcError> {
        state.transfers.iter().position(|stored| stored.transfer.id == id).ok_or(BsvHomeMpcError::TransferChanged)
    }
    fn confirmed_transfer(&self, stored: &StoredTransfer, commitment: &str) -> Result<(), BsvHomeMpcError> {
        if stored.transfer.review.commitment != commitment {
            return Err(BsvHomeMpcError::TransferChanged);
        }
        Ok(())
    }
    pub(super) fn update_transfer(&self, stored: &mut StoredTransfer, value: &Value) -> Result<(), BsvHomeMpcError> {
        transfer_unchanged(value, stored)?;
        stored.set_status(value.get_string("status")?);
        stored.transfer.transaction_id = optional_text(value, "txid")?;
        stored.transfer.confirmations = optional_number(value, "confirmations")?
            .map(u32::try_from)
            .transpose()
            .map_err(|_| BsvHomeMpcError::InvalidResponse)?;
        stored.transfer.error_code = optional_text(value, "errorCode")?.map(|code| public_code(&code));
        Ok(())
    }
    fn verify_signed(&self, stored: &StoredTransfer, raw: &str) -> Result<String, BsvHomeMpcError> {
        Ok(mpc::verify_signed_json(
            &serde_json::to_string(&stored.proposal)?,
            &stored.public_key,
            &stored.transfer.recipient,
            stored.transfer.amount,
            stored.transfer.fee,
            raw,
        )?)
    }
    pub(super) async fn retain_signed(&self, lease: &PlatformSessionLease, state: &mut StoredState, task: &Task, complete: &Value) -> Result<(), BsvHomeMpcError> {
        let id = task.context.get_string("transferId")?;
        let index = self.transfer_index(state, id)?;
        let raw = complete.get_value("result")?.get_string("signedTransaction")?.to_string();
        let transaction_id = self.verify_signed(&state.transfers[index], &raw)?;
        let current = self.request(lease, "portal/chain/transfers/one", json!({"id":id})).await?;
        transfer_unchanged(&current, &state.transfers[index])?;
        if current.get_string("txid")? != transaction_id {
            return Err(BsvHomeMpcError::TransferChanged);
        }
        self.update_transfer(&mut state.transfers[index], &current)?;
        state.transfers[index].signed_transaction = Some(raw);
        state.transfers[index].transfer.actions = transfer_actions(&state.transfers[index].transfer.status, true);
        self.save(lease, state)
    }
}

impl StoredTransfer {
    fn set_status(&mut self, status: &str) {
        self.transfer.status = status.into();
        self.transfer.actions = transfer_actions(status, self.signed_transaction.is_some());
    }
}
