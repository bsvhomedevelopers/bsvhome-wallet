use std::str::FromStr;

use number_formatter::BigNumberFormatter;
use primitives::{
    Asset, AssetId, AssetPrice, AssetType, BlockExplorerLink, Chain, PerpetualDirection, Price, Transaction, TransactionDirection, TransactionExtended,
    TransactionNFTTransferMetadata, TransactionPerpetualMetadata, TransactionResourceTypeMetadata, TransactionState, TransactionSwapMetadata, TransactionType,
    TransactionWalletConnectMetadata, TransferDataOutputAction,
};

use super::model::{
    BsvHomeAmountSign, BsvHomeSwapAgain, BsvHomeSwapProgress, BsvHomeSwapProgressStep, BsvHomeTransactionAmount, BsvHomeTransactionDetailRow, BsvHomeTransactionDetailRows,
    BsvHomeTransactionDetailSection, BsvHomeTransactionDetails, BsvHomeTransactionFilter, BsvHomeTransactionHeader, BsvHomeTransactionHeaderAction, BsvHomeTransactionHeaderKind,
    BsvHomeTransactionParticipant, BsvHomeTransactionParticipantRole, BsvHomeTransactionRow, BsvHomeTransactionRowSubtitle, BsvHomeTransactionRowValue,
    BsvHomeTransactionStateTone, BsvHomeTransactionStatus, BsvHomeTransactionSubtitle, BsvHomeTransactionTitle, BsvHomeTransactionValue,
};
use crate::address_formatter::{BsvHomeAddressFormatStyle, format_address};
use crate::config::image::BsvHomeImage;
use crate::models::asset::wallet_default_assets;
use crate::services::collections::unique;
use crate::services::swap::model::BsvHomeSwapRate;
use crate::services::swap::rules as swap_rules;
use swapper::{ProviderType as SwapperProviderType, SwapperProvider, SwapperProviderMode};

pub fn transaction_filters() -> Vec<BsvHomeTransactionFilter> {
    vec![
        BsvHomeTransactionFilter::Transfers,
        BsvHomeTransactionFilter::Swaps,
        BsvHomeTransactionFilter::Stake,
        BsvHomeTransactionFilter::SmartContract,
        BsvHomeTransactionFilter::Perpetuals,
        BsvHomeTransactionFilter::Others,
    ]
}

pub fn transaction_filter(transaction_type: &TransactionType) -> BsvHomeTransactionFilter {
    match transaction_type {
        TransactionType::Transfer | TransactionType::TransferNFT => BsvHomeTransactionFilter::Transfers,
        TransactionType::Swap | TransactionType::TokenApproval => BsvHomeTransactionFilter::Swaps,
        TransactionType::StakeDelegate
        | TransactionType::StakeUndelegate
        | TransactionType::StakeRewards
        | TransactionType::StakeRedelegate
        | TransactionType::StakeWithdraw
        | TransactionType::StakeFreeze
        | TransactionType::StakeUnfreeze
        | TransactionType::EarnDeposit
        | TransactionType::EarnWithdraw => BsvHomeTransactionFilter::Stake,
        TransactionType::SmartContractCall => BsvHomeTransactionFilter::SmartContract,
        TransactionType::PerpetualOpenPosition | TransactionType::PerpetualClosePosition | TransactionType::PerpetualModifyPosition => BsvHomeTransactionFilter::Perpetuals,
        TransactionType::AssetActivation => BsvHomeTransactionFilter::Others,
    }
}

pub fn filter_transaction_types(filter: BsvHomeTransactionFilter) -> Vec<TransactionType> {
    TransactionType::all()
        .into_iter()
        .filter(|transaction_type| transaction_filter(transaction_type) == filter)
        .collect()
}

pub fn pending_transactions(transactions: &[Transaction]) -> Vec<Transaction> {
    transactions.iter().filter(|transaction| !transaction.state.is_completed()).cloned().collect()
}

pub fn transaction_asset_ids(transactions: &[Transaction]) -> Vec<AssetId> {
    unique(transactions.iter().flat_map(|transaction| transaction.associated_asset_ids()))
}

pub fn row(extended: &TransactionExtended) -> BsvHomeTransactionRow {
    let transaction = &extended.transaction;
    BsvHomeTransactionRow {
        status: status(transaction.state),
        title: transaction_title(transaction),
        subtitle: row_subtitle(extended),
        value: row_value(extended, transaction_value(transaction)),
        equivalent_value: row_value(extended, transaction_equivalent_value(transaction)),
        nft_image_url: transaction.nft_asset_id().map(|asset_id| BsvHomeImage::NftAsset { asset_id: asset_id.to_string() }.url()),
    }
}

pub fn participant(extended: &TransactionExtended, link: impl FnOnce(&str) -> BlockExplorerLink) -> Option<BsvHomeTransactionParticipant> {
    let transaction = &extended.transaction;
    let (role, address) = transaction_participant(transaction)?;
    let name = address_name(extended, &address);
    let can_add_contact = name.is_none() && matches!(transaction.transaction_type, TransactionType::Transfer | TransactionType::TransferNFT);
    Some(BsvHomeTransactionParticipant {
        role,
        link: link(&address),
        name,
        address,
        can_add_contact,
    })
}

pub fn detail_rows(extended: &TransactionExtended, participant: Option<BsvHomeTransactionParticipant>, explorer: BlockExplorerLink) -> BsvHomeTransactionDetailRows {
    let transaction = &extended.transaction;
    let details = details(extended);
    BsvHomeTransactionDetailRows {
        status: status(transaction.state),
        title: transaction_title(transaction),
        header: header(extended),
        header_action: header_action(transaction),
        swap_progress: details.swap_progress,
        swap_again: details.swap_again,
        estimated_confirmation_seconds: details.estimated_confirmation_seconds,
        participant,
        provider_name: details.provider_name,
        memo: transaction.memo.clone().filter(|memo| !memo.is_empty()),
        resource: resource(transaction),
        rate: swap_rate(extended),
        pnl: details.pnl,
        price: details.price,
        fee: BsvHomeTransactionAmount {
            asset: extended.fee_asset.clone(),
            value: transaction.fee.clone(),
            sign: BsvHomeAmountSign::None,
            price: asset_price(extended.fee_price.as_ref(), &extended.fee_asset.id),
        },
        explorer,
    }
}

pub fn detail_sections(rows: &BsvHomeTransactionDetailRows) -> Vec<BsvHomeTransactionDetailSection> {
    use BsvHomeTransactionDetailRow::*;
    let details = [
        Some(Date),
        Some(Status),
        rows.estimated_confirmation_seconds.is_some().then_some(EstimatedConfirmation),
        rows.participant.is_some().then_some(Participant),
        rows.memo.is_some().then_some(Memo),
        rows.resource.is_some().then_some(Resource),
        rows.rate.is_some().then_some(Rate),
        Some(Network),
        rows.provider_name.is_some().then_some(Provider),
        rows.pnl.is_some().then_some(Pnl),
        rows.price.is_some().then_some(Price),
    ];
    [
        vec![Header],
        rows.swap_progress.is_some().then_some(SwapProgress).into_iter().collect(),
        rows.swap_again.is_some().then_some(SwapAgain).into_iter().collect(),
        details.into_iter().flatten().collect(),
        vec![Fee],
        vec![Explorer],
    ]
    .into_iter()
    .filter(|rows| !rows.is_empty())
    .map(|rows| BsvHomeTransactionDetailSection { rows })
    .collect()
}

fn row_subtitle(extended: &TransactionExtended) -> BsvHomeTransactionRowSubtitle {
    match transaction_subtitle(&extended.transaction) {
        BsvHomeTransactionSubtitle::None => BsvHomeTransactionRowSubtitle::None,
        BsvHomeTransactionSubtitle::ToAddress { address } => BsvHomeTransactionRowSubtitle::ToAddress {
            participant: participant_name(extended, &address),
        },
        BsvHomeTransactionSubtitle::FromAddress { address } => BsvHomeTransactionRowSubtitle::FromAddress {
            participant: participant_name(extended, &address),
        },
        BsvHomeTransactionSubtitle::ToResource { resource } => BsvHomeTransactionRowSubtitle::ToResource { resource },
        BsvHomeTransactionSubtitle::FromResource { resource } => BsvHomeTransactionRowSubtitle::FromResource { resource },
        BsvHomeTransactionSubtitle::Price { value } => BsvHomeTransactionRowSubtitle::Price { value },
    }
}

