use crate::services::error::BsvHomeServiceError;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum BsvHomeWalletConnectError {
    UnsupportedChains,
    InvalidOrigin,
    UnsupportedWallets,
    Service { msg: String },
}

impl std::fmt::Display for BsvHomeWalletConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedChains => write!(f, "unsupported chains"),
            Self::InvalidOrigin => write!(f, "invalid origin"),
            Self::UnsupportedWallets => write!(f, "wallets unsupported"),
            Self::Service { msg } => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for BsvHomeWalletConnectError {}

impl From<BsvHomeServiceError> for BsvHomeWalletConnectError {
    fn from(error: BsvHomeServiceError) -> Self {
        Self::Service { msg: error.to_string() }
    }
}
