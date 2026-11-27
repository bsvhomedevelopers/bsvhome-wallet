use std::collections::BTreeMap;

use bsv_home_bsv::{BsvAddress, mpc};
use bsv_home_hash::sha2::sha256;
use chrono::{DateTime, Utc};
use number_formatter::BigNumberFormatter;
use primitives::{Address, ValueAccess};
use serde_json::Value;

use super::BsvHomeMpcError;
use super::model::{
    BsvHomeMpcBalance, BsvHomeMpcHistoryItem, BsvHomeMpcRecoveryAction, BsvHomeMpcTransferAction, BsvHomeMpcTransferReview, BsvHomeMpcWallet, BsvHomeMpcWalletAction,
    StoredTransfer, Task,
};

pub(super) const PROTOCOL: &str = "DKLs23-secp256k1-v0.5.1-2of2";
pub(super) const MAX_DOCUMENT_BYTES: usize = 4_000_000;
pub(super) const MAX_MONEY: u64 = 2_100_000_000_000_000;

pub(super) fn hash(bytes: &[u8]) -> String {
    hex::encode(sha256(bytes))
}
pub(super) fn hex_text(text: &str, length: usize) -> bool {
    text.len() == length && text.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
pub(super) fn identifier(value: &str) -> Result<(), BsvHomeMpcError> {
    if value.is_empty() || value.len() > 128 || !value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') {
        return Err(BsvHomeMpcError::InvalidInput);
    }
    Ok(())
}
pub(super) fn number(value: &Value) -> Result<u64, BsvHomeMpcError> {
    match value {
        Value::Number(value) => value.as_u64().ok_or(BsvHomeMpcError::InvalidResponse),
        Value::String(value) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => value.parse().map_err(|_| BsvHomeMpcError::InvalidResponse),
        _ => Err(BsvHomeMpcError::InvalidResponse),
    }
}
pub(super) fn optional_number(value: &Value, key: &str) -> Result<Option<u64>, BsvHomeMpcError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => number(value).map(Some),
    }
}
pub(super) fn optional_text(value: &Value, key: &str) -> Result<Option<String>, BsvHomeMpcError> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(BsvHomeMpcError::InvalidResponse),
    }
}
pub(super) fn wallet(value: &Value, name: String, restored: bool) -> Result<BsvHomeMpcWallet, BsvHomeMpcError> {
    let id = value.get_string("walletId")?;
    identifier(id)?;
    let public_key = value.get_string("publicKey")?;
    let address = value.get_string("address")?;
    if value.get_string("protocol")? != PROTOCOL
        || !hex_text(public_key, 66)
        || BsvAddress::from_public_key(&hex::decode(public_key).map_err(|_| BsvHomeMpcError::IdentityMismatch)?)
            .map_err(|_| BsvHomeMpcError::IdentityMismatch)?
            .encode()
            != address
    {
        return Err(BsvHomeMpcError::IdentityMismatch);
    }
    Ok(BsvHomeMpcWallet {
        wallet_id: id.into(),
        name,
        public_key: public_key.into(),
        address: address.into(),
        epoch: number(value.get_value("epoch")?)?,
        status: value.get_string("status")?.into(),
        restored,
        balance: None,
        actions: Vec::new(),
    })
}
pub(super) fn balance(value: &Value) -> Result<BsvHomeMpcBalance, BsvHomeMpcError> {
    let unconfirmed_satoshis = match value.get("unconfirmedSatoshis") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.parse::<i64>().map_err(|_| BsvHomeMpcError::InvalidResponse)?),
        Some(Value::Number(value)) => Some(value.as_i64().ok_or(BsvHomeMpcError::InvalidResponse)?),
        Some(_) => return Err(BsvHomeMpcError::InvalidResponse),
    };
    Ok(BsvHomeMpcBalance {
        state: value.get_string("state")?.into(),
        confirmed_satoshis: optional_number(value, "confirmedSatoshis")?,
        unconfirmed_satoshis,
        available_confirmed_satoshis: optional_number(value, "availableConfirmedSatoshis")?,
        reserved_confirmed_satoshis: optional_number(value, "reservedConfirmedSatoshis")?,
        history_state: optional_text(value, "historyState")?,
        synced_at: optional_text(value, "syncedAt")?,
        source: optional_text(value, "source")?,
    })
}
pub(super) fn review(proposal: &Value, public_key: &str, recipient: &str, amount: u64, fee: u64) -> Result<BsvHomeMpcTransferReview, BsvHomeMpcError> {
    Ok(serde_json::from_str(&mpc::review_json(
        &serde_json::to_string(proposal)?,
        public_key,
        recipient,
        amount,
        fee,
    )?)?)
}
pub(super) fn transfer_unchanged(value: &Value, stored: &StoredTransfer) -> Result<(), BsvHomeMpcError> {
    let original = &stored.transfer;
    if value.get_string("id")? != original.id
        || value.get_string("walletId")? != original.wallet_id
        || value.get_string("recipient")? != original.recipient
        || number(value.get_value("amountSatoshis")?)? != original.amount
        || number(value.get_value("feeSatoshis")?)? != original.fee
        || review(value.get_value("proposal")?, &stored.public_key, &original.recipient, original.amount, original.fee)?.commitment != original.review.commitment
    {
        return Err(BsvHomeMpcError::TransferChanged);
    }
    if let Some(expected) = &original.transaction_id
        && value.get_string("txid")? != expected
    {
        return Err(BsvHomeMpcError::TransferChanged);
    }
    if let Some(transaction_id) = optional_text(value, "txid")?
        && !hex_text(&transaction_id, 64)
    {
        return Err(BsvHomeMpcError::TransferChanged);
    }
    Ok(())
}
pub(super) fn task_matches(task: &Task, request: &Value, device_public_key: &str, epoch: Option<u64>) -> Result<(), BsvHomeMpcError> {
    identifier(&task.id)?;
    identifier(&task.wallet_id)?;
    if task.kind != request.get_string("kind")?
        || !hex_text(&task.action_hash, 64)
        || task.context.get_string("protocol")? != PROTOCOL
        || task.context.get_string("devicePublicKey")? != device_public_key
    {
        return Err(BsvHomeMpcError::InvalidResponse);
    }
    if let Some(wallet_id) = request.get("walletId")
        && wallet_id.string()? != task.wallet_id
    {
        return Err(BsvHomeMpcError::IdentityMismatch);
    }
    if epoch.is_some_and(|epoch| epoch != task.epoch) {
        return Err(BsvHomeMpcError::EpochMismatch);
    }
    for key in ["transferId", "recoveryId", "backupCommitment", "paymailChallengeId"] {
        if let Some(expected) = request.get(key)
            && task.context.get_value(key)? != expected
        {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
    }
    Ok(())
}
pub(super) fn history(value: &Value, wallet_id: &str) -> Result<BsvHomeMpcHistoryItem, BsvHomeMpcError> {
    let id = value.get_string("id")?;
    let transaction_id = value.get_string("txid")?;
    if value.get_string("walletId")? != wallet_id || !hex_text(id, 64) || !hex_text(transaction_id, 64) {
        return Err(BsvHomeMpcError::InvalidResponse);
    }
    let received = number(value.get_value("receivedSatoshis")?)?;
    let spent = number(value.get_value("spentSatoshis")?)?;
    let direction = value.get_string("direction")?;
    if received > MAX_MONEY
        || spent > MAX_MONEY
        || (received == 0 && spent == 0)
        || !match direction {
            "IN" => spent == 0 && received > 0,
            "OUT" => spent > received,
            "SELF" => spent > 0,
            _ => false,
        }
    {
        return Err(BsvHomeMpcError::InvalidResponse);
    }
    Ok(BsvHomeMpcHistoryItem {
        id: id.into(),
        wallet_id: wallet_id.into(),
        transaction_id: transaction_id.into(),
        received,
        spent,
        amount: received.abs_diff(spent),
        incoming: received > spent,
        fee: if value.get_value("feeSatoshis")?.is_null() {
            None
        } else {
            Some(number(value.get_value("feeSatoshis")?)?)
        },
        direction: direction.into(),
        status: value.get_string("status")?.into(),
        confirmations: u32::try_from(number(value.get_value("confirmations")?)?).map_err(|_| BsvHomeMpcError::InvalidResponse)?,
        observed_at: optional_text(value, "observedAt")?,
        source: optional_text(value, "source")?,
    })
}
pub(super) fn paymail_fields(message: &str, expected: &BTreeMap<&str, String>, now: DateTime<Utc>) -> Result<(), BsvHomeMpcError> {
    let mut lines = message.split('\n');
    if lines.next() != Some("WowPay Paymail Binding") {
        return Err(BsvHomeMpcError::InvalidResponse);
    }
    let mut fields = BTreeMap::new();
    for line in lines {
        let (key, value) = line.split_once(": ").ok_or(BsvHomeMpcError::InvalidResponse)?;
        if fields.insert(key, value).is_some() {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
    }
    if fields.len() != expected.len() + 1 {
        return Err(BsvHomeMpcError::InvalidResponse);
    }
    for (key, value) in expected {
        if fields.get(key) != Some(&value.as_str()) {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
    }
    let expiry = DateTime::parse_from_rfc3339(fields.get("Expires").ok_or(BsvHomeMpcError::InvalidResponse)?)
        .map_err(|_| BsvHomeMpcError::InvalidResponse)?
        .with_timezone(&Utc);
    if expiry <= now || expiry.signed_duration_since(now).num_seconds() > 900 {
        return Err(BsvHomeMpcError::InvalidResponse);
    }
    Ok(())
}

pub(super) fn wallet_actions(wallet: &BsvHomeMpcWallet, recovery_status: Option<&str>) -> Vec<BsvHomeMpcWalletAction> {
    if wallet.restored {
        return if recovery_status == Some("AWAIT_BACKUP") {
            vec![BsvHomeMpcWalletAction::ExportBackup]
        } else if recovery_status.is_none() || [Some("CANCELLED"), Some("EXPIRED"), Some("REJECTED")].contains(&recovery_status) {
            vec![BsvHomeMpcWalletAction::StartRecovery]
        } else {
            Vec::new()
        };
    }
    match wallet.status.as_str() {
        "ACTIVE" => {
            let mut actions = vec![BsvHomeMpcWalletAction::ExportBackup];
            if wallet
                .balance
                .as_ref()
                .is_some_and(|balance| balance.state == "CURRENT" && balance.available_confirmed_satoshis.is_some_and(|amount| amount > 0))
            {
                actions.push(BsvHomeMpcWalletAction::Send);
            }
            actions
        }
        "AWAIT_BACKUP" => vec![BsvHomeMpcWalletAction::ExportBackup],
        _ => Vec::new(),
    }
}
pub(super) fn transfer_actions(status: &str, signed: bool) -> Vec<BsvHomeMpcTransferAction> {
    match status {
        "PREPARED" => vec![BsvHomeMpcTransferAction::Sign, BsvHomeMpcTransferAction::Cancel, BsvHomeMpcTransferAction::Refresh],
        "SIGNING" => vec![BsvHomeMpcTransferAction::ResumeSigning, BsvHomeMpcTransferAction::Refresh],
        "SIGNED" if signed => vec![BsvHomeMpcTransferAction::Broadcast, BsvHomeMpcTransferAction::Refresh],
        "UNKNOWN" if signed => vec![BsvHomeMpcTransferAction::RetryBroadcast, BsvHomeMpcTransferAction::Refresh],
        _ => vec![BsvHomeMpcTransferAction::Refresh],
    }
}
pub(super) fn recovery_actions(status: &str) -> Vec<BsvHomeMpcRecoveryAction> {
    match status {
        "WAIT_REVIEW" => vec![BsvHomeMpcRecoveryAction::Status, BsvHomeMpcRecoveryAction::Cancel],
        "NEEDS_INFO" => vec![
            BsvHomeMpcRecoveryAction::Status,
            BsvHomeMpcRecoveryAction::SupplyInformation,
            BsvHomeMpcRecoveryAction::Cancel,
        ],
        "WAITING" => vec![BsvHomeMpcRecoveryAction::Status, BsvHomeMpcRecoveryAction::Refresh, BsvHomeMpcRecoveryAction::Cancel],
        "PROTOCOL" => vec![BsvHomeMpcRecoveryAction::Status, BsvHomeMpcRecoveryAction::Cancel],
        "AWAIT_BACKUP" => vec![BsvHomeMpcRecoveryAction::Status, BsvHomeMpcRecoveryAction::ExportBackup, BsvHomeMpcRecoveryAction::Cancel],
        "COMMITTING" => vec![BsvHomeMpcRecoveryAction::Status, BsvHomeMpcRecoveryAction::Reconcile],
        _ => vec![BsvHomeMpcRecoveryAction::Status],
    }
}

pub(super) fn bsv_amount(text: &str) -> Result<u64, BsvHomeMpcError> {
    let amount = text.trim();
    if amount.is_empty() || amount.len() > 40 || !amount.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.') || amount.bytes().filter(|byte| *byte == b'.').count() > 1 {
        return Err(BsvHomeMpcError::InvalidInput);
    }
    let satoshis = BigNumberFormatter::value_from_amount_exact(amount, 8)
        .map_err(|_| BsvHomeMpcError::InvalidInput)?
        .to_string()
        .parse::<u64>()
        .map_err(|_| BsvHomeMpcError::InvalidInput)?;
    if satoshis == 0 || satoshis > MAX_MONEY {
        return Err(BsvHomeMpcError::InvalidInput);
    }
    Ok(satoshis)
}
