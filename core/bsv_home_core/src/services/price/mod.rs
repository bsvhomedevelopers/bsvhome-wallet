pub(crate) mod client;
#[cfg(test)]
mod currency_tests;
pub mod model;
pub mod rules;
pub mod store;
#[cfg(test)]
pub(crate) mod testkit;

use std::sync::Arc;

use futures::lock::Mutex;
use primitives::currency::Currency;
use primitives::{AssetId, AssetMarket, AssetPrice, FiatRate};

use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomePreferencesService;

pub use model::BsvHomePriceUpdate;
pub use store::BsvHomePriceStore;

#[derive(uniffi::Object)]
pub struct BsvHomePriceService {
    store: Arc<dyn BsvHomePriceStore>,
    preferences: Arc<BsvHomePreferencesService>,
    currency_lock: Mutex<()>,
}

#[uniffi::export]
impl BsvHomePriceService {
    #[uniffi::constructor]
    pub fn new(store: Arc<dyn BsvHomePriceStore>, preferences: Arc<BsvHomePreferencesService>) -> Self {
        Self {
            store,
            preferences,
            currency_lock: Mutex::new(()),
        }
    }
}

impl BsvHomePriceService {
    pub async fn prices(&self, asset_ids: Vec<AssetId>) -> Result<Vec<AssetPrice>, BsvHomeServiceError> {
        let asset_ids = asset_ids.into_iter().filter(|asset_id| !asset_id.chain.is_testnet()).collect::<Vec<_>>();
        if asset_ids.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self
            .store
            .get_prices(asset_ids)
            .await?
            .into_iter()
            .filter(|price| !price.asset_id.chain.is_testnet() && price.has_price())
            .collect())
    }

    pub async fn update_prices(&self, prices: Vec<AssetPrice>) -> Result<(), BsvHomeServiceError> {
        let prices = prices
            .into_iter()
            .filter(|price| !price.asset_id.chain.is_testnet() && rules::defillama_coin_id(&price.asset_id).is_none())
            .collect();
        let _guard = self.currency_lock.lock().await;
        update_prices(self.store.as_ref(), prices, self.preferences.get_currency()).await
    }

    pub(crate) async fn update_defillama_prices(&self, prices: Vec<AssetPrice>) -> Result<(), BsvHomeServiceError> {
        let prices = prices.into_iter().filter(|price| rules::defillama_coin_id(&price.asset_id).is_some()).collect();
        let _guard = self.currency_lock.lock().await;
        update_prices(self.store.as_ref(), prices, self.preferences.get_currency()).await
    }

    pub async fn update_rates(&self, rates: Vec<FiatRate>) -> Result<(), BsvHomeServiceError> {
        let _guard = self.currency_lock.lock().await;
        update_rates(self.store.as_ref(), rates, self.preferences.get_currency()).await
    }

    pub async fn update_market(&self, asset_id: AssetId, market: AssetMarket) -> Result<(), BsvHomeServiceError> {
        if asset_id.chain.is_testnet() {
            return Ok(());
        }
        let _guard = self.currency_lock.lock().await;
        let Some(rate) = self.rate(self.preferences.get_currency()).await? else {
            return Ok(());
        };
        self.store.save_market(asset_id, rules::market_in_currency(market, rate.rate)).await
    }

    pub async fn change_currency(&self, currency: Currency) -> Result<bool, BsvHomeServiceError> {
        let _guard = self.currency_lock.lock().await;
        if currency == self.preferences.get_currency() {
            return Ok(false);
        }
        let Some(rate) = rules::rate_or_base(currency.clone(), self.store.get_rate(currency.clone()).await?) else {
            return Err(BsvHomeServiceError::InvalidInput {
                msg: format!("unknown currency: {currency}"),
            });
        };
        self.store.convert_prices(currency.clone(), rate.rate).await?;
        self.preferences.set_currency(currency)?;
        Ok(true)
    }

    pub async fn update_asset_price(&self, asset_id: AssetId, price: Option<AssetPrice>) -> Result<(), BsvHomeServiceError> {
        self.update_prices(vec![price.unwrap_or_else(|| AssetPrice::empty(asset_id))]).await
    }

    pub async fn rate(&self, currency: Currency) -> Result<Option<FiatRate>, BsvHomeServiceError> {
        Ok(rules::rate_or_base(currency.clone(), self.store.get_rate(currency).await?))
    }
}

async fn update_prices(store: &dyn BsvHomePriceStore, prices: Vec<AssetPrice>, currency: Currency) -> Result<(), BsvHomeServiceError> {
    if prices.is_empty() {
        return Ok(());
    }
    let Some(rate) = rules::rate_or_base(currency.clone(), store.get_rate(currency.clone()).await?) else {
        return Ok(());
    };
    let stored = store.get_prices(prices.iter().map(|price| price.asset_id.clone()).collect()).await?;
    let updates = rules::changed_prices(stored, rules::fiat_prices(prices, &rate));
    if updates.is_empty() {
        return Ok(());
    }
    store.save_prices(currency, updates).await
}

