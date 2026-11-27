use super::testkit::{TEST_BSV_ADDRESS, TEST_BSV_HASH, TEST_BSV_PREVIOUS_HASH, TransactionsTestkit, bsv_history_responses, bsv_response};
use crate::alien::AlienResponse;
use crate::config::public::API_URL;
use crate::services::{chain::BsvHomeRuntimeEnvironment, error::BsvHomeServiceError};
use crate::testkit::TestAlienProvider;
use futures::executor::block_on;
use primitives::known_assets::{TRON_SHASTA_USDT, TRON_USDT};
use primitives::{Account, AssetId, Chain, Transaction, TransactionDirection, TransactionState, TransactionsResponse, Wallet};
use serde_json::Value;
use std::sync::Arc;

async fn bsv_testkit(provider: Arc<TestAlienProvider>) -> TransactionsTestkit {
    let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, TEST_BSV_ADDRESS)]);
    let kit = TransactionsTestkit::new(wallet, provider);
    kit.assets.ensure_default_assets().await.unwrap();
    kit
}

fn mixed_wallet() -> Wallet {
    Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, TEST_BSV_ADDRESS), Account::mock(Chain::Ethereum, "0xpublicfixture")])
}

#[test]
fn test_wowpay_history_selections_reject_other_tokens_without_changing_cached_history() {
    block_on(async {
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let wallet = Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Ethereum, Chain::Tron, Chain::BitcoinSv, Chain::TronShasta], "public-fixture"));
        let kit = TransactionsTestkit::new_with_environment(wallet.clone(), provider.clone(), BsvHomeRuntimeEnvironment::WowPay);
        let cached = vec![Transaction::mock()];
        *kit.store.transactions.lock().unwrap() = cached.clone();
        for asset_id in [
            Chain::Ethereum.as_asset_id(),
            Chain::Tron.as_asset_id(),
            TRON_USDT.id.clone(),
            Chain::TronShasta.as_asset_id(),
            TRON_SHASTA_USDT.id.clone(),
            AssetId::from_token(Chain::Tron, "other-usdt"),
            AssetId::from_token(Chain::BitcoinSv, "mnee"),
        ] {
            assert!(kit.service.sync_wallet(kit.wallet_id.clone(), Some(asset_id)).await.is_err());
        }
        assert_eq!(kit.service.filter_chains(wallet), vec![Chain::BitcoinSv]);
        assert_eq!(*kit.store.transactions.lock().unwrap(), cached);
        assert_eq!(kit.store.saved.lock().unwrap().len(), 0);
        assert_eq!(provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_explicit_history_missing_chain_fails_before_provider_access_without_touching_cache() {
    block_on(async {
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::Ethereum, "0xpublicfixture")]);
        let kit = TransactionsTestkit::new_with_environment(wallet, provider.clone(), BsvHomeRuntimeEnvironment::LocalDevelopment);
        let cached = vec![Transaction::mock()];
        *kit.store.transactions.lock().unwrap() = cached.clone();
        for chain in [Chain::BitcoinSv, Chain::TronShasta, Chain::Tron] {
            assert_eq!(
                kit.service.sync_wallet(kit.wallet_id.clone(), Some(chain.as_asset_id())).await.unwrap_err(),
                BsvHomeServiceError::InvalidInput {
                    msg: format!("wallet has no {chain} account for transaction history"),
                }
            );
        }
        assert_eq!(*kit.store.transactions.lock().unwrap(), cached);
        assert_eq!(kit.store.saved.lock().unwrap().len(), 0);
        assert_eq!(provider.requested_paths(), Vec::<String>::new());
    });
}

fn bsv_home_response(timestamp: u64, status: u16, transactions: Vec<Transaction>) -> (String, AlienResponse) {
    let body = if status == 200 {
        serde_json::to_vec(&TransactionsResponse::new(transactions, Vec::new())).unwrap()
    } else {
        br#"{"error":{"message":"history unavailable"}}"#.to_vec()
    };
    (
        format!("{API_URL}/v2/devices/transactions?from_timestamp={timestamp}&limit=100&offset=0"),
        AlienResponse::new(Some(status), body),
    )
}

