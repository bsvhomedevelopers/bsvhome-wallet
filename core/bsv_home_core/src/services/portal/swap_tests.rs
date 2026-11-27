use chrono::{Duration, TimeZone, Utc};
use num_bigint::BigUint;
use primitives::{Account, Chain, Wallet};
use serde_json::Value;

use super::swap_error::BsvHomePortalSwapError;
use super::swap_model::{
    BsvHomePortalSwapDirection, BsvHomePortalSwapEntryMode, BsvHomePortalSwapOrder, BsvHomePortalSwapOrderStatus, BsvHomePortalSwapVerificationStatus, WireAsset, WireNavigation,
    WireOrder, WireOrderPage, WirePair, WireQuote,
};
use super::swap_rules;

fn pair() -> WirePair {
    serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap()
}

fn wire_quote() -> WireQuote {
    serde_json::from_str(include_str!("../../../testdata/portal/quote.json")).unwrap()
}

fn wire_order() -> WireOrder {
    serde_json::from_str(include_str!("../../../testdata/portal/order.json")).unwrap()
}

fn trx() -> WireAsset {
    WireAsset {
        chain: "TRON".into(),
        kind: "NATIVE".into(),
        contract: Some(String::new()),
        symbol: "TRX".into(),
        network: "TRON".into(),
        scale: 6,
    }
}

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 27, 9, 12, 30).unwrap()
}

#[test]
fn test_decimal_amount_preserves_precision_without_truncation() {
    assert_eq!(swap_rules::decimal_amount("10.9", 6).unwrap(), BigUint::from(10_900_000u32));
    assert_eq!(swap_rules::decimal_amount("1.50000000", 8).unwrap(), BigUint::from(150_000_000u32));
    assert_eq!(swap_rules::decimal_amount("9007199254740993.123456", 6).unwrap().to_string(), "9007199254740993123456");
    for invalid in ["0", "-1", "1e6", "+1", "1,5", "1.2345678", "1.1.1", " 1", ""] {
        assert_eq!(swap_rules::decimal_amount(invalid, 6), Err(BsvHomePortalSwapError::InvalidAmount));
    }
}

#[test]
fn test_display_pads_four_fractional_digits_without_truncating() {
    let asset = swap_rules::pair(&pair()).unwrap().from;
    assert_eq!(swap_rules::display(&BigUint::from(0u32), &asset).unwrap(), "0.0000 BSV");
    assert_eq!(swap_rules::display(&BigUint::from(100_000_000u32), &asset).unwrap(), "1.0000 BSV");
    assert_eq!(swap_rules::display(&BigUint::from(123_400_000u32), &asset).unwrap(), "1.2340 BSV");
    assert_eq!(swap_rules::display(&BigUint::from(123_456_789u32), &asset).unwrap(), "1.23456789 BSV");
    assert_eq!(swap_rules::display(&BigUint::from(1u32), &asset).unwrap(), "0.00000001 BSV");
    assert_eq!(swap_rules::display(&BigUint::from(100_000_000_000_000u64), &asset).unwrap(), "1,000,000.0000 BSV");
}

#[test]
fn test_pair_rejects_symbol_only_identity_and_incorrect_precision() {
    let mut wrong_contract = pair();
    wrong_contract.to.contract = Some("wrong-contract".into());
    assert_eq!(swap_rules::pair(&wrong_contract), Err(BsvHomePortalSwapError::UnsupportedPair));
    let mut wrong_scale = pair();
    wrong_scale.to.scale = 18;
    assert_eq!(swap_rules::pair(&wrong_scale), Err(BsvHomePortalSwapError::InvalidResponse));
    let mut fixed = pair();
    fixed.floating_rate = false;
    assert_eq!(swap_rules::pair(&fixed), Err(BsvHomePortalSwapError::UnsupportedPair));
}

