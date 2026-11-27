use primitives::AssetId;
use swapper::{Quote as SwapperQuote, SwapperError, SwapperProvider};

use super::model::{BsvHomeSwapButtonAction, BsvHomeSwapButtonInput};
use super::rules;
use crate::models::custom_types::{BsvHomeBigInt, BsvHomeBigUint};

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapRequest {
    pub pay_asset_id: AssetId,
    pub receive_asset_id: AssetId,
    pub value: BsvHomeBigUint,
    pub slippage_bps: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapQuotesResult {
    pub request: BsvHomeSwapRequest,
    pub quotes: Vec<SwapperQuote>,
    pub error: Option<SwapperError>,
}

#[derive(Debug, Clone, PartialEq, Default, uniffi::Enum)]
pub enum BsvHomeSwapQuotePhase {
    #[default]
    NoInput,
    Loading {
        request: BsvHomeSwapRequest,
    },
    Ready,
    Failed {
        request: BsvHomeSwapRequest,
        error: SwapperError,
    },
}

#[derive(Debug, Clone, PartialEq, Default, uniffi::Enum)]
pub enum BsvHomeSwapTransferPhase {
    #[default]
    Idle,
    Loading {
        request: BsvHomeSwapRequest,
        provider: SwapperProvider,
    },
    Failed {
        request: BsvHomeSwapRequest,
        provider: SwapperProvider,
        error: SwapperError,
    },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeSwapSessionAction {
    None,
    QuoteLoading,
    Ready,
    TransferLoading,
    QuoteError { error: SwapperError },
    TransferError { error: SwapperError },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeSwapButtonState {
    Disabled,
    Loading,
    Enabled,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSwapViewState {
    pub action: BsvHomeSwapSessionAction,
    pub button_action: BsvHomeSwapButtonAction,
    pub button_state: BsvHomeSwapButtonState,
    pub quote: Option<SwapperQuote>,
    pub quote_error: Option<SwapperError>,
    pub error: Option<SwapperError>,
    pub is_quote_loading: bool,
    pub is_transfer_loading: bool,
    pub is_input_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Default, uniffi::Record)]
pub struct BsvHomeSwapSession {
    #[uniffi(default = None)]
    pub quotes: Option<BsvHomeSwapQuotesResult>,
    #[uniffi(default = None)]
    pub selected_provider: Option<SwapperProvider>,
    #[uniffi(default = None)]
    pub selected_quote: Option<SwapperQuote>,
    pub quote_phase: BsvHomeSwapQuotePhase,
    pub transfer_phase: BsvHomeSwapTransferPhase,
    #[uniffi(default = false)]
    pub refresh_paused_until_restart: bool,
}

#[uniffi::export]
impl BsvHomeSwapSession {
    pub fn on_request_changed(&self, request: Option<BsvHomeSwapRequest>) -> BsvHomeSwapSession {
        let Some(request) = request else {
            return BsvHomeSwapSession::default();
        };
        let same_pair = self
            .current_request()
            .is_some_and(|current| current.pay_asset_id == request.pay_asset_id && current.receive_asset_id == request.receive_asset_id);
        BsvHomeSwapSession {
            quotes: None,
            selected_provider: self.selected_provider.filter(|_| same_pair),
            ..self.on_refresh_requested(request)
        }
    }

    pub fn on_refresh_requested(&self, request: BsvHomeSwapRequest) -> BsvHomeSwapSession {
        BsvHomeSwapSession {
            quote_phase: BsvHomeSwapQuotePhase::Loading { request },
            ..self.on_quote_invalidated()
        }
    }

    pub fn on_fetch_started(&self, request: BsvHomeSwapRequest) -> BsvHomeSwapSession {
        if !self.accepts_quote_phase() {
            return self.clone();
        }
        BsvHomeSwapSession {
            quote_phase: BsvHomeSwapQuotePhase::Loading { request },
            ..self.clone()
        }
    }

    pub fn on_quote_results(&self, results: BsvHomeSwapQuotesResult) -> BsvHomeSwapSession {
        let error = results.error.clone().or_else(|| results.quotes.is_empty().then_some(SwapperError::NoQuoteAvailable));
        let quotes = if self.accepts_quotes() {
            error.is_none().then_some(results.clone())
        } else {
            self.quotes.clone()
        };
        let quote_phase = if self.accepts_quote_phase() {
            match error {
                Some(error) => BsvHomeSwapQuotePhase::Failed { request: results.request, error },
                None => BsvHomeSwapQuotePhase::Ready,
            }
        } else {
            self.quote_phase.clone()
        };
        BsvHomeSwapSession {
            selected_quote: quotes.as_ref().and_then(|quotes| rules::selected_quote(&quotes.quotes, self.selected_provider)),
            quotes,
            quote_phase,
            ..self.clone()
        }
    }

    pub fn on_provider_selected(&self, provider: SwapperProvider) -> BsvHomeSwapSession {
        BsvHomeSwapSession {
            selected_provider: Some(provider),
            selected_quote: self.quotes.as_ref().and_then(|quotes| rules::selected_quote(&quotes.quotes, Some(provider))),
            ..self.on_quote_invalidated()
        }
    }

    pub fn on_quote_invalidated(&self) -> BsvHomeSwapSession {
        BsvHomeSwapSession {
            transfer_phase: BsvHomeSwapTransferPhase::Idle,
            refresh_paused_until_restart: false,
            ..self.clone()
        }
    }

    pub fn on_refresh_resumed(&self) -> BsvHomeSwapSession {
        BsvHomeSwapSession {
            refresh_paused_until_restart: false,
            ..self.clone()
        }
    }

    pub fn start_transfer(&self) -> Option<BsvHomeSwapSession> {
        if matches!(self.transfer_phase, BsvHomeSwapTransferPhase::Loading { .. }) {
            return None;
        }
        let request = self.quotes.as_ref()?.request.clone();
        let quote = self.quote()?;
        Some(BsvHomeSwapSession {
            transfer_phase: BsvHomeSwapTransferPhase::Loading {
                request,
                provider: quote.data.provider.id,
            },
            ..self.clone()
        })
    }

    pub fn on_transfer_failed(&self, transfer: BsvHomeSwapTransferPhase, error: SwapperError) -> BsvHomeSwapSession {
        let BsvHomeSwapTransferPhase::Loading { request, provider } = &transfer else {
            return self.clone();
        };
        if self.transfer_phase != transfer {
            return self.clone();
        }
        BsvHomeSwapSession {
            transfer_phase: BsvHomeSwapTransferPhase::Failed {
                request: request.clone(),
                provider: *provider,
                error,
            },
            ..self.clone()
        }
    }

    pub fn on_transfer_handed_off(&self, transfer: BsvHomeSwapTransferPhase) -> BsvHomeSwapSession {
        if !matches!(transfer, BsvHomeSwapTransferPhase::Loading { .. }) || self.transfer_phase != transfer {
            return self.clone();
        }
        BsvHomeSwapSession {
            transfer_phase: BsvHomeSwapTransferPhase::Idle,
            refresh_paused_until_restart: true,
            ..self.clone()
        }
    }

    pub fn view_state(&self, value: BsvHomeBigInt, available_balance: BsvHomeBigInt) -> BsvHomeSwapViewState {
        let button_action = self.button_action(value, available_balance);
        BsvHomeSwapViewState {
            action: self.action(),
            button_state: self.button_state(button_action.clone()),
            button_action,
            quote: self.quote(),
            quote_error: self.quote_error(),
            error: self.error(),
            is_quote_loading: self.is_quote_loading(),
            is_transfer_loading: self.is_transfer_loading(),
            is_input_empty: self.is_input_empty(),
        }
    }

    pub fn quote(&self) -> Option<SwapperQuote> {
        self.quotes.as_ref()?;
        self.selected_quote.clone()
    }

    fn quote_error(&self) -> Option<SwapperError> {
        match &self.quote_phase {
            BsvHomeSwapQuotePhase::Failed { error, .. } => Some(error.clone()),
            BsvHomeSwapQuotePhase::NoInput | BsvHomeSwapQuotePhase::Loading { .. } | BsvHomeSwapQuotePhase::Ready => None,
        }
    }

    fn error(&self) -> Option<SwapperError> {
        self.transfer_error().or_else(|| self.quote_error())
    }

    fn is_quote_loading(&self) -> bool {
        matches!(self.quote_phase, BsvHomeSwapQuotePhase::Loading { .. })
    }

    pub fn is_transfer_loading(&self) -> bool {
        matches!(self.transfer_phase, BsvHomeSwapTransferPhase::Loading { .. })
    }

    fn is_input_empty(&self) -> bool {
        matches!(self.quote_phase, BsvHomeSwapQuotePhase::NoInput)
    }

    pub fn refreshes_quotes(&self, is_screen_active: bool) -> bool {
        is_screen_active && !self.refresh_paused_until_restart && !self.is_transfer_loading()
    }

    fn action(&self) -> BsvHomeSwapSessionAction {
        match (&self.transfer_phase, &self.quote_phase) {
            (BsvHomeSwapTransferPhase::Loading { .. }, _) => BsvHomeSwapSessionAction::TransferLoading,
            (BsvHomeSwapTransferPhase::Failed { error, .. }, _) => BsvHomeSwapSessionAction::TransferError { error: error.clone() },
            (BsvHomeSwapTransferPhase::Idle, BsvHomeSwapQuotePhase::Loading { .. }) => BsvHomeSwapSessionAction::QuoteLoading,
            (BsvHomeSwapTransferPhase::Idle, BsvHomeSwapQuotePhase::Failed { error, .. }) => BsvHomeSwapSessionAction::QuoteError { error: error.clone() },
            (BsvHomeSwapTransferPhase::Idle, BsvHomeSwapQuotePhase::Ready | BsvHomeSwapQuotePhase::NoInput) => {
                if self.quote().is_some() {
                    BsvHomeSwapSessionAction::Ready
                } else {
                    BsvHomeSwapSessionAction::None
                }
            }
        }
    }

    fn button_action(&self, value: BsvHomeBigInt, available_balance: BsvHomeBigInt) -> BsvHomeSwapButtonAction {
        BsvHomeSwapButtonInput {
            value,
            available_balance,
            quote_error: self.quote_error(),
            transfer_error: self.transfer_error(),
        }
        .action()
    }

    fn button_state(&self, action: BsvHomeSwapButtonAction) -> BsvHomeSwapButtonState {
        match action {
            BsvHomeSwapButtonAction::InsufficientBalance => BsvHomeSwapButtonState::Disabled,
            _ if self.is_quote_loading() || self.is_transfer_loading() => BsvHomeSwapButtonState::Loading,
            BsvHomeSwapButtonAction::Swap if self.quote().is_none() => BsvHomeSwapButtonState::Disabled,
            BsvHomeSwapButtonAction::Swap | BsvHomeSwapButtonAction::RetryQuote | BsvHomeSwapButtonAction::RetryTransfer | BsvHomeSwapButtonAction::UseMinimumAmount { .. } => {
                BsvHomeSwapButtonState::Enabled
            }
        }
    }
}

impl BsvHomeSwapSession {
    pub(crate) fn transfer_error(&self) -> Option<SwapperError> {
        match &self.transfer_phase {
            BsvHomeSwapTransferPhase::Failed { error, .. } => Some(error.clone()),
            BsvHomeSwapTransferPhase::Idle | BsvHomeSwapTransferPhase::Loading { .. } => None,
        }
    }

    pub fn accepts_quotes(&self) -> bool {
        matches!(self.transfer_phase, BsvHomeSwapTransferPhase::Idle)
    }

    fn accepts_quote_phase(&self) -> bool {
        !self.is_transfer_loading() && !self.refresh_paused_until_restart
    }

    fn current_request(&self) -> Option<&BsvHomeSwapRequest> {
        match &self.quote_phase {
            BsvHomeSwapQuotePhase::Loading { request } | BsvHomeSwapQuotePhase::Failed { request, .. } => Some(request),
            BsvHomeSwapQuotePhase::Ready | BsvHomeSwapQuotePhase::NoInput => self.quotes.as_ref().map(|quotes| &quotes.request),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;
    use primitives::{Account, Asset, AssetType, Chain, Wallet};

    fn request(value: u32) -> BsvHomeSwapRequest {
        BsvHomeSwapRequest {
            pay_asset_id: AssetId::from_chain(Chain::Ethereum),
            receive_asset_id: AssetId::from_chain(Chain::Solana),
            value: BigUint::from(value),
            slippage_bps: None,
        }
    }

    fn quote(provider: SwapperProvider, to_value: u32) -> SwapperQuote {
        let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::Ethereum, "ethereum-address"), Account::mock(Chain::Solana, "solana-address")]);
        let asset = |chain: Chain| Asset::new(AssetId::from_chain(chain), chain.to_string(), chain.to_string().to_uppercase(), 18, AssetType::NATIVE);
        SwapperQuote {
            from_value: BigUint::from(100u32),
            min_from_value: None,
            to_value: BigUint::from(to_value),
            data: swapper::ProviderData {
                provider: swapper::ProviderType::new(provider),
                slippage_bps: 50,
                routes: vec![],
            },
            request: rules::quote_request(&wallet, &asset(Chain::Ethereum), &asset(Chain::Solana), BigUint::from(100u32), false, None).unwrap(),
            eta_in_seconds: None,
        }
    }

    fn results(quotes: Vec<SwapperQuote>) -> BsvHomeSwapQuotesResult {
        BsvHomeSwapQuotesResult {
            request: request(100),
            quotes,
            error: None,
        }
    }

    fn ready() -> BsvHomeSwapSession {
        BsvHomeSwapSession::default()
            .on_request_changed(Some(request(100)))
            .on_quote_results(results(vec![quote(SwapperProvider::Okx, 10), quote(SwapperProvider::Jupiter, 9)]))
    }

    #[test]
    fn test_a_changed_request_loads_and_an_empty_one_resets() {
        let loading = BsvHomeSwapSession::default().on_request_changed(Some(request(100)));
        assert_eq!(loading.quote_phase, BsvHomeSwapQuotePhase::Loading { request: request(100) });
        assert!(loading.quotes.is_none());
        assert_eq!(loading.action(), BsvHomeSwapSessionAction::QuoteLoading);

        let reset = ready().on_request_changed(None);
        assert_eq!(reset, BsvHomeSwapSession::default());
        assert!(reset.is_input_empty());
        assert_eq!(reset.action(), BsvHomeSwapSessionAction::None);
    }

    #[test]
    fn test_quote_results_pick_the_best_quote_and_report_failures() {
        let session = ready();
        assert_eq!(session.quote().unwrap().data.provider.id, SwapperProvider::Okx);
        assert_eq!(session.action(), BsvHomeSwapSessionAction::Ready);

        let failed = session.on_quote_results(BsvHomeSwapQuotesResult {
            error: Some(SwapperError::NoAvailableProvider),
            ..results(vec![])
        });
        assert!(failed.quotes.is_none());
        assert!(failed.quote().is_none());
        assert_eq!(failed.quote_error(), Some(SwapperError::NoAvailableProvider));
        assert_eq!(
            failed.action(),
            BsvHomeSwapSessionAction::QuoteError {
                error: SwapperError::NoAvailableProvider
            }
        );

        let empty = session.on_quote_results(results(vec![]));
        assert_eq!(empty.quote_error(), Some(SwapperError::NoQuoteAvailable));
    }

    #[test]
    fn test_the_chosen_provider_survives_a_refresh_and_falls_back_when_it_disappears() {
        let chosen = ready().on_provider_selected(SwapperProvider::Jupiter);
        assert_eq!(chosen.quote().unwrap().data.provider.id, SwapperProvider::Jupiter);

        let refreshed = chosen.on_quote_results(results(vec![quote(SwapperProvider::Okx, 11), quote(SwapperProvider::Jupiter, 8)]));
        assert_eq!(refreshed.quote().unwrap().to_value, BigUint::from(8u32));

        let without = refreshed.on_quote_results(results(vec![quote(SwapperProvider::Okx, 12)]));
        assert_eq!(without.quote().unwrap().data.provider.id, SwapperProvider::Okx);

        let new_amount = chosen.on_request_changed(Some(request(200)));
        assert!(new_amount.quote().is_none());
        assert_eq!(new_amount.selected_provider, Some(SwapperProvider::Jupiter));

        let new_pair = chosen.on_request_changed(Some(BsvHomeSwapRequest {
            receive_asset_id: AssetId::from_chain(Chain::Bitcoin),
            ..request(200)
        }));
        assert_eq!(new_pair.selected_provider, None);
    }

    #[test]
    fn test_a_loading_transfer_keeps_its_quote_and_ignores_new_results() {
        let started = ready().start_transfer().unwrap();
        assert!(started.is_transfer_loading());
        assert_eq!(started.action(), BsvHomeSwapSessionAction::TransferLoading);
        assert!(started.start_transfer().is_none());
        assert!(!started.accepts_quotes());

        let refreshed = started.on_quote_results(results(vec![quote(SwapperProvider::Okx, 99)]));
        assert_eq!(refreshed.quote().unwrap().to_value, BigUint::from(10u32));
        assert!(refreshed.is_transfer_loading());
        assert_eq!(refreshed.on_fetch_started(request(100)), refreshed);

        assert!(BsvHomeSwapSession::default().start_transfer().is_none());
    }

    #[test]
    fn test_transfer_outcomes_apply_only_to_the_transfer_that_started() {
        let started = ready().start_transfer().unwrap();
        let transfer = started.transfer_phase.clone();

        let failed = started.on_transfer_failed(transfer.clone(), SwapperError::TransactionError("boom".into()));
        assert_eq!(failed.transfer_error(), Some(SwapperError::TransactionError("boom".into())));
        assert_eq!(failed.error(), failed.transfer_error(), "a failed transfer outranks the quote it came from");
        assert_eq!(
            failed.action(),
            BsvHomeSwapSessionAction::TransferError {
                error: SwapperError::TransactionError("boom".into())
            }
        );
        assert!(failed.quote().is_some());
        assert_eq!(failed.on_transfer_failed(transfer.clone(), SwapperError::NoQuoteAvailable), failed);

        let handed_off = started.on_transfer_handed_off(transfer);
        assert_eq!(handed_off.transfer_phase, BsvHomeSwapTransferPhase::Idle);
        assert!(handed_off.refresh_paused_until_restart);
        assert!(!handed_off.refreshes_quotes(true));
        assert!(handed_off.on_refresh_resumed().refreshes_quotes(true));
        assert!(!handed_off.on_refresh_resumed().refreshes_quotes(false));
        assert_eq!(handed_off.on_fetch_started(request(100)), handed_off);

        assert_eq!(ready().on_transfer_handed_off(BsvHomeSwapTransferPhase::Idle), ready());
    }

    #[test]
    fn test_quote_changes_clear_a_failed_transfer() {
        let started = ready().start_transfer().unwrap();
        let failed = started.on_transfer_failed(started.transfer_phase.clone(), SwapperError::NoQuoteAvailable);

        assert_eq!(failed.on_provider_selected(SwapperProvider::Jupiter).transfer_phase, BsvHomeSwapTransferPhase::Idle);
        assert_eq!(failed.on_refresh_requested(request(100)).transfer_phase, BsvHomeSwapTransferPhase::Idle);
        assert_eq!(failed.on_quote_invalidated().transfer_phase, BsvHomeSwapTransferPhase::Idle);
    }

    #[test]
    fn test_button_state_follows_the_phases() {
        let idle = BsvHomeSwapSession::default();
        assert_eq!(idle.button_state(BsvHomeSwapButtonAction::Swap), BsvHomeSwapButtonState::Disabled);

        let loading = idle.on_request_changed(Some(request(100)));
        assert_eq!(loading.button_state(BsvHomeSwapButtonAction::Swap), BsvHomeSwapButtonState::Loading);

        let session = ready();
        assert_eq!(session.button_action(BsvHomeBigInt::from(1), BsvHomeBigInt::from(2)), BsvHomeSwapButtonAction::Swap);
        assert_eq!(session.button_state(BsvHomeSwapButtonAction::Swap), BsvHomeSwapButtonState::Enabled);
        assert_eq!(
            session.button_action(BsvHomeBigInt::from(3), BsvHomeBigInt::from(2)),
            BsvHomeSwapButtonAction::InsufficientBalance
        );
        assert_eq!(session.button_state(BsvHomeSwapButtonAction::InsufficientBalance), BsvHomeSwapButtonState::Disabled);

        let started = session.start_transfer().unwrap();
        assert_eq!(started.button_state(BsvHomeSwapButtonAction::Swap), BsvHomeSwapButtonState::Loading);
        let failed = started.on_transfer_failed(started.transfer_phase.clone(), SwapperError::TransactionError("boom".into()));
        assert_eq!(failed.button_action(BsvHomeBigInt::from(1), BsvHomeBigInt::from(2)), BsvHomeSwapButtonAction::RetryTransfer);
        assert_eq!(failed.button_state(BsvHomeSwapButtonAction::RetryTransfer), BsvHomeSwapButtonState::Enabled);
    }

    #[test]
    fn test_view_state_carries_the_quote_and_button_at_once() {
        let idle = BsvHomeSwapSession::default().view_state(BsvHomeBigInt::from(0), BsvHomeBigInt::from(0));
        assert!(idle.is_input_empty);
        assert_eq!(idle.button_state, BsvHomeSwapButtonState::Disabled);

        let session = ready();
        let state = session.view_state(BsvHomeBigInt::from(1), BsvHomeBigInt::from(2));
        assert_eq!(state.quote, session.quote());
        assert_eq!(state.action, BsvHomeSwapSessionAction::Ready);
        assert_eq!(state.button_action, BsvHomeSwapButtonAction::Swap);
        assert_eq!(state.button_state, BsvHomeSwapButtonState::Enabled);
        assert!(!state.is_quote_loading);
        assert_eq!(
            session.view_state(BsvHomeBigInt::from(3), BsvHomeBigInt::from(2)).button_action,
            BsvHomeSwapButtonAction::InsufficientBalance
        );
    }
}