#[test]
fn test_production_transaction_sync_skips_retained_test_accounts_and_rejects_explicit_test_assets_without_egress() {
    block_on(async {
        let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::TronShasta, "retained-development")]);
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let kit = TransactionsTestkit::new(wallet.clone(), provider.clone());
        kit.service.sync_wallet(wallet.id.clone(), None).await.unwrap();
        assert!(kit.service.sync_wallet(wallet.id, Some(Chain::TronShasta.as_asset_id())).await.is_err());
        assert_eq!(kit.store.transactions.lock().unwrap().len(), 0);
        assert_eq!(provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_bsv_history_deduplicates_hashes_and_reaches_existing_store_without_backend_indexing() {
    block_on(async {
        let provider = Arc::new(TestAlienProvider::with_responses(bsv_history_responses(false)));
        let kit = bsv_testkit(provider.clone()).await;
        kit.service.sync_wallet(kit.wallet_id.clone(), Some(Chain::BitcoinSv.as_asset_id())).await.unwrap();
        let stored = kit.store.transactions.lock().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].value.to_string(), "5000");
        assert_eq!(stored[0].fee.to_string(), "200");
        assert_eq!(stored[0].direction, TransactionDirection::Incoming);
        assert_eq!(stored[0].asset_id, Chain::BitcoinSv.as_asset_id());
        assert_eq!(stored[0].state, TransactionState::Confirmed);
        assert_eq!(kit.store.saved.lock().unwrap().len(), 1);
        assert_eq!(*kit.status.0.lock().unwrap(), Vec::new());
        assert_eq!(
            provider.requested_paths(),
            vec![
                "/v1/bsv/main/addresses/history/all".to_string(),
                format!("/v1/bsv/main/tx/hash/{TEST_BSV_HASH}"),
                format!("/v1/bsv/main/tx/hash/{TEST_BSV_PREVIOUS_HASH}"),
            ]
        );
    });
}

#[test]
fn test_bsv_empty_history_is_saved_without_backend_indexing() {
    block_on(async {
        let history: Value = serde_json::from_str(include_str!("../../../testdata/bsv/address_history_empty.json")).unwrap();
        let provider = Arc::new(TestAlienProvider::with_responses(vec![bsv_response("/addresses/history/all", history)]));
        let kit = bsv_testkit(provider.clone()).await;
        kit.service.sync_wallet(kit.wallet_id.clone(), Some(Chain::BitcoinSv.as_asset_id())).await.unwrap();
        assert_eq!(*kit.store.transactions.lock().unwrap(), Vec::new());
        assert_eq!(kit.store.saved.lock().unwrap().len(), 1);
        assert_eq!(*kit.status.0.lock().unwrap(), Vec::new());
        assert_eq!(provider.requested_paths(), vec!["/v1/bsv/main/addresses/history/all".to_string()]);
    });
}

#[test]
fn test_bsv_history_errors_preserve_stored_transactions() {
    block_on(async {
        let mut history: Value = serde_json::from_str(include_str!("../../../testdata/bsv/address_history_empty.json")).unwrap();
        history[0]["confirmed"]["error"] = Value::String("supplier unavailable".to_string());
        let cases = [
            (TestAlienProvider::with_status(404), "Network error: HTTP error: status 404"),
            (
                TestAlienProvider::with_responses(vec![bsv_response("/addresses/history/all", history)]),
                "Network error: invalid WhatsOnChain address history response",
            ),
        ];
        for (provider, expected_error) in cases {
            let provider = Arc::new(provider);
            let kit = bsv_testkit(provider.clone()).await;
            let existing = vec![Transaction::mock()];
            *kit.store.transactions.lock().unwrap() = existing.clone();
            assert_eq!(
                kit.service
                    .sync_wallet(kit.wallet_id.clone(), Some(Chain::BitcoinSv.as_asset_id()))
                    .await
                    .unwrap_err()
                    .to_string(),
                expected_error
            );
            assert_eq!(*kit.store.transactions.lock().unwrap(), existing);
            assert_eq!(kit.store.saved.lock().unwrap().len(), 0);
            assert_eq!(*kit.status.0.lock().unwrap(), Vec::new());
            assert_eq!(provider.requested_paths(), vec!["/v1/bsv/main/addresses/history/all".to_string()]);
        }
    });
}

