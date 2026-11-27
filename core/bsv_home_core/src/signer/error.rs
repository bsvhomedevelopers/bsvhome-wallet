use primitives::SignerError;

pub type BsvHomeSignerError = SignerError;

#[uniffi::remote(Enum)]
pub enum BsvHomeSignerError {
    InvalidInput(String),
    SigningError(String),
    DustThreshold,
    InsufficientFunds,
    SwapValueBelowMinimum,
}
