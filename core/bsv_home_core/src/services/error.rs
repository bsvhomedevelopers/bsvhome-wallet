use crate::BsvHomeCoreError;
use crate::api::BsvHomeApiError;
use crate::gateway::GatewayError;
use crate::services::wallet::BsvHomeWalletImportError;

#[derive(Debug, Clone, PartialEq, uniffi::Error)]
pub enum BsvHomeServiceError {
    Api { msg: String },
    Gateway { msg: String },
    Store { msg: String },
    Core { msg: String },
    Platform { msg: String },
    InvalidInput { msg: String },
    NotFound { msg: String },
    Unsupported { msg: String },
    WalletImport { error: BsvHomeWalletImportError },
    Cancelled,
}

impl std::fmt::Display for BsvHomeServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Api { msg }
            | Self::Gateway { msg }
            | Self::Store { msg }
            | Self::Core { msg }
            | Self::Platform { msg }
            | Self::InvalidInput { msg }
            | Self::NotFound { msg }
            | Self::Unsupported { msg } => write!(f, "{msg}"),
            Self::WalletImport { error } => write!(f, "{error}"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for BsvHomeServiceError {}

impl From<uniffi::UnexpectedUniFFICallbackError> for BsvHomeServiceError {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Platform { msg: error.reason }
    }
}

impl From<BsvHomeApiError> for BsvHomeServiceError {
    fn from(error: BsvHomeApiError) -> Self {
        Self::Api { msg: error.to_string() }
    }
}

impl From<GatewayError> for BsvHomeServiceError {
    fn from(error: GatewayError) -> Self {
        Self::Gateway { msg: error.to_string() }
    }
}

impl From<BsvHomeCoreError> for BsvHomeServiceError {
    fn from(error: BsvHomeCoreError) -> Self {
        Self::Core { msg: error.to_string() }
    }
}