#[test]
fn test_bsv_unconfirmed_history_is_saved_and_tracked() {
    block_on(async {
        let provider = Arc::new(TestAlienProvider::with_responses(bsv_history_responses(true)));
        let kit = bsv_testkit(provider).await;
        kit.service.sync_wallet(kit.wallet_id.clone(), Some(Chain::BitcoinSv.as_asset_id())).await.unwrap();
        let stored = kit.store.transactions.lock().unwrap().clone();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].hash(), TEST_BSV_HASH);
        assert_eq!(stored[0].state, TransactionState::Pending);
        assert_eq!(stored[0].block_number, None);
        assert_eq!(kit.store.saved.lock().unwrap().len(), 1);
        assert_eq!(*kit.status.0.lock().unwrap(), vec![(kit.wallet_id, stored)]);
    });
}

#[test]
fn test_wallet_history_bsv_failure_preserves_bsv_home_success_and_advances_only_bsv_home_cursor() {
    block_on(async {
        let transaction = Transaction::mock();
        let provider = Arc::new(TestAlienProvider::with_responses(vec![bsv_home_response(17, 200, vec![transaction.clone()])]));
        let kit = TransactionsTestkit::new(mixed_wallet(), provider.clone());
        kit.assets.ensure_default_assets().await.unwrap();
        kit.wallet_preferences.set_transactions_timestamp(kit.wallet_id.clone(), None, 17).unwrap();

        let result = kit.service.sync_wallet(kit.wallet_id.clone(), None).await;
        assert_eq!(*kit.store.transactions.lock().unwrap(), vec![transaction]);
        assert_eq!(
            result.unwrap_err(),
            BsvHomeServiceError::Core {
                msg: "BSV: Network error: HTTP error: status 404".to_string(),
            }
        );
        assert_eq!(kit.store.saved.lock().unwrap().len(), 1);
        assert!(kit.wallet_preferences.get_transactions_timestamp(kit.wallet_id, None) > 17);
        assert_eq!(
            provider.requested_paths(),
            vec![
                "/v1/bsv/main/addresses/history/all".to_string(),
                "/v2/devices/transactions?from_timestamp=17&limit=100&offset=0".to_string(),
            ]
        );
    });
}

#[test]
fn test_wallet_history_bsv_home_failure_preserves_bsv_success_and_pending_tracking_without_advancing_cursor() {
    block_on(async {
        let mut responses = bsv_history_responses(true);
        responses.push(bsv_home_response(17, 400, Vec::new()));
        let provider = Arc::new(TestAlienProvider::with_responses(responses));
        let kit = TransactionsTestkit::new(mixed_wallet(), provider.clone());
        kit.assets.ensure_default_assets().await.unwrap();
        kit.wallet_preferences.set_transactions_timestamp(kit.wallet_id.clone(), None, 17).unwrap();

        assert_eq!(
            kit.service.sync_wallet(kit.wallet_id.clone(), None).await.unwrap_err(),
            BsvHomeServiceError::Core {
                msg: "Hosted: history unavailable".to_string(),
            }
        );
        let stored = kit.store.transactions.lock().unwrap().clone();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].hash(), TEST_BSV_HASH);
        assert_eq!(stored[0].state, TransactionState::Pending);
        assert_eq!(kit.store.saved.lock().unwrap().len(), 1);
        assert_eq!(*kit.status.0.lock().unwrap(), vec![(kit.wallet_id.clone(), stored)]);
        assert_eq!(kit.wallet_preferences.get_transactions_timestamp(kit.wallet_id, None), 17);
        assert_eq!(provider.requested_paths().len(), 4);
    });
}