#[test]
fn test_quote_requires_exact_amount_assets_provider_and_estimated_nature() {
    let pair = pair();
    let request = swap_rules::quote_request(&pair, BigUint::from(150_000_000u32));
    for maximum in [None, Some(String::new())] {
        let response = || {
            let mut wire = wire_quote();
            wire.max_from_amount = maximum.clone();
            wire
        };
        let quote = swap_rules::quote(response(), &request, &pair, now()).unwrap();
        assert_eq!(quote.from_amount, BigUint::from(150_000_000u32));
        assert_eq!(quote.estimated_to_amount, BigUint::from(12_345_678u32));
        assert_eq!(quote.from_amount_display, "1.5000 BSV");
        assert_eq!(quote.estimated_to_amount_display, "12.345678 USDT");
        assert_eq!(quote.min_from_amount_display, "0.1000 BSV");
        assert_eq!(quote.rate_display, "1.0000 BSV ≈ 8.230452 USDT");
        let mut amount = response();
        amount.from_amount = "150000001".into();
        assert_eq!(swap_rules::quote(amount, &request, &pair, now()), Err(BsvHomePortalSwapError::InvalidResponse));
        let mut contract = response();
        contract.to_contract = Some("wrong-token".into());
        assert_eq!(swap_rules::quote(contract, &request, &pair, now()), Err(BsvHomePortalSwapError::InvalidResponse));
        let mut provider = response();
        provider.provider_code = "other-provider".into();
        assert_eq!(swap_rules::quote(provider, &request, &pair, now()), Err(BsvHomePortalSwapError::InvalidResponse));
        let mut fixed = response();
        fixed.quote_nature = "FIXED".into();
        assert_eq!(swap_rules::quote(fixed, &request, &pair, now()), Err(BsvHomePortalSwapError::InvalidResponse));
    }
}

#[test]
fn test_usdt_to_bsv_quote_accepts_empty_missing_and_null_maximum() {
    let original = pair();
    let pair = WirePair {
        from: original.to,
        to: original.from,
        provider_code: "CHANGENOW".into(),
        provider_display_name: "ChangeNOW".into(),
        ..original
    };
    let request = swap_rules::quote_request(&pair, BigUint::from(20_000_000u32));
    let response: Value = serde_json::from_str(include_str!("../../../testdata/portal/quote_usdt_bsv_empty_maximum.json")).unwrap();
    let mut without_maximum = response.clone();
    without_maximum.as_object_mut().unwrap().remove("maxFromAmount");
    let quote = swap_rules::quote(serde_json::from_value(without_maximum).unwrap(), &request, &pair, now()).unwrap();
    assert_eq!(quote.direction, BsvHomePortalSwapDirection::UsdtToBsv);
    assert_eq!(quote.from_amount, BigUint::from(20_000_000u32));
    assert_eq!(quote.estimated_to_amount, BigUint::from(72_258_550u32));
    assert_eq!(quote.from_amount_display, "20.0000 USDT");
    assert_eq!(quote.estimated_to_amount_display, "0.7225855 BSV");
    assert_eq!(quote.min_from_amount_display, "12.028945 USDT");
    assert_eq!(quote.max_from_amount_display, None);
    for maximum in [Value::Null, Value::from("")] {
        let mut response = response.clone();
        response["maxFromAmount"] = maximum;
        assert_eq!(swap_rules::quote(serde_json::from_value(response).unwrap(), &request, &pair, now()), Ok(quote.clone()));
    }
    for (maximum, display) in [("20000000", "20.0000 USDT"), ("29000000", "29.0000 USDT")] {
        let mut response = response.clone();
        response["maxFromAmount"] = maximum.into();
        let mut expected = quote.clone();
        expected.max_from_amount_display = Some(display.into());
        assert_eq!(swap_rules::quote(serde_json::from_value(response).unwrap(), &request, &pair, now()), Ok(expected));
    }
}

