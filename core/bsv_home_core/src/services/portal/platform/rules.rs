use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;
use url::Url;

use super::{BsvHomePlatformAccountError, BsvHomePlatformEmailChallenge, BsvHomePlatformSession};

pub(super) fn base_url(input: &str) -> Result<String, BsvHomePlatformAccountError> {
    let url = Url::parse(input).map_err(|_| BsvHomePlatformAccountError::InvalidConfiguration)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || input.contains('%')
        || input.contains("/../")
        || input.contains("/./")
        || url.path() != "/v2/api/"
    {
        return Err(BsvHomePlatformAccountError::InvalidConfiguration);
    }
    Ok(url.to_string())
}

pub(super) fn identifier(input: &str) -> Result<(), BsvHomePlatformAccountError> {
    if input.len() != 32 || !input.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(BsvHomePlatformAccountError::InvalidInput);
    }
    Ok(())
}

pub(super) fn action_hash(input: &str) -> Result<(), BsvHomePlatformAccountError> {
    if input.len() != 64 || !input.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    Ok(())
}

pub(super) fn email(input: &str) -> Result<String, BsvHomePlatformAccountError> {
    let email = input.trim().to_ascii_lowercase();
    let parts = email.split('@').collect::<Vec<_>>();
    if !email.is_ascii() || email.len() > 254 || parts.len() != 2 || parts[0].is_empty() || parts[0].len() > 64 {
        return Err(BsvHomePlatformAccountError::InvalidInput);
    }
    let local = parts[0];
    let domain = parts[1];
    if !local.bytes().all(|byte| byte.is_ascii_alphanumeric() || b".!#$%&'*+/=?^_`{|}~-".contains(&byte))
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
        || !domain.contains('.')
        || domain.split('.').any(|label| {
            label.is_empty() || label.len() > 63 || label.starts_with('-') || label.ends_with('-') || !label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(BsvHomePlatformAccountError::InvalidInput);
    }
    Ok(email)
}

pub(super) fn code(input: &str) -> Result<(), BsvHomePlatformAccountError> {
    if input.len() != 6 || !input.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(BsvHomePlatformAccountError::InvalidInput);
    }
    Ok(())
}

pub(super) fn label(input: &str) -> Result<(), BsvHomePlatformAccountError> {
    if input.trim().is_empty() || input.chars().count() > 80 || input.chars().any(char::is_control) {
        return Err(BsvHomePlatformAccountError::InvalidInput);
    }
    Ok(())
}

pub(super) fn session(input: &BsvHomePlatformSession, token: &str) -> Result<(), BsvHomePlatformAccountError> {
    if token.len() != 43
        || !token.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        || input.user_id.is_empty()
        || input.user_id.len() > 20
        || !input.user_id.bytes().all(|byte| byte.is_ascii_digit())
        || input.expires_at.trim().is_empty()
    {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    timestamp(&input.expires_at)?;
    Ok(())
}

pub(super) fn email_challenge(input: &BsvHomePlatformEmailChallenge) -> Result<(), BsvHomePlatformAccountError> {
    identifier(&input.id).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
    unexpired(&input.expires_at)?;
    Ok(())
}

pub(super) fn timestamp(input: &str) -> Result<(), BsvHomePlatformAccountError> {
    if DateTime::parse_from_rfc3339(input).is_err()
        && NaiveDateTime::parse_from_str(input, "%Y-%m-%dT%H:%M:%S%.f").is_err()
        && NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S%.f").is_err()
    {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    Ok(())
}

pub(super) fn unexpired(input: &str) -> Result<(), BsvHomePlatformAccountError> {
    timestamp(input)?;
    if let Ok(expiry) = DateTime::parse_from_rfc3339(input)
        && expiry <= Utc::now()
    {
        return Err(BsvHomePlatformAccountError::LoginRequired);
    }
    Ok(())
}

pub(super) fn string<'a>(input: &'a Value, key: &str) -> Result<&'a str, BsvHomePlatformAccountError> {
    input.get(key).and_then(Value::as_str).ok_or(BsvHomePlatformAccountError::InvalidResponse)
}