#[test]
fn test_wallet_history_all_source_failures_are_reported_without_advancing_cursor() {
    block_on(async {
        let mut wallet = mixed_wallet();
        wallet.accounts.insert(0, Account::mock(Chain::TronShasta, "TEB39Rt69QkgD1BKhqaRNqGxfQzCarkRCb"));
        let provider = Arc::new(TestAlienProvider::with_responses(vec![bsv_home_response(17, 400, Vec::new())]));
        let kit = TransactionsTestkit::new_with_environment(wallet, provider.clone(), BsvHomeRuntimeEnvironment::LocalDevelopment);
        kit.wallet_preferences.set_transactions_timestamp(kit.wallet_id.clone(), None, 17).unwrap();

        assert_eq!(
            kit.service.sync_wallet(kit.wallet_id.clone(), None).await.unwrap_err(),
            BsvHomeServiceError::Core {
                msg: "Shasta: Network error: HTTP error: status 404; BSV: Network error: HTTP error: status 404; Hosted: history unavailable".to_string(),
            }
        );
        assert_eq!(*kit.store.saved.lock().unwrap(), Vec::new());
        assert_eq!(*kit.status.0.lock().unwrap(), Vec::new());
        assert_eq!(kit.wallet_preferences.get_transactions_timestamp(kit.wallet_id, None), 17);
        assert_eq!(provider.requested_paths().len(), 3);
    });
}

#[test]
fn test_wallet_history_shasta_failure_preserves_bsv_and_bsv_home_transactions() {
    block_on(async {
        let mut wallet = mixed_wallet();
        wallet.accounts.insert(0, Account::mock(Chain::TronShasta, "TEB39Rt69QkgD1BKhqaRNqGxfQzCarkRCb"));
        let transaction = Transaction::mock();
        let mut responses = bsv_history_responses(false);
        responses.push(bsv_home_response(0, 200, vec![transaction.clone()]));
        let provider = Arc::new(TestAlienProvider::with_responses(responses));
        let kit = TransactionsTestkit::new_with_environment(wallet, provider.clone(), BsvHomeRuntimeEnvironment::LocalDevelopment);
        kit.assets.ensure_default_assets().await.unwrap();

        assert_eq!(
            kit.service.sync_wallet(kit.wallet_id.clone(), None).await.unwrap_err(),
            BsvHomeServiceError::Core {
                msg: "Shasta: Network error: HTTP error: status 404".to_string(),
            }
        );
        let stored = kit.store.transactions.lock().unwrap().clone();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].hash(), TEST_BSV_HASH);
        assert_eq!(stored[1], transaction);
        assert_eq!(kit.store.saved.lock().unwrap().len(), 2);
        assert!(kit.wallet_preferences.get_transactions_timestamp(kit.wallet_id, None) > 0);
        assert_eq!(provider.requested_paths().len(), 5);
    });
}

#[test]
fn test_bsv_home_history_store_failure_does_not_advance_cursor_or_track_pending_transactions() {
    block_on(async {
        let mut transaction = Transaction::mock();
        transaction.state = TransactionState::Pending;
        let provider = Arc::new(TestAlienProvider::with_responses(vec![bsv_home_response(17, 200, vec![transaction])]));
        let kit = TransactionsTestkit::new(Wallet::mock(), provider);
        kit.assets.ensure_default_assets().await.unwrap();
        kit.wallet_preferences.set_transactions_timestamp(kit.wallet_id.clone(), None, 17).unwrap();
        *kit.store.failure.lock().unwrap() = Some(BsvHomeServiceError::Store {
            msg: "history store unavailable".to_string(),
        });

        assert_eq!(
            kit.service.sync_wallet(kit.wallet_id.clone(), None).await.unwrap_err(),
            BsvHomeServiceError::Core {
                msg: "Hosted: history store unavailable".to_string(),
            }
        );
        assert_eq!(*kit.store.saved.lock().unwrap(), Vec::new());
        assert_eq!(*kit.status.0.lock().unwrap(), Vec::new());
        assert_eq!(kit.wallet_preferences.get_transactions_timestamp(kit.wallet_id, None), 17);
    });
}

#[test]
fn test_wallet_history_bsv_only_skips_bsv_home_and_leaves_bsv_home_cursor_unchanged() {
    block_on(async {
        let provider = Arc::new(TestAlienProvider::with_responses(bsv_history_responses(false)));
        let kit = bsv_testkit(provider.clone()).await;
        kit.wallet_preferences.set_transactions_timestamp(kit.wallet_id.clone(), None, 17).unwrap();

        kit.service.sync_wallet(kit.wallet_id.clone(), None).await.unwrap();

        assert_eq!(kit.store.transactions.lock().unwrap().len(), 1);
        assert_eq!(kit.wallet_preferences.get_transactions_timestamp(kit.wallet_id, None), 17);
        assert_eq!(provider.requested_paths().len(), 3);
    });
}
