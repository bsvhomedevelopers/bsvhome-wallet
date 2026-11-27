pub mod address;
pub mod address_formatter;
pub mod alien;
pub mod api;
pub mod application;
pub mod auth;
pub mod block_explorer;
pub mod bsv_home_swapper;
mod bsv_mpc;
pub mod config;
pub mod crypto_fiat_converter;
pub mod deeplink;
pub mod device;
pub mod fee;
pub mod gateway;
pub mod keystore;
pub mod message;
pub mod mnemonic;
pub mod models;
pub mod network;
pub mod payment;
pub mod perpetual;
pub mod price;
pub mod price_alert_formatter;
pub mod service_status;
pub mod services;
pub mod signer;
pub mod siwe;
pub mod support;
#[cfg(test)]
pub(crate) mod testkit;
pub mod transaction_state;
pub mod transfer_amount;
pub mod url_action;
pub mod wallet_connect;

use alien::AlienError;

uniffi::setup_scaffolding!("bsv_home_core");

/// BsvHomeCoreError
#[derive(Debug, PartialEq, Eq, uniffi::Error)]
pub enum BsvHomeCoreError {
    AnyError { msg: String },
    SignerError { error: signer::BsvHomeSignerError, msg: String },
    Cancelled,
}

impl std::fmt::Display for BsvHomeCoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AnyError { msg } | Self::SignerError { msg, .. } => write!(f, "{}", msg),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for BsvHomeCoreError {}

impl From<uniffi::UnexpectedUniFFICallbackError> for BsvHomeCoreError {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::AnyError { msg: error.reason }
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for BsvHomeCoreError {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::AnyError { msg: error.to_string() }
    }
}

impl From<&str> for BsvHomeCoreError {
    fn from(error: &str) -> Self {
        Self::AnyError { msg: error.to_string() }
    }
}

impl From<number_formatter::NumberFormatterError> for BsvHomeCoreError {
    fn from(error: number_formatter::NumberFormatterError) -> Self {
        Self::AnyError { msg: error.to_string() }
    }
}

impl From<String> for BsvHomeCoreError {
    fn from(error: String) -> Self {
        Self::AnyError { msg: error }
    }
}

impl From<Box<dyn std::error::Error>> for BsvHomeCoreError {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        Self::AnyError { msg: error.to_string() }
    }
}

impl From<primitives::payment_decoder::PaymentDecoderError> for BsvHomeCoreError {
    fn from(error: primitives::payment_decoder::PaymentDecoderError) -> Self {
        Self::AnyError { msg: error.to_string() }
    }
}

impl From<AlienError> for BsvHomeCoreError {
    fn from(error: AlienError) -> Self {
        Self::AnyError { msg: error.to_string() }
    }
}

impl From<primitives::SignerError> for BsvHomeCoreError {
    fn from(error: primitives::SignerError) -> Self {
        Self::SignerError { msg: error.to_string(), error }
    }
}

impl From<bsv_home_keystore::KeystoreError> for BsvHomeCoreError {
    fn from(error: bsv_home_keystore::KeystoreError) -> Self {
        Self::AnyError { msg: error.to_string() }
    }
}

impl From<bsv_home_derivation::AccountDerivationError> for BsvHomeCoreError {
    fn from(error: bsv_home_derivation::AccountDerivationError) -> Self {
        Self::AnyError { msg: error.to_string() }
    }
}

impl From<serde_json::Error> for BsvHomeCoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::AnyError { msg: error.to_string() }
    }
}

impl From<std::string::FromUtf8Error> for BsvHomeCoreError {
    fn from(error: std::string::FromUtf8Error) -> Self {
        Self::AnyError { msg: error.to_string() }
    }
}
