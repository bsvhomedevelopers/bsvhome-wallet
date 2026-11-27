use std::error::Error;
use std::fmt::{Display, Formatter};

use primitives::SignerError;

use crate::services::error::BsvHomeServiceError;
use crate::services::portal::platform::BsvHomePlatformAccountError;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum BsvHomeMpcError {
    Platform { error: BsvHomePlatformAccountError },
    InvalidInput,
    PaymailSenderRequired,
    PaymailUnavailable,
    InvalidResponse,
    WalletNotFound,
    IdentityMismatch,
    EpochMismatch,
    BackupMismatch,
    TransferChanged,
    OperationPending,
    UnsupportedState { state: String },
    NativeUnavailable,
    SecureStorageUnavailable,
    Cancelled,
}

impl Display for BsvHomeMpcError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Platform { error } => return Display::fmt(error, formatter),
            Self::InvalidInput => "INVALID_REQUEST",
            Self::PaymailSenderRequired => "PAYMAIL_SENDER_REQUIRED",
            Self::PaymailUnavailable => "PAYMAIL_UNAVAILABLE",
            Self::InvalidResponse => "MPC_RESPONSE_INVALID",
            Self::WalletNotFound => "MPC_LOCAL_STATE_UNAVAILABLE",
            Self::IdentityMismatch => "MPC_WALLET_IDENTITY_MISMATCH",
            Self::EpochMismatch => "MPC_EPOCH_MISMATCH",
            Self::BackupMismatch => "BACKUP_CONTEXT_INVALID",
            Self::TransferChanged => "TRANSFER_PROPOSAL_MISMATCH",
            Self::OperationPending => "MPC_OPERATION_PENDING",
            Self::UnsupportedState { .. } => "MPC_STATE_UNSUPPORTED",
            Self::NativeUnavailable => "MPC_NATIVE_UNAVAILABLE",
            Self::SecureStorageUnavailable => "SECURE_STORAGE_UNAVAILABLE",
            Self::Cancelled => "MPC_CANCELLED",
        })
    }
}

impl Error for BsvHomeMpcError {}

impl From<BsvHomePlatformAccountError> for BsvHomeMpcError {
    fn from(error: BsvHomePlatformAccountError) -> Self {
        Self::Platform { error }
    }
}
impl From<BsvHomeServiceError> for BsvHomeMpcError {
    fn from(_: BsvHomeServiceError) -> Self {
        Self::SecureStorageUnavailable
    }
}
impl From<serde_json::Error> for BsvHomeMpcError {
    fn from(_: serde_json::Error) -> Self {
        Self::InvalidResponse
    }
}
impl From<SignerError> for BsvHomeMpcError {
    fn from(_: SignerError) -> Self {
        Self::TransferChanged
    }
}
impl From<uniffi::UnexpectedUniFFICallbackError> for BsvHomeMpcError {
    fn from(_: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::NativeUnavailable
    }
}
impl From<String> for BsvHomeMpcError {
    fn from(_: String) -> Self {
        Self::InvalidResponse
    }
}