#[test]
fn test_quote_rejects_nonempty_invalid_maximum() {
    let pair = pair();
    let request = swap_rules::quote_request(&pair, BigUint::from(150_000_000u32));
    for maximum in [" ", "\t", "0", "-1", "1.2", "invalid", " 150000000", "150000000 "] {
        let mut response = wire_quote();
        response.max_from_amount = Some(maximum.into());
        assert_eq!(swap_rules::quote(response, &request, &pair, now()), Err(BsvHomePortalSwapError::InvalidResponse));
    }
}

#[test]
fn test_quote_reference_rate_pads_four_fractional_digits() {
    let pair = pair();
    let request = swap_rules::quote_request(&pair, BigUint::from(100_000_000u32));
    let mut response = wire_quote();
    response.from_amount = "100000000".into();
    response.estimated_to_amount = "1000000".into();
    let quote = swap_rules::quote(response, &request, &pair, now()).unwrap();
    assert_eq!(quote.rate_display, "1.0000 BSV ≈ 1.0000 USDT");
}

#[test]
fn test_quote_rejects_expired_and_invalid_range_values() {
    let pair = pair();
    let request = swap_rules::quote_request(&pair, BigUint::from(150_000_000u32));
    for maximum in [None, Some(String::new())] {
        let response = || {
            let mut wire = wire_quote();
            wire.max_from_amount = maximum.clone();
            wire
        };
        let mut expired = response();
        expired.expires_at = now();
        assert_eq!(swap_rules::quote(expired, &request, &pair, now()), Err(BsvHomePortalSwapError::QuoteExpired));
        let mut minimum = response();
        minimum.min_from_amount = "150000001".into();
        assert_eq!(swap_rules::quote(minimum, &request, &pair, now()), Err(BsvHomePortalSwapError::InvalidResponse));
        let mut decimal = response();
        decimal.estimated_to_amount = "1.2".into();
        assert_eq!(swap_rules::quote(decimal, &request, &pair, now()), Err(BsvHomePortalSwapError::InvalidResponse));
    }
    let mut maximum = wire_quote();
    maximum.max_from_amount = Some("149999999".into());
    assert_eq!(swap_rules::quote(maximum, &request, &pair, now()), Err(BsvHomePortalSwapError::InvalidResponse));
}

#[test]
fn test_order_unknown_asset_identity_never_enables_payment() {
    let mut wire = wire_order();
    wire.from_chain = None;
    wire.from_kind = None;
    wire.from_contract = None;
    wire.to_chain = None;
    wire.to_kind = None;
    wire.to_contract = None;
    let order = swap_rules::order(wire, None, None, None, now()).unwrap();
    assert_eq!(order.direction, None);
    assert!(!order.can_pay);
    assert_eq!(order.from_amount_display, None);
}

#[test]
fn test_order_payment_is_independent_of_verification_and_sync() {
    let order = swap_rules::order(wire_order(), None, None, None, now()).unwrap();
    assert_eq!(order.from_amount_display, Some("1.5000 BSV".into()));
    assert_eq!(order.estimated_to_amount_display, Some("12.345678 USDT".into()));
    assert!(order.can_pay);
    assert!(!swap_rules::can_pay(&order, now() + Duration::hours(2)));
    let mut verification = wire_order();
    verification.verification_status = BsvHomePortalSwapVerificationStatus::Required;
    assert!(!swap_rules::order(verification, None, None, None, now()).unwrap().can_pay);
    let mut hold = wire_order();
    hold.order_status = BsvHomePortalSwapOrderStatus::OnHold;
    assert!(!swap_rules::order(hold, None, None, None, now()).unwrap().can_pay);
    let mut source = wire_order();
    source.source_txid = Some("aa".repeat(32));
    assert!(!swap_rules::order(source, None, None, None, now()).unwrap().can_pay);
}

