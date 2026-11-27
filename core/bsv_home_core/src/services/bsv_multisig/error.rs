use super::super::portal::BsvHomePortalAuthError;
use primitives::SignerError;
use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum BsvHomeBsvMultisigError {
    InvalidPackage { reason: String },
    InvalidRules,
    ConfigurationConflict,
    ConcurrentUpdate,
    MissingSigner,
    NotActivated,
    MissingRecord,
    Provider { reason: String },
    Storage { reason: String },
    Authentication { reason: String },
    Api { code: String },
    ConfigurationNotConfirmed,
    WalletChanged,
    WatchOnly,
    FundingExpired,
    FundingPending,
    FundingChanged,
    FundingPayerMissing,
    Cancelled,
    CreationUncertain { draft_id: String },
}
impl fmt::Display for BsvHomeBsvMultisigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPackage { reason } | Self::Provider { reason } | Self::Storage { reason } | Self::Authentication { reason } => reason,
            Self::Api { code } => code,
            Self::InvalidRules => "INVALID_POLICY",
            Self::ConfigurationConflict => "CONFIGURATION_CHANGED",
            Self::ConcurrentUpdate => "STALE_VERSION",
            Self::MissingSigner => "SIGNER_IDENTITY_MISMATCH",
            Self::NotActivated => "WALLET_NOT_ACTIVE",
            Self::MissingRecord => "WALLET_NOT_FOUND",
            Self::ConfigurationNotConfirmed => "CONFIGURATION_NOT_CONFIRMED",
            Self::WalletChanged => "WALLET_CHANGED",
            Self::WatchOnly => "WATCH_ONLY",
            Self::FundingExpired => "FUNDING_EXPIRED",
            Self::FundingPending => "FUNDING_PENDING",
            Self::FundingChanged => "FUNDING_CHANGED",
            Self::FundingPayerMissing => "FUNDING_PAYER_MISSING",
            Self::Cancelled => "CANCELLED",
            Self::CreationUncertain { .. } => "CREATION_UNCERTAIN",
        })
    }
}
impl Error for BsvHomeBsvMultisigError {}
impl From<serde_json::Error> for BsvHomeBsvMultisigError {
    fn from(_: serde_json::Error) -> Self {
        Self::invalid("INVALID_REQUEST")
    }
}
impl From<SignerError> for BsvHomeBsvMultisigError {
    fn from(error: SignerError) -> Self {
        Self::invalid(error.to_string())
    }
}
impl From<BsvHomePortalAuthError> for BsvHomeBsvMultisigError {
    fn from(error: BsvHomePortalAuthError) -> Self {
        match error {
            BsvHomePortalAuthError::Api { code, .. } => Self::Api { code },
            BsvHomePortalAuthError::WalletChanged => Self::WalletChanged,
            BsvHomePortalAuthError::WatchOnly => Self::WatchOnly,
            BsvHomePortalAuthError::Service {
                error: crate::services::BsvHomeServiceError::Cancelled,
            } => Self::Cancelled,
            other => Self::Authentication { reason: other.to_string() },
        }
    }
}
impl BsvHomeBsvMultisigError {
    pub(crate) fn invalid(reason: impl Into<String>) -> Self {
        Self::InvalidPackage { reason: reason.into() }
    }
}
