use std::sync::Arc;

use primitives::currency::Currency;
use primitives::{AssetId, FiatQuote, FiatQuoteType, FiatQuoteUrl};

use super::model::BsvHomeFiatAmountCheck;
use super::session::BsvHomeFiatSession;
use super::{BsvHomeFiatService, rules};
use crate::config::fiat_config::get_fiat_config;
use crate::models::custom_types::BsvHomeBigUint;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet_session::BsvHomeWalletSessionService;

const CURRENCY: Currency = Currency::USD;

#[derive(uniffi::Object)]
pub struct BsvHomeFiatQuoteService {
    fiat: Arc<BsvHomeFiatService>,
    balances: Arc<BsvHomeBalanceService>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomeFiatQuoteService {
    #[uniffi::constructor]
    pub fn new(fiat: Arc<BsvHomeFiatService>, balances: Arc<BsvHomeBalanceService>, session: Arc<BsvHomeWalletSessionService>) -> Self {
        Self { fiat, balances, session }
    }

    pub fn get_currency(&self) -> Currency {
        CURRENCY
    }

    pub fn suggested_amounts(&self) -> Vec<i32> {
        get_fiat_config().suggested_amounts
    }

    pub fn new_session(&self, quote_type: FiatQuoteType, amount: Option<u32>) -> BsvHomeFiatSession {
        BsvHomeFiatSession::new(quote_type, amount)
    }

    pub fn random_amount(&self) -> u32 {
        rules::random_amount(&get_fiat_config())
    }

    pub fn amount_check(&self, quote_type: FiatQuoteType, amount: f64, quote: Option<FiatQuote>, available: BsvHomeBigUint) -> BsvHomeFiatAmountCheck {
        rules::amount_check(&get_fiat_config(), quote_type, amount, quote.as_ref(), &available)
    }

    pub fn quote_debounce_milliseconds(&self) -> u64 {
        self.fiat.quote_debounce_milliseconds()
    }

    pub fn quote_refresh_interval_milliseconds(&self) -> u64 {
        self.fiat.quote_refresh_interval_milliseconds()
    }

    pub async fn sync_transactions(&self) -> Result<(), BsvHomeServiceError> {
        self.fiat.sync_transactions(self.session.current_wallet_id()?).await
    }

    pub async fn quotes(&self, quote_type: FiatQuoteType, asset_id: AssetId, amount: f64) -> Result<Vec<FiatQuote>, BsvHomeServiceError> {
        self.fiat.get_quotes(self.session.current_wallet_id()?, quote_type, asset_id, amount, CURRENCY).await
    }

    pub async fn quote_url(&self, asset_id: AssetId, quote_id: String) -> Result<FiatQuoteUrl, BsvHomeServiceError> {
        let wallet_id = self.session.current_wallet_id()?;
        let url = self.fiat.get_quote_url(wallet_id.clone(), quote_id).await?;
        self.balances.set_assets_enabled(wallet_id, vec![asset_id], true).await?;
        Ok(url)
    }
}
