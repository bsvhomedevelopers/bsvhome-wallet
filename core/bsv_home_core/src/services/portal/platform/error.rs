use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::services::error::BsvHomeServiceError;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum BsvHomePlatformAccountError {
    InvalidConfiguration,
    InvalidInput,
    InvalidResponse,
    LoginRequired,
    SessionChanged,
    Cancelled,
    NetworkUnavailable,
    SecureStorageUnavailable,
    AuthenticationUnavailable,
    Api { code: String },
}

impl Display for BsvHomePlatformAccountError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "PLATFORM_CONFIGURATION_INVALID",
            Self::InvalidInput => "INVALID_REQUEST",
            Self::InvalidResponse => "INVALID_RESPONSE",
            Self::LoginRequired => "LOGIN_REQUIRED",
            Self::SessionChanged => "AUTH_SESSION_MISMATCH",
            Self::Cancelled => "AUTHENTICATION_CANCELLED",
            Self::NetworkUnavailable => "NETWORK_UNAVAILABLE",
            Self::SecureStorageUnavailable => "SECURE_STORAGE_UNAVAILABLE",
            Self::AuthenticationUnavailable => "PASSKEY_PROVIDER_UNAVAILABLE",
            Self::Api { code } => code,
        })
    }
}

impl Error for BsvHomePlatformAccountError {}

impl From<serde_json::Error> for BsvHomePlatformAccountError {
    fn from(_: serde_json::Error) -> Self {
        Self::InvalidResponse
    }
}

impl From<BsvHomeServiceError> for BsvHomePlatformAccountError {
    fn from(error: BsvHomeServiceError) -> Self {
        match error {
            BsvHomeServiceError::Cancelled => Self::Cancelled,
            BsvHomeServiceError::Store { .. } => Self::SecureStorageUnavailable,
            BsvHomeServiceError::Api { .. } | BsvHomeServiceError::Gateway { .. } => Self::NetworkUnavailable,
            BsvHomeServiceError::Platform { .. } => Self::AuthenticationUnavailable,
            BsvHomeServiceError::Core { .. }
            | BsvHomeServiceError::InvalidInput { .. }
            | BsvHomeServiceError::NotFound { .. }
            | BsvHomeServiceError::Unsupported { .. }
            | BsvHomeServiceError::WalletImport { .. } => Self::InvalidInput,
        }
    }
}

impl From<uniffi::UnexpectedUniFFICallbackError> for BsvHomePlatformAccountError {
    fn from(_: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::AuthenticationUnavailable
    }
}
