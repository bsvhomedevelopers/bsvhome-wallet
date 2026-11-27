use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::services::error::BsvHomeServiceError;

use super::BsvHomePortalAuthError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Error)]
pub enum BsvHomeTransferPasswordError {
    InvalidConfiguration,
    InvalidSnapshot,
    WatchOnly,
    MissingBsvAccount,
    WalletChanged,
    Cancelled,
    InvalidPassword,
    ConfirmationMismatch,
    CurrentPasswordRequired,
    NotConfigured,
    IncorrectPassword,
    RateLimited,
    InvalidResponse,
    Unavailable,
}

#[uniffi::export]
impl BsvHomeTransferPasswordError {
    pub fn requires_status_reload(self) -> bool {
        match self {
            Self::Unavailable | Self::InvalidResponse | Self::InvalidSnapshot | Self::NotConfigured => true,
            Self::InvalidConfiguration
            | Self::WatchOnly
            | Self::MissingBsvAccount
            | Self::WalletChanged
            | Self::Cancelled
            | Self::InvalidPassword
            | Self::ConfirmationMismatch
            | Self::CurrentPasswordRequired
            | Self::IncorrectPassword
            | Self::RateLimited => false,
        }
    }
}

impl Display for BsvHomeTransferPasswordError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "invalid transfer password configuration",
            Self::InvalidSnapshot => "transfer password status changed",
            Self::WatchOnly => "a signing wallet is required",
            Self::MissingBsvAccount => "a BSV account is required",
            Self::WalletChanged => "the current wallet changed",
            Self::Cancelled => "cancelled",
            Self::InvalidPassword => "invalid transfer password",
            Self::ConfirmationMismatch => "transfer password confirmation does not match",
            Self::CurrentPasswordRequired => "the current transfer password is required",
            Self::NotConfigured => "the transfer password is not configured",
            Self::IncorrectPassword => "the transfer password is incorrect",
            Self::RateLimited => "transfer password requests are rate limited",
            Self::InvalidResponse => "invalid transfer password response",
            Self::Unavailable => "transfer password service unavailable",
        })
    }
}

impl Error for BsvHomeTransferPasswordError {}

impl From<BsvHomeServiceError> for BsvHomeTransferPasswordError {
    fn from(error: BsvHomeServiceError) -> Self {
        match error {
            BsvHomeServiceError::Cancelled => Self::Cancelled,
            BsvHomeServiceError::NotFound { .. } => Self::WalletChanged,
            _ => Self::Unavailable,
        }
    }
}

impl From<BsvHomePortalAuthError> for BsvHomeTransferPasswordError {
    fn from(error: BsvHomePortalAuthError) -> Self {
        match error {
            BsvHomePortalAuthError::Service { error } => error.into(),
            BsvHomePortalAuthError::WatchOnly => Self::WatchOnly,
            BsvHomePortalAuthError::MissingBsvAccount => Self::MissingBsvAccount,
            BsvHomePortalAuthError::WalletChanged => Self::WalletChanged,
            BsvHomePortalAuthError::InvalidResponse => Self::InvalidResponse,
            BsvHomePortalAuthError::InvalidSession | BsvHomePortalAuthError::InvalidChallenge => Self::Unavailable,
            BsvHomePortalAuthError::Api { code, .. } => match code.as_str() {
                "TRANSFER_PASSWORD_INCORRECT" => Self::IncorrectPassword,
                "TRANSFER_PASSWORD_RATE_LIMITED" | "RATE_LIMITED" | "SYS_REQUEST_LIMITED" | "HTTP_429" => Self::RateLimited,
                _ => Self::Unavailable,
            },
        }
    }
}
