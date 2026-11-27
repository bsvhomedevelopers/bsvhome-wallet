use super::rules;
use crate::models::custom_types::BsvHomeBigInt;
use crate::services::amount::model::BsvHomeAmountType;
use crate::services::amount::rules as amount_rules;
use crate::services::error::BsvHomeServiceError;
use crate::services::transfer::BsvHomeTransferData;
use primitives::{Delegation, DelegationState, DelegationValidator, Resource, StakeType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeDelegationTone {
    Positive,
    Pending,
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeDelegationCompletion {
    ActiveIn,
    AvailableIn,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct BsvHomeDelegationStatus {
    pub state: DelegationState,
    pub tone: BsvHomeDelegationTone,
    pub completion: Option<BsvHomeDelegationCompletion>,
}

#[uniffi::export]
pub fn delegation_status(delegation: Delegation) -> BsvHomeDelegationStatus {
    rules::delegation_status(&delegation)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeDelegationAction {
    Stake,
    Unstake,
    Redelegate,
    Withdraw,
    Deposit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeStakeAction {
    Stake,
    Freeze,
    Unfreeze,
    ClaimRewards,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeStakeActionItem {
    pub action: BsvHomeStakeAction,
    pub is_enabled: bool,
    pub requires_frozen_balance: bool,
}

#[derive(Debug, Clone, uniffi::Enum)]
#[allow(clippy::large_enum_variant)]
pub enum BsvHomeDelegationDestination {
    Details,
    Withdraw { transfer: BsvHomeTransferData },
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeClaimRewards {
    pub value: BsvHomeBigInt,
    pub destination: BsvHomeClaimRewardsDestination,
}

#[derive(Debug, Clone, uniffi::Enum)]
#[allow(clippy::large_enum_variant)]
pub enum BsvHomeClaimRewardsDestination {
    Transfer { transfer: BsvHomeTransferData },
    Amount { delegations: Vec<Delegation> },
}

#[derive(Debug, Clone, uniffi::Enum)]
#[allow(clippy::large_enum_variant)]
pub enum BsvHomeStakeAmountInput {
    Stake {
        validators: Vec<DelegationValidator>,
        validator: Option<DelegationValidator>,
    },
    Redelegate {
        validators: Vec<DelegationValidator>,
        delegation: Delegation,
        validator: Option<DelegationValidator>,
    },
    Unstake {
        delegation: Delegation,
    },
    Withdraw {
        delegation: Delegation,
    },
    Rewards {
        delegations: Vec<Delegation>,
        validator: Option<DelegationValidator>,
    },
    Freeze {
        resource: Resource,
    },
    Unfreeze {
        resource: Resource,
    },
}

#[uniffi::export]
impl BsvHomeStakeAmountInput {
    pub fn amount_type(&self) -> BsvHomeAmountType {
        amount_rules::stake_amount_type(self)
    }

    pub fn stake_type(&self) -> Result<StakeType, BsvHomeServiceError> {
        rules::stake_type(self)
    }

    pub fn with_validator(&self, validator: DelegationValidator) -> BsvHomeStakeAmountInput {
        rules::with_validator(self, validator)
    }

    pub fn with_resource(&self, resource: Resource) -> BsvHomeStakeAmountInput {
        rules::with_resource(self, resource)
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeStakeValidatorSelection {
    pub options: Vec<DelegationValidator>,
    pub recommended: Vec<DelegationValidator>,
    pub validator: Option<DelegationValidator>,
    pub can_select: bool,
}
