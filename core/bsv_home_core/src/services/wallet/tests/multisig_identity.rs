use std::sync::Arc;
use std::sync::atomic::Ordering;

use futures::executor::block_on;
use primitives::{Account, Chain, Wallet, WalletSource};

use super::{BsvHomeRuntimeEnvironment, BsvHomeWalletImportResult, BsvHomeWalletImportType, OTHER_PHRASE, PHRASE, TestContext};
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::wallet::store::BsvHomeWalletStore;

#[test]
fn test_dedicated_creation_preserves_personal_selection_and_restore_keeps_classification() {
    block_on(async {
        let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
        let personal = import_bsv(&context, "Personal", PHRASE).await;
        context.service.set_current_wallet_id(personal.id.clone()).unwrap();
        let result = context
            .service
            .import_wallet(
                "Signing identity".into(),
                BsvHomeWalletImportType::MulticoinPhrase {
                    words: OTHER_PHRASE.map(str::to_string).to_vec(),
                    chains: vec![Chain::BitcoinSv],
                },
                WalletSource::Create,
            )
            .await
            .unwrap();
        let dedicated = context.service.complete_multisig_identity_import(result, WalletSource::Create).unwrap();
        assert_eq!(context.service.current_wallet_id().unwrap(), Some(personal.id.clone()));
        assert!(context.service.is_multisig_identity(dedicated.id.clone()));
        assert!(!context.service.is_multisig_identity(personal.id.clone()));
        assert_eq!(
            context
                .service
                .sorted_wallets(context.wallets.get_wallets().await.unwrap())
                .into_iter()
                .map(|wallet| wallet.id)
                .collect::<Vec<_>>(),
            vec![personal.id.clone()]
        );

        context.service.delete_wallet(dedicated.id.clone()).await.unwrap();
        assert!(context.service.is_multisig_identity(dedicated.id.clone()));
        let restored = import_bsv(&context, "Recovered identity", OTHER_PHRASE).await;
        assert_eq!(restored.id, dedicated.id);
        assert!(context.service.is_multisig_identity(restored.id.clone()));
        context
            .service
            .complete_multisig_identity_import(BsvHomeWalletImportResult::Existing { wallet: restored.clone() }, WalletSource::Import)
            .unwrap();
        assert_eq!(context.service.current_wallet_id().unwrap(), Some(restored.id));

        context
            .service
            .complete_multisig_identity_import(BsvHomeWalletImportResult::Existing { wallet: personal.clone() }, WalletSource::Import)
            .unwrap();
        assert!(!context.service.is_multisig_identity(personal.id.clone()));
        assert_eq!(context.service.current_wallet_id().unwrap(), Some(personal.id));
    });
}

#[test]
fn test_first_dedicated_identity_becomes_current_without_a_personal_row() {
    block_on(async {
        let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
        let wallet = import_bsv(&context, "First identity", PHRASE).await;
        context
            .service
            .complete_multisig_identity_import(BsvHomeWalletImportResult::New { wallet: wallet.clone() }, WalletSource::Create)
            .unwrap();
        assert_eq!(context.service.current_wallet_id().unwrap(), Some(wallet.id.clone()));
        assert!(context.service.sorted_wallets(vec![wallet]).is_empty());
        assert_eq!(context.service.wallets().await.unwrap().len(), 1);
    });
}

#[test]
fn test_dedicated_metadata_failure_keeps_current_and_retries_the_same_import_result() {
    block_on(async {
        let mut context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
        let personal = import_bsv(&context, "Personal", PHRASE).await;
        context.service.set_current_wallet_id(personal.id.clone()).unwrap();
        let wallet = import_bsv(&context, "Signing identity", OTHER_PHRASE).await;
        let result = BsvHomeWalletImportResult::New { wallet: wallet.clone() };
        let preferences = Arc::new(MemoryPreferencesStore::default());
        context.service.app_preferences = Arc::new(BsvHomePreferencesService::new(preferences.clone()));
        preferences.fail_set.store(true, Ordering::SeqCst);
        assert_eq!(
            context.service.complete_multisig_identity_import(result.clone(), WalletSource::Import).unwrap_err(),
            BsvHomeServiceError::Store {
                msg: "preference write failed".into()
            }
        );
        assert_eq!(context.service.current_wallet_id().unwrap(), Some(personal.id));
        assert!(!context.service.is_multisig_identity(wallet.id.clone()));

        preferences.fail_set.store(false, Ordering::SeqCst);
        let completed = context.service.complete_multisig_identity_import(result, WalletSource::Import).unwrap();
        assert_eq!(completed.id, wallet.id);
        assert_eq!(context.service.current_wallet_id().unwrap(), Some(wallet.id.clone()));
        assert!(context.service.is_multisig_identity(wallet.id));
        assert_eq!(context.wallets.get_wallets().await.unwrap().len(), 2);
    });
}

#[test]
fn test_dedicated_completion_rejects_watch_foreign_and_retained_environment() {
    let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
    for wallet in [
        Wallet::mock_with_accounts(vec![Account::mock(Chain::Ethereum, "public")]),
        super::super::rules::view_wallet("Watch".into(), Chain::BitcoinSv, "1BitcoinEaterAddressDontSendf59kuE".into()),
    ] {
        let result = context
            .service
            .complete_multisig_identity_import(BsvHomeWalletImportResult::New { wallet: wallet.clone() }, WalletSource::Import);
        assert_eq!(
            result.err(),
            Some(BsvHomeServiceError::InvalidInput {
                msg: "a signing identity requires a local BSV wallet".into()
            })
        );
        assert!(!context.service.is_multisig_identity(wallet.id));
        assert_eq!(context.service.current_wallet_id().unwrap(), None);
    }
    let retained = TestContext::new();
    let result = retained.service.complete_multisig_identity_import(
        BsvHomeWalletImportResult::New {
            wallet: Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, "1BitcoinEaterAddressDontSendf59kuE")]),
        },
        WalletSource::Create,
    );
    assert_eq!(
        result.err(),
        Some(BsvHomeServiceError::Unsupported {
            msg: "dedicated multisig identities are unavailable in this environment".into()
        })
    );
}

async fn import_bsv(context: &TestContext, name: &str, words: [&str; 12]) -> Wallet {
    let result = context
        .service
        .import_wallet(
            name.into(),
            BsvHomeWalletImportType::MulticoinPhrase {
                words: words.map(str::to_string).to_vec(),
                chains: vec![Chain::BitcoinSv],
            },
            WalletSource::Import,
        )
        .await
        .unwrap();
    match result {
        BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
    }
}
