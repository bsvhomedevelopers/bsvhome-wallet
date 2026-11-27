use primitives::{AssetFiatValue, Banner, BannerEvent, Chain, Currency, FiatRate, PerpetualBalance, WalletType};

use crate::services::assets::model::{BsvHomeHeaderActions, BsvHomeHeaderButton, BsvHomeHeaderButtonKind};
use crate::services::price::rules::rate_or_base;

pub fn convert_total_value(value: f64, currency: Currency, rates: Vec<FiatRate>) -> Option<f64> {
    let stored = rates.into_iter().find(|rate| rate.symbol == currency);
    let rate = rate_or_base(currency, stored).filter(|rate| rate.rate.is_finite() && rate.rate > 0.0)?;
    let converted = rate.multiplier(value);
    converted.is_finite().then_some(converted)
}

pub fn shows_initial_loading(initial_load_completed: bool, assets_timestamp: u64) -> bool {
    !initial_load_completed && assets_timestamp == 0
}

pub fn wallet_balances(balances: Vec<AssetFiatValue>, perpetual: Option<PerpetualBalance>) -> Vec<AssetFiatValue> {
    balances
        .into_iter()
        .chain(perpetual.map(|balance| AssetFiatValue {
            amount: balance.available + balance.reserved,
            price: 1.0,
            price_change_percentage_24h: 0.0,
        }))
        .collect()
}

pub fn header_buttons_enabled(banners: &[Banner]) -> bool {
    !banners.iter().any(|banner| banner.event == BannerEvent::AccountBlockedMultiSignature)
}

pub fn header_actions(wallet_type: WalletType, chains: &[Chain], is_enabled: bool) -> BsvHomeHeaderActions {
    match wallet_type {
        WalletType::View => BsvHomeHeaderActions::WatchOnly,
        WalletType::Multicoin | WalletType::Single | WalletType::PrivateKey => BsvHomeHeaderActions::Buttons {
            buttons: header_buttons(wallet_type, chains, is_enabled),
        },
    }
}

fn header_buttons(wallet_type: WalletType, chains: &[Chain], is_enabled: bool) -> Vec<BsvHomeHeaderButton> {
    [
        Some(BsvHomeHeaderButtonKind::Send),
        Some(BsvHomeHeaderButtonKind::Receive),
        Some(BsvHomeHeaderButtonKind::Buy),
        swaps(wallet_type, chains).then_some(BsvHomeHeaderButtonKind::Swap),
    ]
    .into_iter()
    .flatten()
    .map(|kind| BsvHomeHeaderButton { kind, is_enabled })
    .collect()
}

fn swaps(wallet_type: WalletType, chains: &[Chain]) -> bool {
    match wallet_type {
        WalletType::Multicoin => true,
        WalletType::Single | WalletType::PrivateKey => chains.first().is_some_and(|chain| chain.is_swap_supported()),
        WalletType::View => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_total_value() {
        let rates = |rate| vec![FiatRate { symbol: Currency::CNY, rate }];
        assert_eq!(convert_total_value(125.0, Currency::CNY, rates(7.2)), Some(900.0));
        assert_eq!(convert_total_value(125.0, Currency::USD, vec![]), Some(125.0));
        assert_eq!(convert_total_value(125.0, Currency::CNY, vec![]), None);
        assert_eq!(convert_total_value(125.0, Currency::EUR, rates(7.2)), None);
        assert_eq!(convert_total_value(0.0, Currency::CNY, rates(7.2)), Some(0.0));
        assert_eq!(convert_total_value(0.0, Currency::CNY, vec![]), None);
        assert_eq!(convert_total_value(-125.0, Currency::CNY, rates(7.2)), Some(-900.0));
        for rate in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(convert_total_value(125.0, Currency::CNY, rates(rate)), None);
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
            assert_eq!(convert_total_value(value, Currency::CNY, rates(7.2)), None);
        }
        assert_eq!(convert_total_value(f64::NAN, Currency::USD, vec![]), None);
    }

    fn buttons(wallet_type: WalletType, chain: Chain, is_enabled: bool) -> Vec<BsvHomeHeaderButton> {
        match header_actions(wallet_type, &[chain], is_enabled) {
            BsvHomeHeaderActions::Buttons { buttons } => buttons,
            BsvHomeHeaderActions::WatchOnly => panic!("the header offers no buttons"),
        }
    }

    fn kinds(wallet_type: WalletType, chain: Chain) -> Vec<BsvHomeHeaderButtonKind> {
        buttons(wallet_type, chain, true).into_iter().map(|button| button.kind).collect()
    }

    #[test]
    fn test_wallet_balances_count_perpetual_collateral_at_par_without_a_day_change() {
        let eth = AssetFiatValue {
            amount: 2.0,
            price: 10.0,
            price_change_percentage_24h: 5.0,
        };
        let collateral = PerpetualBalance {
            available: 30.0,
            reserved: 20.0,
            withdrawable: 25.0,
        };
        assert_eq!(wallet_balances(vec![eth], None), vec![eth]);
        assert_eq!(
            wallet_balances(vec![eth], Some(collateral)),
            vec![
                eth,
                AssetFiatValue {
                    amount: 50.0,
                    price: 1.0,
                    price_change_percentage_24h: 0.0
                }
            ],
            "collateral is what is available plus what positions hold, not what can be withdrawn"
        );
    }

    #[test]
    fn test_shows_initial_loading_only_before_the_first_discovery() {
        assert!(shows_initial_loading(false, 0));
        assert!(!shows_initial_loading(true, 0));
        assert!(!shows_initial_loading(false, 1));
        assert!(!shows_initial_loading(true, 1));
    }

    #[test]
    fn test_a_blocked_multi_signature_account_disables_the_header_buttons() {
        use primitives::BannerState;
        let banner = |event| Banner {
            wallet_id: None,
            asset: None,
            event,
            state: BannerState::Active,
        };
        assert!(header_buttons_enabled(&[]));
        assert!(header_buttons_enabled(&[banner(BannerEvent::Onboarding)]));
        assert!(!header_buttons_enabled(&[
            banner(BannerEvent::Onboarding),
            banner(BannerEvent::AccountBlockedMultiSignature)
        ]));
    }

    #[test]
    fn test_the_header_offers_swap_to_multicoin_and_swappable_single_chain_wallets_only() {
        use BsvHomeHeaderButtonKind::*;
        assert_eq!(kinds(WalletType::Multicoin, Chain::Bitcoin), vec![Send, Receive, Buy, Swap]);
        assert_eq!(kinds(WalletType::Single, Chain::Ethereum), vec![Send, Receive, Buy, Swap]);
        assert_eq!(kinds(WalletType::PrivateKey, Chain::Solana), vec![Send, Receive, Buy, Swap]);
        assert_eq!(kinds(WalletType::Single, Chain::Mayachain), vec![Send, Receive, Buy]);
        assert_eq!(header_actions(WalletType::View, &[Chain::Ethereum], true), BsvHomeHeaderActions::WatchOnly);
        assert!(buttons(WalletType::Multicoin, Chain::Ethereum, false).iter().all(|button| !button.is_enabled));
    }
}
