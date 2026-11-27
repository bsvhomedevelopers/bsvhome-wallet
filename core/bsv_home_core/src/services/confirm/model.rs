use super::error::BsvHomeConfirmError;
use super::rules::approval_value_from;
use crate::models::custom_types::{BsvHomeBigInt, BsvHomeBigUint};
use crate::models::gateway::BsvHomeFeeRate;
use crate::models::transaction::{BsvHomeTransactionLoadFee, BsvHomeTransactionLoadMetadata};
use crate::services::balance::BsvHomeAssetBalance;
use crate::services::simulation::BsvHomeSimulationWarningRow;
use crate::services::transactions::BsvHomeAmountSign;
use crate::services::transfer::BsvHomeTransferData;
use crate::transfer_amount::BsvHomeTransferAmount;
use primitives::AssetPrice;
use primitives::{
    Account, AddressName, Asset, AssetId, Chain, ChainAddress, FeePriority, FeeUnitType, SimulationPayloadField, SimulationPayloadFieldType, SimulationResult, Transaction, Wallet,
};

pub type BsvHomeAccount = Account;

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeConfirmInput {
    pub from: BsvHomeAccount,
    pub transfer: BsvHomeTransferData,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum BsvHomeConfirmFeeSelection {
    Priority { priority: FeePriority },
    Custom { gas_price: BsvHomeBigInt },
}

#[uniffi::export]
impl BsvHomeConfirmFeeSelection {
    pub fn selected_priority(&self) -> Option<FeePriority> {
        match self {
            Self::Priority { priority } => Some(*priority),
            Self::Custom { .. } => None,
        }
    }

    pub fn custom_gas_price(&self) -> Option<BsvHomeBigInt> {
        match self {
            Self::Priority { .. } => None,
            Self::Custom { gas_price } => Some(gas_price.clone()),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeConfirmLoadOptions {
    pub fee_selection: BsvHomeConfirmFeeSelection,
    pub fee_asset_id: Option<AssetId>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeConfirmData {
    pub input: BsvHomeConfirmInput,
    pub fee: BsvHomeTransactionLoadFee,
    pub selected_priority: FeePriority,
    pub fee_rates: Vec<BsvHomeFeeRate>,
    pub metadata: BsvHomeTransactionLoadMetadata,
    pub simulation: Option<SimulationResult>,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum BsvHomeExecuteResult {
    Signed { data: Vec<String> },
    Sent { hashes: Vec<String>, transactions: Vec<Transaction> },
}

pub(super) struct BsvHomeSendResult {
    pub(super) hashes: Vec<String>,
    pub(super) transactions: Vec<Transaction>,
}

#[derive(Debug, Clone)]
pub struct SendInput {
    pub wallet: Wallet,
    pub confirm: BsvHomeConfirmData,
    pub value: BsvHomeBigInt,
    pub network_fee: BsvHomeBigInt,
    pub simulation: Option<SimulationResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAcquireAssetFlow {
    Options,
    Fiat,
    Receive,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeConfirmMetadata {
    pub asset_balance: BsvHomeAssetBalance,
    pub fee_asset_balance: BsvHomeAssetBalance,
    pub prices: Vec<AssetPrice>,
}

#[uniffi::export]
impl BsvHomeConfirmMetadata {
    pub fn price(&self, asset_id: AssetId) -> Option<AssetPrice> {
        self.prices.iter().find(|price| price.asset_id == asset_id).cloned()
    }

    pub fn asset_price(&self) -> Option<AssetPrice> {
        self.price(self.asset_balance.asset_id.clone())
    }

    pub fn fee_price(&self) -> Option<AssetPrice> {
        self.price(self.fee_asset_balance.asset_id.clone())
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeFeeRateRow {
    pub priority: FeePriority,
    pub unit_value: BsvHomeBigInt,
    pub fee: Option<BsvHomeBigInt>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeFeeRateRows {
    pub rows: Vec<BsvHomeFeeRateRow>,
    pub unit_type: FeeUnitType,
    pub unit_decimals: u32,
    pub supports_custom_fee: bool,
    pub selected_total: Option<BsvHomeBigInt>,
    pub normal_total: Option<BsvHomeBigInt>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeFeeAsset {
    pub asset: Asset,
    pub balance: BsvHomeAssetBalance,
    pub price: Option<AssetPrice>,
}

impl BsvHomeConfirmSimulation {
    pub(super) fn address_requests(&self, chain: Chain) -> Vec<ChainAddress> {
        self.primary_fields
            .iter()
            .chain(self.secondary_fields.iter())
            .filter(|field| field.field_type == SimulationPayloadFieldType::Address)
            .map(|field| ChainAddress::new(chain, field.value.clone()))
            .collect()
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeConfirmLoad {
    pub sender: BsvHomeAccount,
    pub fee_asset: Asset,
    pub metadata: BsvHomeConfirmMetadata,
    pub fee_assets: Vec<BsvHomeFeeAsset>,
    pub simulation: BsvHomeConfirmSimulationState,
    pub address_name: Option<AddressName>,
    pub preload: Option<BsvHomeConfirmPreload>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeConfirmSimulationState {
    pub chain: Chain,
    pub result: Option<SimulationResult>,
    pub warnings: Vec<BsvHomeSimulationWarningRow>,
    pub simulation: Option<BsvHomeConfirmSimulation>,
    pub address_names: Vec<AddressName>,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum BsvHomeTransferAmountResult {
    Amount { amount: BsvHomeTransferAmount },
    Error { error: BsvHomeConfirmError },
}

#[derive(Debug, Clone)]
pub struct BsvHomeConfirmFeeLoad {
    pub fee_asset: Asset,
    pub metadata: BsvHomeConfirmMetadata,
    pub preload: BsvHomeConfirmPreload,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeConfirmPreload {
    pub confirm_data: BsvHomeConfirmData,
    pub amount: BsvHomeTransferAmountResult,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeApprovalValue {
    Exact { value: BsvHomeBigUint },
    Unlimited,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSimulationValue {
    pub asset: Asset,
    pub value: BsvHomeApprovalValue,
}

impl BsvHomeSimulationValue {
    pub(crate) fn from_simulation(simulation: &SimulationResult, assets: &[Asset]) -> Option<Self> {
        let header = simulation.valid_header()?;
        let asset = assets.iter().find(|asset| asset.id == header.asset_id)?.clone();
        Some(Self {
            asset,
            value: approval_value_from(header.value.as_ref(), header.is_unlimited),
        })
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeSimulationBalanceChange {
    pub asset: Asset,
    pub value: BsvHomeBigInt,
    pub sign: BsvHomeAmountSign,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeConfirmSimulation {
    pub primary_fields: Vec<SimulationPayloadField>,
    pub secondary_fields: Vec<SimulationPayloadField>,
    pub header: Option<BsvHomeSimulationValue>,
    pub balance_changes: Vec<BsvHomeSimulationBalanceChange>,
    pub has_critical_warning: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomeConfirmPhase {
    Loading,
    Ready,
    Confirming,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeConfirmStage {
    Load,
    Execute,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeConfirmFailure {
    pub stage: BsvHomeConfirmStage,
    pub error: BsvHomeConfirmError,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeConfirmScreen {
    pub phase: BsvHomeConfirmPhase,
    pub amount_failed: bool,
    pub has_critical_warning: bool,
    pub failure: Option<BsvHomeConfirmFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeConfirmAction {
    Load,
    Execute,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomeConfirmButtonKind {
    Confirm,
    Retry,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomeConfirmButtonState {
    Disabled,
    Loading,
    Enabled,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeConfirmButton {
    pub kind: BsvHomeConfirmButtonKind,
    pub state: BsvHomeConfirmButtonState,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomeConfirmFeeRow {
    Loading,
    Ready,
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_pairs_each_balance_with_its_own_price() {
        let balance = |chain: primitives::Chain| BsvHomeAssetBalance {
            asset_id: AssetId::from_chain(chain),
            ..BsvHomeAssetBalance::mock()
        };
        let price = |chain: primitives::Chain, value: f64| AssetPrice {
            asset_id: AssetId::from_chain(chain),
            price: value,
            price_change_percentage_24h: 0.0,
            updated_at: chrono::Utc::now(),
        };
        let metadata = BsvHomeConfirmMetadata {
            asset_balance: balance(primitives::Chain::Solana),
            fee_asset_balance: balance(primitives::Chain::Bitcoin),
            prices: vec![price(primitives::Chain::Bitcoin, 2.0), price(primitives::Chain::Solana, 1.0)],
        };

        assert_eq!(metadata.asset_price().map(|price| price.price), Some(1.0));
        assert_eq!(metadata.fee_price().map(|price| price.price), Some(2.0));
        assert_eq!(metadata.price(AssetId::from_chain(primitives::Chain::Ethereum)), None);
    }

    #[test]
    fn test_fee_selection_answers_only_for_its_own_case() {
        let priority = BsvHomeConfirmFeeSelection::Priority { priority: FeePriority::Fast };
        let custom = BsvHomeConfirmFeeSelection::Custom { gas_price: 7.into() };

        assert_eq!(priority.selected_priority(), Some(FeePriority::Fast));
        assert_eq!(priority.custom_gas_price(), None);
        assert_eq!(custom.selected_priority(), None);
        assert_eq!(custom.custom_gas_price(), Some(7.into()));
    }
}