pub(super) fn boolean(input: &Value, key: &str) -> Result<bool, BsvHomePlatformAccountError> {
    input.get(key).and_then(Value::as_bool).ok_or(BsvHomePlatformAccountError::InvalidResponse)
}

pub(crate) fn public_code(code: &str) -> String {
    if PUBLIC_CODES.contains(&code) { code.into() } else { "SERVICE_ERROR".into() }
}

const PUBLIC_CODES: &[&str] = &[
    "ACCOUNT_ACTION_INVALID",
    "ACCOUNT_DISABLED",
    "ACCOUNT_REOPEN_AUTHORIZATION_REVOKED",
    "ACCOUNT_REOPEN_NOT_ALLOWED",
    "ACCOUNT_REOPEN_OLD_PASSKEY_REQUIRED",
    "ACTION_HASH_INVALID",
    "ALTERNATIVE_IDENTITY_POLICY_INVALID",
    "AUTHORIZATION_REVOKED",
    "AUTH_CHALLENGE_INVALID",
    "AUTH_GRANT_INVALID",
    "AUTH_LIMITER_UNAVAILABLE",
    "AUTH_RATE_LIMITED",
    "AUTH_SESSION_MISMATCH",
    "BACKUP_COMMITMENT_REQUIRED",
    "BROADCAST_DISABLED",
    "BROADCAST_RESULT_UNKNOWN",
    "BUSINESS_NOTE_REQUIRED",
    "CHAIN_CONFIGURATION_INVALID",
    "CHAIN_DISABLED",
    "CHAIN_DUPLICATE_OUTPUT",
    "CHAIN_FEE_INVALID",
    "CHAIN_HISTORY_INCONSISTENT",
    "CHAIN_HISTORY_NOT_INDEXED",
    "CHAIN_OUTPUT_INVALID",
    "CHAIN_PAGINATION_INVALID",
    "CHAIN_PROVIDER_UNAVAILABLE",
    "CHAIN_RESPONSE_INVALID",
    "CHAIN_RESPONSE_TOO_LARGE",
    "CHAIN_SNAPSHOT_INCOMPLETE",
    "CHAIN_SNAPSHOT_INCONSISTENT",
    "CHAIN_SYNC_FAILED",
    "CHAIN_TIP_CHANGED_RETRY",
    "CHAIN_TRANSACTION_INVALID",
    "CHAIN_TRANSACTION_MISMATCH",
    "CHAIN_UNAVAILABLE",
    "CLOSURE_ACKNOWLEDGEMENT_REQUIRED",
    "CLOSURE_ACTION_INVALID",
    "CLOSURE_ACTIVE_PAYMAIL",
    "CLOSURE_ALREADY_PENDING",
    "CLOSURE_BALANCE_UNKNOWN",
    "CLOSURE_CHAIN_UNSUPPORTED",
    "CLOSURE_FUNDS_REMAIN",
    "CLOSURE_HD_SCAN_IN_PROGRESS",
    "CLOSURE_HD_SCAN_REQUIRED",
    "CLOSURE_ID_REQUIRED",
    "CLOSURE_NOTICE_WAIT_REQUIRED",
    "CLOSURE_NOT_CANCELLABLE",
    "CLOSURE_NOT_FOUND",
    "CLOSURE_PENDING_PROTOCOL",
    "CLOSURE_PENDING_TRANSFER",
    "CLOSURE_POLICY_INVALID",
    "CLOSURE_REOPEN_CREDENTIAL_REQUIRED",
    "CLOSURE_WAIT_REQUIRED",
    "CLOSURE_WALLET_NOT_READY",
    "DEVICE_INVALID",
    "DEVICE_NOT_FOUND",
    "EMAIL_CHANGE_AUTHORIZATION_CHANGED",
    "EMAIL_CHANGE_NOT_AVAILABLE",
    "EMAIL_CODE_INVALID",
    "EMAIL_DELIVERY_FAILED",
    "EMAIL_DELIVERY_UNAVAILABLE",
    "EMAIL_INVALID",
    "EMAIL_UNAVAILABLE",
    "EXISTING_PASSKEY_REQUIRED",
    "FEE_LIMIT_EXCEEDED",
    "HD_ACCOUNT_ID_REQUIRED",
    "HD_ACCOUNT_INACTIVE",
    "HD_ACCOUNT_LIMIT",
    "HD_ACCOUNT_NOT_FOUND",
    "HD_ACCOUNT_REQUIRED",
    "HD_ADDRESS_ALREADY_MANAGED",
    "HD_ADDRESS_ALREADY_OWNED",
    "HD_ADDRESS_HISTORY_UNKNOWN",
    "HD_ADDRESS_LIMIT",
    "HD_CHALLENGE_INVALID",
    "HD_CHILD_INVALID",
    "HD_CONTROL_PROOF_REQUIRED",
    "HD_DERIVATION_RANGE_INVALID",
    "HD_LABEL_INVALID",
    "HD_ROOT_WALLET_REQUIRED",
    "HD_SCAN_ALREADY_ACTIVE",
    "HD_SCAN_NOT_FOUND",
    "HD_SCAN_NOT_RESUMABLE",
    "HD_SCAN_RANGE_INVALID",
    "HD_SOURCE_WALLETS_INVALID",
    "HD_XPUB_OR_PATH_INVALID",
    "IDEMPOTENCY_CONFLICT",
    "IDENTITY_RECOVERY_ALREADY_PENDING",
    "IDENTITY_RECOVERY_FACTORS_CHANGED",
    "IDENTITY_RECOVERY_NOT_CANCELLABLE",
    "IDENTITY_RECOVERY_NOT_FOUND",
    "IDENTITY_RECOVERY_NOT_READY",
    "IDENTITY_RECOVERY_STATE_CHANGED",
    "IDENTITY_RECOVERY_WAIT_REQUIRED",
    "IDENTITY_REGISTRATION_SUPERSEDED",
    "INDEPENDENT_DEVICE_KEY_REQUIRED",
    "INDEPENDENT_SECOND_REVIEW_REQUIRED",
    "INPUT_ALREADY_SPENT",
    "INSUFFICIENT_AVAILABLE_CONFIRMED_FUNDS",
    "INSUFFICIENT_BALANCE",
    "INVALID_REQUEST",
    "LAST_PASSKEY_MUST_BE_REPLACED_FIRST",
    "LOGIN_REQUIRED",
    "MPC_ADDRESS_MISMATCH",
    "MPC_AUTHORIZATION_REVOKED",
    "MPC_BACKUP_REQUIRED",
    "MPC_CANCEL_ACTIVE_TASK_FIRST",
    "MPC_CAPACITY_RETRY_LATER",
    "MPC_COMMIT_EPOCH_MISMATCH",
    "MPC_COMMIT_IDENTITY_MISMATCH",
    "MPC_CONTEXT_INVALID",
    "MPC_DEVICE_KEY_INVALID",
    "MPC_DEVICE_PROOF_INVALID",
    "MPC_DEVICE_PROOF_REQUIRED",
    "MPC_EPOCH_MISMATCH",
    "MPC_MESSAGE_INVALID",
    "MPC_MESSAGE_ROUTE_INVALID",
    "MPC_PAYMAIL_CHALLENGE_INVALID",
    "MPC_PAYMAIL_TASK_INVALID",
    "MPC_PHASE_INVALID",
    "MPC_PROTOCOL_ABORTED",
    "MPC_PROTOCOL_REQUIRED",
    "MPC_PROTOCOL_UNAVAILABLE",
    "MPC_PUBLIC_KEY_MISMATCH",
    "MPC_RECOVERY_IDENTITY_MISMATCH",
    "MPC_RECOVERY_REQUIRED",
    "MPC_SESSION_RESTART_REQUIRED",
    "MPC_SIGNER_CONFIGURATION_INVALID",
    "MPC_SIGNER_UNAVAILABLE",
    "MPC_SIGNING_PAUSED",
    "MPC_STORAGE_INVALID",
    "MPC_TASK_EXPIRED",
    "MPC_TASK_INVALID",
    "MPC_TASK_NOT_FOUND",
    "MPC_TASK_STATE_CHANGED",
    "MPC_TASK_STILL_ACTIVE",
    "MPC_TRANSACTION_INVALID",
    "MPC_UNAVAILABLE",
    "MPC_WALLET_BUSY",
    "MPC_WALLET_NOT_FOUND",
    "NICKNAME_INVALID",
    "NOTICE_ATTEMPT_INVALID",
    "NOTICE_PROVIDER_AUTH_REQUIRED",
    "NOTICE_RECEIPT_INVALID",
    "NOTICE_RECEIPT_REPLAY_MISMATCH",
    "NOTICE_RECEIPT_TIME_INVALID",
    "NOTICE_RECEIPT_UNBOUND",
    "OFFLINE_IDENTITY_CHALLENGE_INVALID",
    "OFFLINE_IDENTITY_KEY_NOT_FOUND",
    "OFFLINE_IDENTITY_PROOF_REQUIRED",
    "OFFLINE_KEY_ALREADY_ENROLLED",
    "OFFLINE_KEY_LABEL_INVALID",
    "PASSKEY_ASSERTION_INVALID",
    "PASSKEY_AUTHORIZATION_REQUIRED",
    "PASSKEY_LABEL_INVALID",
    "PASSKEY_LAST_ACTIVE",
    "PASSKEY_NOT_FOUND",
    "PASSKEY_REGISTRATION_INVALID",
    "PASSKEY_REQUIRED",
    "PASSKEY_UNAVAILABLE",
    "PAYMAIL_ALREADY_EXISTS",
    "PAYMAIL_CONFLICT",
    "PAYMAIL_DISABLED",
    "PAYMAIL_NOT_FOUND",
    "PAYMAIL_UNAVAILABLE",
    "PORTAL_AUTH_UNAVAILABLE",
    "PREPARED_TRANSFER_REQUIRED",
    "RECENT_AUTH_REQUIRED",
    "RECENT_EMAIL_VERIFICATION_REQUIRED",
    "RECOVERY_ALREADY_COMMITTING",
    "RECOVERY_EPOCH_ALREADY_CHANGED",
    "RECOVERY_NOT_FOUND",
    "RECOVERY_NOT_READY",
    "RECOVERY_POLICY_INVALID",
    "RECOVERY_SAFETY_CONDITIONS_PENDING",
    "RECOVERY_STATE_CHANGED",
    "RECOVERY_TARGET_MUST_BE_NEW_DEVICE",
    "REVIEW_REASON_REQUIRED",
    "SECURITY_EMAIL_UNAVAILABLE",
    "SECURITY_NOTICE_INVALID",
    "SECURITY_TEMPLATE_VERSION_UNAVAILABLE",
    "SERVICE_UNAVAILABLE",
    "SIGNED_TRANSACTION_FEE_MISMATCH",
    "SIGNED_TRANSACTION_REQUIRED",
    "SIGNED_TRANSFER_CANNOT_CANCEL",
    "STEPUP_REQUIRED",
    "TIMEZONE_INVALID",
    "TRANSFER_EXPIRED",
    "TRANSFER_NOT_BROADCASTABLE",
    "TRANSFER_NOT_FOUND",
    "TRANSFER_NOT_RESUMABLE",
    "TRANSFER_NOT_SIGNABLE",
    "TRANSFER_PROPOSAL_MISMATCH",
    "TRANSFER_SESSION_MISMATCH",
    "TRANSFER_TASK_CHANGED",
    "USER_VERIFICATION_REQUIRED",
    "VERIFIED_EMAIL_REQUIRED",
    "WALLET_NOT_FOUND",
    "WALLET_PUBLIC_KEY_REQUIRED",
];
