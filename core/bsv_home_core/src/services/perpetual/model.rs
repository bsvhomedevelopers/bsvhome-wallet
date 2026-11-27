use super::rules;
use crate::models::custom_types::BsvHomeBigInt;
use crate::perpetual::BsvHomePerpetual;
use crate::services::failures::StepFailure;
use crate::services::transfer::model::BsvHomeRecipient;
use primitives::chart::{ChartCandleStick, ChartCandleUpdate};
use primitives::{Asset, PerpetualAccountMode, PerpetualConfirmData, PerpetualDirection, PerpetualMarginType, PerpetualPosition, PerpetualProvider, PerpetualType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomePerpetualSocketUpdate {
    Applied,
    Candle { candle: ChartCandleUpdate },
    SubscriptionResponse { subscription_type: String },
    Error { message: String },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomePerpetualOrderAction {
    Open,
    Increase,
    Reduce { position_direction: PerpetualDirection },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePerpetualOrderInput {
    pub action: BsvHomePerpetualOrderAction,
    pub direction: PerpetualDirection,
    pub margin_type: PerpetualMarginType,
    pub base_asset: Asset,
    pub asset: Asset,
    pub asset_index: i32,
    pub price: f64,
    pub usdc_value: BsvHomeBigInt,
    pub usdc_decimals: i32,
    pub leverage: u8,
    pub slippage: Option<f64>,
    pub take_profit: Option<String>,
    pub stop_loss: Option<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePerpetualCloseInput {
    pub asset_index: i32,
    pub direction: PerpetualDirection,
    pub margin_type: PerpetualMarginType,
    pub base_asset: Asset,
    pub asset: Asset,
    pub market_price: f64,
    pub size: f64,
    pub leverage: u8,
    pub pnl: f64,
    pub entry_price: f64,
    pub margin_amount: f64,
    pub slippage: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePerpetualConnection {
    pub address: String,
    pub mode: PerpetualAccountMode,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeAutocloseSummary {
    pub take_profit: Option<f64>,
    pub stop_loss: Option<f64>,
    pub take_profit_cleared: bool,
    pub stop_loss_cleared: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomePerpetualDetailsAction {
    Open,
    Close,
    Increase,
    Reduce,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomePerpetualDetails {
    pub action: BsvHomePerpetualDetailsAction,
    pub direction: PerpetualDirection,
    pub data: PerpetualConfirmData,
}

#[uniffi::export]
pub fn perpetual_details(perpetual_type: PerpetualType) -> Option<BsvHomePerpetualDetails> {
    rules::details(&perpetual_type)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomePerpetualChartLineKind {
    Entry,
    TakeProfit,
    StopLoss,
    Liquidation,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct BsvHomePerpetualChartLine {
    pub kind: BsvHomePerpetualChartLineKind,
    pub price: f64,
    pub overlap_level: u32,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePerpetualChartLayout {
    pub price_low: f64,
    pub price_high: f64,
    pub ticks: Vec<f64>,
    pub lines: Vec<BsvHomePerpetualChartLine>,
}

#[uniffi::export]
pub fn perpetual_chart_layout(candles: Vec<ChartCandleStick>, position: Option<PerpetualPosition>) -> BsvHomePerpetualChartLayout {
    rules::chart_layout(&candles, position.as_ref())
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomeMarketsRefreshTrigger {
    Scheduled,
    UserRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomePerpetualRefreshStep {
    Positions,
    Markets,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomePerpetualRefreshFailure {
    pub step: BsvHomePerpetualRefreshStep,
    pub message: String,
}

impl StepFailure for BsvHomePerpetualRefreshFailure {
    type Step = BsvHomePerpetualRefreshStep;

    fn new(step: BsvHomePerpetualRefreshStep, message: String) -> Self {
        Self { step, message }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
pub enum BsvHomePerpetualPositionKind {
    Open { direction: PerpetualDirection },
    Increase,
    Reduce,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct BsvHomePerpetualTransferData {
    pub provider: PerpetualProvider,
    pub direction: PerpetualDirection,
    pub asset: Asset,
    pub base_asset: Asset,
    pub asset_index: i32,
    pub price: f64,
    pub leverage: u8,
    pub margin_type: PerpetualMarginType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
#[allow(clippy::large_enum_variant)]
pub enum BsvHomePerpetualPositionAction {
    Open { data: BsvHomePerpetualTransferData },
    Increase { data: BsvHomePerpetualTransferData },
    Reduce { data: BsvHomePerpetualTransferData, position: PerpetualPosition },
}

#[uniffi::export]
impl BsvHomePerpetualPositionAction {
    pub fn transfer_data(&self) -> BsvHomePerpetualTransferData {
        self.data().clone()
    }

    pub fn recipient(&self) -> BsvHomeRecipient {
        BsvHomePerpetual::new(self.data().provider.clone()).recipient()
    }

    pub fn shows_autoclose(&self) -> bool {
        matches!(self, Self::Open { .. })
    }
}

impl BsvHomePerpetualPositionAction {
    pub fn data(&self) -> &BsvHomePerpetualTransferData {
        match self {
            Self::Open { data } | Self::Increase { data } | Self::Reduce { data, .. } => data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_action_recipient_names_the_provider_without_an_address() {
        let data = BsvHomePerpetualTransferData {
            provider: PerpetualProvider::Hypercore,
            direction: PerpetualDirection::Long,
            asset: Asset::mock(),
            base_asset: Asset::mock(),
            asset_index: 0,
            price: 100.0,
            leverage: 3,
            margin_type: PerpetualMarginType::Cross,
        };
        let action = BsvHomePerpetualPositionAction::Open { data };

        let recipient = action.recipient();

        assert_eq!(recipient.name.as_deref(), Some("Hyperliquid"));
        assert!(recipient.address.is_empty());
    }

    #[test]
    fn test_only_opening_a_position_shows_autoclose() {
        let data = BsvHomePerpetualTransferData {
            provider: PerpetualProvider::Hypercore,
            direction: PerpetualDirection::Long,
            asset: Asset::mock(),
            base_asset: Asset::mock(),
            asset_index: 0,
            price: 100.0,
            leverage: 3,
            margin_type: PerpetualMarginType::Cross,
        };

        assert!(BsvHomePerpetualPositionAction::Open { data: data.clone() }.shows_autoclose());
        assert!(!BsvHomePerpetualPositionAction::Increase { data }.shows_autoclose());
    }
}