#[test]
fn test_order_never_discards_unreported_source_transaction() {
    let mut local = swap_rules::order(wire_order(), None, None, None, now()).unwrap();
    local.source_txid = Some("aa".repeat(32));
    let merged = swap_rules::order(wire_order(), Some(&local), None, None, now()).unwrap();
    assert_eq!(merged.source_txid, local.source_txid);
    assert!(!merged.can_pay);
    let mut conflict = wire_order();
    conflict.source_txid = Some("bb".repeat(32));
    assert_eq!(swap_rules::order(conflict, Some(&local), None, None, now()), Err(BsvHomePortalSwapError::BroadcastConflict));
}

#[test]
fn test_order_rejects_mutated_payment_destination_and_amount() {
    let previous = swap_rules::order(wire_order(), None, None, None, now()).unwrap();
    let mut destination = wire_order();
    destination.deposit_address = Some("1BoatSLRHtKNngkdXEeobR76b53LETtpyT".into());
    assert_eq!(
        swap_rules::order(destination, Some(&previous), None, None, now()),
        Err(BsvHomePortalSwapError::OrderMismatch)
    );
    let mut amount = wire_order();
    amount.from_amount = "149999999".into();
    assert_eq!(swap_rules::order(amount, Some(&previous), None, None, now()), Err(BsvHomePortalSwapError::OrderMismatch));
}

