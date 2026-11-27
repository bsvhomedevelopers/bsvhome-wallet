use crate::models::custom_types::{BsvHomeBigInt, BsvHomeBigUint};
use crate::payment::BsvHomePaymentRecipient;
use crate::services::balance::BsvHomeBalanceRequirement;
use primitives::{Asset, Delegation, PerpetualDirection, Resource};

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeAmountType {
    Transfer,
    Deposit,
    Withdraw,
    Stake {
        stake_type: BsvHomeAmountStakeType,
    },
    Earn {
        earn_type: BsvHomeAmountEarnType,
    },
    Perpetual {
        position: BsvHomeAmountPerpetualPosition,
        direction: PerpetualDirection,
        price: f64,
        leverage: u8,
        size_decimals: i32,
    },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeAmountTransfer {
    Send { payment: BsvHomePaymentRecipient },
    Deposit,
    Withdraw,
}

#[uniffi::export]
impl BsvHomeAmountTransfer {
    pub fn amount_type(&self) -> BsvHomeAmountType {
        super::rules::transfer_amount_type(self)
    }

    pub fn display_asset(&self, asset: Asset) -> Asset {
        super::rules::transfer_display_asset(self, asset)
    }

    pub fn prefilled_amount(&self) -> Option<String> {
        super::rules::transfer_prefilled_amount(self)
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeAmountStakeType {
    Stake,
    Unstake { delegation: Delegation },
    Redelegate { delegation: Delegation },
    Withdraw { delegation: Delegation },
    Rewards { delegations: Vec<Delegation> },
    Freeze { resource: Resource },
    Unfreeze { resource: Resource },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
#[allow(clippy::large_enum_variant)]
pub enum BsvHomeAmountEarnType {
    Deposit,
    Withdraw { delegation: Delegation },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeAmountPerpetualPosition {
    Open,
    Increase,
    Reduce { available: BsvHomeBigUint },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeAmountInput {
    pub available_value: BsvHomeBigInt,
    pub max_value: BsvHomeBigInt,
    pub reserved_fee: Option<BsvHomeBigInt>,
    pub can_change_value: bool,
    pub shows_asset_balance: bool,
    pub uses_whole_amounts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomeAmountInputType {
    Asset,
    Fiat,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeAmountEquivalent {
    Fiat { amount: f64 },
    Asset { value: BsvHomeBigInt },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeAmountEntry {
    pub value: Option<BsvHomeBigInt>,
    pub error: Option<BsvHomeAmountError>,
    pub equivalent: Option<BsvHomeAmountEquivalent>,
    pub is_max: bool,
    pub reserved_fee: Option<BsvHomeBigInt>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeAmountMaxEntry {
    pub input_type: BsvHomeAmountInputType,
    pub value: BsvHomeBigInt,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePerpetualAutoclose {
    pub take_profit: Option<f64>,
    pub stop_loss: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Error)]
pub enum BsvHomeAmountError {
    InvalidNumber,
    PriceMissing,
    Zero,
    BelowMinimum { asset: Asset, minimum: BsvHomeBigInt },
    InsufficientBalance { asset: Asset, requirement: BsvHomeBalanceRequirement },
}

impl std::fmt::Display for BsvHomeAmountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidNumber => write!(f, "amount is not a number"),
            Self::PriceMissing => write!(f, "amount needs a price to convert from fiat"),
            Self::Zero => write!(f, "amount must be positive"),
            Self::BelowMinimum { asset, minimum } => write!(f, "amount is below the minimum {minimum} {}", asset.symbol),
            Self::InsufficientBalance { asset, requirement } => write!(f, "amount exceeds the available {} balance {}", asset.symbol, requirement.available),
        }
    }
}

impl std::error::Error for BsvHomeAmountError {}
