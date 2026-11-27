use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::BsvHomeCoreError;
use crate::services::error::BsvHomeServiceError;

use super::BsvHomePortalAuthError;

#[derive(Debug, Clone, PartialEq, uniffi::Error)]
pub enum BsvHomePaymailError {
    Auth { error: BsvHomePortalAuthError },
    InvalidAlias,
    InvalidDomain,
    InvalidHandle,
    InvalidConfiguration,
    InvalidChallenge,
    InvalidResponse,
    IdentityMismatch,
    BindingChanged,
    PendingVerification,
    UnsupportedCapability,
    UnsupportedOutput,
    InvalidAmount,
    SenderRequired,
    Api { code: String, msg: String },
}

impl Display for BsvHomePaymailError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth { error } => Display::fmt(error, formatter),
            Self::InvalidAlias => formatter.write_str("invalid Paymail alias"),
            Self::InvalidDomain => formatter.write_str("invalid Paymail domain"),
            Self::InvalidHandle => formatter.write_str("invalid Paymail handle"),
            Self::InvalidConfiguration => formatter.write_str("unsupported or untrusted Paymail configuration"),
            Self::InvalidChallenge => formatter.write_str("invalid Paymail binding challenge"),
            Self::InvalidResponse => formatter.write_str("invalid Paymail response"),
            Self::IdentityMismatch => formatter.write_str("restore the original Paymail identity"),
            Self::BindingChanged => formatter.write_str("the Paymail binding changed; refresh and confirm again"),
            Self::PendingVerification => formatter.write_str("the previous Paymail change is pending verification"),
            Self::UnsupportedCapability => formatter.write_str("Paymail payment destinations are unsupported"),
            Self::UnsupportedOutput => formatter.write_str("unsupported Paymail payment output"),
            Self::InvalidAmount => formatter.write_str("invalid Paymail amount"),
            Self::SenderRequired => formatter.write_str("an active sender Paymail is required"),
            Self::Api { msg, .. } => formatter.write_str(msg),
        }
    }
}

impl Error for BsvHomePaymailError {}

impl From<BsvHomePortalAuthError> for BsvHomePaymailError {
    fn from(error: BsvHomePortalAuthError) -> Self {
        match error {
            BsvHomePortalAuthError::Api { code, msg } => Self::Api { code, msg },
            error => Self::Auth { error },
        }
    }
}

impl From<BsvHomeServiceError> for BsvHomePaymailError {
    fn from(error: BsvHomeServiceError) -> Self {
        BsvHomePortalAuthError::from(error).into()
    }
}

impl From<BsvHomeCoreError> for BsvHomePaymailError {
    fn from(error: BsvHomeCoreError) -> Self {
        BsvHomePortalAuthError::from(error).into()
    }
}
