use bsv_home_bsv::BsvAddress;
use bsv_home_tron::address::TronAddress;
use chrono::{DateTime, Utc};
use num_bigint::BigUint;
use number_formatter::{BigNumberFormatter, ValueFormatter, ValueStyle};
use primitives::{Address, AssetId, Chain, Wallet};
use url::Url;

use super::swap_error::BsvHomePortalSwapError;
use super::swap_model::{
    BsvHomePortalSwapAction, BsvHomePortalSwapActionNavigation, BsvHomePortalSwapAsset, BsvHomePortalSwapDirection, BsvHomePortalSwapOrder, BsvHomePortalSwapOrderStatus,
    BsvHomePortalSwapPair, BsvHomePortalSwapQuote, BsvHomePortalSwapResolutionStatus, BsvHomePortalSwapSyncStatus, BsvHomePortalSwapVerificationStatus, OrderRequest, QuoteRequest,
    USDT_CONTRACT, WireAsset, WireNavigation, WireOrder, WirePair, WireQuote,
};

pub(super) fn asset(chain: &str, kind: &str, contract: Option<&str>) -> Result<BsvHomePortalSwapAsset, BsvHomePortalSwapError> {
    match (chain, kind, contract.unwrap_or_default()) {
        ("BSV", "NATIVE", "") => Ok(BsvHomePortalSwapAsset {
            asset_id: Chain::BitcoinSv.as_asset_id(),
            symbol: "BSV".into(),
            network: "Bitcoin SV".into(),
            scale: 8,
        }),
        ("TRON", "NATIVE", "") => Ok(BsvHomePortalSwapAsset {
            asset_id: Chain::Tron.as_asset_id(),
            symbol: "TRX".into(),
            network: "TRON".into(),
            scale: 6,
        }),
        ("TRON", "TOKEN", USDT_CONTRACT) => Ok(BsvHomePortalSwapAsset {
            asset_id: AssetId::from(Chain::Tron, Some(USDT_CONTRACT.into())),
            symbol: "USDT".into(),
            network: "TRON / TRC20".into(),
            scale: 6,
        }),
        _ => Err(BsvHomePortalSwapError::UnsupportedPair),
    }
}

fn pair_asset(wire: &WireAsset) -> Result<BsvHomePortalSwapAsset, BsvHomePortalSwapError> {
    let mut asset = asset(&wire.chain, &wire.kind, wire.contract.as_deref())?;
    if wire.scale != asset.scale || wire.symbol != asset.symbol || wire.network.is_empty() {
        return Err(BsvHomePortalSwapError::InvalidResponse);
    }
    asset.network = wire.network.clone();
    Ok(asset)
}

pub(super) fn pair(wire: &WirePair) -> Result<BsvHomePortalSwapPair, BsvHomePortalSwapError> {
    let from = pair_asset(&wire.from)?;
    let to = pair_asset(&wire.to)?;
    let direction = BsvHomePortalSwapDirection::for_pair(&from.asset_id, &to.asset_id).ok_or(BsvHomePortalSwapError::UnsupportedPair)?;
    if wire.provider_code.is_empty() || wire.provider_display_name.is_empty() {
        return Err(BsvHomePortalSwapError::InvalidResponse);
    }
    validate_supported_direction(direction)?;
    if !wire.floating_rate || !wire.amount_from || wire.amount_to {
        return Err(BsvHomePortalSwapError::UnsupportedPair);
    }
    Ok(BsvHomePortalSwapPair {
        direction,
        from,
        to,
        provider_display_name: wire.provider_display_name.clone(),
    })
}

