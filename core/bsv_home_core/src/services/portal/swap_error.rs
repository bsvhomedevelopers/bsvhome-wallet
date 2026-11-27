use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::BsvHomeCoreError;
use crate::services::error::BsvHomeServiceError;

use super::BsvHomePortalAuthError;

#[derive(Debug, Clone, PartialEq, uniffi::Error)]
pub enum BsvHomePortalSwapError {
    Auth {
        error: BsvHomePortalAuthError,
    },
    Service {
        error: BsvHomeServiceError,
    },
    InvalidResponse,
    InvalidAmount,
    UnsupportedPair,
    MissingAccount,
    QuoteExpired,
    OrderMismatch,
    PaymentUnavailable,
    BroadcastConflict,
    PendingCreation,
    ActionUnavailable,
    InsufficientBalance {
        symbol: String,
        available_display: String,
        required_display: String,
    },
    NetworkFeeUnavailable,
    PayoutUnconfirmed,
}

impl Display for BsvHomePortalSwapError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth { error } => Display::fmt(error, formatter),
            Self::Service { error } => Display::fmt(error, formatter),
            Self::InvalidResponse => formatter.write_str("invalid Portal swap response"),
            Self::InvalidAmount => formatter.write_str("invalid swap amount or precision"),
            Self::UnsupportedPair => formatter.write_str("swap pair is unavailable"),
            Self::MissingAccount => formatter.write_str("a source and destination account are required"),
            Self::QuoteExpired => formatter.write_str("the swap quote expired"),
            Self::OrderMismatch => formatter.write_str("the order does not match the confirmed swap"),
            Self::PaymentUnavailable => formatter.write_str("the order cannot be paid"),
            Self::BroadcastConflict => formatter.write_str("the order already has a different payment"),
            Self::PendingCreation => formatter.write_str("an unresolved order creation must be retried"),
            Self::ActionUnavailable => formatter.write_str("the order action is unavailable"),
            Self::InsufficientBalance { symbol, .. } => write!(formatter, "not enough {symbol} balance"),
            Self::NetworkFeeUnavailable => formatter.write_str("TRX or sufficient TRON network resources are required"),
            Self::PayoutUnconfirmed => formatter.write_str("the payout is not yet verified on the destination chain"),
        }
    }
}

impl Error for BsvHomePortalSwapError {}

impl From<BsvHomePortalAuthError> for BsvHomePortalSwapError {
    fn from(error: BsvHomePortalAuthError) -> Self {
        Self::Auth { error }
    }
}

impl From<BsvHomeServiceError> for BsvHomePortalSwapError {
    fn from(error: BsvHomeServiceError) -> Self {
        Self::Service { error }
    }
}

impl From<BsvHomeCoreError> for BsvHomePortalSwapError {
    fn from(error: BsvHomeCoreError) -> Self {
        BsvHomeServiceError::from(error).into()
    }
}
