use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::BsvHomeCoreError;
use crate::services::error::BsvHomeServiceError;

#[derive(Debug, Clone, PartialEq, uniffi::Error)]
pub enum BsvHomePortalAuthError {
    Service { error: BsvHomeServiceError },
    MissingBsvAccount,
    WatchOnly,
    WalletChanged,
    InvalidChallenge,
    InvalidSession,
    Api { code: String, msg: String },
    InvalidResponse,
}

impl Display for BsvHomePortalAuthError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Service { error } => Display::fmt(error, formatter),
            Self::MissingBsvAccount => formatter.write_str("a BSV account is required"),
            Self::WatchOnly => formatter.write_str("a signing wallet is required"),
            Self::WalletChanged => formatter.write_str("the current wallet changed"),
            Self::InvalidChallenge => formatter.write_str("invalid wallet authentication challenge"),
            Self::InvalidSession => formatter.write_str("invalid wallet authentication session"),
            Self::Api { msg, .. } => formatter.write_str(msg),
            Self::InvalidResponse => formatter.write_str("invalid Portal response"),
        }
    }
}

impl Error for BsvHomePortalAuthError {}

impl From<BsvHomeServiceError> for BsvHomePortalAuthError {
    fn from(error: BsvHomeServiceError) -> Self {
        Self::Service { error }
    }
}

impl From<BsvHomeCoreError> for BsvHomePortalAuthError {
    fn from(error: BsvHomeCoreError) -> Self {
        BsvHomeServiceError::from(error).into()
    }
}