pub(super) fn pairs(wires: Vec<WirePair>) -> Result<Vec<(WirePair, BsvHomePortalSwapPair)>, BsvHomePortalSwapError> {
    wires
        .into_iter()
        .filter_map(|wire| match pair(&wire) {
            Ok(pair) => Some(Ok((wire, pair))),
            Err(BsvHomePortalSwapError::UnsupportedPair) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

pub(super) fn validate_supported_direction(direction: BsvHomePortalSwapDirection) -> Result<(), BsvHomePortalSwapError> {
    match direction {
        BsvHomePortalSwapDirection::BsvToUsdt | BsvHomePortalSwapDirection::UsdtToBsv | BsvHomePortalSwapDirection::UsdtToTrx | BsvHomePortalSwapDirection::TrxToUsdt => Ok(()),
        BsvHomePortalSwapDirection::BsvToTrx | BsvHomePortalSwapDirection::TrxToBsv => Err(BsvHomePortalSwapError::UnsupportedPair),
    }
}

pub(super) fn decimal_amount(value: &str, scale: u32) -> Result<BigUint, BsvHomePortalSwapError> {
    if value.is_empty() || value.len() > 80 || value.bytes().any(|byte| !byte.is_ascii_digit() && byte != b'.') || value.bytes().filter(|byte| *byte == b'.').count() > 1 {
        return Err(BsvHomePortalSwapError::InvalidAmount);
    }
    let amount = BigNumberFormatter::value_from_amount_exact(value, scale).map_err(|_| BsvHomePortalSwapError::InvalidAmount)?;
    if amount == BigUint::from(0u32) {
        return Err(BsvHomePortalSwapError::InvalidAmount);
    }
    Ok(amount)
}

pub(super) fn minor_amount(value: &str, positive: bool) -> Result<BigUint, BsvHomePortalSwapError> {
    if value.is_empty() || value.len() > 80 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(BsvHomePortalSwapError::InvalidResponse);
    }
    let amount = value.parse::<BigUint>().map_err(|_| BsvHomePortalSwapError::InvalidResponse)?;
    if positive && amount == BigUint::from(0u32) {
        return Err(BsvHomePortalSwapError::InvalidResponse);
    }
    Ok(amount)
}

fn display_value(amount: &BigUint, scale: u32) -> Result<String, BsvHomePortalSwapError> {
    let formatted = ValueFormatter::format(ValueStyle::Full, &amount.to_string(), scale as i32).map_err(|_| BsvHomePortalSwapError::InvalidResponse)?;
    let (integer, fraction) = formatted.split_once('.').unwrap_or((&formatted, ""));
    Ok(format!("{integer}.{fraction:0<4}"))
}

pub(super) fn display(amount: &BigUint, asset: &BsvHomePortalSwapAsset) -> Result<String, BsvHomePortalSwapError> {
    Ok(format!("{} {}", display_value(amount, asset.scale)?, asset.symbol))
}

pub(super) fn quote_request(pair: &WirePair, amount: BigUint) -> QuoteRequest {
    QuoteRequest {
        from_chain: pair.from.chain.clone(),
        from_kind: pair.from.kind.clone(),
        from_contract: pair.from.contract.clone().unwrap_or_default(),
        to_chain: pair.to.chain.clone(),
        to_kind: pair.to.kind.clone(),
        to_contract: pair.to.contract.clone().unwrap_or_default(),
        from_amount: amount.to_string(),
    }
}

pub(super) fn quote(wire: WireQuote, request: &QuoteRequest, wire_pair: &WirePair, now: DateTime<Utc>) -> Result<BsvHomePortalSwapQuote, BsvHomePortalSwapError> {
    let pair = pair(wire_pair)?;
    if wire.quote_id.is_empty()
        || wire.from_chain != request.from_chain
        || wire.from_kind != request.from_kind
        || wire.from_contract.as_deref().unwrap_or_default() != request.from_contract
        || wire.to_chain != request.to_chain
        || wire.to_kind != request.to_kind
        || wire.to_contract.as_deref().unwrap_or_default() != request.to_contract
        || wire.provider_code != wire_pair.provider_code
        || !wire.floating_rate
        || wire.quote_nature != "ESTIMATED"
        || wire.provider_display_name.is_empty()
    {
        return Err(BsvHomePortalSwapError::InvalidResponse);
    }
    if wire.expires_at <= now {
        return Err(BsvHomePortalSwapError::QuoteExpired);
    }
    let from_amount = minor_amount(&wire.from_amount, true)?;
    let requested = minor_amount(&request.from_amount, true)?;
    let estimated_to_amount = minor_amount(&wire.estimated_to_amount, true)?;
    let minimum = minor_amount(&wire.min_from_amount, false)?;
    let maximum = wire
        .max_from_amount
        .as_deref()
        .filter(|amount| !amount.is_empty())
        .map(|amount| minor_amount(amount, true))
        .transpose()?;
    if from_amount != requested || from_amount < minimum || maximum.as_ref().is_some_and(|maximum| &from_amount > maximum || maximum < &minimum) {
        return Err(BsvHomePortalSwapError::InvalidResponse);
    }
    let rate_scale = 8;
    let rate = (&estimated_to_amount * BigUint::from(10u32).pow(pair.from.scale + rate_scale)) / (&from_amount * BigUint::from(10u32).pow(pair.to.scale));
    let rate_display = display_value(&rate, rate_scale)?;
    Ok(BsvHomePortalSwapQuote {
        quote_id: wire.quote_id,
        direction: pair.direction,
        from_amount_display: display(&from_amount, &pair.from)?,
        estimated_to_amount_display: display(&estimated_to_amount, &pair.to)?,
        min_from_amount_display: display(&minimum, &pair.from)?,
        max_from_amount_display: maximum.as_ref().map(|maximum| display(maximum, &pair.from)).transpose()?,
        rate_display: format!("{} {} ≈ {} {}", display_value(&BigUint::from(1u32), 0)?, pair.from.symbol, rate_display, pair.to.symbol),
        from: pair.from,
        to: pair.to,
        from_amount,
        estimated_to_amount,
        provider_display_name: wire.provider_display_name,
        expires_at: wire.expires_at.timestamp(),
    })
}

pub(super) fn valid_address(chain: Chain, address: &str) -> bool {
    match chain {
        Chain::BitcoinSv => BsvAddress::from_str(address).is_ok(),
        Chain::Tron => TronAddress::parse(address).is_ok_and(|parsed| parsed.encode() == address),
        _ => false,
    }
}

pub(super) fn wallet_addresses(wallet: &Wallet, direction: BsvHomePortalSwapDirection) -> Result<(String, String), BsvHomePortalSwapError> {
    let (source, target) = match direction {
        BsvHomePortalSwapDirection::BsvToUsdt | BsvHomePortalSwapDirection::BsvToTrx => (Chain::BitcoinSv, Chain::Tron),
        BsvHomePortalSwapDirection::UsdtToBsv | BsvHomePortalSwapDirection::TrxToBsv => (Chain::Tron, Chain::BitcoinSv),
        BsvHomePortalSwapDirection::UsdtToTrx | BsvHomePortalSwapDirection::TrxToUsdt => (Chain::Tron, Chain::Tron),
    };
    let refund = wallet.account(source).ok_or(BsvHomePortalSwapError::MissingAccount)?.address.clone();
    let payout = wallet.account(target).ok_or(BsvHomePortalSwapError::MissingAccount)?.address.clone();
    if !valid_address(source, &refund) || !valid_address(target, &payout) {
        return Err(BsvHomePortalSwapError::MissingAccount);
    }
    Ok((payout, refund))
}

pub(super) fn resource_id(value: &str) -> Result<&str, BsvHomePortalSwapError> {
    if value.is_empty() || value.len() > 128 || !value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) {
        return Err(BsvHomePortalSwapError::InvalidResponse);
    }
    Ok(value)
}

pub(super) fn transaction_id(value: &str) -> Result<&str, BsvHomePortalSwapError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(BsvHomePortalSwapError::InvalidResponse);
    }
    Ok(value)
}

