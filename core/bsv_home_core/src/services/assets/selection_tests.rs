use std::sync::{Arc, Mutex};

use primitives::{Account, Chain, NFTData, VerificationStatus, Wallet};

use super::BsvHomeAssetSelectionService;
use crate::bsv_home_swapper::BsvHomeSwapper;
use crate::keystore::BsvHomeKeystore;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::perpetual::{BsvHomePerpetualService, testkit::UnusedPerpetualStore};
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::price::{BsvHomePriceService, testkit::MemoryPriceStore};
use crate::services::price_alert::testkit::service as price_alert_service;
use crate::services::search::{BsvHomeSearchService, testkit::UnusedSearchStore};
use crate::services::stream::testkit::SubscriptionTestkit;
use crate::services::swap::{BsvHomeSwapService, testkit::UnusedSwapStore};
use crate::services::transactions::testkit::TransactionsTestkit;
use crate::services::transfer::{BsvHomeRecentActivityService, testkit::MemoryRecentActivityStore};
use crate::services::wallet::{addresses::testkit::pool, testkit::MemoryKeystorePassword};
use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
use crate::testkit::TestAlienProvider;

fn selection_service(wallet: Wallet, environment: BsvHomeRuntimeEnvironment) -> BsvHomeAssetSelectionService {
    let provider = Arc::new(TestAlienProvider::with_status(503));
    let kit = TransactionsTestkit::new_with_environment(wallet.clone(), provider.clone(), environment);
    let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
        Arc::new(MemoryWalletSessionStore {
            current: Mutex::new(Some(wallet.id)),
        }),
        kit.wallet_store.clone(),
        environment,
    ));
    let subscriptions = SubscriptionTestkit::new(&[], &[]);
    let balances = Arc::new(BsvHomeBalanceService::new(
        kit.gateway.clone(),
        kit.wallet_store.clone(),
        kit.asset_store.clone(),
        subscriptions.balances,
        kit.assets.clone(),
        Arc::new(subscriptions.service),
        pool(kit.gateway.clone(), kit.wallet_store),
    ));
    let preferences_store = Arc::new(MemoryPreferencesStore::default());
    let preferences = Arc::new(BsvHomePreferencesService::new(preferences_store.clone()));
    let price = Arc::new(BsvHomePriceService::new(Arc::new(MemoryPriceStore::default()), preferences.clone()));
    let perpetual_store = Arc::new(UnusedPerpetualStore);
    let search = Arc::new(BsvHomeSearchService::new(
        kit.assets,
        kit.asset_store.clone(),
        price.clone(),
        perpetual_store.clone(),
        Arc::new(UnusedSearchStore),
    ));
    let perpetuals = Arc::new(BsvHomePerpetualService::new(
        kit.gateway,
        price,
        perpetual_store,
        kit.asset_store,
        preferences.clone(),
        balances.clone(),
        kit.wallet_preferences,
        session.clone(),
    ));
    let directory = tempfile::tempdir().unwrap();
    let swap = Arc::new(BsvHomeSwapService::new_with_environment(
        Arc::new(BsvHomeSwapper::new(provider.clone(), preferences_store)),
        BsvHomeKeystore::new_with_environment(directory.path().to_string_lossy().into_owned(), environment).unwrap(),
        Arc::new(MemoryKeystorePassword::default()),
        Arc::new(UnusedSwapStore),
        environment,
    ));
    BsvHomeAssetSelectionService::new(
        search,
        balances,
        Arc::new(price_alert_service(provider, vec![]).0),
        Arc::new(BsvHomeRecentActivityService::new(Arc::new(MemoryRecentActivityStore::default()), session.clone())),
        preferences,
        perpetuals,
        session,
        swap,
    )
}

#[test]
fn test_wowpay_selection_hides_custom_tokens_and_retained_matching_collections_while_production_keeps_them() {
    let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::Tron, "public-fixture")]);
    let collection = NFTData::mock_with("Saved collection", VerificationStatus::Verified, 2);
    let saved_collection = serde_json::to_value(&collection).unwrap();
    for (environment, supports_tokens, collection_count) in [(BsvHomeRuntimeEnvironment::WowPay, false, 0), (BsvHomeRuntimeEnvironment::Production, true, 1)] {
        let service = selection_service(wallet.clone(), environment);
        assert_eq!(service.supports_tokens(Some(wallet.clone())), supports_tokens);
        assert!(!service.supports_tokens(None));
        let items = service.search_collections(vec![collection.clone()], "saved".into());
        assert_eq!(items.len(), collection_count);
        assert_eq!(serde_json::to_value(&collection).unwrap(), saved_collection);
    }
}
