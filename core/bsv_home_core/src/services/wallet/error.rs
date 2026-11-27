use crate::services::error::BsvHomeServiceError;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum BsvHomeWalletImportError {
    InvalidSecretPhrase,
    InvalidSecretPhraseWords { words: Vec<String> },
    InvalidPrivateKey,
    InvalidAddress,
    MissingChain,
}

impl std::fmt::Display for BsvHomeWalletImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSecretPhrase => write!(f, "invalid secret phrase"),
            Self::InvalidSecretPhraseWords { words } => write!(f, "invalid secret phrase words: {}", words.join(", ")),
            Self::InvalidPrivateKey => write!(f, "invalid private key"),
            Self::InvalidAddress => write!(f, "invalid address"),
            Self::MissingChain => write!(f, "import requires a chain"),
        }
    }
}

impl std::error::Error for BsvHomeWalletImportError {}

impl From<BsvHomeWalletImportError> for BsvHomeServiceError {
    fn from(error: BsvHomeWalletImportError) -> Self {
        Self::WalletImport { error }
    }
}
