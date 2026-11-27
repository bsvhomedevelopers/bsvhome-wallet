use crate::models::custom_types::BsvHomeBigUint;
use crate::services::swap::model::BsvHomeSwapRate;
use primitives::{AddressName, Asset, AssetId, AssetPrice, NFTAssetId, PerpetualDirection, Resource, TransactionExtended, TransactionType};

use super::rules;
use primitives::BlockExplorerLink;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum BsvHomeTransactionFilter {
    Transfers,
    Swaps,
    Stake,
    SmartContract,
    Perpetuals,
    Others,
}

#[uniffi::export]
impl BsvHomeTransactionFilter {
    pub fn transaction_types(&self) -> Vec<TransactionType> {
        rules::filter_transaction_types(*self)
    }
}

#[uniffi::export]
pub fn transaction_filters() -> Vec<BsvHomeTransactionFilter> {
    rules::transaction_filters()
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeTransactionTitle {
    Received,
    Sent,
    Transfer,
    SmartContract,
    Swap,
    Approve,
    Stake,
    Unstake,
    Redelegate,
    Rewards,
    Withdraw,
    ActivateAsset,
    Freeze,
    Unfreeze,
    Earn,
    PerpetualOpen { direction: Option<PerpetualDirection> },
    PerpetualClose { direction: Option<PerpetualDirection> },
    PerpetualModify,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BsvHomeTransactionSubtitle {
    None,
    ToAddress { address: String },
    FromAddress { address: String },
    ToResource { resource: Resource },
    FromResource { resource: Resource },
    Price { value: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAmountSign {
    None,
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BsvHomeTransactionValue {
    None,
    AssetSymbol,
    Amount { sign: BsvHomeAmountSign },
    SwapReceived,
    SwapSpent,
    PerpetualNotional,
    PerpetualPnl { value: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeTransactionParticipantRole {
    Sender,
    Recipient,
    Contract,
    Validator,
    Provider,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeTransactionParticipant {
    pub role: BsvHomeTransactionParticipantRole,
    pub address: String,
    pub name: Option<AddressName>,
    pub link: BlockExplorerLink,
    pub can_add_contact: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeTransactionAmount {
    pub asset: Asset,
    pub value: BsvHomeBigUint,
    pub sign: BsvHomeAmountSign,
    pub price: Option<AssetPrice>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeTransactionRowSubtitle {
    None,
    ToAddress { participant: String },
    FromAddress { participant: String },
    ToResource { resource: Resource },
    FromResource { resource: Resource },
    Price { value: f64 },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeTransactionRowValue {
    None,
    AssetSymbol { asset: Asset },
    Amount { amount: BsvHomeTransactionAmount },
    Fiat { value: f64 },
    Pnl { value: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomeTransactionStateTone {
    Pending,
    Success,
    Error,
    Refunded,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct BsvHomeTransactionStatus {
    pub tone: BsvHomeTransactionStateTone,
    pub shows_badge: bool,
    pub shows_progress: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeTransactionRow {
    pub status: BsvHomeTransactionStatus,
    pub title: BsvHomeTransactionTitle,
    pub subtitle: BsvHomeTransactionRowSubtitle,
    pub value: BsvHomeTransactionRowValue,
    pub equivalent_value: BsvHomeTransactionRowValue,
    pub nft_image_url: Option<String>,
}

#[uniffi::export]
pub fn transactions_list_limit() -> u32 {
    primitives::TRANSACTIONS_LIMIT as u32
}

#[uniffi::export]
pub fn transaction_row(transaction: TransactionExtended) -> BsvHomeTransactionRow {
    rules::row(&transaction)
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeTransactionHeader {
    Amount { amount: BsvHomeTransactionAmount, shows_fiat: bool },
    Swap { from: BsvHomeTransactionAmount, to: BsvHomeTransactionAmount },
    Nft { asset_id: NFTAssetId, name: Option<String>, image_url: String },
    Symbol { asset: Asset },
    AssetImage { asset: Asset },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeTransactionHeaderAction {
    Asset { asset_id: AssetId },
    Nft { asset_id: NFTAssetId },
    Swap { from_asset_id: AssetId, to_asset_id: AssetId },
    Perpetual { asset_id: AssetId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeTransactionDetailRow {
    Header,
    SwapProgress,
    SwapAgain,
    Date,
    Status,
    EstimatedConfirmation,
    Participant,
    Memo,
    Resource,
    Rate,
    Network,
    Provider,
    Pnl,
    Price,
    Fee,
    Explorer,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeTransactionDetailSection {
    pub rows: Vec<BsvHomeTransactionDetailRow>,
}

#[uniffi::export]
pub fn transaction_detail_sections(rows: BsvHomeTransactionDetailRows) -> Vec<BsvHomeTransactionDetailSection> {
    rules::detail_sections(&rows)
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeTransactionDetailRows {
    pub status: BsvHomeTransactionStatus,
    pub title: BsvHomeTransactionTitle,
    pub header: BsvHomeTransactionHeader,
    pub header_action: Option<BsvHomeTransactionHeaderAction>,
    pub swap_progress: Option<BsvHomeSwapProgress>,
    pub swap_again: Option<BsvHomeSwapAgain>,
    pub estimated_confirmation_seconds: Option<u32>,
    pub participant: Option<BsvHomeTransactionParticipant>,
    pub provider_name: Option<String>,
    pub memo: Option<String>,
    pub resource: Option<Resource>,
    pub rate: Option<BsvHomeSwapRate>,
    pub pnl: Option<f64>,
    pub price: Option<f64>,
    pub fee: BsvHomeTransactionAmount,
    pub explorer: BlockExplorerLink,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeTransactionHeaderKind {
    Amount { shows_fiat: bool },
    Swap,
    Nft,
    Symbol,
    AssetImage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BsvHomeTransactionDetails {
    pub swap_progress: Option<BsvHomeSwapProgress>,
    pub swap_again: Option<BsvHomeSwapAgain>,
    pub provider_name: Option<String>,
    pub estimated_confirmation_seconds: Option<u32>,
    pub pnl: Option<f64>,
    pub price: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapProgress {
    pub from_asset: Asset,
    pub from_value: BsvHomeBigUint,
    pub provider_name: String,
    pub transfer: BsvHomeSwapProgressStep,
    pub swap: BsvHomeSwapProgressStep,
    pub eta_seconds: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeSwapProgressStep {
    Pending,
    Waiting,
    Completed,
    Failed,
    Reverted,
    Refunded,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapAgain {
    pub from_asset_id: AssetId,
    pub to_asset_id: AssetId,
}
