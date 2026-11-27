use std::sync::{Arc, Mutex};

use bsv_home_derivation::{bsv_account_xpub, derive_accounts_from_mnemonic};
use primitives::testkit::ABANDON_PHRASE;
use primitives::{Account, AssetFiatValue, Chain, PerpetualBalance, TotalFiatValue, Wallet, WalletId, WalletType};
use serde_json::to_value;

use super::BsvHomeWalletHomeService;
use crate::api::BsvHomeDeviceApiClient;
use crate::gateway::EmptyPreferences;
use crate::services::asset_discovery::BsvHomeAssetDiscoveryService;
use crate::services::assets::model::{BsvHomeHeaderActions, BsvHomeHeaderButton, BsvHomeHeaderButtonKind};
use crate::services::balance::BsvHomeBalanceService;
use crate::services::banner::{BsvHomeBannerService, testkit::MemoryBannerStore};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::nft::{BsvHomeNftService, testkit::MemoryNftStore};
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::stream::testkit::SubscriptionTestkit;
use crate::services::transactions::testkit::TransactionsTestkit;
use crate::services::wallet::{addresses::testkit::pool, testkit::MemoryWalletStore};
use crate::services::wallet_preferences::{BsvHomeWalletPreferencesService, testkit::MemoryWalletPreferencesStore};
use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
use crate::testkit::TestAlienProvider;

fn home_service(wallet: Wallet, environment: BsvHomeRuntimeEnvironment) -> (BsvHomeWalletHomeService, Arc<MemoryWalletStore>, Arc<MemoryWalletPreferencesStore>) {
    let provider = Arc::new(TestAlienProvider::with_status(503));
    let kit = TransactionsTestkit::new_with_environment(wallet.clone(), provider.clone(), environment);
    let subscriptions = SubscriptionTestkit::new(&[], &[]);
    let api = Arc::new(BsvHomeDeviceApiClient::new(provider, Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences)))));
    let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
        Arc::new(MemoryWalletSessionStore {
            current: Mutex::new(Some(wallet.id)),
        }),
        kit.wallet_store.clone(),
        environment,
    ));
    let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
    preferences.set_perpetual_enabled(true).unwrap();
    let preference_store = Arc::new(MemoryWalletPreferencesStore::default());
    let wallet_preferences = Arc::new(BsvHomeWalletPreferencesService::new(preference_store.clone()));
    let balance = Arc::new(BsvHomeBalanceService::new(
        kit.gateway.clone(),
        kit.wallet_store.clone(),
        kit.asset_store.clone(),
        subscriptions.balances,
        kit.assets.clone(),
        Arc::new(subscriptions.service),
        pool(kit.gateway, kit.wallet_store.clone()),
    ));
    let discovery = Arc::new(BsvHomeAssetDiscoveryService::new(
        api.clone(),
        balance.clone(),
        kit.service,
        Arc::new(BsvHomeNftService::new(api, Arc::new(MemoryNftStore::default()), session.clone())),
        kit.wallet_store.clone(),
        wallet_preferences.clone(),
    ));
    (
        BsvHomeWalletHomeService::new(
            balance,
            kit.assets,
            discovery,
            Arc::new(BsvHomeBannerService::new(Arc::new(MemoryBannerStore::default()))),
            wallet_preferences,
            preferences,
            session,
        ),
        kit.wallet_store,
        preference_store,
    )
}

#[test]
fn test_wowpay_home_excludes_retained_perpetual_collateral_without_changing_saved_wallet_or_preferences() {
    let wallet = Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Tron, Chain::HyperCore], "public-fixture"));
    let balances = vec![AssetFiatValue {
        amount: 2.0,
        price: 10.0,
        price_change_percentage_24h: 0.0,
    }];
    let perpetual = PerpetualBalance {
        available: 30.0,
        reserved: 5.0,
        withdrawable: 30.0,
    };
    let saved_perpetual = serde_json::to_value(&perpetual).unwrap();
    for (environment, value) in [(BsvHomeRuntimeEnvironment::WowPay, 20.0), (BsvHomeRuntimeEnvironment::Production, 55.0)] {
        let (service, wallets, preferences) = home_service(wallet.clone(), environment);
        let saved_wallets = serde_json::to_value(&*wallets.wallets.lock().unwrap()).unwrap();
        let saved_preferences = preferences.values.lock().unwrap().clone();
        let state = service.view_state(wallet.clone(), balances.clone(), Some(perpetual.clone()), vec![], false);
        assert_eq!(
            state.total_value,
            TotalFiatValue {
                value,
                pnl_amount: 0.0,
                pnl_percentage: 0.0
            }
        );
        assert_eq!(serde_json::to_value(&perpetual).unwrap(), saved_perpetual);
        assert_eq!(serde_json::to_value(&*wallets.wallets.lock().unwrap()).unwrap(), saved_wallets);
        assert_eq!(*preferences.values.lock().unwrap(), saved_preferences);
        assert_eq!(*preferences.writes.lock().unwrap(), 0);
    }
}

