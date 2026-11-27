use crate::models::custom_types::{BsvHomeBigInt, BsvHomeBigUint};
use number_formatter::BigNumberFormatter;
use primitives::{AssetId, asset_balance::BalanceMetadata};

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeBalanceRequirement {
    pub required: BsvHomeBigInt,
    pub available: BsvHomeBigInt,
    pub shortfall: BsvHomeBigInt,
}

impl BsvHomeBalanceRequirement {
    pub fn new(required: BsvHomeBigInt, available: BsvHomeBigInt) -> Self {
        let shortfall = (&required - &available).max(BsvHomeBigInt::ZERO);
        Self { required, available, shortfall }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeBalanceValue {
    pub value: BsvHomeBigUint,
    pub amount: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeBalanceUpdateType {
    Coin {
        available: BsvHomeBigUint,
        frozen: BsvHomeBigUint,
        reserved: BsvHomeBigUint,
        pending_unconfirmed: BsvHomeBigUint,
    },
    Token {
        available: BsvHomeBigUint,
    },
    Stake {
        staked: BsvHomeBigUint,
        pending: BsvHomeBigUint,
        rewards: BsvHomeBigUint,
        locked: BsvHomeBigUint,
        frozen: BsvHomeBigUint,
        metadata: Option<BalanceMetadata>,
    },
    Earn {
        balance: BsvHomeBigUint,
    },
    Perpetual {
        available: BsvHomeBigUint,
        reserved: BsvHomeBigUint,
        withdrawable: BsvHomeBigUint,
    },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeBalanceUpdate {
    pub asset_id: AssetId,
    pub update_type: BsvHomeBalanceUpdateType,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeAssetBalance {
    pub asset_id: AssetId,
    pub available: BsvHomeBigUint,
    pub frozen: BsvHomeBigUint,
    pub locked: BsvHomeBigUint,
    pub staked: BsvHomeBigUint,
    pub pending: BsvHomeBigUint,
    pub pending_unconfirmed: BsvHomeBigUint,
    pub rewards: BsvHomeBigUint,
    pub reserved: BsvHomeBigUint,
    pub withdrawable: BsvHomeBigUint,
    pub earn: BsvHomeBigUint,
    pub metadata: Option<BalanceMetadata>,
    pub is_active: bool,
}

impl BsvHomeAssetBalance {
    pub fn zero(asset_id: AssetId) -> Self {
        Self {
            asset_id,
            available: BsvHomeBigUint::ZERO,
            frozen: BsvHomeBigUint::ZERO,
            locked: BsvHomeBigUint::ZERO,
            staked: BsvHomeBigUint::ZERO,
            pending: BsvHomeBigUint::ZERO,
            pending_unconfirmed: BsvHomeBigUint::ZERO,
            rewards: BsvHomeBigUint::ZERO,
            reserved: BsvHomeBigUint::ZERO,
            withdrawable: BsvHomeBigUint::ZERO,
            earn: BsvHomeBigUint::ZERO,
            metadata: None,
            is_active: true,
        }
    }

    pub fn votes(&self) -> u32 {
        self.metadata.as_ref().map(|metadata| metadata.votes).unwrap_or_default()
    }

    pub fn applying(&self, update: &BsvHomeBalanceUpdate) -> Self {
        let mut balance = self.clone();
        balance.is_active = update.is_active;
        match &update.update_type {
            BsvHomeBalanceUpdateType::Coin {
                available,
                frozen,
                reserved,
                pending_unconfirmed,
            } => {
                balance.available = available.clone();
                balance.frozen = frozen.clone();
                balance.reserved = reserved.clone();
                balance.pending_unconfirmed = pending_unconfirmed.clone();
            }
            BsvHomeBalanceUpdateType::Token { available } => balance.available = available.clone(),
            BsvHomeBalanceUpdateType::Stake {
                staked,
                pending,
                rewards,
                locked,
                frozen,
                metadata,
            } => {
                balance.staked = staked.clone();
                balance.pending = pending.clone();
                balance.rewards = rewards.clone();
                balance.locked = locked.clone();
                balance.frozen = frozen.clone();
                if metadata.is_some() {
                    balance.metadata = metadata.clone();
                }
            }
            BsvHomeBalanceUpdateType::Earn { balance: earn } => balance.earn = earn.clone(),
            BsvHomeBalanceUpdateType::Perpetual {
                available,
                reserved,
                withdrawable,
            } => {
                balance.available = available.clone();
                balance.reserved = reserved.clone();
                balance.withdrawable = withdrawable.clone();
            }
        }
        balance
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeBalanceRow {
    Available { value: BsvHomeBigUint },
    Staked { value: BsvHomeBigUint },
    Earn { value: BsvHomeBigUint },
    PendingUnconfirmed { value: BsvHomeBigUint },
    Reserved { value: BsvHomeBigUint, url: Option<String> },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeBalanceRecord {
    pub asset_id: AssetId,
    pub available: BsvHomeBalanceValue,
    pub frozen: BsvHomeBalanceValue,
    pub locked: BsvHomeBalanceValue,
    pub staked: BsvHomeBalanceValue,
    pub pending: BsvHomeBalanceValue,
    pub pending_unconfirmed: BsvHomeBalanceValue,
    pub rewards: BsvHomeBalanceValue,
    pub reserved: BsvHomeBalanceValue,
    pub withdrawable: BsvHomeBalanceValue,
    pub earn: BsvHomeBalanceValue,
    pub metadata: Option<BalanceMetadata>,
    pub is_active: bool,
}

impl BsvHomeBalanceRecord {
    pub fn new(balance: BsvHomeAssetBalance, decimals: u32) -> Self {
        let value = |amount: BsvHomeBigUint| BsvHomeBalanceValue {
            amount: BigNumberFormatter::value_as_f64(&amount.to_string(), decimals).unwrap_or_default(),
            value: amount,
        };
        Self {
            asset_id: balance.asset_id,
            available: value(balance.available),
            frozen: value(balance.frozen),
            locked: value(balance.locked),
            staked: value(balance.staked),
            pending: value(balance.pending),
            pending_unconfirmed: value(balance.pending_unconfirmed),
            rewards: value(balance.rewards),
            reserved: value(balance.reserved),
            withdrawable: value(balance.withdrawable),
            earn: value(balance.earn),
            metadata: balance.metadata,
            is_active: balance.is_active,
        }
    }
}
