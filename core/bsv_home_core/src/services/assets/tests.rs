use std::sync::Arc;

use futures::executor::block_on;
use primitives::known_assets::{TRON_SHASTA_USDT, TRON_USDT};
use primitives::search::AssetList as SearchAssetList;
use primitives::{
    Account, Asset, AssetAssociation, AssetAssociationType, AssetFull, AssetId, Chain, Currency, NFTCollection, Perpetual, PerpetualBasic, PerpetualId, PerpetualProvider,
    PerpetualSearchData, SearchResponse, Wallet,
};

use super::{BsvHomeAssetStore, BsvHomeAssetsService, rules, testkit::MemoryAssetStore};
use crate::api::{BsvHomeApiClient, BsvHomeApiError};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::services::{
    chain::BsvHomeRuntimeEnvironment,
    preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore},
    price::{BsvHomePriceService, testkit::MemoryPriceStore},
    wallet::testkit::MemoryWalletStore,
    wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore},
};
use crate::testkit::TestAlienProvider;

fn service(environment: BsvHomeRuntimeEnvironment) -> (BsvHomeAssetsService, Arc<MemoryAssetStore>, Arc<TestAlienProvider>) {
    let provider = Arc::new(TestAlienProvider::with_status(503));
    service_with_provider(environment, provider)
}

fn service_with_provider(environment: BsvHomeRuntimeEnvironment, provider: Arc<TestAlienProvider>) -> (BsvHomeAssetsService, Arc<MemoryAssetStore>, Arc<TestAlienProvider>) {
    let preferences_store = Arc::new(MemoryPreferencesStore::default());
    let preferences = Arc::new(BsvHomePreferencesService::new(preferences_store.clone()));
    let store = Arc::new(MemoryAssetStore::default());
    let service = BsvHomeAssetsService::new(
        Arc::new(BsvHomeApiClient::new(provider.clone())),
        Arc::new(BsvHomeGateway::new_with_environment(
            provider.clone(),
            preferences_store.clone(),
            Arc::new(EmptyPreferences),
            environment,
        )),
        store.clone(),
        Arc::new(BsvHomePriceService::new(Arc::new(MemoryPriceStore::default()), preferences.clone())),
        preferences,
        Arc::new(BsvHomeWalletSessionService::new_with_environment(
            Arc::new(MemoryWalletSessionStore::default()),
            Arc::new(MemoryWalletStore::default()),
            environment,
        )),
    );
    (service, store, provider)
}

#[test]
fn test_sync_assets_requests_canonical_usd_prices_when_current_currency_is_different() {
    let provider = Arc::new(TestAlienProvider::with_json(200, "[]"));
    let (service, _, provider) = service_with_provider(BsvHomeRuntimeEnvironment::Production, provider);
    service.preferences.set_currency(Currency::CNY).unwrap();

    block_on(service.sync_assets(vec![Chain::Solana.as_asset_id()])).unwrap();

    assert_eq!(provider.requested_paths(), vec!["/v1/assets?currency=USD"]);
    assert_eq!(service.preferences.get_currency(), Currency::CNY);
}

