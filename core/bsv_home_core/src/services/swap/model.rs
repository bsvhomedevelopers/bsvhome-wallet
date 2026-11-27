use crate::models::custom_types::BsvHomeBigInt;
use crate::models::custom_types::BsvHomeBigUint;
use crate::services::transfer::{BsvHomeRecipient, BsvHomeTransferData};
use primitives::swap::{SwapData, SwapQuote, SwapQuoteData};
use primitives::{Asset, AssetId};
use swapper::{Quote, SwapperError};

use super::rules;
use primitives::TransactionInputType;

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeAssetRate {
    pub base_symbol: String,
    pub quote_symbol: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapRate {
    pub direct: BsvHomeAssetRate,
    pub inverse: BsvHomeAssetRate,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapQuoteSummary {
    pub quote: SwapQuote,
    pub min_receive_value: BsvHomeBigUint,
    pub eta_minutes: Option<u32>,
    pub rate: Option<BsvHomeSwapRate>,
}

#[uniffi::export]
pub fn swap_quote_summary(quote: SwapQuote, from_asset: Asset, to_asset: Asset) -> BsvHomeSwapQuoteSummary {
    BsvHomeSwapQuoteSummary {
        min_receive_value: rules::min_receive_value(&quote.to_value, quote.slippage_bps),
        eta_minutes: quote.eta_in_seconds.and_then(rules::eta_minutes),
        rate: rules::swap_rate(&from_asset, &quote.from_value, &to_asset, &quote.to_value),
        quote,
    }
}

#[uniffi::export]
pub fn swapper_quote_summary(quote: Quote, from_asset: Asset, to_asset: Asset) -> BsvHomeSwapQuoteSummary {
    swap_quote_summary(rules::swap_quote(&quote), from_asset, to_asset)
}

#[uniffi::export]
pub fn swap_quote(quote: Quote) -> SwapQuote {
    rules::swap_quote(&quote)
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapTransfer {
    pub quote: SwapQuote,
    pub data: SwapQuoteData,
    pub recipient: String,
    pub value: BsvHomeBigUint,
    pub use_max_amount: bool,
}

#[uniffi::export]
impl BsvHomeSwapTransfer {
    pub fn transfer_data(&self, from_asset: Asset, to_asset: Asset) -> BsvHomeTransferData {
        BsvHomeTransferData {
            input_type: TransactionInputType::Swap {
                from_asset,
                to_asset,
                swap_data: SwapData {
                    quote: self.quote.clone(),
                    data: self.data.clone(),
                },
            },
            recipient: BsvHomeRecipient {
                address: self.recipient.clone(),
                name: None,
                memo: self.data.memo.clone(),
                references: vec![],
            },
            value: self.value.clone().into(),
            use_max_amount: self.use_max_amount,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapPair {
    pub from_asset_id: AssetId,
    pub to_asset_id: AssetId,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapPairSuggestion {
    pub pay_asset_id: AssetId,
    pub receive_asset_id: Option<AssetId>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapButtonInput {
    pub value: BsvHomeBigInt,
    pub available_balance: BsvHomeBigInt,
    pub quote_error: Option<SwapperError>,
    pub transfer_error: Option<SwapperError>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeSwapButtonAction {
    Swap,
    RetryQuote,
    RetryTransfer,
    UseMinimumAmount { value: BsvHomeBigInt },
    InsufficientBalance,
}
