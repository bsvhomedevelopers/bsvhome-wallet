use std::num::NonZeroU64;
use std::str::FromStr;
use std::sync::Arc;

use bigdecimal::{BigDecimal, RoundingMode, ToPrimitive};
use number_formatter::BigNumberFormatter;

use crate::config::swap_config::get_swap_config;
use crate::models::custom_types::BsvHomeBigUint;

pub use primitives::swap::{ApprovalData, SwapData, SwapPriceImpact, SwapPriceImpactType, SwapProviderData, SwapQuote, SwapQuoteData};
pub use swapper::SwapperProvider;

pub type BsvHomeApprovalData = ApprovalData;
pub type BsvHomeSwapData = SwapData;
pub type BsvHomeSwapQuoteData = SwapQuoteData;

const PRICE_IMPACT_SIGNIFICANT_DIGITS: u64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeSlippageCheck {
    Valid,
    High,
    BelowMinimum,
    AboveMaximum,
}

#[derive(Debug, Clone, PartialEq, uniffi::Object)]
pub struct BsvHomeSwapValue {
    value: BsvHomeBigUint,
    decimals: u32,
    price: Option<f64>,
}

#[uniffi::export]
impl BsvHomeSwapValue {
    #[uniffi::constructor]
    pub fn new(value: BsvHomeBigUint, decimals: u32, price: Option<f64>) -> Self {
        Self { value, decimals, price }
    }

    pub fn price_impact(&self, receive: Arc<BsvHomeSwapValue>) -> Option<SwapPriceImpact> {
        calculate_swap_price_impact(self.fiat_value()?, receive.fiat_value()?)
    }
}

impl BsvHomeSwapValue {
    fn fiat_value(&self) -> Option<BigDecimal> {
        let amount = BigNumberFormatter::big_decimal_value(&self.value.to_string(), self.decimals).ok()?;
        let price = BigDecimal::from_str(&self.price?.to_string()).ok()?;
        Some(amount * price)
    }
}

fn calculate_swap_price_impact(pay_fiat_value: BigDecimal, receive_fiat_value: BigDecimal) -> Option<SwapPriceImpact> {
    if pay_fiat_value <= 0 || receive_fiat_value <= 0 {
        return None;
    }

    let percentage = ((receive_fiat_value - &pay_fiat_value) * BigDecimal::from(100) / pay_fiat_value)
        .with_precision_round(NonZeroU64::new(PRICE_IMPACT_SIGNIFICANT_DIGITS)?, RoundingMode::HalfEven)
        .to_f64()?;
    if !percentage.is_finite() {
        return None;
    }
    let rounded_percentage = round_to_places(percentage, 2);
    let impact_type = match rounded_percentage {
        value if value > 0.0 => SwapPriceImpactType::Positive,
        value if value >= -1.0 => SwapPriceImpactType::Low,
        value if value >= -5.0 => SwapPriceImpactType::Medium,
        _ => SwapPriceImpactType::High,
    };

    Some(SwapPriceImpact {
        percentage,
        impact_type,
        is_high: rounded_percentage.abs() >= get_swap_config().high_price_impact_percent as f64,
        shows_in_summary: matches!(impact_type, SwapPriceImpactType::Medium | SwapPriceImpactType::High),
    })
}

fn round_to_places(value: f64, places: i32) -> f64 {
    let factor = 10_f64.powi(places);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::{BsvHomeSwapValue, SwapPriceImpact, SwapPriceImpactType, calculate_swap_price_impact, round_to_places};
    use std::sync::Arc;

    #[test]
    fn test_swap_price_impact_needs_a_price_on_both_sides() {
        let priced = |value: u32, price: Option<f64>| Arc::new(BsvHomeSwapValue::new(value.into(), 2, price));

        assert_eq!(priced(100, None).price_impact(priced(100, Some(1.0))), None);
        assert_eq!(priced(100, Some(1.0)).price_impact(priced(100, None)), None);

        let impact = priced(200, Some(1.0)).price_impact(priced(100, Some(1.0))).expect("impact");
        assert_eq!(impact.percentage, -50.0);
    }

    #[test]
    fn test_calculate_swap_price_impact() {
        let calculate = |pay: &str, receive: &str| calculate_swap_price_impact(pay.parse().unwrap(), receive.parse().unwrap());

        assert_eq!(calculate("0", "100"), None);
        assert_eq!(calculate("100", "0"), None);

        assert_eq!(
            calculate("100", "100.5").map(|impact| SwapPriceImpact {
                percentage: round_to_places(impact.percentage, 2),
                impact_type: impact.impact_type,
                is_high: impact.is_high,
                shows_in_summary: impact.shows_in_summary,
            }),
            Some(SwapPriceImpact {
                percentage: 0.5,
                impact_type: SwapPriceImpactType::Positive,
                is_high: false,
                shows_in_summary: false,
            })
        );

        assert_eq!(
            calculate("100", "99").map(|impact| SwapPriceImpact {
                percentage: round_to_places(impact.percentage, 2),
                impact_type: impact.impact_type,
                is_high: impact.is_high,
                shows_in_summary: impact.shows_in_summary,
            }),
            Some(SwapPriceImpact {
                percentage: -1.0,
                impact_type: SwapPriceImpactType::Low,
                is_high: false,
                shows_in_summary: false,
            })
        );

        assert_eq!(
            calculate("100", "95").map(|impact| SwapPriceImpact {
                percentage: round_to_places(impact.percentage, 2),
                impact_type: impact.impact_type,
                is_high: impact.is_high,
                shows_in_summary: impact.shows_in_summary,
            }),
            Some(SwapPriceImpact {
                percentage: -5.0,
                impact_type: SwapPriceImpactType::Medium,
                is_high: false,
                shows_in_summary: true,
            })
        );

        assert_eq!(
            calculate("100", "89").map(|impact| SwapPriceImpact {
                percentage: round_to_places(impact.percentage, 2),
                impact_type: impact.impact_type,
                is_high: impact.is_high,
                shows_in_summary: impact.shows_in_summary,
            }),
            Some(SwapPriceImpact {
                percentage: -11.0,
                impact_type: SwapPriceImpactType::High,
                is_high: true,
                shows_in_summary: true,
            })
        );
    }

    #[test]
    fn test_price_impact_preserves_decimal_precision() {
        let priced = |value: u64, decimals: u32, price: f64| Arc::new(BsvHomeSwapValue::new(value.into(), decimals, Some(price)));

        assert_eq!(priced(100_000_000, 6, 1.0).price_impact(priced(70_000_000, 6, 1.0)).unwrap().percentage, -30.0);
        assert_eq!(priced(100_000_000, 6, 0.1).price_impact(priced(100_123_456, 6, 0.1)).unwrap().percentage, 0.123456);
        assert_eq!(priced(3, 0, 1.0).price_impact(priced(4, 0, 1.0)).unwrap().percentage, 33.33333333333333);
        assert_eq!(priced(1, 0, f64::NAN).price_impact(priced(1, 0, 1.0)), None);
    }
}
