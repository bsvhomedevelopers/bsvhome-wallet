use super::rules;
use crate::services::balance::BsvHomeBalanceRequirement;
use primitives::{FiatProviderName, FiatTransactionStatus};

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeFiatAmountCheck {
    BelowMinimum { minimum: u32 },
    AboveMaximum { maximum: u32 },
    InsufficientBalance { requirement: BsvHomeBalanceRequirement },
    Valid,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeFiatQuoteRow {
    pub quote_id: String,
    pub provider: FiatProviderName,
    pub provider_name: String,
    pub provider_image_url: Option<String>,
    pub crypto_amount: f64,
    pub fiat_amount: f64,
    pub rate: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeFiatTransactionBadge {
    Pending,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeFiatTransactionStatus {
    pub badge: Option<BsvHomeFiatTransactionBadge>,
    pub is_dimmed: bool,
}

#[uniffi::export]
pub fn fiat_transaction_status(status: FiatTransactionStatus) -> BsvHomeFiatTransactionStatus {
    rules::transaction_status(status)
}
