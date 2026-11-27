use std::sync::{Arc, Mutex};
use std::task::Poll;

use chrono::Utc;
use futures::channel::oneshot;
use futures::executor::block_on;
use futures::{join, poll};
use primitives::{AssetMarket, AssetPrice, Chain, Currency, FiatRate};

use super::BsvHomePriceService;
use super::testkit::MemoryPriceStore;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::preferences::testkit::MemoryPreferencesStore;

fn preferences() -> Arc<BsvHomePreferencesService> {
    Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())))
}

#[test]
fn test_late_price_updates_use_current_currency_for_both_price_sources() {
    block_on(async {
        let store = Arc::new(MemoryPriceStore {
            rates: Mutex::new(vec![FiatRate { symbol: Currency::CNY, rate: 7.0 }]),
            ..Default::default()
        });
        let preferences = preferences();
        let service = BsvHomePriceService::new(store.clone(), preferences.clone());
        let late_prices = vec![AssetPrice::new(Chain::Solana.as_asset_id(), 100.0, 1.5, Utc::now())];
        let late_defillama_prices = vec![AssetPrice::new(Chain::BitcoinSv.as_asset_id(), 20.0, 1.5, Utc::now())];

        service.change_currency(Currency::CNY).await.unwrap();
        service.update_prices(late_prices).await.unwrap();
        service.update_defillama_prices(late_defillama_prices).await.unwrap();

        assert_eq!(preferences.get_currency(), Currency::CNY);
        assert_eq!(
            store
                .saved
                .lock()
                .unwrap()
                .iter()
                .map(|(currency, updates)| (currency.clone(), updates[0].price, updates[0].price_usd))
                .collect::<Vec<_>>(),
            vec![(Currency::CNY, 700.0, 100.0), (Currency::CNY, 140.0, 20.0)]
        );
    });
}

#[test]
fn test_price_write_waits_for_currency_conversion_and_preference_commit() {
    block_on(async {
        let (started, conversion_started) = oneshot::channel();
        let (resume, conversion_resume) = oneshot::channel();
        let store = Arc::new(MemoryPriceStore {
            rates: Mutex::new(vec![FiatRate { symbol: Currency::CNY, rate: 7.0 }]),
            conversion_pause: Mutex::new(Some((started, conversion_resume))),
            ..Default::default()
        });
        let preferences = preferences();
        let service = BsvHomePriceService::new(store.clone(), preferences.clone());

        let (changed, updated) = join!(service.change_currency(Currency::CNY), async {
            conversion_started.await.unwrap();
            let mut update = Box::pin(service.update_prices(vec![AssetPrice::new(Chain::Solana.as_asset_id(), 100.0, 1.5, Utc::now())]));
            assert_eq!(poll!(update.as_mut()), Poll::Pending);
            assert_eq!(preferences.get_currency(), Currency::USD);
            assert_eq!(store.saved.lock().unwrap().len(), 0);
            resume.send(()).unwrap();
            update.await
        });

        assert_eq!(changed, Ok(true));
        assert_eq!(updated, Ok(()));
        assert_eq!(preferences.get_currency(), Currency::CNY);
        let saved = store.saved.lock().unwrap();
        assert_eq!(saved[0].0, Currency::CNY);
        assert_eq!(saved[0].1[0].price, 700.0);
        assert_eq!(saved[0].1[0].price_usd, 100.0);
    });
}

#[test]
fn test_late_market_update_projects_fiat_values_in_current_currency() {
    block_on(async {
        let store = Arc::new(MemoryPriceStore {
            rates: Mutex::new(vec![FiatRate { symbol: Currency::CNY, rate: 7.0 }]),
            ..Default::default()
        });
        let service = BsvHomePriceService::new(store.clone(), preferences());
        let market = AssetMarket {
            market_cap: Some(1_000.0),
            market_cap_fdv: Some(1_500.0),
            total_volume: Some(200.0),
            circulating_supply: Some(10.0),
            ..Default::default()
        };

        service.change_currency(Currency::CNY).await.unwrap();
        service.update_market(Chain::Solana.as_asset_id(), market).await.unwrap();

        let markets = store.markets.lock().unwrap();
        assert_eq!(markets[0].0, Chain::Solana.as_asset_id());
        assert_eq!(
            (
                markets[0].1.market_cap,
                markets[0].1.market_cap_fdv,
                markets[0].1.total_volume,
                markets[0].1.circulating_supply
            ),
            (Some(7_000.0), Some(10_500.0), Some(1_400.0), Some(10.0))
        );
    });
}

#[test]
fn test_late_rate_update_reconverts_current_currency() {
    block_on(async {
        let store = Arc::new(MemoryPriceStore {
            rates: Mutex::new(vec![FiatRate { symbol: Currency::CNY, rate: 7.0 }]),
            ..Default::default()
        });
        let preferences = preferences();
        let service = BsvHomePriceService::new(store.clone(), preferences.clone());

        service.change_currency(Currency::CNY).await.unwrap();
        service.update_rates(vec![FiatRate { symbol: Currency::CNY, rate: 8.0 }]).await.unwrap();

        assert_eq!(preferences.get_currency(), Currency::CNY);
        assert_eq!(*store.converted.lock().unwrap(), vec![(Currency::CNY, 7.0), (Currency::CNY, 8.0)]);
    });
}