pub(super) fn order(
    wire: WireOrder,
    previous: Option<&BsvHomePortalSwapOrder>,
    quote: Option<&BsvHomePortalSwapQuote>,
    request: Option<&OrderRequest>,
    now: DateTime<Utc>,
) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
    resource_id(&wire.order_id)?;
    let from_amount = minor_amount(&wire.from_amount, true)?;
    let estimated_to_amount = minor_amount(&wire.estimated_to_amount, false)?;
    let actual_to_amount = wire.actual_to_amount.as_deref().map(|amount| minor_amount(amount, false)).transpose()?;
    let wire_assets = match (wire.from_chain.as_deref(), wire.from_kind.as_deref(), wire.to_chain.as_deref(), wire.to_kind.as_deref()) {
        (Some(from_chain), Some(from_kind), Some(to_chain), Some(to_kind)) => Some((
            asset(from_chain, from_kind, wire.from_contract.as_deref())?,
            asset(to_chain, to_kind, wire.to_contract.as_deref())?,
        )),
        (None, None, None, None) if wire.from_contract.is_none() && wire.to_contract.is_none() => None,
        _ => return Err(BsvHomePortalSwapError::InvalidResponse),
    };
    let confirmed = quote
        .map(|quote| (quote.from.clone(), quote.to.clone()))
        .or_else(|| previous.and_then(|previous| Some((previous.from.clone()?, previous.to.clone()?))));
    if let (Some((wire_from, wire_to)), Some((confirmed_from, confirmed_to))) = (&wire_assets, &confirmed)
        && (wire_from.asset_id != confirmed_from.asset_id || wire_to.asset_id != confirmed_to.asset_id)
    {
        return Err(BsvHomePortalSwapError::OrderMismatch);
    }
    let assets = confirmed.or(wire_assets);
    let direction = assets
        .as_ref()
        .map(|(from, to)| BsvHomePortalSwapDirection::for_pair(&from.asset_id, &to.asset_id).ok_or(BsvHomePortalSwapError::UnsupportedPair))
        .transpose()?;
    if let Some(quote) = quote
        && (from_amount != quote.from_amount || estimated_to_amount != quote.estimated_to_amount || wire.provider_display_name != quote.provider_display_name)
    {
        return Err(BsvHomePortalSwapError::OrderMismatch);
    }
    if let Some(request) = request
        && (wire.payout_address != request.payout_address
            || wire.refund_address != request.refund_address
            || wire.client_operation_id.as_ref().is_some_and(|value| value != &request.client_operation_id))
    {
        return Err(BsvHomePortalSwapError::OrderMismatch);
    }
    if let Some(previous) = previous
        && (from_amount != previous.from_amount
            || wire.payout_address != previous.payout_address
            || wire.refund_address != previous.refund_address
            || previous.deposit_address.as_ref().is_some_and(|deposit| wire.deposit_address.as_ref() != Some(deposit))
            || previous.deposit_extra_id.as_ref().is_some_and(|extra| wire.deposit_extra_id.as_ref() != Some(extra)))
    {
        return Err(BsvHomePortalSwapError::OrderMismatch);
    }
    for transaction in [&wire.source_txid, &wire.payout_txid, &wire.refund_txid].into_iter().flatten() {
        transaction_id(transaction)?;
    }
    let (from, to) = match assets {
        Some((from, to)) => {
            if !valid_address(from.asset_id.chain, &wire.refund_address)
                || !valid_address(to.asset_id.chain, &wire.payout_address)
                || wire.deposit_address.as_ref().is_some_and(|address| !valid_address(from.asset_id.chain, address))
            {
                return Err(BsvHomePortalSwapError::OrderMismatch);
            }
            (Some(from), Some(to))
        }
        None => (None, None),
    };
    let source_txid = match (&wire.source_txid, previous.and_then(|previous| previous.source_txid.as_ref())) {
        (Some(remote), Some(local)) if remote != local => return Err(BsvHomePortalSwapError::BroadcastConflict),
        (Some(remote), _) => Some(remote.clone()),
        (None, Some(local)) => Some(local.clone()),
        (None, None) => None,
    };
    let mut order = BsvHomePortalSwapOrder {
        order_id: wire.order_id,
        client_operation_id: wire
            .client_operation_id
            .or_else(|| request.map(|request| request.client_operation_id.clone()))
            .or_else(|| previous.and_then(|previous| previous.client_operation_id.clone())),
        direction,
        from_amount_display: from.as_ref().map(|asset| display(&from_amount, asset)).transpose()?,
        estimated_to_amount_display: to.as_ref().map(|asset| display(&estimated_to_amount, asset)).transpose()?,
        actual_to_amount_display: actual_to_amount.as_ref().zip(to.as_ref()).map(|(amount, asset)| display(amount, asset)).transpose()?,
        payout_verified: previous.is_some_and(|previous| previous.payout_verified && previous.payout_txid == wire.payout_txid && previous.actual_to_amount == actual_to_amount),
        actual_to_amount,
        from,
        to,
        from_amount,
        estimated_to_amount,
        deposit_address: wire.deposit_address,
        deposit_extra_id: wire.deposit_extra_id,
        payout_address: wire.payout_address,
        refund_address: wire.refund_address,
        provider_display_name: wire.provider_display_name,
        provider_order_id: wire.provider_order_id,
        order_status: wire.order_status,
        verification_status: wire.verification_status,
        sync_status: wire.sync_status,
        resolution_status: wire.resolution_status,
        last_active_phase: wire.last_active_phase,
        source_txid,
        broadcast_report_pending: wire.source_txid.is_none() && previous.is_some_and(|previous| previous.source_txid.is_some()),
        payout_txid: wire.payout_txid,
        refund_txid: wire.refund_txid,
        deposit_expires_at: wire.deposit_expires_at.map(|date| date.timestamp()),
        updated_at: wire.updated_at.timestamp(),
        version: wire.version,
        reason_code: wire.reason_code,
        required_actions: wire
            .required_actions
            .into_iter()
            .map(|action| {
                resource_id(&action.action_id)?;
                Ok(BsvHomePortalSwapAction {
                    action_id: action.action_id,
                    action_type: action.action_type,
                    version: action.version,
                    entry_mode: action.entry_mode,
                    expires_at: action.expires_at.map(|date| date.timestamp()),
                })
            })
            .collect::<Result<Vec<_>, BsvHomePortalSwapError>>()?,
        can_pay: false,
    };
    order.can_pay = can_pay(&order, now);
    Ok(order)
}

