pub mod model;
pub mod quote;
pub mod rules;
pub mod session;
pub mod store;

use crate::services::error::BsvHomeServiceError;
use std::sync::Arc;

use primitives::currency::Currency;
use primitives::{AssetId, FiatQuote, FiatQuoteType, FiatQuoteUrl, WalletId};

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::services::assets::BsvHomeAssetsService;

pub use model::{BsvHomeFiatAmountCheck, BsvHomeFiatTransactionBadge, BsvHomeFiatTransactionStatus};
pub use quote::BsvHomeFiatQuoteService;
pub use session::{
    BsvHomeFiatButtonAction, BsvHomeFiatButtonState, BsvHomeFiatOperation, BsvHomeFiatQuotePhase, BsvHomeFiatQuoteRequest, BsvHomeFiatQuotesResult, BsvHomeFiatSession,
};
pub use store::BsvHomeFiatStore;

const QUOTE_DEBOUNCE_MILLISECONDS: u64 = 250;
const QUOTE_REFRESH_INTERVAL_MILLISECONDS: u64 = 5 * 60 * 1_000;

#[derive(uniffi::Object)]
pub struct BsvHomeFiatService {
    api: Arc<BsvHomeDeviceApiClient>,
    assets: Arc<BsvHomeAssetsService>,
    store: Arc<dyn BsvHomeFiatStore>,
}

#[uniffi::export]
impl BsvHomeFiatService {
    #[uniffi::constructor]
    pub fn new(api: Arc<BsvHomeDeviceApiClient>, assets: Arc<BsvHomeAssetsService>, store: Arc<dyn BsvHomeFiatStore>) -> Self {
        Self { api, assets, store }
    }
}

impl BsvHomeFiatService {
    pub fn quote_debounce_milliseconds(&self) -> u64 {
        QUOTE_DEBOUNCE_MILLISECONDS
    }

    pub fn quote_refresh_interval_milliseconds(&self) -> u64 {
        QUOTE_REFRESH_INTERVAL_MILLISECONDS
    }

    pub async fn sync_transactions(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        let transactions = self.api.client.get_fiat_transactions(wallet_id.id()).await.map_err(BsvHomeApiError::from)?;
        let asset_ids = transactions.iter().map(|data| data.transaction.asset_id.clone()).collect();
        self.assets.sync_missing_assets(asset_ids).await?;
        self.store.set_transactions(wallet_id, transactions).await
    }

    pub async fn get_quotes(
        &self,
        wallet_id: WalletId,
        quote_type: FiatQuoteType,
        asset_id: AssetId,
        amount: f64,
        currency: Currency,
    ) -> Result<Vec<FiatQuote>, BsvHomeServiceError> {
        Ok(self
            .api
            .client
            .get_fiat_quotes(wallet_id.id(), quote_type, asset_id.to_string(), amount, currency.to_string())
            .await
            .map_err(BsvHomeApiError::from)?
            .quotes)
    }

    pub async fn get_quote_url(&self, wallet_id: WalletId, quote_id: String) -> Result<FiatQuoteUrl, BsvHomeServiceError> {
        Ok(self.api.client.get_fiat_quote_url(wallet_id.id(), quote_id).await.map_err(BsvHomeApiError::from)?)
    }
}
