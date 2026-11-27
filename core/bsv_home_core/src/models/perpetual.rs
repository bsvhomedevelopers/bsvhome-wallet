use std::collections::HashMap;

use bsv_home_hypercore::models::order::OpenOrder;
use bsv_home_hypercore::models::websocket::{HyperliquidSocketMessage, PositionsDiff};
use primitives::{
    PerpetualMarketData, PerpetualPosition,
    chart::{ChartCandleStick, ChartCandleUpdate},
    perpetual::{PerpetualAccountMode, PerpetualBalance},
};

pub type BsvHomeHyperliquidOpenOrder = OpenOrder;
pub type BsvHomePositionsDiff = PositionsDiff;
pub type BsvHomePerpetualAccountMode = PerpetualAccountMode;
pub type BsvHomeChartCandleStick = ChartCandleStick;
pub type BsvHomePerpetualMarketData = PerpetualMarketData;

#[uniffi::remote(Record)]
pub struct BsvHomePositionsDiff {
    pub delete_position_ids: Vec<String>,
    pub positions: Vec<PerpetualPosition>,
}

#[uniffi::remote(Record)]
pub struct BsvHomeHyperliquidOpenOrder {
    pub coin: String,
    pub oid: u64,
    pub trigger_px: Option<f64>,
    pub limit_px: Option<f64>,
    pub is_position_tpsl: bool,
    pub order_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum BsvHomePerpetualSubscription {
    AccountState { address: String },
    SpotState { address: String },
    OpenOrders { address: String },
    Candle { symbol: String, interval: String },
    MarketData { symbol: String },
    MarketPrices,
}

pub type BsvHomeHyperliquidSocketMessage = HyperliquidSocketMessage;

#[uniffi::remote(Enum)]
pub enum BsvHomeHyperliquidSocketMessage {
    AccountState {
        balance: Option<PerpetualBalance>,
        positions: Vec<PerpetualPosition>,
    },
    SpotState {
        balance: PerpetualBalance,
    },
    OpenOrders {
        orders: Vec<BsvHomeHyperliquidOpenOrder>,
    },
    Candle {
        candle: ChartCandleUpdate,
    },
    MarketData {
        market: BsvHomePerpetualMarketData,
    },
    MarketPrices {
        prices: HashMap<String, f64>,
    },
    SubscriptionResponse {
        subscription_type: String,
    },
    Error {
        message: String,
    },
    Unknown,
}
