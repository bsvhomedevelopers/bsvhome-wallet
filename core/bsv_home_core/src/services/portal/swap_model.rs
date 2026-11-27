use std::sync::Arc;

use chrono::{DateTime, Utc};
use primitives::{AssetId, Chain, WalletId};
use serde::{Deserialize, Serialize};

use crate::models::custom_types::BsvHomeBigUint;
use crate::services::confirm::BsvHomeTransactionCheckpoint;
use crate::services::transfer::BsvHomeTransferData;

use super::model::{deserialize_optional_datetime, deserialize_optional_string, deserialize_u64};

pub(super) const USDT_CONTRACT: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum BsvHomePortalSwapDirection {
    BsvToUsdt,
    UsdtToBsv,
    BsvToTrx,
    TrxToBsv,
    UsdtToTrx,
    TrxToUsdt,
}

impl BsvHomePortalSwapDirection {
    pub fn for_asset(asset_id: AssetId) -> Option<Self> {
        if asset_id == Chain::BitcoinSv.as_asset_id() {
            Some(Self::BsvToUsdt)
        } else if asset_id == AssetId::from(Chain::Tron, Some(USDT_CONTRACT.into())) {
            Some(Self::UsdtToBsv)
        } else {
            None
        }
    }

    pub fn reversed(&self) -> Self {
        match self {
            Self::BsvToUsdt => Self::UsdtToBsv,
            Self::UsdtToBsv => Self::BsvToUsdt,
            Self::BsvToTrx => Self::TrxToBsv,
            Self::TrxToBsv => Self::BsvToTrx,
            Self::UsdtToTrx => Self::TrxToUsdt,
            Self::TrxToUsdt => Self::UsdtToTrx,
        }
    }

