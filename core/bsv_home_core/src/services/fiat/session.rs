use primitives::{FiatProviderName, FiatQuote, FiatQuoteType};

use super::model::{BsvHomeFiatAmountCheck, BsvHomeFiatQuoteRow};
use super::rules;
use crate::config::fiat_config::get_fiat_config;
use crate::models::custom_types::BsvHomeBigUint;
use crate::services::error::BsvHomeServiceError;

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeFiatQuoteRequest {
    pub quote_type: FiatQuoteType,
    pub amount: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeFiatQuotesResult {
    pub request: BsvHomeFiatQuoteRequest,
    pub quotes: Vec<FiatQuote>,
    pub error: Option<BsvHomeServiceError>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeFiatQuotePhase {
    NoInput,
    InvalidInput,
    Invalid { check: BsvHomeFiatAmountCheck },
    Loading { amount: f64 },
    Ready,
    NoQuotes,
    Failed { error: BsvHomeServiceError },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeFiatButtonAction {
    Continue,
    RetryQuote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeFiatButtonState {
    Disabled,
    Loading,
    Enabled,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeFiatOperation {
    pub quote_type: FiatQuoteType,
    pub amount: String,
    pub quotes: Vec<FiatQuote>,
    pub selected_provider: Option<FiatProviderName>,
    pub phase: BsvHomeFiatQuotePhase,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeFiatViewState {
    pub quote_type: FiatQuoteType,
    pub amount: String,
    pub phase: BsvHomeFiatQuotePhase,
    pub quote_rows: Vec<BsvHomeFiatQuoteRow>,
    pub selected_quote_row: Option<BsvHomeFiatQuoteRow>,
    pub can_select_provider: bool,
    pub amount_check: BsvHomeFiatAmountCheck,
    pub button_action: BsvHomeFiatButtonAction,
    pub button_state: BsvHomeFiatButtonState,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeFiatSession {
    pub quote_type: FiatQuoteType,
    pub buy: BsvHomeFiatOperation,
    pub sell: BsvHomeFiatOperation,
    pub available: BsvHomeBigUint,
}

impl BsvHomeFiatSession {
    pub fn new(quote_type: FiatQuoteType, amount: Option<u32>) -> Self {
        let config = get_fiat_config();
        let operation = |operation_type: FiatQuoteType| {
            let default = rules::default_amount(&config, operation_type);
            let initial = amount.filter(|_| operation_type == quote_type).unwrap_or(default);
            BsvHomeFiatOperation::new(operation_type, initial.to_string())
        };
        Self {
            quote_type,
            buy: operation(FiatQuoteType::Buy),
            sell: operation(FiatQuoteType::Sell),
            available: BsvHomeBigUint::default(),
        }
    }

    fn operation(&self, quote_type: FiatQuoteType) -> &BsvHomeFiatOperation {
        match quote_type {
            FiatQuoteType::Buy => &self.buy,
            FiatQuoteType::Sell => &self.sell,
        }
    }

    fn with_operation(&self, operation: BsvHomeFiatOperation) -> Self {
        match operation.quote_type {
            FiatQuoteType::Buy => Self { buy: operation, ..self.clone() },
            FiatQuoteType::Sell => Self { sell: operation, ..self.clone() },
        }
    }
}

#[uniffi::export]
impl BsvHomeFiatSession {
    pub fn view_state(&self, asset_price: Option<f64>, is_url_loading: bool) -> BsvHomeFiatViewState {
        let operation = self.current();
        BsvHomeFiatViewState {
            quote_type: operation.quote_type,
            amount: operation.amount.clone(),
            phase: operation.phase.clone(),
            quote_rows: self.quote_rows(asset_price),
            selected_quote_row: self.selected_quote_row(asset_price),
            can_select_provider: self.can_select_provider(),
            amount_check: self.amount_check(),
            button_action: self.button_action(),
            button_state: self.button_state(is_url_loading),
        }
    }

    fn current(&self) -> BsvHomeFiatOperation {
        self.operation(self.quote_type).clone()
    }

    pub fn on_type_changed(&self, quote_type: FiatQuoteType) -> BsvHomeFiatSession {
        BsvHomeFiatSession { quote_type, ..self.clone() }
    }

    pub fn on_sell_enabled_changed(&self, is_sell_enabled: bool) -> BsvHomeFiatSession {
        if is_sell_enabled || self.quote_type != FiatQuoteType::Sell {
            return self.clone();
        }
        BsvHomeFiatSession {
            quote_type: FiatQuoteType::Buy,
            buy: self.buy.on_amount_changed(self.sell.amount.clone()),
            ..self.clone()
        }
    }

    pub fn on_amount_changed(&self, amount: String) -> BsvHomeFiatSession {
        self.with_operation(self.current().on_amount_changed(amount))
    }

    pub fn on_balance_changed(&self, available: BsvHomeBigUint) -> BsvHomeFiatSession {
        BsvHomeFiatSession { available, ..self.clone() }
    }

    pub fn quote_request(&self) -> Option<BsvHomeFiatQuoteRequest> {
        self.current().quote_request()
    }

    pub fn on_fetch_started(&self, request: BsvHomeFiatQuoteRequest) -> BsvHomeFiatSession {
        self.with_operation(self.operation(request.quote_type).on_fetch_started(&request))
    }

    pub fn on_quote_results(&self, results: BsvHomeFiatQuotesResult) -> BsvHomeFiatSession {
        self.with_operation(self.operation(results.request.quote_type).on_quote_results(results))
    }

    pub fn on_provider_selected(&self, provider: FiatProviderName) -> BsvHomeFiatSession {
        self.with_operation(self.current().on_provider_selected(provider))
    }

    fn quote_rows(&self, asset_price: Option<f64>) -> Vec<BsvHomeFiatQuoteRow> {
        self.current().quotes.iter().map(|quote| rules::quote_row(quote, asset_price)).collect()
    }

    fn selected_quote_row(&self, asset_price: Option<f64>) -> Option<BsvHomeFiatQuoteRow> {
        self.selected_quote().map(|quote| rules::quote_row(&quote, asset_price))
    }

    pub fn selected_quote(&self) -> Option<FiatQuote> {
        self.current().selected_quote()
    }

    fn can_select_provider(&self) -> bool {
        self.current().quotes.len() > 1
    }

    fn amount_check(&self) -> BsvHomeFiatAmountCheck {
        let operation = self.current();
        match operation.parsed_amount() {
            Some(amount) => rules::amount_check(&get_fiat_config(), operation.quote_type, amount, operation.selected_quote().as_ref(), &self.available),
            None => BsvHomeFiatAmountCheck::Valid,
        }
    }

    fn button_action(&self) -> BsvHomeFiatButtonAction {
        match self.current().phase {
            BsvHomeFiatQuotePhase::Failed { .. } => BsvHomeFiatButtonAction::RetryQuote,
            _ => BsvHomeFiatButtonAction::Continue,
        }
    }

    fn button_state(&self, is_url_loading: bool) -> BsvHomeFiatButtonState {
        if is_url_loading {
            return BsvHomeFiatButtonState::Loading;
        }
        match self.current().phase {
            BsvHomeFiatQuotePhase::Loading { .. } => BsvHomeFiatButtonState::Loading,
            BsvHomeFiatQuotePhase::Failed { .. } => BsvHomeFiatButtonState::Enabled,
            BsvHomeFiatQuotePhase::Ready if self.selected_quote().is_some() && self.amount_check() == BsvHomeFiatAmountCheck::Valid => BsvHomeFiatButtonState::Enabled,
            _ => BsvHomeFiatButtonState::Disabled,
        }
    }
}

impl BsvHomeFiatOperation {
    fn new(quote_type: FiatQuoteType, amount: String) -> Self {
        Self {
            quote_type,
            phase: Self::input_phase(quote_type, &amount),
            amount,
            quotes: vec![],
            selected_provider: None,
        }
    }

    fn input_phase(quote_type: FiatQuoteType, amount: &str) -> BsvHomeFiatQuotePhase {
        match rules::parse_amount(amount) {
            rules::FiatAmountInput::Empty => BsvHomeFiatQuotePhase::NoInput,
            rules::FiatAmountInput::Invalid => BsvHomeFiatQuotePhase::InvalidInput,
            rules::FiatAmountInput::Value(value) => match rules::amount_check(&get_fiat_config(), quote_type, value, None, &Default::default()) {
                BsvHomeFiatAmountCheck::Valid => BsvHomeFiatQuotePhase::Loading { amount: value },
                check => BsvHomeFiatQuotePhase::Invalid { check },
            },
        }
    }

    fn parsed_amount(&self) -> Option<f64> {
        match rules::parse_amount(&self.amount) {
            rules::FiatAmountInput::Value(value) => Some(value),
            rules::FiatAmountInput::Empty | rules::FiatAmountInput::Invalid => None,
        }
    }

    fn on_amount_changed(&self, amount: String) -> Self {
        if amount == self.amount {
            return self.clone();
        }
        Self {
            selected_provider: self.selected_provider,
            ..Self::new(self.quote_type, amount)
        }
    }

    fn quote_request(&self) -> Option<BsvHomeFiatQuoteRequest> {
        match self.phase {
            BsvHomeFiatQuotePhase::NoInput | BsvHomeFiatQuotePhase::InvalidInput | BsvHomeFiatQuotePhase::Invalid { .. } => None,
            BsvHomeFiatQuotePhase::Loading { .. } | BsvHomeFiatQuotePhase::Ready | BsvHomeFiatQuotePhase::NoQuotes | BsvHomeFiatQuotePhase::Failed { .. } => {
                Some(BsvHomeFiatQuoteRequest {
                    quote_type: self.quote_type,
                    amount: self.parsed_amount()?,
                })
            }
        }
    }

    fn on_fetch_started(&self, request: &BsvHomeFiatQuoteRequest) -> Self {
        if self.parsed_amount() != Some(request.amount) {
            return self.clone();
        }
        Self {
            quotes: vec![],
            phase: BsvHomeFiatQuotePhase::Loading { amount: request.amount },
            ..self.clone()
        }
    }

    fn on_quote_results(&self, results: BsvHomeFiatQuotesResult) -> Self {
        if self.phase != (BsvHomeFiatQuotePhase::Loading { amount: results.request.amount }) {
            return self.clone();
        }
        let (quotes, phase) = match results.error {
            Some(error) => (vec![], BsvHomeFiatQuotePhase::Failed { error }),
            None if results.quotes.is_empty() => (vec![], BsvHomeFiatQuotePhase::NoQuotes),
            None => (results.quotes, BsvHomeFiatQuotePhase::Ready),
        };
        Self { quotes, phase, ..self.clone() }
    }

    fn on_provider_selected(&self, provider: FiatProviderName) -> Self {
        if !self.quotes.iter().any(|quote| quote.provider.id == provider) {
            return self.clone();
        }
        Self {
            selected_provider: Some(provider),
            ..self.clone()
        }
    }

    fn selected_quote(&self) -> Option<FiatQuote> {
        rules::selected_quote(&self.quotes, self.selected_provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use primitives::{Asset, Chain, FiatProvider, FiatProviderName};

    fn quote(provider: FiatProviderName, crypto_amount: f64) -> FiatQuote {
        FiatQuote::new(
            format!("{}-{crypto_amount}", provider.id()),
            Asset::from_chain(Chain::Ethereum),
            FiatProvider {
                id: provider,
                name: provider.name().to_string(),
                image_url: None,
                priority: None,
                threshold_bps: None,
                enabled: true,
                buy_enabled: true,
                sell_enabled: true,
                payment_methods: vec![],
            },
            FiatQuoteType::Buy,
            100.0,
            "USD".to_string(),
            crypto_amount,
            BigUint::ZERO,
            10,
            vec![],
        )
    }

    fn results(quote_type: FiatQuoteType, amount: f64, quotes: Vec<FiatQuote>) -> BsvHomeFiatQuotesResult {
        BsvHomeFiatQuotesResult {
            request: BsvHomeFiatQuoteRequest { quote_type, amount },
            quotes,
            error: None,
        }
    }

    fn ready(quotes: Vec<FiatQuote>) -> BsvHomeFiatSession {
        let session = BsvHomeFiatSession::new(FiatQuoteType::Buy, None);
        session.on_quote_results(results(FiatQuoteType::Buy, 50.0, quotes))
    }

    #[test]
    fn test_a_new_session_starts_each_type_on_its_default_and_the_initial_amount_on_its_type() {
        let session = BsvHomeFiatSession::new(FiatQuoteType::Sell, Some(25));

        assert_eq!(session.quote_type, FiatQuoteType::Sell);
        assert_eq!(session.buy.amount, "50");
        assert_eq!(session.sell.amount, "25");
        assert_eq!(
            session.quote_request(),
            Some(BsvHomeFiatQuoteRequest {
                quote_type: FiatQuoteType::Sell,
                amount: 25.0
            })
        );
        assert_eq!(session.on_type_changed(FiatQuoteType::Buy).quote_request().map(|request| request.amount), Some(50.0));
    }

    #[test]
    fn test_the_amount_decides_the_phase_and_whether_a_quote_is_requested() {
        let session = BsvHomeFiatSession::new(FiatQuoteType::Buy, None);

        assert_eq!(session.on_amount_changed("".to_string()).current().phase, BsvHomeFiatQuotePhase::NoInput);
        assert_eq!(session.on_amount_changed("0".to_string()).current().phase, BsvHomeFiatQuotePhase::NoInput);
        assert_eq!(session.on_amount_changed("abc".to_string()).current().phase, BsvHomeFiatQuotePhase::InvalidInput);
        assert_eq!(
            session.on_amount_changed("4".to_string()).current().phase,
            BsvHomeFiatQuotePhase::Invalid {
                check: BsvHomeFiatAmountCheck::BelowMinimum { minimum: 5 }
            }
        );
        assert_eq!(session.on_amount_changed("4".to_string()).quote_request(), None);
        assert_eq!(
            session.on_amount_changed("12,5".to_string()).current().phase,
            BsvHomeFiatQuotePhase::Loading { amount: 12.5 }
        );
        assert_eq!(session.on_amount_changed("4".to_string()).button_state(false), BsvHomeFiatButtonState::Disabled);
    }

    #[test]
    fn test_changing_the_amount_clears_quotes_and_keeps_the_same_amount_untouched() {
        let session = ready(vec![quote(FiatProviderName::Transak, 1.0)]);

        assert_eq!(session.on_amount_changed("50".to_string()), session);
        let changed = session.on_amount_changed("75".to_string());
        assert!(changed.current().quotes.is_empty());
        assert_eq!(changed.current().phase, BsvHomeFiatQuotePhase::Loading { amount: 75.0 });
        assert_eq!(changed.selected_quote(), None);
        assert_eq!(changed.button_state(false), BsvHomeFiatButtonState::Loading);
    }

    #[test]
    fn test_quote_results_only_apply_to_the_amount_still_loading() {
        let session = BsvHomeFiatSession::new(FiatQuoteType::Buy, None);
        let stale = session.on_quote_results(results(FiatQuoteType::Buy, 40.0, vec![quote(FiatProviderName::Transak, 1.0)]));
        assert_eq!(stale, session);

        let ready = session.on_quote_results(results(FiatQuoteType::Buy, 50.0, vec![quote(FiatProviderName::Transak, 1.0)]));
        assert_eq!(ready.current().phase, BsvHomeFiatQuotePhase::Ready);
        assert_eq!(ready.selected_quote().map(|quote| quote.provider.id), Some(FiatProviderName::Transak));
        assert_eq!(ready.button_state(false), BsvHomeFiatButtonState::Enabled);
        assert_eq!(ready.button_action(), BsvHomeFiatButtonAction::Continue);

        let refreshed = ready.on_fetch_started(BsvHomeFiatQuoteRequest {
            quote_type: FiatQuoteType::Buy,
            amount: 50.0,
        });
        assert_eq!(refreshed.current().phase, BsvHomeFiatQuotePhase::Loading { amount: 50.0 });
        assert!(refreshed.current().quotes.is_empty());
        assert_eq!(
            ready.on_fetch_started(BsvHomeFiatQuoteRequest {
                quote_type: FiatQuoteType::Buy,
                amount: 60.0
            }),
            ready
        );
        assert_eq!(ready.on_quote_results(results(FiatQuoteType::Buy, 50.0, vec![])), ready);
    }

    #[test]
    fn test_empty_quotes_and_failures_are_distinct_and_only_a_failure_offers_a_retry() {
        let session = BsvHomeFiatSession::new(FiatQuoteType::Buy, None);

        let empty = session.on_quote_results(results(FiatQuoteType::Buy, 50.0, vec![]));
        assert_eq!(empty.current().phase, BsvHomeFiatQuotePhase::NoQuotes);
        assert_eq!(empty.button_state(false), BsvHomeFiatButtonState::Disabled);
        assert_eq!(empty.button_action(), BsvHomeFiatButtonAction::Continue);
        assert!(empty.quote_request().is_some());

        let failed = session.on_quote_results(BsvHomeFiatQuotesResult {
            error: Some(BsvHomeServiceError::Api { msg: "offline".to_string() }),
            ..results(FiatQuoteType::Buy, 50.0, vec![quote(FiatProviderName::Transak, 1.0)])
        });
        assert_eq!(
            failed.current().phase,
            BsvHomeFiatQuotePhase::Failed {
                error: BsvHomeServiceError::Api { msg: "offline".to_string() }
            }
        );
        assert!(failed.current().quotes.is_empty());
        assert_eq!(failed.button_state(false), BsvHomeFiatButtonState::Enabled);
        assert_eq!(failed.button_action(), BsvHomeFiatButtonAction::RetryQuote);
        assert_eq!(failed.button_state(true), BsvHomeFiatButtonState::Loading);
    }

    #[test]
    fn test_the_chosen_provider_survives_a_refresh_and_an_unknown_one_is_ignored() {
        let session = ready(vec![quote(FiatProviderName::Transak, 1.0), quote(FiatProviderName::MoonPay, 2.0)]);
        assert!(session.can_select_provider());

        let selected = session.on_provider_selected(FiatProviderName::MoonPay);
        assert_eq!(selected.selected_quote().map(|quote| quote.provider.id), Some(FiatProviderName::MoonPay));
        assert_eq!(session.on_provider_selected(FiatProviderName::Banxa), session);

        let refreshed = selected
            .on_fetch_started(BsvHomeFiatQuoteRequest {
                quote_type: FiatQuoteType::Buy,
                amount: 50.0,
            })
            .on_quote_results(results(
                FiatQuoteType::Buy,
                50.0,
                vec![quote(FiatProviderName::MoonPay, 3.0), quote(FiatProviderName::Transak, 1.0)],
            ));
        assert_eq!(refreshed.selected_quote().map(|quote| quote.crypto_amount), Some(3.0));

        let gone = selected
            .on_fetch_started(BsvHomeFiatQuoteRequest {
                quote_type: FiatQuoteType::Buy,
                amount: 50.0,
            })
            .on_quote_results(results(FiatQuoteType::Buy, 50.0, vec![quote(FiatProviderName::Transak, 1.0)]));
        assert_eq!(gone.selected_quote().map(|quote| quote.provider.id), Some(FiatProviderName::Transak));
        assert!(!gone.can_select_provider());
    }

    #[test]
    fn test_a_sell_quote_above_the_balance_disables_the_button_until_the_balance_covers_it() {
        let mut sell = quote(FiatProviderName::Transak, 1.0);
        sell.quote_type = FiatQuoteType::Sell;
        let session = BsvHomeFiatSession::new(FiatQuoteType::Sell, None).on_quote_results(results(FiatQuoteType::Sell, 100.0, vec![sell]));

        assert_eq!(session.current().phase, BsvHomeFiatQuotePhase::Ready);
        assert!(matches!(session.amount_check(), BsvHomeFiatAmountCheck::InsufficientBalance { .. }));
        assert_eq!(session.button_state(false), BsvHomeFiatButtonState::Disabled);

        let funded = session.on_balance_changed(BigUint::from(1_000_000_000_000_000_000u64));
        assert_eq!(funded.amount_check(), BsvHomeFiatAmountCheck::Valid);
        assert_eq!(funded.button_state(false), BsvHomeFiatButtonState::Enabled);
    }

    #[test]
    fn test_losing_sell_support_moves_to_buy_with_the_sell_amount() {
        let session = BsvHomeFiatSession::new(FiatQuoteType::Sell, Some(25));

        assert_eq!(session.on_sell_enabled_changed(true), session);
        let buy = session.on_sell_enabled_changed(false);
        assert_eq!(buy.quote_type, FiatQuoteType::Buy);
        assert_eq!(buy.buy.amount, "25");
        assert_eq!(
            buy.quote_request(),
            Some(BsvHomeFiatQuoteRequest {
                quote_type: FiatQuoteType::Buy,
                amount: 25.0
            })
        );
        assert_eq!(
            BsvHomeFiatSession::new(FiatQuoteType::Buy, None).on_sell_enabled_changed(false).quote_type,
            FiatQuoteType::Buy
        );
    }

    #[test]
    fn test_view_state_carries_the_rows_selection_and_button_at_once() {
        let session = BsvHomeFiatSession::new(FiatQuoteType::Buy, Some(100))
            .on_fetch_started(BsvHomeFiatQuoteRequest {
                quote_type: FiatQuoteType::Buy,
                amount: 100.0,
            })
            .on_quote_results(results(
                FiatQuoteType::Buy,
                100.0,
                vec![quote(FiatProviderName::Banxa, 2.0), quote(FiatProviderName::MoonPay, 1.0)],
            ));

        let state = session.view_state(Some(50.0), false);
        assert_eq!(state.quote_type, FiatQuoteType::Buy);
        assert_eq!(state.amount, "100");
        assert_eq!(state.phase, BsvHomeFiatQuotePhase::Ready);
        assert_eq!(state.quote_rows.len(), 2);
        assert_eq!(state.selected_quote_row.map(|row| row.provider), session.selected_quote().map(|quote| quote.provider.id));
        assert!(state.can_select_provider);
        assert_eq!(state.button_action, BsvHomeFiatButtonAction::Continue);
        assert_eq!(state.button_state, BsvHomeFiatButtonState::Enabled);
        assert_eq!(session.view_state(None, true).button_state, BsvHomeFiatButtonState::Loading);
    }
}