pub fn status(state: TransactionState) -> BsvHomeTransactionStatus {
    let tone = match state {
        TransactionState::Pending | TransactionState::InTransit => BsvHomeTransactionStateTone::Pending,
        TransactionState::Confirmed => BsvHomeTransactionStateTone::Success,
        TransactionState::Failed | TransactionState::Reverted => BsvHomeTransactionStateTone::Error,
        TransactionState::Refunded => BsvHomeTransactionStateTone::Refunded,
    };
    BsvHomeTransactionStatus {
        tone,
        shows_badge: state != TransactionState::Confirmed,
        shows_progress: tone == BsvHomeTransactionStateTone::Pending,
    }
}

fn participant_name(extended: &TransactionExtended, address: &str) -> String {
    address_name(extended, address)
        .map(|name| name.name)
        .unwrap_or_else(|| format_address(address, Some(extended.transaction.asset_id.chain), BsvHomeAddressFormatStyle::Short))
}

fn row_value(extended: &TransactionExtended, value: BsvHomeTransactionValue) -> BsvHomeTransactionRowValue {
    let transaction = &extended.transaction;
    match value {
        BsvHomeTransactionValue::None => BsvHomeTransactionRowValue::None,
        BsvHomeTransactionValue::AssetSymbol => BsvHomeTransactionRowValue::AssetSymbol { asset: extended.asset.clone() },
        BsvHomeTransactionValue::Amount { sign } => BsvHomeTransactionRowValue::Amount {
            amount: transaction_amount(extended, sign),
        },
        BsvHomeTransactionValue::SwapReceived => {
            swap_leg(extended, SwapLeg::To, BsvHomeAmountSign::Incoming).map_or(BsvHomeTransactionRowValue::None, |amount| BsvHomeTransactionRowValue::Amount { amount })
        }
        BsvHomeTransactionValue::SwapSpent => {
            swap_leg(extended, SwapLeg::From, BsvHomeAmountSign::Outgoing).map_or(BsvHomeTransactionRowValue::None, |amount| BsvHomeTransactionRowValue::Amount { amount })
        }
        BsvHomeTransactionValue::PerpetualNotional => perpetual_collateral_asset()
            .and_then(|asset| BigNumberFormatter::value_as_f64(&transaction.value.to_string(), asset.decimals as u32).ok())
            .map_or(BsvHomeTransactionRowValue::None, |value| BsvHomeTransactionRowValue::Fiat { value }),
        BsvHomeTransactionValue::PerpetualPnl { value } => BsvHomeTransactionRowValue::Pnl { value },
    }
}

fn header(extended: &TransactionExtended) -> BsvHomeTransactionHeader {
    let transaction = &extended.transaction;
    let amount = |shows_fiat: bool| BsvHomeTransactionHeader::Amount {
        amount: transaction_amount(extended, value_sign(transaction)),
        shows_fiat,
    };
    match header_kind(transaction) {
        BsvHomeTransactionHeaderKind::Amount { shows_fiat } => amount(shows_fiat),
        BsvHomeTransactionHeaderKind::Swap => match (
            swap_leg(extended, SwapLeg::From, BsvHomeAmountSign::None),
            swap_leg(extended, SwapLeg::To, BsvHomeAmountSign::None),
        ) {
            (Some(from), Some(to)) => BsvHomeTransactionHeader::Swap { from, to },
            _ => amount(true),
        },
        BsvHomeTransactionHeaderKind::Nft => match nft_metadata(transaction) {
            Some(metadata) => BsvHomeTransactionHeader::Nft {
                image_url: BsvHomeImage::NftAsset {
                    asset_id: metadata.asset_id.to_string(),
                }
                .url(),
                asset_id: metadata.asset_id,
                name: metadata.name,
            },
            None => amount(false),
        },
        BsvHomeTransactionHeaderKind::Symbol => BsvHomeTransactionHeader::Symbol { asset: extended.asset.clone() },
        BsvHomeTransactionHeaderKind::AssetImage => BsvHomeTransactionHeader::AssetImage { asset: extended.asset.clone() },
    }
}

fn header_action(transaction: &Transaction) -> Option<BsvHomeTransactionHeaderAction> {
    match transaction.transaction_type {
        TransactionType::Transfer
        | TransactionType::TokenApproval
        | TransactionType::StakeDelegate
        | TransactionType::StakeUndelegate
        | TransactionType::StakeRewards
        | TransactionType::StakeRedelegate
        | TransactionType::StakeWithdraw
        | TransactionType::StakeFreeze
        | TransactionType::StakeUnfreeze => Some(BsvHomeTransactionHeaderAction::Asset {
            asset_id: transaction.asset_id.clone(),
        }),
        TransactionType::TransferNFT => transaction.nft_asset_id().map(|asset_id| BsvHomeTransactionHeaderAction::Nft { asset_id }),
        TransactionType::Swap => transaction.swap_metadata().map(|metadata| BsvHomeTransactionHeaderAction::Swap {
            from_asset_id: metadata.from_asset,
            to_asset_id: metadata.to_asset,
        }),
        TransactionType::PerpetualOpenPosition | TransactionType::PerpetualClosePosition | TransactionType::PerpetualModifyPosition => {
            Some(BsvHomeTransactionHeaderAction::Perpetual {
                asset_id: transaction.asset_id.clone(),
            })
        }
        TransactionType::SmartContractCall | TransactionType::AssetActivation | TransactionType::EarnDeposit | TransactionType::EarnWithdraw => None,
    }
}

fn swap_rate(extended: &TransactionExtended) -> Option<BsvHomeSwapRate> {
    let from = swap_leg(extended, SwapLeg::From, BsvHomeAmountSign::None)?;
    let to = swap_leg(extended, SwapLeg::To, BsvHomeAmountSign::None)?;
    swap_rules::swap_rate(&from.asset, &from.value, &to.asset, &to.value)
}

enum SwapLeg {
    From,
    To,
}

fn swap_leg(extended: &TransactionExtended, leg: SwapLeg, sign: BsvHomeAmountSign) -> Option<BsvHomeTransactionAmount> {
    let metadata = extended.transaction.swap_metadata()?;
    let (asset_id, value) = match leg {
        SwapLeg::From => (metadata.from_asset, metadata.from_value),
        SwapLeg::To => (metadata.to_asset, metadata.to_value),
    };
    let asset = extended.assets.iter().chain([&extended.asset]).find(|asset| asset.id == asset_id)?.clone();
    let price = extended.prices.iter().find(|price| price.asset_id == asset_id && price.has_price()).cloned();
    Some(BsvHomeTransactionAmount { asset, value, sign, price })
}

fn transaction_amount(extended: &TransactionExtended, sign: BsvHomeAmountSign) -> BsvHomeTransactionAmount {
    BsvHomeTransactionAmount {
        asset: extended.asset.clone(),
        value: extended.transaction.value.clone(),
        sign,
        price: asset_price(extended.price.as_ref(), &extended.asset.id),
    }
}

fn value_sign(transaction: &Transaction) -> BsvHomeAmountSign {
    match transaction_value(transaction) {
        BsvHomeTransactionValue::Amount { sign } => sign,
        _ => BsvHomeAmountSign::None,
    }
}

fn asset_price(price: Option<&Price>, asset_id: &AssetId) -> Option<AssetPrice> {
    price
        .map(|price| AssetPrice {
            asset_id: asset_id.clone(),
            price: price.price,
            price_change_percentage_24h: price.price_change_percentage_24h,
            updated_at: price.updated_at,
        })
        .filter(AssetPrice::has_price)
}