#[test]
fn test_wowpay_asset_access_and_defaults_preserve_hidden_rows_without_provider_requests() {
    block_on(async {
        let (service, store, provider) = service(BsvHomeRuntimeEnvironment::WowPay);
        let mut impersonated = TRON_USDT.clone();
        impersonated.id = AssetId::from_token(Chain::Tron, "same-symbol-usdt");
        let hidden = vec![
            Asset::from_chain(Chain::Ethereum),
            Asset::from_chain(Chain::Tron),
            TRON_USDT.clone(),
            Asset::from_chain(Chain::TronShasta),
            TRON_SHASTA_USDT.clone(),
            impersonated,
            Asset {
                id: AssetId::from_token(Chain::BitcoinSv, "mnee"),
                ..Asset::from_chain(Chain::BitcoinSv)
            },
        ];
        let ids = hidden.iter().map(|asset| asset.id.clone()).collect::<Vec<_>>();
        store.save_assets(hidden.clone().into_iter().map(rules::default_asset_basic).collect()).await.unwrap();
        service.ensure_default_assets().await.unwrap();
        let allowed = vec![Asset::from_chain(Chain::BitcoinSv)];
        for asset in &allowed {
            assert_eq!(service.ensure_asset(asset.id.clone()).await.unwrap(), *asset);
        }
        let wallet = Wallet::mock_with_accounts(Account::mock_chains(&[Chain::BitcoinSv, Chain::Tron, Chain::Ethereum, Chain::TronShasta], "public-fixture"));
        for id in &ids {
            assert!(service.ensure_asset(id.clone()).await.is_err());
            assert!(service.ensure_token_asset(id.clone()).await.is_err());
            assert!(service.get_asset(id.clone()).await.is_err());
            assert!(service.get_assets(vec![id.clone()], None).await.is_err());
            assert!(service.assets(vec![id.clone()]).await.is_err());
            assert!(service.sync_asset(id.clone()).await.is_err());
            assert!(service.open_wallet_asset(wallet.clone(), id.clone()).await.is_err());
        }
        assert_eq!(service.sync_missing_assets(ids.clone()).await.unwrap(), Vec::<AssetId>::new());
        assert_eq!(store.get_assets(ids).await.unwrap(), hidden);
        assert_eq!(provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_wowpay_search_scopes_empty_chains_and_filters_remote_products() {
    block_on(async {
        let known = vec![Asset::from_chain(Chain::BitcoinSv)];
        let mut same_symbol = TRON_USDT.clone();
        same_symbol.id = AssetId::from_token(Chain::Tron, "different-contract");
        let response = SearchResponse {
            assets: [
                known.clone(),
                vec![
                    same_symbol,
                    Asset::from_chain(Chain::Ethereum),
                    Asset::from_chain(Chain::Tron),
                    TRON_USDT.clone(),
                    TRON_SHASTA_USDT.clone(),
                ],
            ]
            .concat()
            .into_iter()
            .map(rules::default_asset_basic)
            .collect(),
            perpetuals: vec![PerpetualSearchData {
                perpetual: Perpetual {
                    id: PerpetualId::new(PerpetualProvider::Hypercore, "BTC"),
                    name: "BTC".into(),
                    provider: PerpetualProvider::Hypercore,
                    asset_id: Chain::HyperCore.as_asset_id(),
                    identifier: "BTC".into(),
                    price: 1.0,
                    price_percent_change_24h: 0.0,
                    open_interest: 0.0,
                    volume_24h: 0.0,
                    funding: 0.0,
                    max_leverage: 1,
                    is_isolated_only: false,
                },
                asset: Asset::from_chain(Chain::HyperCore),
            }],
            nfts: vec![NFTCollection::mock()],
            lists: vec![SearchAssetList {
                id: "other-products".into(),
                name: "Other products".into(),
                count: 2,
            }],
        };
        let body = serde_json::to_string(&response).unwrap();
        let provider = Arc::new(TestAlienProvider::with_json(200, &body));
        let (service, _, _) = service_with_provider(BsvHomeRuntimeEnvironment::WowPay, provider.clone());
        let result = service.search(String::new(), Vec::new(), Vec::new()).await.unwrap();
        assert_eq!(result.assets.into_iter().map(|basic| basic.asset).collect::<Vec<_>>(), known);
        assert_eq!((result.perpetuals.len(), result.nfts.len(), result.lists.len()), (0, 0, 0));
        assert_eq!(provider.requested_paths(), Vec::<String>::new());
        assert!(service.search(String::new(), vec![Chain::Tron], Vec::new()).await.is_err());
        let requests = provider.requested_paths();
        assert_eq!(service.search_tokens("different-contract".into(), vec![Chain::Tron]).await.len(), 0);
        assert_eq!(provider.requested_paths(), requests);
        let local = service.search_assets("bsv".into(), vec![Chain::BitcoinSv]).await.unwrap();
        assert_eq!(local.into_iter().map(|basic| basic.asset).collect::<Vec<_>>(), vec![Asset::from_chain(Chain::BitcoinSv)]);
        assert_eq!(provider.requested_paths(), requests);
    });
}

#[test]
fn test_wowpay_remote_metadata_cannot_cache_unrequested_assets_or_expose_hidden_associations() {
    block_on(async {
        let mut hidden = TRON_USDT.clone();
        hidden.id = AssetId::from_token(Chain::Tron, "same-symbol-unrequested");
        let unrequested = vec![hidden.clone(), Asset::from_chain(Chain::Ethereum), Asset::from_chain(Chain::Tron)];
        let metadata = [unrequested.clone(), vec![TRON_USDT.clone()]]
            .concat()
            .into_iter()
            .map(rules::default_asset_basic)
            .collect::<Vec<_>>();
        let provider = Arc::new(TestAlienProvider::with_json(200, &serde_json::to_string(&metadata).unwrap()));
        let (service, store, provider) = service_with_provider(BsvHomeRuntimeEnvironment::Production, provider);
        assert_eq!(service.ensure_asset(TRON_USDT.id.clone()).await.unwrap(), TRON_USDT.clone());
        assert_eq!(
            store.get_assets(unrequested.into_iter().map(|asset| asset.id).collect()).await.unwrap(),
            Vec::<Asset>::new()
        );
        assert_eq!(provider.requested_paths().len(), 1);

        let basic = rules::default_asset_basic(TRON_USDT.clone());
        let full = AssetFull {
            asset: basic.asset,
            properties: basic.properties,
            score: basic.score,
            tags: Vec::new(),
            links: Vec::new(),
            associations: vec![
                AssetAssociation {
                    asset_id: hidden.id.clone(),
                    association_type: AssetAssociationType::Official,
                },
                AssetAssociation {
                    asset_id: Chain::BitcoinSv.as_asset_id(),
                    association_type: AssetAssociationType::Official,
                },
                AssetAssociation {
                    asset_id: Chain::Ethereum.as_asset_id(),
                    association_type: AssetAssociationType::Official,
                },
            ],
            perpetuals: vec![PerpetualBasic {
                asset_id: Chain::HyperCore.as_asset_id(),
                perpetual_id: PerpetualId::new(PerpetualProvider::Hypercore, "BTC"),
                provider: PerpetualProvider::Hypercore,
            }],
            price: None,
            market: None,
        };
        let provider = Arc::new(TestAlienProvider::with_json(200, &serde_json::to_string(&full).unwrap()));
        let (service, _, provider) = service_with_provider(BsvHomeRuntimeEnvironment::WowPay, provider);
        let result = service.get_asset(Chain::BitcoinSv.as_asset_id()).await.unwrap();
        assert_eq!(result.asset, Asset::from_chain(Chain::BitcoinSv));
        assert_eq!(
            result.associations.into_iter().map(|association| association.asset_id).collect::<Vec<_>>(),
            Vec::<AssetId>::new()
        );
        assert_eq!(result.perpetuals.len(), 0);
        assert!(service.get_asset(TRON_USDT.id.clone()).await.is_err());
        assert_eq!(provider.requested_paths(), Vec::<String>::new());

        let mismatched = AssetFull { asset: hidden, ..full };
        let provider = Arc::new(TestAlienProvider::with_json(200, &serde_json::to_string(&mismatched).unwrap()));
        let (service, _, _) = service_with_provider(BsvHomeRuntimeEnvironment::Production, provider);
        assert!(service.get_asset(TRON_USDT.id.clone()).await.is_err());
    });
}

#[test]
fn test_development_asset_metadata_is_local_and_unknown_test_tokens_never_reach_the_backend() {
    block_on(async {
        for environment in [BsvHomeRuntimeEnvironment::Production, BsvHomeRuntimeEnvironment::LocalDevelopment] {
            let (service, store, provider) = service(environment);
            let supported = [Asset::from_chain(Chain::TronShasta), TRON_SHASTA_USDT.clone()];
            for expected in &supported {
                let result = service.get_asset(expected.id.clone()).await;
                if environment.allows_chain(Chain::TronShasta) {
                    let asset = result.unwrap();
                    assert_eq!(asset.asset, *expected);
                    assert!(asset.price.is_none());
                    assert!(asset.market.is_none());
                } else {
                    assert!(matches!(result, Err(BsvHomeApiError::Unsupported { .. })));
                }
            }
            let unknown = AssetId::from_token(Chain::TronShasta, "unknown-test-contract");
            let mut cached = TRON_SHASTA_USDT.clone();
            cached.id = unknown.clone();
            store.save_assets(vec![rules::default_asset_basic(cached.clone())]).await.unwrap();
            assert!(matches!(service.get_asset(unknown.clone()).await, Err(BsvHomeApiError::Unsupported { .. })));
            assert!(matches!(service.get_assets(vec![unknown.clone()], None).await, Err(BsvHomeApiError::Unsupported { .. })));
            assert!(service.ensure_asset(unknown.clone()).await.is_err());
            assert!(service.ensure_token_asset(unknown.clone()).await.is_err());
            assert!(service.assets(vec![unknown.clone()]).await.is_err());
            assert_eq!(store.get_assets(vec![unknown]).await.unwrap(), vec![cached]);
            assert_eq!(provider.requested_paths(), Vec::<String>::new());
        }
    });
}

#[test]
fn test_production_asset_access_rejects_retained_test_rows_without_removing_them() {
    block_on(async {
        let (service, store, provider) = service(BsvHomeRuntimeEnvironment::Production);
        let assets = vec![Asset::from_chain(Chain::TronShasta), TRON_SHASTA_USDT.clone()];
        store.save_assets(assets.clone().into_iter().map(rules::default_asset_basic).collect()).await.unwrap();
        let ids = assets.iter().map(|asset| asset.id.clone()).collect::<Vec<_>>();
        assert!(service.assets(ids.clone()).await.is_err());
        for id in &ids {
            assert!(service.ensure_asset(id.clone()).await.is_err());
            assert!(service.ensure_token_asset(id.clone()).await.is_err());
        }
        assert_eq!(store.get_assets(ids).await.unwrap(), assets);
        assert_eq!(provider.requested_paths(), Vec::<String>::new());
    });
}