async fn update_rates(store: &dyn BsvHomePriceStore, rates: Vec<FiatRate>, currency: Currency) -> Result<(), BsvHomeServiceError> {
    let changed = rules::changed_rates(store.get_rates().await?, rates);
    if changed.is_empty() {
        return Ok(());
    }
    let current = changed.iter().find(|rate| rate.symbol == currency).map(|rate| rate.rate);
    store.save_rates(changed).await?;
    if let Some(rate) = current {
        store.convert_prices(currency, rate).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::testkit::MemoryPriceStore;
    use super::*;
    use crate::services::preferences::testkit::MemoryPreferencesStore;
    use chrono::Utc;
    use primitives::Chain;
    use std::sync::Mutex;

    fn store_with_rate(currency: Currency, rate: f64) -> MemoryPriceStore {
        MemoryPriceStore {
            rates: Mutex::new(vec![FiatRate { symbol: currency, rate }]),
            ..Default::default()
        }
    }

    fn price(value: f64) -> AssetPrice {
        AssetPrice::new(AssetId::from_chain(Chain::Solana), value, 1.5, Utc::now())
    }

    #[test]
    fn test_prices_are_converted_with_the_stored_rate() {
        let store = store_with_rate(Currency::EUR, 0.5);

        futures::executor::block_on(update_prices(&store, vec![price(100.0)], Currency::EUR)).unwrap();

        let saved = store.saved.lock().unwrap();
        assert_eq!(saved[0].0, Currency::EUR);
        assert_eq!(saved[0].1[0].price, 50.0);
        assert_eq!(saved[0].1[0].price_usd, 100.0);
    }

    #[test]
    fn test_defillama_prices_cannot_be_replaced_by_backend_or_empty_prices() {
        futures::executor::block_on(async {
            let store = Arc::new(MemoryPriceStore::default());
            let service = BsvHomePriceService::new(store.clone(), Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default()))));
            let now = Utc::now();
            let bsv = Chain::BitcoinSv.as_asset_id();
            let usdt = primitives::asset_constants::TRON_USDT_ASSET_ID.clone();
            let solana = Chain::Solana.as_asset_id();
            let fresh = vec![AssetPrice::new(bsv.clone(), 20.31, 0.0, now), AssetPrice::new(usdt.clone(), 0.9997, 0.0, now)];
            service.update_defillama_prices(fresh.clone()).await.unwrap();
            service.update_asset_price(bsv.clone(), None).await.unwrap();
            service
                .update_prices(vec![
                    AssetPrice::new(bsv.clone(), 10.0, 3.0, now),
                    AssetPrice::new(usdt.clone(), 1.0, 1.0, now),
                    AssetPrice::new(solana.clone(), 150.0, 2.0, now),
                ])
                .await
                .unwrap();
            assert_eq!(service.prices(vec![bsv, usdt]).await.unwrap(), fresh);
            assert_eq!(service.prices(vec![solana.clone()]).await.unwrap(), vec![AssetPrice::new(solana, 150.0, 2.0, now)]);
            assert_eq!(store.saved.lock().unwrap().len(), 2);
        });
    }

    #[test]
    fn test_prices_are_dropped_without_a_stored_rate_except_usd() {
        let store = MemoryPriceStore::default();

        futures::executor::block_on(update_prices(&store, vec![price(100.0)], Currency::EUR)).unwrap();
        assert!(store.saved.lock().unwrap().is_empty());

        futures::executor::block_on(update_prices(&store, vec![price(100.0)], Currency::USD)).unwrap();
        assert_eq!(store.saved.lock().unwrap()[0].1[0].price, 100.0);
    }

    #[test]
    fn test_asset_without_price_stores_zeroed_row() {
        let store = store_with_rate(Currency::EUR, 0.5);
        let asset_id = AssetId::from_chain(Chain::Solana);

        futures::executor::block_on(update_prices(&store, vec![AssetPrice::empty(asset_id.clone())], Currency::EUR)).unwrap();

        let saved = store.saved.lock().unwrap();
        assert_eq!(saved[0].1[0].asset_id, asset_id);
        assert_eq!(saved[0].1[0].price, 0.0);
        assert_eq!(saved[0].1[0].price_usd, 0.0);
    }

    #[test]
    fn test_unchanged_prices_are_not_saved() {
        let store = store_with_rate(Currency::EUR, 0.5);

        futures::executor::block_on(update_prices(&store, vec![price(100.0)], Currency::EUR)).unwrap();
        futures::executor::block_on(update_prices(&store, vec![price(100.0)], Currency::EUR)).unwrap();
        assert_eq!(store.saved.lock().unwrap().len(), 1);

        futures::executor::block_on(update_prices(&store, vec![price(101.0)], Currency::EUR)).unwrap();
        let saved = store.saved.lock().unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[1].1[0].price_usd, 101.0);
    }

    #[test]
    fn test_new_rate_for_current_currency_reconverts_stored_prices() {
        let store = MemoryPriceStore::default();
        let rates = vec![FiatRate { symbol: Currency::EUR, rate: 0.9 }, FiatRate { symbol: Currency::GBP, rate: 0.8 }];

        futures::executor::block_on(update_rates(&store, rates.clone(), Currency::EUR)).unwrap();
        futures::executor::block_on(update_rates(&store, rates, Currency::JPY)).unwrap();

        assert_eq!(*store.converted.lock().unwrap(), vec![(Currency::EUR, 0.9)]);
    }

    #[test]
    fn test_unchanged_rate_does_not_reconvert_stored_prices() {
        let store = MemoryPriceStore::default();
        let rate = |rate| vec![FiatRate { symbol: Currency::EUR, rate }];

        futures::executor::block_on(update_rates(&store, rate(0.9), Currency::EUR)).unwrap();
        futures::executor::block_on(update_rates(&store, rate(0.9), Currency::EUR)).unwrap();
        assert_eq!(*store.converted.lock().unwrap(), vec![(Currency::EUR, 0.9)]);

        futures::executor::block_on(update_rates(&store, rate(1.1), Currency::EUR)).unwrap();
        assert_eq!(*store.converted.lock().unwrap(), vec![(Currency::EUR, 0.9), (Currency::EUR, 1.1)]);
    }

    #[test]
    fn test_only_moved_rates_are_saved() {
        let store = MemoryPriceStore::default();
        let rate = |symbol, rate| FiatRate { symbol, rate };

        futures::executor::block_on(update_rates(&store, vec![rate(Currency::EUR, 0.9), rate(Currency::GBP, 0.8)], Currency::EUR)).unwrap();
        futures::executor::block_on(update_rates(&store, vec![rate(Currency::EUR, 0.9), rate(Currency::GBP, 0.8)], Currency::EUR)).unwrap();
        futures::executor::block_on(update_rates(&store, vec![rate(Currency::EUR, 0.9), rate(Currency::GBP, 0.7)], Currency::EUR)).unwrap();

        assert_eq!(
            *store.rate_writes.lock().unwrap(),
            vec![vec![rate(Currency::EUR, 0.9), rate(Currency::GBP, 0.8)], vec![rate(Currency::GBP, 0.7)]]
        );
        assert_eq!(*store.converted.lock().unwrap(), vec![(Currency::EUR, 0.9)]);
    }

    #[test]
    fn test_testnet_prices_are_hidden_and_never_written() {
        futures::executor::block_on(async {
            let now = Utc::now();
            let mainnet = Chain::Solana.as_asset_id();
            let test_assets = [Chain::TronShasta.as_asset_id(), primitives::known_assets::TRON_SHASTA_USDT.id.clone()];
            let stale_prices = test_assets.iter().map(|asset_id| AssetPrice::new(asset_id.clone(), 99.0, 1.5, now)).collect::<Vec<_>>();
            let store = Arc::new(MemoryPriceStore {
                prices: Mutex::new([vec![AssetPrice::new(mainnet.clone(), 100.0, 1.5, now)], stale_prices.clone()].concat()),
                ..Default::default()
            });
            let service = BsvHomePriceService::new(store.clone(), Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default()))));

            assert!(service.prices(test_assets.to_vec()).await.unwrap().is_empty());
            assert_eq!(
                service.prices([vec![mainnet.clone()], test_assets.to_vec()].concat()).await.unwrap(),
                vec![AssetPrice::new(mainnet.clone(), 100.0, 1.5, now)]
            );

            let test_updates = test_assets.iter().map(|asset_id| AssetPrice::new(asset_id.clone(), 1.0, 0.0, now)).collect::<Vec<_>>();
            service
                .update_prices([vec![AssetPrice::new(mainnet.clone(), 120.0, 2.0, now)], test_updates.clone()].concat())
                .await
                .unwrap();
            service.update_prices(test_updates).await.unwrap();
            for asset_id in &test_assets {
                service.update_asset_price(asset_id.clone(), None).await.unwrap();
            }

            assert!(service.prices(test_assets.to_vec()).await.unwrap().is_empty());

            let saved = store.saved.lock().unwrap();
            assert_eq!(saved.len(), 1);
            assert_eq!(saved[0].1.len(), 1);
            assert_eq!(saved[0].1[0].asset_id, mainnet);
            assert_eq!(saved[0].1[0].price_usd, 120.0);
            assert_eq!(
                store
                    .prices
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|price| price.asset_id.chain.is_testnet())
                    .cloned()
                    .collect::<Vec<_>>(),
                stale_prices
            );
        });
    }

    #[test]
    fn test_testnet_market_updates_are_never_saved() {
        futures::executor::block_on(async {
            let store = Arc::new(MemoryPriceStore::default());
            let service = BsvHomePriceService::new(store.clone(), Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default()))));
            let mainnet = Chain::Solana.as_asset_id();

            for asset_id in [Chain::TronShasta.as_asset_id(), primitives::known_assets::TRON_SHASTA_USDT.id.clone()] {
                service.update_market(asset_id, AssetMarket::mock()).await.unwrap();
            }
            assert!(store.markets.lock().unwrap().is_empty());

            service.update_market(mainnet.clone(), AssetMarket::mock()).await.unwrap();
            let markets = store.markets.lock().unwrap();
            assert_eq!(markets.len(), 1);
            assert_eq!(markets[0].0, mainnet);
        });
    }
}
