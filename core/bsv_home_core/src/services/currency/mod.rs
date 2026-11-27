pub mod model;
mod rules;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use primitives::Currency;

pub use model::{BsvHomeCurrencies, BsvHomeCurrencyRow};

use crate::services::device::BsvHomeDeviceService;
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::price::BsvHomePriceService;

#[derive(uniffi::Object)]
pub struct BsvHomeCurrencyService {
    preferences: Arc<BsvHomePreferencesService>,
    prices: Arc<BsvHomePriceService>,
    device: Arc<BsvHomeDeviceService>,
}

#[uniffi::export]
impl BsvHomeCurrencyService {
    #[uniffi::constructor]
    pub fn new(preferences: Arc<BsvHomePreferencesService>, prices: Arc<BsvHomePriceService>, device: Arc<BsvHomeDeviceService>) -> Self {
        Self { preferences, prices, device }
    }

    pub fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }

    pub fn currencies(&self, locale: Option<Currency>) -> BsvHomeCurrencies {
        rules::currencies(self.get_currency(), locale)
    }

    pub async fn set_currency(&self, currency: Currency) -> Result<(), BsvHomeServiceError> {
        if self.prices.change_currency(currency).await? {
            let _ = self.device.synchronize_if_needed().await;
        }
        Ok(())
    }
}
