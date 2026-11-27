use std::sync::Arc;

use primitives::{Asset, AssetId, Chain, Currency};
use swapper::{Quote, SwapperError, SwapperSlippage};

use super::rules;
use super::{BsvHomeSwapPairSuggestion, BsvHomeSwapService, BsvHomeSwapSession, BsvHomeSwapTransfer};
use crate::config::swap_config::{get_default_slippage, get_swap_config};
use crate::models::custom_types::BsvHomeBigUint;
use crate::models::swap::BsvHomeSlippageCheck;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::stream::BsvHomeStreamSubscriptionService;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomeSwapQuoteService {
    swap: Arc<BsvHomeSwapService>,
    preferences: Arc<BsvHomePreferencesService>,
    balances: Arc<BsvHomeBalanceService>,
    stream: Arc<BsvHomeStreamSubscriptionService>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomeSwapQuoteService {
    #[uniffi::constructor]
    pub fn new(
        swap: Arc<BsvHomeSwapService>,
        preferences: Arc<BsvHomePreferencesService>,
        balances: Arc<BsvHomeBalanceService>,
        stream: Arc<BsvHomeStreamSubscriptionService>,
        session: Arc<BsvHomeWalletSessionService>,
    ) -> Self {
        Self {
            swap,
            preferences,
            balances,
            stream,
            session,
        }
    }

    pub fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }

    pub fn new_session(&self) -> BsvHomeSwapSession {
        BsvHomeSwapSession::default()
    }

    pub fn slippage_bps(&self) -> Option<u32> {
        self.preferences.get_swap_slippage_bps()
    }

    pub fn set_slippage_bps(&self, bps: Option<u32>) -> Result<(), BsvHomeServiceError> {
        self.preferences.set_swap_slippage_bps(bps)
    }

    pub fn slippage_check(&self, bps: u32) -> BsvHomeSlippageCheck {
        rules::slippage_check(bps, &get_swap_config())
    }

    pub fn default_slippage(&self, chain: Chain) -> SwapperSlippage {
        get_default_slippage(&chain)
    }

    pub fn refresh_interval_milliseconds(&self) -> u64 {
        rules::quote_refresh_interval_milliseconds()
    }

    pub fn quote_debounce_milliseconds(&self) -> u64 {
        rules::quote_debounce_milliseconds()
    }

    pub async fn get_quotes(&self, from_asset: Asset, to_asset: Asset, value: BsvHomeBigUint, use_max_amount: bool, slippage_bps: Option<u32>) -> Result<Vec<Quote>, SwapperError> {
        let wallet = self.session.current_wallet().await.map_err(|error| SwapperError::ComputeQuoteError(error.to_string()))?;
        self.swap.get_quotes(wallet, from_asset, to_asset, value, use_max_amount, slippage_bps).await
    }

    pub async fn suggest_pair(&self, pay_asset_id: Option<AssetId>) -> Result<Option<BsvHomeSwapPairSuggestion>, BsvHomeServiceError> {
        self.swap.suggest_pair(self.session.current_wallet_id()?, pay_asset_id).await
    }

    pub async fn get_transfer(&self, quote: Quote) -> Result<BsvHomeSwapTransfer, SwapperError> {
        let wallet = self.session.current_wallet().await.map_err(|error| SwapperError::TransactionError(error.to_string()))?;
        self.swap.get_transfer(wallet, quote).await
    }

    pub async fn update_balances(&self, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        self.balances.update(self.session.current_wallet_id()?, asset_ids).await
    }

    pub async fn add_prices(&self, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        self.stream.add_prices(asset_ids).await
    }
}
