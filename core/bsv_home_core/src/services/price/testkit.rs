use std::sync::Mutex;

use futures::channel::oneshot;
use primitives::currency::Currency;
use primitives::{AssetId, AssetMarket, AssetPrice, FiatRate};

use super::{BsvHomePriceStore, BsvHomePriceUpdate};
use crate::services::error::BsvHomeServiceError;

#[derive(Default)]
pub struct MemoryPriceStore {
    pub rates: Mutex<Vec<FiatRate>>,
    pub rate_writes: Mutex<Vec<Vec<FiatRate>>>,
    pub prices: Mutex<Vec<AssetPrice>>,
    pub saved: Mutex<Vec<(Currency, Vec<BsvHomePriceUpdate>)>>,
    pub converted: Mutex<Vec<(Currency, f64)>>,
    pub convert_error: Option<BsvHomeServiceError>,
    pub conversion_pause: Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
    pub markets: Mutex<Vec<(AssetId, AssetMarket)>>,
}

#[async_trait::async_trait]
impl BsvHomePriceStore for MemoryPriceStore {
    async fn get_prices(&self, asset_ids: Vec<AssetId>) -> Result<Vec<AssetPrice>, BsvHomeServiceError> {
        let stored = self.prices.lock().unwrap();
        Ok(asset_ids
            .iter()
            .filter_map(|asset_id| stored.iter().rev().find(|price| &price.asset_id == asset_id).cloned())
            .collect())
    }
    async fn get_rate(&self, currency: Currency) -> Result<Option<FiatRate>, BsvHomeServiceError> {
        Ok(self.rates.lock().unwrap().iter().find(|rate| rate.symbol == currency).cloned())
    }
    async fn get_rates(&self) -> Result<Vec<FiatRate>, BsvHomeServiceError> {
        Ok(self.rates.lock().unwrap().clone())
    }
    async fn save_rates(&self, rates: Vec<FiatRate>) -> Result<(), BsvHomeServiceError> {
        let mut stored = self.rates.lock().unwrap();
        stored.retain(|stored| rates.iter().all(|rate| rate.symbol != stored.symbol));
        stored.extend(rates.clone());
        self.rate_writes.lock().unwrap().push(rates);
        Ok(())
    }
    async fn save_prices(&self, currency: Currency, prices: Vec<BsvHomePriceUpdate>) -> Result<(), BsvHomeServiceError> {
        self.prices.lock().unwrap().extend(
            prices
                .iter()
                .map(|price| AssetPrice::new(price.asset_id.clone(), price.price, price.price_change_percentage_24h, price.updated_at)),
        );
        self.saved.lock().unwrap().push((currency, prices));
        Ok(())
    }
    async fn convert_prices(&self, currency: Currency, rate: f64) -> Result<(), BsvHomeServiceError> {
        if let Some(error) = &self.convert_error {
            return Err(error.clone());
        }
        let pause = self.conversion_pause.lock().unwrap().take();
        if let Some((started, resume)) = pause {
            started.send(()).unwrap();
            resume.await.unwrap();
        }
        self.converted.lock().unwrap().push((currency, rate));
        Ok(())
    }
    async fn save_market(&self, asset_id: AssetId, market: AssetMarket) -> Result<(), BsvHomeServiceError> {
        self.markets.lock().unwrap().push((asset_id, market));
        Ok(())
    }
}