fn address_name(extended: &TransactionExtended, address: &str) -> Option<primitives::AddressName> {
    [extended.from_address.as_ref(), extended.to_address.as_ref()]
        .into_iter()
        .flatten()
        .find(|name| name.address == address)
        .cloned()
}

fn perpetual_collateral_asset() -> Option<Asset> {
    wallet_default_assets(Chain::HyperCore).into_iter().find(|asset| asset.asset_type == AssetType::PERPETUAL)
}

fn nft_metadata(transaction: &Transaction) -> Option<TransactionNFTTransferMetadata> {
    let metadata = transaction.metadata.clone()?;
    serde_json::from_value::<TransactionNFTTransferMetadata>(metadata).ok()
}

fn transaction_title(transaction: &Transaction) -> BsvHomeTransactionTitle {
    match transaction.transaction_type {
        TransactionType::Transfer | TransactionType::TransferNFT => transfer_title(transaction),
        TransactionType::SmartContractCall => BsvHomeTransactionTitle::SmartContract,
        TransactionType::Swap => BsvHomeTransactionTitle::Swap,
        TransactionType::TokenApproval => BsvHomeTransactionTitle::Approve,
        TransactionType::StakeDelegate => BsvHomeTransactionTitle::Stake,
        TransactionType::StakeUndelegate => BsvHomeTransactionTitle::Unstake,
        TransactionType::StakeRedelegate => BsvHomeTransactionTitle::Redelegate,
        TransactionType::StakeRewards => BsvHomeTransactionTitle::Rewards,
        TransactionType::StakeWithdraw | TransactionType::EarnWithdraw => BsvHomeTransactionTitle::Withdraw,
        TransactionType::AssetActivation => BsvHomeTransactionTitle::ActivateAsset,
        TransactionType::StakeFreeze => BsvHomeTransactionTitle::Freeze,
        TransactionType::StakeUnfreeze => BsvHomeTransactionTitle::Unfreeze,
        TransactionType::EarnDeposit => BsvHomeTransactionTitle::Earn,
        TransactionType::PerpetualOpenPosition => BsvHomeTransactionTitle::PerpetualOpen {
            direction: perpetual_direction(transaction),
        },
        TransactionType::PerpetualClosePosition => BsvHomeTransactionTitle::PerpetualClose {
            direction: perpetual_direction(transaction),
        },
        TransactionType::PerpetualModifyPosition => BsvHomeTransactionTitle::PerpetualModify,
    }
}

fn transfer_title(transaction: &Transaction) -> BsvHomeTransactionTitle {
    if transaction.state != TransactionState::Confirmed {
        return BsvHomeTransactionTitle::Transfer;
    }
    match transaction.direction {
        TransactionDirection::Incoming => BsvHomeTransactionTitle::Received,
        TransactionDirection::Outgoing | TransactionDirection::SelfTransfer => BsvHomeTransactionTitle::Sent,
    }
}

fn transaction_subtitle(transaction: &Transaction) -> BsvHomeTransactionSubtitle {
    match transaction.transaction_type {
        TransactionType::Transfer | TransactionType::TransferNFT | TransactionType::TokenApproval | TransactionType::SmartContractCall => match transaction.direction {
            TransactionDirection::Incoming => BsvHomeTransactionSubtitle::FromAddress {
                address: transaction.from.clone(),
            },
            TransactionDirection::Outgoing | TransactionDirection::SelfTransfer => BsvHomeTransactionSubtitle::ToAddress { address: transaction.to.clone() },
        },
        TransactionType::StakeDelegate | TransactionType::StakeRedelegate | TransactionType::EarnDeposit => {
            BsvHomeTransactionSubtitle::ToAddress { address: transaction.to.clone() }
        }
        TransactionType::StakeUndelegate | TransactionType::EarnWithdraw => BsvHomeTransactionSubtitle::FromAddress { address: transaction.to.clone() },
        TransactionType::StakeFreeze => resource(transaction).map_or(BsvHomeTransactionSubtitle::None, |resource| BsvHomeTransactionSubtitle::ToResource { resource }),
        TransactionType::StakeUnfreeze => resource(transaction).map_or(BsvHomeTransactionSubtitle::None, |resource| BsvHomeTransactionSubtitle::FromResource { resource }),
        TransactionType::PerpetualOpenPosition | TransactionType::PerpetualClosePosition | TransactionType::PerpetualModifyPosition => {
            match perpetual_metadata(transaction).map(|metadata| metadata.price).filter(|price| *price > 0.0) {
                Some(value) => BsvHomeTransactionSubtitle::Price { value },
                None => BsvHomeTransactionSubtitle::None,
            }
        }
        TransactionType::Swap | TransactionType::StakeRewards | TransactionType::StakeWithdraw | TransactionType::AssetActivation => BsvHomeTransactionSubtitle::None,
    }
}

fn transaction_participant(transaction: &Transaction) -> Option<(BsvHomeTransactionParticipantRole, String)> {
    let role = match transaction.transaction_type {
        TransactionType::Transfer | TransactionType::TransferNFT => match transaction.direction {
            TransactionDirection::Incoming => BsvHomeTransactionParticipantRole::Sender,
            TransactionDirection::Outgoing | TransactionDirection::SelfTransfer => BsvHomeTransactionParticipantRole::Recipient,
        },
        TransactionType::TokenApproval => BsvHomeTransactionParticipantRole::Contract,
        TransactionType::SmartContractCall => match wallet_connect_metadata(transaction).map(|metadata| metadata.output_action) {
            Some(TransferDataOutputAction::Send) => BsvHomeTransactionParticipantRole::Recipient,
            Some(TransferDataOutputAction::Sign) | None => BsvHomeTransactionParticipantRole::Contract,
        },
        TransactionType::StakeDelegate => BsvHomeTransactionParticipantRole::Validator,
        TransactionType::EarnDeposit | TransactionType::EarnWithdraw => BsvHomeTransactionParticipantRole::Provider,
        TransactionType::Swap
        | TransactionType::StakeUndelegate
        | TransactionType::StakeRedelegate
        | TransactionType::StakeRewards
        | TransactionType::StakeWithdraw
        | TransactionType::StakeFreeze
        | TransactionType::StakeUnfreeze
        | TransactionType::AssetActivation
        | TransactionType::PerpetualOpenPosition
        | TransactionType::PerpetualClosePosition
        | TransactionType::PerpetualModifyPosition => return None,
    };
    let address = match transaction.direction {
        TransactionDirection::Incoming => &transaction.from,
        TransactionDirection::Outgoing | TransactionDirection::SelfTransfer => &transaction.to,
    };
    (!address.is_empty()).then(|| (role, address.clone()))
}

fn transaction_value(transaction: &Transaction) -> BsvHomeTransactionValue {
    match transaction.transaction_type {
        TransactionType::Swap => BsvHomeTransactionValue::SwapReceived,
        TransactionType::TokenApproval => BsvHomeTransactionValue::AssetSymbol,
        TransactionType::PerpetualOpenPosition => BsvHomeTransactionValue::PerpetualNotional,
        TransactionType::PerpetualClosePosition => match perpetual_metadata(transaction).map(|metadata| metadata.pnl).filter(|pnl| *pnl != 0.0) {
            Some(value) => BsvHomeTransactionValue::PerpetualPnl { value },
            None => BsvHomeTransactionValue::None,
        },
        TransactionType::StakeRewards | TransactionType::StakeWithdraw => BsvHomeTransactionValue::Amount {
            sign: BsvHomeAmountSign::Incoming,
        },
        TransactionType::Transfer => BsvHomeTransactionValue::Amount {
            sign: amount_sign(&transaction.direction),
        },
        TransactionType::StakeDelegate
        | TransactionType::StakeUndelegate
        | TransactionType::StakeRedelegate
        | TransactionType::StakeFreeze
        | TransactionType::StakeUnfreeze
        | TransactionType::EarnDeposit
        | TransactionType::EarnWithdraw
        | TransactionType::AssetActivation
        | TransactionType::SmartContractCall => BsvHomeTransactionValue::Amount { sign: BsvHomeAmountSign::None },
        TransactionType::TransferNFT | TransactionType::PerpetualModifyPosition => BsvHomeTransactionValue::None,
    }
}