#[test]
fn test_wowpay_home_header_and_collections_use_only_visible_accounts() {
    let wallet = Wallet {
        id: WalletId::Single(Chain::BitcoinSv, "public-fixture".into()),
        wallet_type: WalletType::Single,
        ..Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Ethereum, Chain::BitcoinSv], "public-fixture"))
    };
    for (environment, expected_kinds, show_collections) in [
        (
            BsvHomeRuntimeEnvironment::WowPay,
            vec![BsvHomeHeaderButtonKind::Send, BsvHomeHeaderButtonKind::Receive, BsvHomeHeaderButtonKind::Buy],
            false,
        ),
        (
            BsvHomeRuntimeEnvironment::Production,
            vec![
                BsvHomeHeaderButtonKind::Send,
                BsvHomeHeaderButtonKind::Receive,
                BsvHomeHeaderButtonKind::Buy,
                BsvHomeHeaderButtonKind::Swap,
            ],
            true,
        ),
    ] {
        let (service, _, _) = home_service(wallet.clone(), environment);
        let state = service.view_state(wallet.clone(), vec![], None, vec![], true);
        assert_eq!(
            state.header_actions,
            BsvHomeHeaderActions::Buttons {
                buttons: expected_kinds.into_iter().map(|kind| BsvHomeHeaderButton { kind, is_enabled: true }).collect()
            }
        );
        assert_eq!(state.show_collections, show_collections);
    }
}

#[test]
fn test_wowpay_home_allows_public_watch_receiving_without_enabling_a_signer() {
    let wallet = Wallet {
        id: WalletId::View(Chain::BitcoinSv, "1BitcoinEaterAddressDontSendf59kuE".into()),
        wallet_type: WalletType::View,
        ..Wallet::mock_with_accounts(Account::mock_chains(&[Chain::BitcoinSv], "1BitcoinEaterAddressDontSendf59kuE"))
    };
    for (environment, actions) in [
        (
            BsvHomeRuntimeEnvironment::WowPay,
            BsvHomeHeaderActions::Buttons {
                buttons: vec![BsvHomeHeaderButton {
                    kind: BsvHomeHeaderButtonKind::Receive,
                    is_enabled: true,
                }],
            },
        ),
        (BsvHomeRuntimeEnvironment::Production, BsvHomeHeaderActions::WatchOnly),
    ] {
        let (service, wallets, _) = home_service(wallet.clone(), environment);
        let before = serde_json::to_value(&*wallets.wallets.lock().unwrap()).unwrap();
        let state = service.view_state(wallet.clone(), vec![], None, vec![], true);
        assert_eq!(state.header_actions, actions);
        assert_eq!(serde_json::to_value(&*wallets.wallets.lock().unwrap()).unwrap(), before);
    }
}

#[test]
fn test_wowpay_home_cold_observer_send_precedes_plain_watch_receive_only() {
    let mut account = derive_accounts_from_mnemonic(ABANDON_PHRASE, vec![Chain::BitcoinSv]).unwrap().remove(0);
    account.extended_public_key = Some(bsv_account_xpub(ABANDON_PHRASE).unwrap());
    let mut wallet = Wallet {
        id: WalletId::View(Chain::BitcoinSv, account.address.clone()),
        wallet_type: WalletType::View,
        accounts: vec![account],
        ..Wallet::mock()
    };
    let (service, wallets, _) = home_service(wallet.clone(), BsvHomeRuntimeEnvironment::WowPay);
    let before = to_value(&*wallets.wallets.lock().unwrap()).unwrap();
    let state = service.view_state(wallet.clone(), vec![], None, vec![], true);
    assert_eq!(
        state.header_actions,
        BsvHomeHeaderActions::Buttons {
            buttons: vec![
                BsvHomeHeaderButton {
                    kind: BsvHomeHeaderButtonKind::Send,
                    is_enabled: true
                },
                BsvHomeHeaderButton {
                    kind: BsvHomeHeaderButtonKind::Receive,
                    is_enabled: true
                }
            ]
        }
    );
    wallet.accounts[0].extended_public_key = None;
    let state = service.view_state(wallet, vec![], None, vec![], true);
    assert_eq!(
        state.header_actions,
        BsvHomeHeaderActions::Buttons {
            buttons: vec![BsvHomeHeaderButton {
                kind: BsvHomeHeaderButtonKind::Receive,
                is_enabled: true
            }]
        }
    );
    assert_eq!(to_value(&*wallets.wallets.lock().unwrap()).unwrap(), before);
}