pub(super) fn can_pay(order: &BsvHomePortalSwapOrder, now: DateTime<Utc>) -> bool {
    order.direction.is_some()
        && order.from.is_some()
        && order.to.is_some()
        && order.deposit_address.is_some()
        && order.source_txid.is_none()
        && matches!(
            order.order_status,
            BsvHomePortalSwapOrderStatus::AwaitingSignature | BsvHomePortalSwapOrderStatus::AwaitingDeposit
        )
        && matches!(
            order.verification_status,
            BsvHomePortalSwapVerificationStatus::NotRequired | BsvHomePortalSwapVerificationStatus::Approved
        )
        && order.sync_status == BsvHomePortalSwapSyncStatus::Fresh
        && order.resolution_status == BsvHomePortalSwapResolutionStatus::Open
        && order.required_actions.is_empty()
        && order.deposit_expires_at.is_none_or(|expires_at| expires_at > now.timestamp())
}

pub(super) fn navigation(wire: WireNavigation, now: DateTime<Utc>) -> Result<BsvHomePortalSwapActionNavigation, BsvHomePortalSwapError> {
    let url = Url::parse(&wire.navigation_url).map_err(|_| BsvHomePortalSwapError::ActionUnavailable)?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || wire.expires_at.is_some_and(|expires_at| expires_at <= now)
    {
        return Err(BsvHomePortalSwapError::ActionUnavailable);
    }
    Ok(BsvHomePortalSwapActionNavigation {
        navigation_url: url.into(),
        entry_mode: wire.entry_mode,
        expires_at: wire.expires_at.map(|date| date.timestamp()),
    })
}