fn transaction_equivalent_value(transaction: &Transaction) -> BsvHomeTransactionValue {
    match transaction.transaction_type {
        TransactionType::Swap => BsvHomeTransactionValue::SwapSpent,
        _ => BsvHomeTransactionValue::None,
    }
}

fn amount_sign(direction: &TransactionDirection) -> BsvHomeAmountSign {
    match direction {
        TransactionDirection::Incoming => BsvHomeAmountSign::Incoming,
        TransactionDirection::Outgoing => BsvHomeAmountSign::Outgoing,
        TransactionDirection::SelfTransfer => BsvHomeAmountSign::None,
    }
}

fn resource(transaction: &Transaction) -> Option<primitives::Resource> {
    let metadata = transaction.metadata.clone()?;
    serde_json::from_value::<TransactionResourceTypeMetadata>(metadata)
        .ok()
        .map(|metadata| metadata.resource_type)
}

pub fn header_kind(transaction: &Transaction) -> BsvHomeTransactionHeaderKind {
    match transaction.transaction_type {
        TransactionType::Transfer
        | TransactionType::StakeDelegate
        | TransactionType::StakeUndelegate
        | TransactionType::StakeRedelegate
        | TransactionType::StakeRewards
        | TransactionType::StakeWithdraw
        | TransactionType::StakeFreeze
        | TransactionType::StakeUnfreeze
        | TransactionType::EarnDeposit
        | TransactionType::EarnWithdraw
        | TransactionType::SmartContractCall => BsvHomeTransactionHeaderKind::Amount { shows_fiat: true },
        TransactionType::Swap => match transaction.swap_metadata() {
            Some(_) => BsvHomeTransactionHeaderKind::Swap,
            None => BsvHomeTransactionHeaderKind::Amount { shows_fiat: true },
        },
        TransactionType::TransferNFT => match transaction.nft_asset_id() {
            Some(_) => BsvHomeTransactionHeaderKind::Nft,
            None => BsvHomeTransactionHeaderKind::Amount { shows_fiat: false },
        },
        TransactionType::TokenApproval => BsvHomeTransactionHeaderKind::AssetImage,
        TransactionType::AssetActivation | TransactionType::PerpetualOpenPosition | TransactionType::PerpetualClosePosition | TransactionType::PerpetualModifyPosition => {
            BsvHomeTransactionHeaderKind::Symbol
        }
    }
}

pub fn details(extended: &TransactionExtended) -> BsvHomeTransactionDetails {
    let transaction = &extended.transaction;
    let swap_metadata = (transaction.transaction_type == TransactionType::Swap).then(|| transaction.swap_metadata()).flatten();
    let provider = swap_metadata
        .as_ref()
        .and_then(|metadata| metadata.provider.as_deref())
        .and_then(|id| SwapperProvider::from_str(id).ok())
        .map(SwapperProviderType::new);
    let swap_progress = swap_progress(extended, swap_metadata.as_ref(), provider.as_ref());
    let perpetual = perpetual_metadata(transaction);
    BsvHomeTransactionDetails {
        estimated_confirmation_seconds: extended
            .confirmation_eta_seconds
            .filter(|seconds| *seconds > 0 && transaction.state == TransactionState::Pending && swap_progress.is_none()),
        swap_again: swap_metadata
            .as_ref()
            .filter(|_| transaction.state == TransactionState::Confirmed)
            .map(|metadata| BsvHomeSwapAgain {
                from_asset_id: metadata.from_asset.clone(),
                to_asset_id: metadata.to_asset.clone(),
            }),
        swap_progress,
        provider_name: provider
            .map(|provider| provider.name)
            .or_else(|| swap_metadata.as_ref().and_then(|metadata| metadata.provider.clone())),
        pnl: perpetual.as_ref().map(|metadata| metadata.pnl).filter(|pnl| *pnl != 0.0),
        price: perpetual.as_ref().map(|metadata| metadata.price).filter(|price| *price > 0.0),
    }
}

fn swap_progress(extended: &TransactionExtended, metadata: Option<&TransactionSwapMetadata>, provider: Option<&SwapperProviderType>) -> Option<BsvHomeSwapProgress> {
    let metadata = metadata?;
    let provider = provider.filter(|provider| provider.mode != SwapperProviderMode::OnChain)?;
    let from_asset = extended.assets.iter().chain([&extended.asset]).find(|asset| asset.id == metadata.from_asset)?;
    let (transfer, swap) = match extended.transaction.state {
        TransactionState::Pending => (BsvHomeSwapProgressStep::Pending, BsvHomeSwapProgressStep::Waiting),
        TransactionState::InTransit => (BsvHomeSwapProgressStep::Completed, BsvHomeSwapProgressStep::Pending),
        TransactionState::Failed => (BsvHomeSwapProgressStep::Completed, BsvHomeSwapProgressStep::Failed),
        TransactionState::Reverted => (BsvHomeSwapProgressStep::Reverted, BsvHomeSwapProgressStep::Waiting),
        TransactionState::Refunded => (BsvHomeSwapProgressStep::Completed, BsvHomeSwapProgressStep::Refunded),
        TransactionState::Confirmed => return None,
    };
    Some(BsvHomeSwapProgress {
        from_asset: from_asset.clone(),
        from_value: metadata.from_value.clone(),
        provider_name: provider.name.clone(),
        transfer,
        swap,
        eta_seconds: extended
            .confirmation_eta_seconds
            .filter(|seconds| *seconds > 0 && !extended.transaction.state.is_completed()),
    })
}

fn wallet_connect_metadata(transaction: &Transaction) -> Option<TransactionWalletConnectMetadata> {
    let metadata = transaction.metadata.clone()?;
    serde_json::from_value::<TransactionWalletConnectMetadata>(metadata).ok()
}

fn perpetual_metadata(transaction: &Transaction) -> Option<TransactionPerpetualMetadata> {
    let metadata = transaction.metadata.clone()?;
    serde_json::from_value::<TransactionPerpetualMetadata>(metadata).ok()
}

