use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use primitives::{Account, Chain, Wallet, WalletId, WalletType};

use super::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::wallet::{BsvHomeWalletStore, testkit::MemoryWalletStore};

#[test]
fn test_session_hides_test_network_wallet_identity_and_preserves_raw_wallets_and_selection() {
    block_on(async {
        let mainnet = Wallet {
            id: WalletId::Single(Chain::Tron, "mainnet".into()),
            wallet_type: WalletType::Single,
            ..Wallet::mock_with_accounts(vec![Account::mock(Chain::Tron, "mainnet"), Account::mock(Chain::TronShasta, "mainnet")])
        };
        let development = Wallet {
            id: WalletId::Single(Chain::TronShasta, "development".into()),
            wallet_type: WalletType::Single,
            ..Wallet::mock_with_accounts(vec![Account::mock(Chain::TronShasta, "development")])
        };
        let private = Wallet {
            id: WalletId::PrivateKey(Chain::TronShasta, "private".into()),
            wallet_type: WalletType::PrivateKey,
            ..Wallet::mock_with_accounts(vec![Account::mock(Chain::TronShasta, "private")])
        };
        let wallets = Arc::new(MemoryWalletStore {
            wallets: Mutex::new(vec![mainnet.clone(), development.clone(), private.clone()]),
            ..Default::default()
        });
        let stored_before = serde_json::to_value(wallets.get_wallets().await.unwrap()).unwrap();
        let selection = Arc::new(MemoryWalletSessionStore {
            current: Mutex::new(Some(development.id.clone())),
        });
        let production = BsvHomeWalletSessionService::new(selection.clone(), wallets.clone());

        assert_eq!(
            production
                .get_wallets()
                .await
                .unwrap()
                .into_iter()
                .map(|wallet| (wallet.id.clone(), wallet.chains()))
                .collect::<Vec<_>>(),
            vec![(mainnet.id.clone(), vec![Chain::Tron])]
        );
        assert!(production.get_wallet(development.id.clone()).await.unwrap().is_none());
        assert!(production.get_current_wallet().await.unwrap().is_none());
        assert!(production.current_wallet().await.is_err());
        assert_eq!(production.get_current_wallet_id().unwrap(), Some(development.id.clone()));

        let local = BsvHomeWalletSessionService::new_with_environment(selection, wallets.clone(), BsvHomeRuntimeEnvironment::LocalDevelopment);
        assert_eq!(local.get_wallet(development.id.clone()).await.unwrap().is_some(), cfg!(debug_assertions));
        assert!(local.get_wallet(private.id).await.unwrap().is_none());
        assert_eq!(serde_json::to_value(wallets.get_wallets().await.unwrap()).unwrap(), stored_before);
    });
}

#[test]
fn test_wowpay_session_projects_accounts_and_hides_unsupported_wallets_without_migrating_selection() {
    block_on(async {
        let visible = Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Ethereum, Chain::BitcoinSv, Chain::Tron, Chain::TronShasta], "public-fixture"));
        let unsupported = Wallet {
            id: WalletId::Single(Chain::Ethereum, "unsupported".into()),
            wallet_type: WalletType::Single,
            ..Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Ethereum], "unsupported"))
        };
        let unsupported_multicoin = Wallet {
            id: WalletId::Multicoin("no-visible-accounts".into()),
            ..Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Ethereum], "no-visible-accounts"))
        };
        let wallets = Arc::new(MemoryWalletStore {
            wallets: Mutex::new(vec![visible.clone(), unsupported.clone(), unsupported_multicoin]),
            ..Default::default()
        });
        let raw_before = serde_json::to_value(wallets.get_wallets().await.unwrap()).unwrap();
        let selection = Arc::new(MemoryWalletSessionStore {
            current: Mutex::new(Some(unsupported.id.clone())),
        });
        let service = BsvHomeWalletSessionService::new_with_environment(selection, wallets.clone(), BsvHomeRuntimeEnvironment::WowPay);
        let projected = service.get_wallets().await.unwrap();
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].id, visible.id);
        assert_eq!(projected[0].chains(), vec![Chain::BitcoinSv]);
        assert_eq!(service.get_current_wallet_id().unwrap(), Some(unsupported.id.clone()));
        assert!(service.get_current_wallet().await.unwrap().is_none());
        assert!(service.get_wallet(unsupported.id).await.unwrap().is_none());
        assert_eq!(serde_json::to_value(wallets.get_wallets().await.unwrap()).unwrap(), raw_before);
    });
}
