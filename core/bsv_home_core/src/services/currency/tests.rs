use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use primitives::{Currency, FiatRate};

use super::BsvHomeCurrencyService;
use crate::api::BsvHomeDeviceApiClient;
use crate::gateway::EmptyPreferences;
use crate::services::device::testkit::MemoryDevicePlatform;
use crate::services::device::{BsvHomeDeviceKeyService, BsvHomeDeviceService};
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::preferences::testkit::MemoryPreferencesStore;
use crate::services::price::BsvHomePriceService;
use crate::services::price::testkit::MemoryPriceStore;
use crate::services::subscription::BsvHomeSubscriptionService;
use crate::services::wallet::testkit::MemoryWalletStore;
use crate::testkit::TestAlienProvider;

fn service(prices: Arc<MemoryPriceStore>) -> (BsvHomeCurrencyService, Arc<MemoryPreferencesStore>, Arc<TestAlienProvider>) {
    let store = Arc::new(MemoryPreferencesStore::default());
    let preferences = Arc::new(BsvHomePreferencesService::new(store.clone()));
    let provider = Arc::new(TestAlienProvider::with_status(500));
    let api = Arc::new(BsvHomeDeviceApiClient::new(
        provider.clone(),
        Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
    ));
    let wallets = Arc::new(MemoryWalletStore::default());
    let device = Arc::new(BsvHomeDeviceService::new(
        api.clone(),
        Arc::new(BsvHomeSubscriptionService::new(api, wallets.clone())),
        wallets,
        Arc::new(MemoryDevicePlatform),
        preferences.clone(),
    ));
    (
        BsvHomeCurrencyService::new(preferences.clone(), Arc::new(BsvHomePriceService::new(prices, preferences)), device),
        store,
        provider,
    )
}

#[test]
fn test_set_currency_preserves_currency_when_rate_is_missing() {
    let prices = Arc::new(MemoryPriceStore::default());
    let (service, preferences, provider) = service(prices.clone());

    assert_eq!(
        block_on(service.set_currency(Currency::EUR)),
        Err(BsvHomeServiceError::InvalidInput {
            msg: "unknown currency: EUR".into(),
        })
    );
    assert_eq!(service.get_currency(), Currency::USD);
    assert_eq!(preferences.values.lock().unwrap().len(), 0);
    assert_eq!(*prices.converted.lock().unwrap(), Vec::new());
    assert_eq!(provider.requested_paths(), Vec::<String>::new());
}

#[test]
fn test_set_currency_preserves_currency_when_price_conversion_fails() {
    let error = BsvHomeServiceError::Store { msg: "conversion failed".into() };
    let prices = Arc::new(MemoryPriceStore {
        rates: Mutex::new(vec![FiatRate { symbol: Currency::EUR, rate: 0.9 }]),
        convert_error: Some(error.clone()),
        ..Default::default()
    });
    let (service, preferences, provider) = service(prices.clone());

    assert_eq!(block_on(service.set_currency(Currency::EUR)), Err(error));
    assert_eq!(service.get_currency(), Currency::USD);
    assert_eq!(preferences.values.lock().unwrap().len(), 0);
    assert_eq!(*prices.converted.lock().unwrap(), Vec::new());
    assert_eq!(provider.requested_paths(), Vec::<String>::new());
}

#[test]
fn test_set_currency_commits_conversion_before_best_effort_device_sync() {
    let prices = Arc::new(MemoryPriceStore {
        rates: Mutex::new(vec![FiatRate { symbol: Currency::EUR, rate: 0.9 }]),
        ..Default::default()
    });
    let (service, _, provider) = service(prices.clone());

    block_on(service.set_currency(Currency::EUR)).unwrap();
    assert_eq!(service.get_currency(), Currency::EUR);
    assert_eq!(*prices.converted.lock().unwrap(), vec![(Currency::EUR, 0.9)]);
    assert_eq!(provider.requested_paths(), vec!["/v2/devices/is_registered"]);

    block_on(service.set_currency(Currency::USD)).unwrap();
    assert_eq!(service.get_currency(), Currency::USD);
    assert_eq!(*prices.converted.lock().unwrap(), vec![(Currency::EUR, 0.9), (Currency::USD, 1.0)]);
}

#[test]
fn test_set_currency_skips_conversion_and_sync_for_current_currency() {
    let prices = Arc::new(MemoryPriceStore::default());
    let (service, preferences, provider) = service(prices.clone());

    block_on(service.set_currency(Currency::USD)).unwrap();

    assert_eq!(service.get_currency(), Currency::USD);
    assert_eq!(preferences.values.lock().unwrap().len(), 0);
    assert_eq!(*prices.converted.lock().unwrap(), Vec::new());
    assert_eq!(provider.requested_paths(), Vec::<String>::new());
}