fn perpetual_direction(transaction: &Transaction) -> Option<PerpetualDirection> {
    perpetual_metadata(transaction).map(|metadata| metadata.direction)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_every_transaction_type_belongs_to_exactly_one_filter_in_list_order() {
        let filters = transaction_filters();
        assert_eq!(filters.len(), 6);
        let grouped: Vec<TransactionType> = filters.iter().flat_map(|filter| filter_transaction_types(*filter)).collect();
        assert_eq!(grouped.len(), TransactionType::all().len());
        for transaction_type in TransactionType::all() {
            assert!(filter_transaction_types(transaction_filter(&transaction_type)).contains(&transaction_type));
        }
        assert_eq!(transaction_filter(&TransactionType::TokenApproval), BsvHomeTransactionFilter::Swaps);
        assert_eq!(transaction_filter(&TransactionType::EarnWithdraw), BsvHomeTransactionFilter::Stake);
        assert_eq!(transaction_filter(&TransactionType::AssetActivation), BsvHomeTransactionFilter::Others);
        assert_eq!(filter_transaction_types(BsvHomeTransactionFilter::Perpetuals).len(), 3);
    }

    use super::*;
    use chrono::Utc;
    use num_bigint::BigUint;
    use primitives::{Chain, Resource};

    fn transaction(asset_id: AssetId, fee_asset_id: AssetId) -> Transaction {
        Transaction::new(
            "hash".into(),
            asset_id,
            "from".into(),
            "to".into(),
            None,
            TransactionType::Transfer,
            TransactionState::Confirmed,
            BigUint::from(1u64),
            fee_asset_id,
            BigUint::from(1u64),
            None,
            None,
            Utc::now(),
        )
    }

    fn typed(transaction_type: TransactionType, state: TransactionState, direction: TransactionDirection) -> Transaction {
        let mut transaction = transaction(AssetId::from_chain(Chain::Ethereum), AssetId::from_chain(Chain::Ethereum));
        transaction.transaction_type = transaction_type;
        transaction.state = state;
        transaction.direction = direction;
        transaction
    }

    #[test]
    fn test_pending_transactions_keeps_what_a_synced_wallet_still_has_to_watch() {
        use TransactionState::{Confirmed, Failed, InTransit, Pending, Reverted};

        let transactions: Vec<Transaction> = [Pending, Confirmed, InTransit, Failed, Reverted]
            .into_iter()
            .map(|state| typed(TransactionType::Transfer, state, TransactionDirection::Outgoing))
            .collect();

        let pending: Vec<TransactionState> = pending_transactions(&transactions).into_iter().map(|transaction| transaction.state).collect();

        assert_eq!(pending, vec![Pending, InTransit], "a swap in transit is not settled yet, so the tracker keeps polling it");
    }

    #[test]
    fn test_transaction_title_reads_a_transfer_from_its_state_and_direction() {
        use TransactionDirection::{Incoming, Outgoing, SelfTransfer};
        use TransactionState::{Confirmed, Failed, InTransit, Pending};

        assert_eq!(transaction_title(&typed(TransactionType::Transfer, Confirmed, Incoming)), BsvHomeTransactionTitle::Received);
        assert_eq!(transaction_title(&typed(TransactionType::Transfer, Confirmed, Outgoing)), BsvHomeTransactionTitle::Sent);
        assert_eq!(transaction_title(&typed(TransactionType::Transfer, Confirmed, SelfTransfer)), BsvHomeTransactionTitle::Sent);
        assert_eq!(
            transaction_title(&typed(TransactionType::TransferNFT, Confirmed, Incoming)),
            BsvHomeTransactionTitle::Received
        );

        for state in [Pending, Failed, InTransit] {
            assert_eq!(transaction_title(&typed(TransactionType::Transfer, state, Incoming)), BsvHomeTransactionTitle::Transfer);
        }
    }

    #[test]
    fn test_transaction_title_separates_earn_from_stake() {
        let confirmed = |transaction_type| typed(transaction_type, TransactionState::Confirmed, TransactionDirection::Outgoing);

        assert_eq!(transaction_title(&confirmed(TransactionType::StakeDelegate)), BsvHomeTransactionTitle::Stake);
        assert_eq!(transaction_title(&confirmed(TransactionType::EarnDeposit)), BsvHomeTransactionTitle::Earn);
        assert_eq!(transaction_title(&confirmed(TransactionType::EarnWithdraw)), BsvHomeTransactionTitle::Withdraw);
        assert_eq!(transaction_title(&confirmed(TransactionType::StakeWithdraw)), BsvHomeTransactionTitle::Withdraw);
    }

    #[test]
    fn test_transaction_title_carries_the_perpetual_direction_when_the_metadata_has_one() {
        let mut open = typed(TransactionType::PerpetualOpenPosition, TransactionState::Confirmed, TransactionDirection::Outgoing);
        assert_eq!(transaction_title(&open), BsvHomeTransactionTitle::PerpetualOpen { direction: None });

        open.metadata = Some(
            serde_json::to_value(TransactionPerpetualMetadata {
                pnl: 0.0,
                price: 1.0,
                direction: PerpetualDirection::Short,
                is_liquidation: None,
                provider: None,
            })
            .unwrap(),
        );
        assert_eq!(
            transaction_title(&open),
            BsvHomeTransactionTitle::PerpetualOpen {
                direction: Some(PerpetualDirection::Short)
            }
        );

        let mut close = open.clone();
        close.transaction_type = TransactionType::PerpetualClosePosition;
        assert_eq!(
            transaction_title(&close),
            BsvHomeTransactionTitle::PerpetualClose {
                direction: Some(PerpetualDirection::Short)
            }
        );
    }

    #[test]
    fn test_transaction_subtitle_names_the_counterparty_the_row_shows() {
        use TransactionDirection::{Incoming, Outgoing};

        let confirmed = |transaction_type, direction| typed(transaction_type, TransactionState::Confirmed, direction);

        assert_eq!(
            transaction_subtitle(&confirmed(TransactionType::Transfer, Incoming)),
            BsvHomeTransactionSubtitle::FromAddress { address: "from".to_string() }
        );
        assert_eq!(
            transaction_subtitle(&confirmed(TransactionType::Transfer, Outgoing)),
            BsvHomeTransactionSubtitle::ToAddress { address: "to".to_string() }
        );
        assert_eq!(
            transaction_subtitle(&confirmed(TransactionType::StakeDelegate, Outgoing)),
            BsvHomeTransactionSubtitle::ToAddress { address: "to".to_string() }
        );
        assert_eq!(
            transaction_subtitle(&confirmed(TransactionType::StakeUndelegate, Outgoing)),
            BsvHomeTransactionSubtitle::FromAddress { address: "to".to_string() }
        );
        assert_eq!(transaction_subtitle(&confirmed(TransactionType::Swap, Outgoing)), BsvHomeTransactionSubtitle::None);
        assert_eq!(transaction_subtitle(&confirmed(TransactionType::StakeRewards, Incoming)), BsvHomeTransactionSubtitle::None);
    }

    #[test]
    fn test_transaction_subtitle_reads_the_resource_and_the_price_from_the_metadata() {
        let mut freeze = typed(TransactionType::StakeFreeze, TransactionState::Confirmed, TransactionDirection::Outgoing);
        assert_eq!(transaction_subtitle(&freeze), BsvHomeTransactionSubtitle::None);

        freeze.metadata = Some(serde_json::to_value(TransactionResourceTypeMetadata::new(Resource::Energy)).unwrap());
        assert_eq!(transaction_subtitle(&freeze), BsvHomeTransactionSubtitle::ToResource { resource: Resource::Energy });

        let mut unfreeze = freeze.clone();
        unfreeze.transaction_type = TransactionType::StakeUnfreeze;
        assert_eq!(transaction_subtitle(&unfreeze), BsvHomeTransactionSubtitle::FromResource { resource: Resource::Energy });

        let mut open = typed(TransactionType::PerpetualOpenPosition, TransactionState::Confirmed, TransactionDirection::Outgoing);
        assert_eq!(transaction_subtitle(&open), BsvHomeTransactionSubtitle::None);

        open.metadata = Some(
            serde_json::to_value(TransactionPerpetualMetadata {
                pnl: 0.0,
                price: 12.5,
                direction: PerpetualDirection::Long,
                is_liquidation: None,
                provider: None,
            })
            .unwrap(),
        );
        assert_eq!(transaction_subtitle(&open), BsvHomeTransactionSubtitle::Price { value: 12.5 });
    }

    #[test]
    fn test_header_kind_falls_back_to_an_amount_without_metadata() {
        let mut swap = Transaction::mock();
        swap.transaction_type = TransactionType::Swap;
        swap.metadata = None;
        assert_eq!(header_kind(&swap), BsvHomeTransactionHeaderKind::Amount { shows_fiat: true });
        swap.metadata = Some(
            serde_json::to_value(primitives::TransactionSwapMetadata {
                from_asset: AssetId::from_chain(primitives::Chain::Ethereum),
                from_value: 1u32.into(),
                to_asset: AssetId::from_chain(primitives::Chain::Bitcoin),
                to_value: 1u32.into(),
                provider: None,
            })
            .unwrap(),
        );
        assert_eq!(header_kind(&swap), BsvHomeTransactionHeaderKind::Swap);
        let mut approval = Transaction::mock();
        approval.transaction_type = TransactionType::TokenApproval;
        assert_eq!(header_kind(&approval), BsvHomeTransactionHeaderKind::AssetImage);
    }

    #[test]
    fn test_transaction_participant_names_the_role_of_the_address_the_screen_shows() {
        use BsvHomeTransactionParticipantRole::{Contract, Provider, Recipient, Sender, Validator};
        use TransactionDirection::{Incoming, Outgoing, SelfTransfer};

        let confirmed = |transaction_type, direction| typed(transaction_type, TransactionState::Confirmed, direction);
        let participant = |transaction_type, direction| transaction_participant(&confirmed(transaction_type, direction));

        assert_eq!(participant(TransactionType::Transfer, Incoming), Some((Sender, "from".to_string())));
        assert_eq!(participant(TransactionType::Transfer, Outgoing), Some((Recipient, "to".to_string())));
        assert_eq!(participant(TransactionType::TransferNFT, SelfTransfer), Some((Recipient, "to".to_string())));
        assert_eq!(participant(TransactionType::TokenApproval, Outgoing), Some((Contract, "to".to_string())));
        assert_eq!(participant(TransactionType::StakeDelegate, Outgoing), Some((Validator, "to".to_string())));
        assert_eq!(participant(TransactionType::EarnWithdraw, Outgoing), Some((Provider, "to".to_string())));
        assert_eq!(participant(TransactionType::SmartContractCall, Outgoing), Some((Contract, "to".to_string())));
        assert_eq!(participant(TransactionType::Swap, Outgoing), None);
        assert_eq!(participant(TransactionType::StakeUndelegate, Outgoing), None);
        assert_eq!(participant(TransactionType::StakeFreeze, Outgoing), None);

        let mut send = confirmed(TransactionType::SmartContractCall, Outgoing);
        send.metadata = Some(
            serde_json::to_value(TransactionWalletConnectMetadata {
                output_action: TransferDataOutputAction::Send,
            })
            .unwrap(),
        );
        assert_eq!(transaction_participant(&send), Some((Recipient, "to".to_string())));
        send.metadata = Some(
            serde_json::to_value(TransactionWalletConnectMetadata {
                output_action: TransferDataOutputAction::Sign,
            })
            .unwrap(),
        );
        assert_eq!(transaction_participant(&send), Some((Contract, "to".to_string())));

        let mut blank = confirmed(TransactionType::Transfer, Outgoing);
        blank.to = String::new();
        assert_eq!(transaction_participant(&blank), None);
    }

    #[test]
    fn test_transaction_value_signs_what_the_row_shows() {
        use TransactionDirection::{Incoming, Outgoing, SelfTransfer};

        let confirmed = |transaction_type, direction| typed(transaction_type, TransactionState::Confirmed, direction);

        assert_eq!(
            transaction_value(&confirmed(TransactionType::Transfer, Incoming)),
            BsvHomeTransactionValue::Amount {
                sign: BsvHomeAmountSign::Incoming
            }
        );
        assert_eq!(
            transaction_value(&confirmed(TransactionType::Transfer, Outgoing)),
            BsvHomeTransactionValue::Amount {
                sign: BsvHomeAmountSign::Outgoing
            }
        );
        assert_eq!(
            transaction_value(&confirmed(TransactionType::Transfer, SelfTransfer)),
            BsvHomeTransactionValue::Amount { sign: BsvHomeAmountSign::None }
        );
        assert_eq!(
            transaction_value(&confirmed(TransactionType::StakeRewards, Outgoing)),
            BsvHomeTransactionValue::Amount {
                sign: BsvHomeAmountSign::Incoming
            }
        );
        assert_eq!(
            transaction_value(&confirmed(TransactionType::StakeWithdraw, Outgoing)),
            BsvHomeTransactionValue::Amount {
                sign: BsvHomeAmountSign::Incoming
            }
        );
        assert_eq!(
            transaction_value(&confirmed(TransactionType::StakeDelegate, Outgoing)),
            BsvHomeTransactionValue::Amount { sign: BsvHomeAmountSign::None }
        );
        assert_eq!(
            transaction_value(&confirmed(TransactionType::TokenApproval, Outgoing)),
            BsvHomeTransactionValue::AssetSymbol
        );
        assert_eq!(transaction_value(&confirmed(TransactionType::TransferNFT, Incoming)), BsvHomeTransactionValue::None);
    }

    #[test]
    fn test_transaction_value_gives_a_swap_both_legs_and_a_perpetual_close_only_a_real_pnl() {
        let mut swap = typed(TransactionType::Swap, TransactionState::Confirmed, TransactionDirection::Outgoing);
        assert_eq!(transaction_value(&swap), BsvHomeTransactionValue::SwapReceived);
        assert_eq!(transaction_equivalent_value(&swap), BsvHomeTransactionValue::SwapSpent);

        swap.transaction_type = TransactionType::Transfer;
        assert_eq!(transaction_equivalent_value(&swap), BsvHomeTransactionValue::None);

        let mut close = typed(TransactionType::PerpetualClosePosition, TransactionState::Confirmed, TransactionDirection::Outgoing);
        assert_eq!(transaction_value(&close), BsvHomeTransactionValue::None);

        let metadata = |pnl| TransactionPerpetualMetadata {
            pnl,
            price: 1.0,
            direction: PerpetualDirection::Long,
            is_liquidation: None,
            provider: None,
        };

        close.metadata = Some(serde_json::to_value(metadata(0.0)).unwrap());
        assert_eq!(transaction_value(&close), BsvHomeTransactionValue::None);

        close.metadata = Some(serde_json::to_value(metadata(-4.5)).unwrap());
        assert_eq!(transaction_value(&close), BsvHomeTransactionValue::PerpetualPnl { value: -4.5 });
    }

    #[test]
    fn test_transaction_asset_ids_includes_fee_assets_once() {
        let solana = AssetId::from_chain(Chain::Solana);
        let ethereum = AssetId::from_chain(Chain::Ethereum);
        let usdc = AssetId::from_token(Chain::Solana, "usdc");

        let mut asset_ids = transaction_asset_ids(&[transaction(usdc.clone(), solana.clone()), transaction(ethereum.clone(), ethereum.clone())]);
        asset_ids.sort_by_key(|asset_id| asset_id.to_string());
        let mut expected = vec![usdc, solana, ethereum];
        expected.sort_by_key(|asset_id| asset_id.to_string());

        assert_eq!(asset_ids, expected);
    }

    fn swap(state: TransactionState, provider: Option<&str>, eta: Option<u32>) -> TransactionExtended {
        let mut transaction = Transaction::mock();
        transaction.transaction_type = TransactionType::Swap;
        transaction.state = state;
        transaction.metadata = Some(
            serde_json::to_value(primitives::TransactionSwapMetadata {
                from_asset: AssetId::from_chain(Chain::Ethereum),
                from_value: 5u32.into(),
                to_asset: AssetId::from_chain(Chain::Bitcoin),
                to_value: 1u32.into(),
                provider: provider.map(str::to_string),
            })
            .unwrap(),
        );
        let mut extended = TransactionExtended::mock_transaction(transaction);
        extended.confirmation_eta_seconds = eta;
        extended
    }

    fn extended_with(transaction: Transaction, assets: Vec<Asset>) -> TransactionExtended {
        let mut extended = TransactionExtended::mock_transaction(transaction);
        extended.assets = assets;
        extended
    }

    fn named(address: &str, name: &str) -> primitives::AddressName {
        primitives::AddressName::mock(address, name, primitives::AddressType::Address, primitives::VerificationStatus::Verified)
    }

    #[test]
    fn test_row_resolves_the_swap_legs_from_the_metadata_and_the_known_assets() {
        let extended = extended_with(swap(TransactionState::Confirmed, None, None).transaction, vec![Asset::mock_eth(), Asset::mock_btc()]);

        let swap_row = row(&extended);

        assert_eq!(swap_row.title, BsvHomeTransactionTitle::Swap);
        match (&swap_row.value, &swap_row.equivalent_value) {
            (BsvHomeTransactionRowValue::Amount { amount: received }, BsvHomeTransactionRowValue::Amount { amount: spent }) => {
                assert_eq!(
                    (received.asset.id.clone(), received.value.clone(), received.sign),
                    (AssetId::from_chain(Chain::Bitcoin), 1u32.into(), BsvHomeAmountSign::Incoming)
                );
                assert_eq!(
                    (spent.asset.id.clone(), spent.value.clone(), spent.sign),
                    (AssetId::from_chain(Chain::Ethereum), 5u32.into(), BsvHomeAmountSign::Outgoing)
                );
            }
            other => panic!("a swap row shows both legs, got {other:?}"),
        }
        assert_eq!(
            row(&extended_with(swap(TransactionState::Confirmed, None, None).transaction, vec![])).value,
            BsvHomeTransactionRowValue::None,
            "a leg whose asset is unknown is not shown as a number"
        );
    }

    #[test]
    fn test_status_groups_the_states_the_screens_render_alike() {
        assert_eq!(status(TransactionState::InTransit).tone, BsvHomeTransactionStateTone::Pending);
        assert_eq!(status(TransactionState::Reverted).tone, BsvHomeTransactionStateTone::Error);
        assert_eq!(status(TransactionState::Refunded).tone, BsvHomeTransactionStateTone::Refunded);
        assert!(!status(TransactionState::Confirmed).shows_badge, "a confirmed transaction carries no badge");
        assert!(status(TransactionState::Pending).shows_progress);
        assert!(!status(TransactionState::Refunded).shows_progress, "a refund is settled, so nothing spins");
    }

    #[test]
    fn test_row_shows_the_counterparty_name_when_the_wallet_knows_the_address() {
        let mut incoming = extended_with(typed(TransactionType::Transfer, TransactionState::Confirmed, TransactionDirection::Incoming), vec![]);
        incoming.from_address = Some(named("from", "Alice"));
        assert_eq!(row(&incoming).subtitle, BsvHomeTransactionRowSubtitle::FromAddress { participant: "Alice".to_string() });

        let outgoing = extended_with(typed(TransactionType::Transfer, TransactionState::Confirmed, TransactionDirection::Outgoing), vec![]);
        assert_eq!(row(&outgoing).subtitle, BsvHomeTransactionRowSubtitle::ToAddress { participant: "to".to_string() });
        match row(&outgoing).value {
            BsvHomeTransactionRowValue::Amount { amount } => assert_eq!(
                (amount.asset.id, amount.value, amount.sign),
                (AssetId::from_chain(Chain::Ethereum), 1u32.into(), BsvHomeAmountSign::Outgoing)
            ),
            other => panic!("a transfer row shows its amount, got {other:?}"),
        }
    }

    #[test]
    fn test_row_carries_the_nft_image_and_the_perpetual_notional_in_collateral_units() {
        let mut nft = typed(TransactionType::TransferNFT, TransactionState::Confirmed, TransactionDirection::Outgoing);
        let asset_id = primitives::NFTAssetId::new(Chain::Ethereum, "0xcontract", "7");
        nft.metadata = Some(serde_json::to_value(TransactionNFTTransferMetadata::new(asset_id.clone(), Some("Punk".to_string()))).unwrap());
        let nft_row = row(&extended_with(nft, vec![]));
        assert!(nft_row.nft_image_url.as_deref().is_some_and(|url| url.contains(&asset_id.to_string())));
        assert_eq!(nft_row.value, BsvHomeTransactionRowValue::None);

        let mut open = typed(TransactionType::PerpetualOpenPosition, TransactionState::Confirmed, TransactionDirection::Outgoing);
        open.value = 1_500_000u32.into();
        assert_eq!(
            row(&extended_with(open, vec![])).value,
            BsvHomeTransactionRowValue::Fiat { value: 1.5 },
            "the notional is the value in collateral units"
        );
    }

    #[test]
    fn test_detail_rows_build_the_header_the_participant_the_rate_and_the_fee() {
        let link = |address: &str| BlockExplorerLink {
            name: "Explorer".to_string(),
            link: format!("https://explorer/{address}"),
        };
        let explorer = BlockExplorerLink {
            name: "Explorer".to_string(),
            link: "https://explorer/tx".to_string(),
        };

        let swap = extended_with(swap(TransactionState::Confirmed, None, None).transaction, vec![Asset::mock_eth(), Asset::mock_btc()]);
        let rows = detail_rows(&swap, participant(&swap, link), explorer.clone());
        match rows.header {
            BsvHomeTransactionHeader::Swap { from, to } => {
                assert_eq!((from.asset.id, to.asset.id), (AssetId::from_chain(Chain::Ethereum), AssetId::from_chain(Chain::Bitcoin)));
                assert_eq!(
                    (from.sign, to.sign),
                    (BsvHomeAmountSign::None, BsvHomeAmountSign::None),
                    "the header shows both legs unsigned"
                );
            }
            other => panic!("a swap with both assets shows the swap header, got {other:?}"),
        }
        assert!(rows.rate.is_some());
        assert!(rows.participant.is_none(), "a swap names its provider, not a participant");
        assert_eq!(
            rows.header_action,
            Some(BsvHomeTransactionHeaderAction::Swap {
                from_asset_id: AssetId::from_chain(Chain::Ethereum),
                to_asset_id: AssetId::from_chain(Chain::Bitcoin)
            })
        );

        let mut transfer = extended_with(typed(TransactionType::Transfer, TransactionState::Confirmed, TransactionDirection::Outgoing), vec![]);
        transfer.transaction.memo = Some(String::new());
        let unnamed = detail_rows(&transfer, participant(&transfer, link), explorer.clone());
        let recipient = unnamed.participant.clone().unwrap();
        assert_eq!(
            (recipient.role, recipient.address.as_str(), recipient.can_add_contact),
            (BsvHomeTransactionParticipantRole::Recipient, "to", true)
        );
        assert_eq!(recipient.link.link, "https://explorer/to");
        assert_eq!(unnamed.memo, None, "an empty memo is not a row");
        assert_eq!(
            (unnamed.fee.asset.id.clone(), unnamed.fee.value.clone(), unnamed.fee.sign),
            (transfer.fee_asset.id.clone(), 1u32.into(), BsvHomeAmountSign::None)
        );
        assert!(matches!(unnamed.header, BsvHomeTransactionHeader::Amount { shows_fiat: true, .. }));
        assert_eq!(
            unnamed.header_action,
            Some(BsvHomeTransactionHeaderAction::Asset {
                asset_id: AssetId::from_chain(Chain::Ethereum)
            })
        );

        transfer.to_address = Some(named("to", "Bob"));
        let named_rows = detail_rows(&transfer, participant(&transfer, link), explorer.clone());
        let recipient = named_rows.participant.unwrap();
        assert_eq!((recipient.name.map(|name| name.name), recipient.can_add_contact), (Some("Bob".to_string()), false));

        let approval = extended_with(typed(TransactionType::TokenApproval, TransactionState::Confirmed, TransactionDirection::Outgoing), vec![]);
        let approval_rows = detail_rows(&approval, participant(&approval, link), explorer);
        assert!(matches!(approval_rows.header, BsvHomeTransactionHeader::AssetImage { .. }));
        let contract = approval_rows.participant.unwrap();
        assert_eq!((contract.role, contract.can_add_contact), (BsvHomeTransactionParticipantRole::Contract, false));
    }

    #[test]
    fn test_detail_sections_list_only_the_rows_the_transaction_has_in_one_order() {
        use BsvHomeTransactionDetailRow::*;
        let link = |address: &str| BlockExplorerLink {
            name: "Explorer".to_string(),
            link: format!("https://explorer/{address}"),
        };
        let explorer = BlockExplorerLink {
            name: "Explorer".to_string(),
            link: "https://explorer/tx".to_string(),
        };
        let rows_of = |sections: Vec<BsvHomeTransactionDetailSection>| sections.into_iter().map(|section| section.rows).collect::<Vec<_>>();

        let mut transfer = extended_with(typed(TransactionType::Transfer, TransactionState::Confirmed, TransactionDirection::Outgoing), vec![]);
        transfer.transaction.memo = Some("gm".to_string());
        assert_eq!(
            rows_of(detail_sections(&detail_rows(&transfer, participant(&transfer, link), explorer.clone()))),
            vec![vec![Header], vec![Date, Status, Participant, Memo, Network], vec![Fee], vec![Explorer]],
            "a transfer has no swap sections and shows its memo beside the recipient"
        );

        let pending = extended_with(
            swap(TransactionState::Pending, Some("near_intents"), Some(720)).transaction,
            vec![Asset::mock_eth(), Asset::mock_btc()],
        );
        assert_eq!(
            rows_of(detail_sections(&detail_rows(&pending, participant(&pending, link), explorer.clone()))),
            vec![vec![Header], vec![SwapProgress], vec![Date, Status, Rate, Network, Provider], vec![Fee], vec![Explorer]],
            "a swap in flight shows its progress instead of a confirmation estimate, and its provider instead of a participant"
        );

        let confirmed = extended_with(
            swap(TransactionState::Confirmed, Some("near_intents"), None).transaction,
            vec![Asset::mock_eth(), Asset::mock_btc()],
        );
        assert_eq!(
            rows_of(detail_sections(&detail_rows(&confirmed, participant(&confirmed, link), explorer.clone())))[1],
            vec![SwapAgain],
            "a confirmed swap offers to swap again"
        );

        let mut open = extended_with(
            typed(TransactionType::PerpetualOpenPosition, TransactionState::Confirmed, TransactionDirection::Outgoing),
            vec![],
        );
        open.transaction.metadata = Some(
            serde_json::to_value(TransactionPerpetualMetadata {
                pnl: -3.5,
                price: 61.0,
                direction: PerpetualDirection::Short,
                is_liquidation: None,
                provider: None,
            })
            .unwrap(),
        );
        assert_eq!(
            rows_of(detail_sections(&detail_rows(&open, participant(&open, link), explorer)))[1],
            vec![Date, Status, Network, Pnl, Price],
            "a perpetual has no participant and shows its pnl and price after the network"
        );
    }

    #[test]
    fn test_swap_rate_needs_both_legs_to_have_a_value() {
        let mut zero = swap(TransactionState::Confirmed, None, None).transaction;
        zero.metadata = Some(
            serde_json::to_value(TransactionSwapMetadata {
                from_asset: AssetId::from_chain(Chain::Ethereum),
                from_value: 0u32.into(),
                to_asset: AssetId::from_chain(Chain::Bitcoin),
                to_value: 1u32.into(),
                provider: None,
            })
            .unwrap(),
        );
        assert!(swap_rate(&extended_with(zero, vec![Asset::mock_eth(), Asset::mock_btc()])).is_none());
    }

    #[test]
    fn test_details_show_swap_progress_only_for_an_unfinished_cross_chain_swap() {
        let pending = details(&swap(TransactionState::Pending, Some("thorchain"), Some(90)));
        let progress = pending.swap_progress.unwrap();
        assert_eq!(
            (progress.transfer, progress.swap, progress.eta_seconds),
            (BsvHomeSwapProgressStep::Pending, BsvHomeSwapProgressStep::Waiting, Some(90))
        );
        assert_eq!(progress.from_value, 5u32.into());
        assert_eq!(pending.provider_name.as_deref(), Some(progress.provider_name.as_str()));
        assert_eq!(pending.estimated_confirmation_seconds, None, "the progress steps carry the eta");
        assert!(pending.swap_again.is_none());

        let in_transit = details(&swap(TransactionState::InTransit, Some("thorchain"), None)).swap_progress.unwrap();
        assert_eq!(
            (in_transit.transfer, in_transit.swap),
            (BsvHomeSwapProgressStep::Completed, BsvHomeSwapProgressStep::Pending)
        );
        let failed = details(&swap(TransactionState::Failed, Some("thorchain"), Some(90))).swap_progress.unwrap();
        assert_eq!(
            (failed.transfer, failed.swap, failed.eta_seconds),
            (BsvHomeSwapProgressStep::Completed, BsvHomeSwapProgressStep::Failed, None)
        );
        let reverted = details(&swap(TransactionState::Reverted, Some("thorchain"), None)).swap_progress.unwrap();
        assert_eq!((reverted.transfer, reverted.swap), (BsvHomeSwapProgressStep::Reverted, BsvHomeSwapProgressStep::Waiting));
        let refunded = details(&swap(TransactionState::Refunded, Some("thorchain"), None)).swap_progress.unwrap();
        assert_eq!((refunded.transfer, refunded.swap), (BsvHomeSwapProgressStep::Completed, BsvHomeSwapProgressStep::Refunded));

        let confirmed = details(&swap(TransactionState::Confirmed, Some("thorchain"), None));
        assert!(confirmed.swap_progress.is_none());
        assert_eq!(
            confirmed.swap_again,
            Some(BsvHomeSwapAgain {
                from_asset_id: AssetId::from_chain(Chain::Ethereum),
                to_asset_id: AssetId::from_chain(Chain::Bitcoin),
            })
        );

        let on_chain = details(&swap(TransactionState::Pending, Some("uniswap_v3"), Some(90)));
        assert!(on_chain.swap_progress.is_none());
        assert_eq!(on_chain.estimated_confirmation_seconds, Some(90));
        assert_eq!(
            details(&swap(TransactionState::Pending, Some("unknown"), None)).provider_name.as_deref(),
            Some("unknown"),
            "an unknown provider still shows its id"
        );
        assert!(details(&swap(TransactionState::Pending, None, None)).swap_progress.is_none());
    }

    #[test]
    fn test_details_show_a_pending_eta_and_nonzero_perpetual_figures() {
        let mut transfer = TransactionExtended::mock();
        transfer.transaction.state = TransactionState::Pending;
        transfer.confirmation_eta_seconds = Some(30);
        assert_eq!(details(&transfer).estimated_confirmation_seconds, Some(30));
        transfer.transaction.state = TransactionState::Confirmed;
        assert_eq!(details(&transfer).estimated_confirmation_seconds, None);
        transfer.transaction.state = TransactionState::Pending;
        transfer.confirmation_eta_seconds = Some(0);
        assert_eq!(details(&transfer).estimated_confirmation_seconds, None);

        let mut close = TransactionExtended::mock();
        close.transaction.transaction_type = TransactionType::PerpetualClosePosition;
        let metadata = |pnl, price| TransactionPerpetualMetadata {
            pnl,
            price,
            direction: PerpetualDirection::Long,
            is_liquidation: None,
            provider: None,
        };
        close.transaction.metadata = Some(serde_json::to_value(metadata(0.0, 0.0)).unwrap());
        assert_eq!((details(&close).pnl, details(&close).price), (None, None));
        close.transaction.metadata = Some(serde_json::to_value(metadata(-4.5, 12.0)).unwrap());
        assert_eq!((details(&close).pnl, details(&close).price), (Some(-4.5), Some(12.0)));
    }

    #[test]
    fn test_a_zero_stored_price_is_not_a_price() {
        let mut transfer = TransactionExtended::mock();
        let price = |value| Price {
            price: value,
            price_change_percentage_24h: 0.0,
            updated_at: Utc::now(),
            provider: Default::default(),
        };
        let amount_price = |extended: &TransactionExtended| transaction_amount(extended, BsvHomeAmountSign::None).price.map(|price| price.price);

        transfer.price = Some(price(0.0));
        assert_eq!(amount_price(&transfer), None);

        transfer.price = Some(price(12.0));
        assert_eq!(amount_price(&transfer), Some(12.0));
    }

    #[test]
    fn test_a_zero_swap_leg_price_is_not_a_price() {
        let mut swap = extended_with(swap(TransactionState::Confirmed, None, None).transaction, vec![Asset::mock_eth(), Asset::mock_btc()]);
        let ethereum = AssetId::from_chain(Chain::Ethereum);
        let leg_price = |extended: &TransactionExtended| {
            swap_leg(extended, SwapLeg::From, BsvHomeAmountSign::Outgoing)
                .and_then(|amount| amount.price)
                .map(|price| price.price)
        };

        swap.prices = vec![AssetPrice::new(ethereum.clone(), 0.0, 0.0, Utc::now())];
        assert_eq!(leg_price(&swap), None);

        swap.prices = vec![AssetPrice::new(ethereum, 12.0, 0.0, Utc::now())];
        assert_eq!(leg_price(&swap), Some(12.0));
    }
}