    pub fn for_pair(from: &AssetId, to: &AssetId) -> Option<Self> {
        match (from.chain, from.token_id.as_deref(), to.chain, to.token_id.as_deref()) {
            (Chain::BitcoinSv, None, Chain::Tron, Some(USDT_CONTRACT)) => Some(Self::BsvToUsdt),
            (Chain::Tron, Some(USDT_CONTRACT), Chain::BitcoinSv, None) => Some(Self::BsvToUsdt.reversed()),
            (Chain::BitcoinSv, None, Chain::Tron, None) => Some(Self::BsvToTrx),
            (Chain::Tron, None, Chain::BitcoinSv, None) => Some(Self::BsvToTrx.reversed()),
            (Chain::Tron, Some(USDT_CONTRACT), Chain::Tron, None) => Some(Self::UsdtToTrx),
            (Chain::Tron, None, Chain::Tron, Some(USDT_CONTRACT)) => Some(Self::UsdtToTrx.reversed()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct BsvHomePortalSwapAsset {
    pub asset_id: AssetId,
    pub symbol: String,
    pub network: String,
    pub scale: u32,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePortalSwapPair {
    pub direction: BsvHomePortalSwapDirection,
    pub from: BsvHomePortalSwapAsset,
    pub to: BsvHomePortalSwapAsset,
    pub provider_display_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct BsvHomePortalSwapQuote {
    pub quote_id: String,
    pub direction: BsvHomePortalSwapDirection,
    pub from: BsvHomePortalSwapAsset,
    pub to: BsvHomePortalSwapAsset,
    pub from_amount: BsvHomeBigUint,
    pub from_amount_display: String,
    pub estimated_to_amount: BsvHomeBigUint,
    pub estimated_to_amount_display: String,
    pub min_from_amount_display: String,
    pub max_from_amount_display: Option<String>,
    pub rate_display: String,
    pub provider_display_name: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BsvHomePortalSwapOrderStatus {
    Creating,
    CreateUnknown,
    AwaitingSignature,
    AwaitingDeposit,
    Confirming,
    Exchanging,
    Sending,
    OnHold,
    Completed,
    Expired,
    Failed,
    RefundPending,
    Refunding,
    Refunded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BsvHomePortalSwapVerificationStatus {
    NotRequired,
    Required,
    InProgress,
    UnderReview,
    AdditionalInfoRequired,
    Approved,
    Rejected,
    Expired,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BsvHomePortalSwapSyncStatus {
    Fresh,
    Stale,
    Reconciling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BsvHomePortalSwapResolutionStatus {
    Open,
    Settled,
    NeedsReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BsvHomePortalSwapActionType {
    VerifyIdentity,
    ContinueVerification,
    SubmitAdditionalInfo,
    ContactSupport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BsvHomePortalSwapEntryMode {
    DirectLink,
    ProviderOrderPage,
    SupportOnly,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct BsvHomePortalSwapAction {
    pub action_id: String,
    pub action_type: BsvHomePortalSwapActionType,
    pub version: u64,
    pub entry_mode: BsvHomePortalSwapEntryMode,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Record)]
pub struct BsvHomePortalSwapOrder {
    pub order_id: String,
    pub client_operation_id: Option<String>,
    pub direction: Option<BsvHomePortalSwapDirection>,
    pub from: Option<BsvHomePortalSwapAsset>,
    pub to: Option<BsvHomePortalSwapAsset>,
    pub from_amount: BsvHomeBigUint,
    pub from_amount_display: Option<String>,
    pub estimated_to_amount: BsvHomeBigUint,
    pub estimated_to_amount_display: Option<String>,
    pub actual_to_amount_display: Option<String>,
    pub actual_to_amount: Option<BsvHomeBigUint>,
    pub payout_verified: bool,
    pub deposit_address: Option<String>,
    pub deposit_extra_id: Option<String>,
    pub payout_address: String,
    pub refund_address: String,
    pub provider_display_name: String,
    pub provider_order_id: Option<String>,
    pub order_status: BsvHomePortalSwapOrderStatus,
    pub verification_status: BsvHomePortalSwapVerificationStatus,
    pub sync_status: BsvHomePortalSwapSyncStatus,
    pub resolution_status: BsvHomePortalSwapResolutionStatus,
    pub last_active_phase: Option<BsvHomePortalSwapOrderStatus>,
    pub source_txid: Option<String>,
    pub broadcast_report_pending: bool,
    pub payout_txid: Option<String>,
    pub refund_txid: Option<String>,
    pub deposit_expires_at: Option<i64>,
    pub updated_at: i64,
    pub version: u64,
    pub reason_code: Option<String>,
    pub required_actions: Vec<BsvHomePortalSwapAction>,
    pub can_pay: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePortalSwapOrderPage {
    pub total: u64,
    pub records: Vec<BsvHomePortalSwapOrder>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePortalSwapActionNavigation {
    pub navigation_url: String,
    pub entry_mode: BsvHomePortalSwapEntryMode,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePortalSwapPaymentOrder {
    pub order_id: String,
    pub wallet_id: WalletId,
    pub from_asset_id: AssetId,
    pub from_amount: BsvHomeBigUint,
    pub deposit_address: String,
    pub deposit_extra_id: Option<String>,
    pub payout_asset_id: AssetId,
    pub payout_address: String,
    pub refund_address: String,
    pub estimated_to_amount: BsvHomeBigUint,
    pub provider_display_name: String,
    pub deposit_expires_at: Option<i64>,
    pub version: u64,
}

#[derive(uniffi::Record)]
pub struct BsvHomePortalSwapPaymentPreparation {
    pub payment: BsvHomePortalSwapPaymentOrder,
    pub order: BsvHomePortalSwapOrder,
    pub transfer: BsvHomeTransferData,
    pub checkpoint: Arc<dyn BsvHomeTransactionCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePortalSwapPaymentRecovery {
    pub order: BsvHomePortalSwapOrder,
    pub can_rebroadcast: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireAsset {
    pub chain: String,
    pub kind: String,
    pub contract: Option<String>,
    pub symbol: String,
    pub network: String,
    pub scale: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WirePair {
    pub from: WireAsset,
    pub to: WireAsset,
    pub provider_code: String,
    pub provider_display_name: String,
    pub floating_rate: bool,
    pub amount_from: bool,
    pub amount_to: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QuoteRequest {
    pub from_chain: String,
    pub from_kind: String,
    pub from_contract: String,
    pub to_chain: String,
    pub to_kind: String,
    pub to_contract: String,
    pub from_amount: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireQuote {
    pub quote_id: String,
    pub from_chain: String,
    pub from_kind: String,
    pub from_contract: Option<String>,
    pub to_chain: String,
    pub to_kind: String,
    pub to_contract: Option<String>,
    pub from_amount: String,
    pub estimated_to_amount: String,
    pub min_from_amount: String,
    pub max_from_amount: Option<String>,
    pub provider_code: String,
    pub provider_display_name: String,
    pub floating_rate: bool,
    pub expires_at: DateTime<Utc>,
    pub quote_nature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct OrderRequest {
    pub quote_id: String,
    pub client_operation_id: String,
    pub payout_address: String,
    pub refund_address: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct PendingOperation {
    pub request: OrderRequest,
    pub quote: BsvHomePortalSwapQuote,
    pub order_id: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
pub(super) struct SwapState {
    pub operations: Vec<PendingOperation>,
    pub orders: Vec<BsvHomePortalSwapOrder>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireOrder {
    pub order_id: String,
    pub client_operation_id: Option<String>,
    pub from_chain: Option<String>,
    pub from_kind: Option<String>,
    pub from_contract: Option<String>,
    pub to_chain: Option<String>,
    pub to_kind: Option<String>,
    pub to_contract: Option<String>,
    pub from_amount: String,
    pub estimated_to_amount: String,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub actual_to_amount: Option<String>,
    pub deposit_address: Option<String>,
    pub deposit_extra_id: Option<String>,
    pub payout_address: String,
    pub refund_address: String,
    pub provider_display_name: String,
    pub provider_order_id: Option<String>,
    pub order_status: BsvHomePortalSwapOrderStatus,
    pub verification_status: BsvHomePortalSwapVerificationStatus,
    pub sync_status: BsvHomePortalSwapSyncStatus,
    pub resolution_status: BsvHomePortalSwapResolutionStatus,
    pub last_active_phase: Option<BsvHomePortalSwapOrderStatus>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub source_txid: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub payout_txid: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub refund_txid: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub deposit_expires_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    #[serde(deserialize_with = "deserialize_u64")]
    pub version: u64,
    pub reason_code: Option<String>,
    pub required_actions: Vec<WireAction>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireAction {
    pub action_id: String,
    #[serde(rename = "type")]
    pub action_type: BsvHomePortalSwapActionType,
    #[serde(deserialize_with = "deserialize_u64")]
    pub version: u64,
    pub entry_mode: BsvHomePortalSwapEntryMode,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub(super) struct WireOrderPage {
    #[serde(deserialize_with = "deserialize_u64")]
    pub total: u64,
    pub records: Vec<WireOrder>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WireNavigation {
    pub navigation_url: String,
    pub entry_mode: BsvHomePortalSwapEntryMode,
    pub expires_at: Option<DateTime<Utc>>,
}