#[test]
fn test_order_page_accepts_live_total_string_without_inventing_missing_fields() {
    let page: WireOrderPage = serde_json::from_str(r#"{"total":"0","records":[]}"#).unwrap();
    assert_eq!(page.total, 0);
    assert_eq!(page.records.len(), 0);
    let numeric: WireOrderPage = serde_json::from_str(r#"{"total":2,"records":[]}"#).unwrap();
    assert_eq!(numeric.total, 2);
    assert!(serde_json::from_str::<WireOrderPage>(r#"{"total":"-1","records":[]}"#).is_err());
    let mut order: Value = serde_json::from_str(include_str!("../../../testdata/portal/order.json")).unwrap();
    order.as_object_mut().unwrap().remove("verificationStatus");
    assert!(serde_json::from_value::<WireOrder>(order).is_err());
}

#[test]
fn test_order_page_accepts_empty_missing_and_null_optional_payment_values() {
    let response: Value = serde_json::from_str(include_str!("../../../testdata/portal/order_page_usdt_bsv_empty_optionals.json")).unwrap();
    let page: WireOrderPage = serde_json::from_value(response.clone()).unwrap();
    assert_eq!(page.total, 1);
    let order = swap_rules::order(page.records.into_iter().next().unwrap(), None, None, None, now()).unwrap();
    assert_eq!(order.direction, Some(BsvHomePortalSwapDirection::UsdtToBsv));
    assert_eq!(order.actual_to_amount, None);
    assert_eq!(order.deposit_expires_at, None);
    assert_eq!(order.source_txid, None);
    assert_eq!(order.payout_txid, None);
    assert_eq!(order.refund_txid, None);
    assert_eq!(order.deposit_extra_id, Some(String::new()));
    assert_eq!(order.reason_code, Some(String::new()));
    assert!(order.can_pay);
    for missing in [false, true] {
        let mut response = response.clone();
        for field in ["actualToAmount", "depositExpiresAt", "sourceTxid", "payoutTxid", "refundTxid"] {
            if missing {
                response["records"][0].as_object_mut().unwrap().remove(field);
            } else {
                response["records"][0][field] = Value::Null;
            }
        }
        let page: WireOrderPage = serde_json::from_value(response).unwrap();
        assert_eq!(swap_rules::order(page.records.into_iter().next().unwrap(), None, None, None, now()), Ok(order.clone()));
    }
    let mut populated = response.clone();
    populated["records"][0]["actualToAmount"] = "72258550".into();
    populated["records"][0]["sourceTxid"] = "aa".repeat(32).into();
    populated["records"][0]["payoutTxid"] = "bb".repeat(32).into();
    populated["records"][0]["refundTxid"] = "cc".repeat(32).into();
    let expiry = now() + Duration::minutes(1);
    populated["records"][0]["depositExpiresAt"] = expiry.to_rfc3339().into();
    let page: WireOrderPage = serde_json::from_value(populated).unwrap();
    let populated = swap_rules::order(page.records.into_iter().next().unwrap(), None, None, None, now()).unwrap();
    assert_eq!(populated.actual_to_amount, Some(BigUint::from(72_258_550u32)));
    assert_eq!(populated.deposit_expires_at, Some(expiry.timestamp()));
    assert_eq!(populated.source_txid, Some("aa".repeat(32)));
    assert_eq!(populated.payout_txid, Some("bb".repeat(32)));
    assert_eq!(populated.refund_txid, Some("cc".repeat(32)));
    let mut local = order.clone();
    local.source_txid = Some("aa".repeat(32));
    let page: WireOrderPage = serde_json::from_value(response).unwrap();
    let merged = swap_rules::order(page.records.into_iter().next().unwrap(), Some(&local), None, None, now()).unwrap();
    assert_eq!(merged.source_txid, local.source_txid);
    assert!(merged.broadcast_report_pending);
    assert!(!merged.can_pay);
}

#[test]
fn test_order_optional_payment_values_reject_nonempty_malformed_values() {
    let response: Value = serde_json::from_str(include_str!("../../../testdata/portal/order_page_usdt_bsv_empty_optionals.json")).unwrap();
    for (field, invalid) in [
        ("actualToAmount", " "),
        ("actualToAmount", "-1"),
        ("actualToAmount", "1.2"),
        ("sourceTxid", " "),
        ("sourceTxid", "invalid"),
        ("payoutTxid", "invalid"),
        ("refundTxid", "invalid"),
    ] {
        let mut response = response.clone();
        response["records"][0][field] = invalid.into();
        let page: WireOrderPage = serde_json::from_value(response).unwrap();
        assert_eq!(
            swap_rules::order(page.records.into_iter().next().unwrap(), None, None, None, now()),
            Err(BsvHomePortalSwapError::InvalidResponse)
        );
    }
    for invalid in [" ", "tomorrow", "2026-09-27 09:13:00"] {
        let mut response = response.clone();
        response["records"][0]["depositExpiresAt"] = invalid.into();
        assert!(serde_json::from_value::<WireOrderPage>(response).is_err());
    }
    let mut expired = response.clone();
    expired["records"][0]["depositExpiresAt"] = now().to_rfc3339().into();
    let page: WireOrderPage = serde_json::from_value(expired).unwrap();
    assert!(!swap_rules::order(page.records.into_iter().next().unwrap(), None, None, None, now()).unwrap().can_pay);
    for field in [
        "fromAmount",
        "payoutAddress",
        "refundAddress",
        "orderStatus",
        "verificationStatus",
        "requiredActions",
        "updatedAt",
    ] {
        let mut response = response.clone();
        response["records"][0].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<WireOrderPage>(response).is_err());
    }
}

#[test]
fn test_navigation_requires_fresh_https_without_url_credentials() {
    let navigation = |url: &str| WireNavigation {
        navigation_url: url.into(),
        entry_mode: BsvHomePortalSwapEntryMode::ProviderOrderPage,
        expires_at: None,
    };
    assert_eq!(
        swap_rules::navigation(navigation("https://provider.example/order"), now()).unwrap().navigation_url,
        "https://provider.example/order"
    );
    for url in ["http://provider.example/order", "javascript:alert(1)", "https://token@provider.example/order"] {
        assert_eq!(swap_rules::navigation(navigation(url), now()), Err(BsvHomePortalSwapError::ActionUnavailable));
    }
    let mut expired = navigation("https://provider.example/order");
    expired.expires_at = Some(now());
    assert_eq!(swap_rules::navigation(expired, now()), Err(BsvHomePortalSwapError::ActionUnavailable));
}

#[test]
fn test_resource_identifier_and_transaction_identifier_reject_path_injection() {
    for invalid in ["../order", "order?action=other", "order#fragment", "order/other", ""] {
        assert_eq!(swap_rules::resource_id(invalid), Err(BsvHomePortalSwapError::InvalidResponse));
    }
    assert_eq!(swap_rules::transaction_id("abc"), Err(BsvHomePortalSwapError::InvalidResponse));
    assert_eq!(swap_rules::transaction_id(&"zz".repeat(32)), Err(BsvHomePortalSwapError::InvalidResponse));
}

#[test]
fn test_supported_pairs_bind_quotes_and_orders_to_both_exact_assets() {
    let original = pair();
    let bsv = original.from.clone();
    let usdt = original.to.clone();
    let wallet = Wallet::mock_with_accounts(vec![
        Account::mock(Chain::BitcoinSv, "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5"),
        Account::mock(Chain::Tron, "TBKwjUtXVsX1r724C1V52nocBgtioDjx9u"),
    ]);
    let cases = [
        (bsv.clone(), usdt.clone(), BsvHomePortalSwapDirection::BsvToUsdt),
        (usdt.clone(), bsv.clone(), BsvHomePortalSwapDirection::UsdtToBsv),
        (usdt.clone(), trx(), BsvHomePortalSwapDirection::UsdtToTrx),
        (trx(), usdt, BsvHomePortalSwapDirection::TrxToUsdt),
    ];
    for (from, to, direction) in cases {
        let pair = WirePair { from, to, ..pair() };
        let amount = swap_rules::decimal_amount("1.123456", pair.from.scale).unwrap();
        let request = swap_rules::quote_request(&pair, amount.clone());
        let mut quote = wire_quote();
        quote.from_chain = request.from_chain.clone();
        quote.from_kind = request.from_kind.clone();
        quote.from_contract = Some(request.from_contract.clone());
        quote.to_chain = request.to_chain.clone();
        quote.to_kind = request.to_kind.clone();
        quote.to_contract = Some(request.to_contract.clone());
        quote.from_amount = request.from_amount.clone();
        quote.min_from_amount = "1".into();
        let quote = swap_rules::quote(quote, &request, &pair, now()).unwrap();
        assert_eq!(quote.direction, direction);
        assert_eq!(quote.from_amount, amount);
        assert_eq!(quote.from, swap_rules::asset(&pair.from.chain, &pair.from.kind, pair.from.contract.as_deref()).unwrap());
        assert_eq!(quote.to, swap_rules::asset(&pair.to.chain, &pair.to.kind, pair.to.contract.as_deref()).unwrap());
        let (payout, refund) = swap_rules::wallet_addresses(&wallet, direction).unwrap();
        let mut wire = wire_order();
        wire.from_chain = Some(request.from_chain);
        wire.from_kind = Some(request.from_kind);
        wire.from_contract = Some(request.from_contract);
        wire.to_chain = Some(request.to_chain);
        wire.to_kind = Some(request.to_kind);
        wire.to_contract = Some(request.to_contract);
        wire.from_amount = request.from_amount;
        wire.payout_address = payout;
        wire.deposit_address = Some(refund.clone());
        wire.refund_address = refund;
        let order = swap_rules::order(wire, None, Some(&quote), None, now()).unwrap();
        assert_eq!(order.direction, Some(direction));
        assert_eq!(order.from, Some(quote.from));
        assert_eq!(order.to, Some(quote.to));
        assert!(order.can_pay);
    }
}

#[test]
fn test_historical_bsv_trx_orders_remain_decodable_without_enabling_new_quotes() {
    let wallet = Wallet::mock_with_accounts(vec![
        Account::mock(Chain::BitcoinSv, "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5"),
        Account::mock(Chain::Tron, "TBKwjUtXVsX1r724C1V52nocBgtioDjx9u"),
    ]);
    for (from, to, direction) in [
        (pair().from, trx(), BsvHomePortalSwapDirection::BsvToTrx),
        (trx(), pair().from, BsvHomePortalSwapDirection::TrxToBsv),
    ] {
        let mut wire = wire_order();
        let (payout, refund) = swap_rules::wallet_addresses(&wallet, direction).unwrap();
        wire.from_chain = Some(from.chain.clone());
        wire.from_kind = Some(from.kind.clone());
        wire.from_contract = from.contract.clone();
        wire.to_chain = Some(to.chain.clone());
        wire.to_kind = Some(to.kind.clone());
        wire.to_contract = to.contract.clone();
        wire.payout_address = payout;
        wire.deposit_address = Some(refund.clone());
        wire.refund_address = refund;
        let order = swap_rules::order(wire, None, None, None, now()).unwrap();
        assert_eq!(order.direction, Some(direction));
        let serialized = serde_json::to_string(&order).unwrap();
        let restored: BsvHomePortalSwapOrder = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored, order);
        let pair = WirePair { from, to, ..pair() };
        assert_eq!(swap_rules::pair(&pair), Err(BsvHomePortalSwapError::UnsupportedPair));
        assert_eq!(
            swap_rules::quote(wire_quote(), &swap_rules::quote_request(&pair, BigUint::from(150_000_000u32)), &pair, now()),
            Err(BsvHomePortalSwapError::UnsupportedPair)
        );
    }
}

#[test]
fn test_trx_pair_requires_native_identity_and_rejects_same_asset() {
    let wire_pair = WirePair { from: trx(), ..pair() };
    let mut wrong_contract = WirePair { from: trx(), ..pair() };
    wrong_contract.from.contract = Some(wire_pair.to.contract.clone().unwrap());
    assert_eq!(swap_rules::pair(&wrong_contract), Err(BsvHomePortalSwapError::UnsupportedPair));
    let mut wrong_kind = WirePair { from: trx(), ..pair() };
    wrong_kind.from.kind = "TOKEN".into();
    assert_eq!(swap_rules::pair(&wrong_kind), Err(BsvHomePortalSwapError::UnsupportedPair));
    let mut wrong_scale = WirePair { from: trx(), ..pair() };
    wrong_scale.from.scale = 8;
    assert_eq!(swap_rules::pair(&wrong_scale), Err(BsvHomePortalSwapError::InvalidResponse));
    let mut wrong_symbol = WirePair { from: trx(), ..pair() };
    wrong_symbol.from.symbol = "USDT".into();
    assert_eq!(swap_rules::pair(&wrong_symbol), Err(BsvHomePortalSwapError::InvalidResponse));
    for asset in [pair().from, pair().to, trx()] {
        assert_eq!(
            swap_rules::pair(&WirePair {
                from: asset.clone(),
                to: asset,
                ..pair()
            }),
            Err(BsvHomePortalSwapError::UnsupportedPair)
        );
    }
    let mut unknown = WirePair { from: trx(), ..pair() };
    unknown.to.symbol = "MNEE".into();
    unknown.to.contract = Some("unconfirmed-mnee-contract".into());
    assert_eq!(swap_rules::pair(&unknown), Err(BsvHomePortalSwapError::UnsupportedPair));
    assert_eq!(BsvHomePortalSwapDirection::for_asset(Chain::Tron.as_asset_id()), None);
}

#[test]
fn test_trx_same_chain_pair_uses_wallet_tron_account_for_payout_and_refund() {
    let address = "TBKwjUtXVsX1r724C1V52nocBgtioDjx9u";
    let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::Tron, address)]);
    for direction in [BsvHomePortalSwapDirection::TrxToUsdt, BsvHomePortalSwapDirection::UsdtToTrx] {
        assert_eq!(swap_rules::wallet_addresses(&wallet, direction).unwrap(), (address.into(), address.into()));
    }
    for direction in [BsvHomePortalSwapDirection::BsvToTrx, BsvHomePortalSwapDirection::TrxToBsv] {
        assert_eq!(swap_rules::wallet_addresses(&wallet, direction), Err(BsvHomePortalSwapError::MissingAccount));
    }
}
