use std::sync::atomic::{AtomicUsize, Ordering};

use bsv_home_bsv::{
    BsvAddress,
    multisig::{Funding, Policy, funding_transaction_id, policy_descriptor, review_funding},
};
use bsv_home_derivation::{bsv_account_xpub, bsv_address};
use futures::executor::block_on;
use primitives::{Address, BsvAddressBranch, BsvAddressLocator, BsvSpendChange, BsvSpendInput, BsvSpendMetadata, Chain, Wallet, WalletSource, WalletType};

use super::addresses::{BsvHomeBsvAddressStore, BsvHomeBsvAddressStoredState};
use super::testkit::TEST_PASSWORD;
use super::{BsvHomeWalletImportResult, BsvHomeWalletImportType, tests::TestContext};
use crate::keystore::{decode_password, keystore_id_for_wallet};
use crate::services::bsv_multisig::{
    BsvHomeBsvMultisigFundingStatus, BsvHomeBsvMultisigFundingTarget, PayerFundingBinding, PayerFundingCheckpoint, bsv_multisig_payment_request_from_script,
};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::error::BsvHomeServiceError;

fn import(single: bool) -> BsvHomeWalletImportType {
    if single {
        BsvHomeWalletImportType::PrivateKey {
            value: ::signer::encode_private_key(&Chain::BitcoinSv, &[9; 32]).unwrap(),
            chain: Chain::BitcoinSv,
        }
    } else {
        BsvHomeWalletImportType::SinglePhrase {
            words: primitives::testkit::ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
            chain: Chain::BitcoinSv,
        }
    }
}
fn wallet(result: BsvHomeWalletImportResult) -> Wallet {
    match result {
        BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
    }
}
fn policy() -> Policy {
    let value: serde_json::Value = serde_json::from_str(include_str!("../../../../crates/bsv_home_bsv/testdata/bsv-multisig-interop.json")).unwrap();
    serde_json::from_value(value["proposal"]["policy"].clone()).unwrap()
}
async fn funded(context: &TestContext, wallet: &Wallet, public: bool, index: u32) -> PayerFundingCheckpoint {
    let root = wallet.account(Chain::BitcoinSv).unwrap().address.clone();
    let at = BsvAddressLocator {
        branch: BsvAddressBranch::Receive,
        index: 0,
    };
    let change = if wallet.wallet_type == WalletType::PrivateKey {
        None
    } else {
        let state = context.wallets.bsv_addresses.get(wallet.id.clone()).await.unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_str(&state.value).unwrap();
        let at = BsvAddressLocator {
            branch: BsvAddressBranch::Change,
            index,
        };
        let address = bsv_address(value["account_xpub"].as_str().unwrap(), at).unwrap();
        Some(BsvSpendChange {
            script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
            address,
            locator: at,
        })
    };
    let funding = Funding::from_spend(
        policy(),
        root.clone(),
        10_000,
        100,
        BsvSpendMetadata {
            inputs: vec![BsvSpendInput {
                transaction_id: format!("{:064x}", index + 1),
                vout: 0,
                value: 50_000,
                address: root.clone(),
                script_pubkey: hex::encode(BsvAddress::from_str(&root).unwrap().script_pubkey().as_bytes()),
                locator: at,
            }],
            change,
        },
    )
    .unwrap();
    let target = if public {
        BsvHomeBsvMultisigFundingTarget::Public {
            request: bsv_multisig_payment_request_from_script("Recovery fixture".into(), policy_descriptor(&funding.policy).unwrap().locking_script, "0.0001".into()).unwrap(),
        }
    } else {
        BsvHomeBsvMultisigFundingTarget::Member {
            wallet_id: "retained-collective".into(),
        }
    };
    let scope = if public {
        format!("https://retained-fixture.invalid/api/portal|{}", wallet.id)
    } else {
        format!("https://retained-fixture.invalid/api/portal|{}|retained-collective", wallet.id)
    };
    let binding = PayerFundingBinding {
        scope,
        operation_id: uuid::Uuid::new_v4().to_string(),
        payer_wallet_id: wallet.id.clone(),
        target,
        funding_json: serde_json::to_string(&funding).unwrap(),
        review_json: serde_json::to_string(&review_funding(&funding).unwrap()).unwrap(),
    };
    let lease = context.service.bsv_addresses.reserve_funding(wallet, binding.clone()).await.unwrap();
    let raw = context
        .service
        .keystore
        .bsv_multisig_funding_sign(
            keystore_id_for_wallet(wallet.id.id()),
            binding.funding_json.clone(),
            binding.review_json.clone(),
            decode_password(TEST_PASSWORD),
        )
        .unwrap();
    let checkpoint = PayerFundingCheckpoint {
        transaction_id: funding_transaction_id(&funding, &review_funding(&funding).unwrap(), &raw).unwrap(),
        binding,
        raw,
        status: BsvHomeBsvMultisigFundingStatus::Pending,
        error_code: None,
    };
    context.service.bsv_addresses.save_funding_signed(wallet, &lease.id, checkpoint.clone()).await.unwrap();
    drop(lease);
    checkpoint
}

#[test]
fn test_same_id_hd_and_single_reimport_preserves_every_actual_signed_operation_and_revision() {
    block_on(async {
        for single in [false, true] {
            let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
            let original = wallet(context.service.import_wallet("Original".into(), import(single), WalletSource::Import).await.unwrap());
            let first = funded(&context, &original, true, 0).await;
            let mut accepted = funded(&context, &original, true, 1).await;
            accepted.status = BsvHomeBsvMultisigFundingStatus::SyncFailed;
            accepted.error_code = Some("FUNDING_BALANCE_SYNC_FAILED".into());
            context.service.bsv_addresses.record_funding_outcome(&original, &accepted).await.unwrap();
            let member = funded(&context, &original, false, 2).await;
            let before = context.wallets.bsv_addresses.get(original.id.clone()).await.unwrap().unwrap();
            assert!(before.revision > 0);
            context.service.delete_wallet(original.id.clone()).await.unwrap();
            assert!(context.service.wallets().await.unwrap().is_empty());
            assert!(!context.service.keystore.exists(keystore_id_for_wallet(original.id.id())));
            assert_eq!(context.wallets.bsv_addresses.get(original.id.clone()).await.unwrap(), Some(before.clone()));
            context.service.bsv_addresses.release_funding_unsigned(&original, &first.binding).await.unwrap();
            assert_eq!(context.wallets.bsv_addresses.get(original.id.clone()).await.unwrap(), Some(before.clone()));
            assert!(context.service.bsv_addresses.require_funding_checkpoint(&original, &first).await.is_err());
            let restored = wallet(context.service.import_wallet("Restored".into(), import(single), WalletSource::Import).await.unwrap());
            assert_eq!(restored.id, original.id);
            assert_eq!(restored.accounts, original.accounts);
            assert_eq!(context.wallets.bsv_addresses.get(restored.id.clone()).await.unwrap(), Some(before));
            assert_eq!(context.service.bsv_addresses.funding_checkpoints(&restored).await.unwrap(), vec![first, accepted, member]);
        }
    });
}

#[test]
fn test_retained_import_rejects_wrong_root_family_and_corrupt_signed_raw_before_wallet_save() {
    block_on(async {
        for single in [false, true] {
            for corruption in ["root", "family", "raw", "payer"] {
                let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
                let original = wallet(context.service.import_wallet("Original".into(), import(single), WalletSource::Import).await.unwrap());
                funded(&context, &original, true, 0).await;
                context.service.delete_wallet(original.id.clone()).await.unwrap();
                let mut state = context.wallets.bsv_addresses.get(original.id.clone()).await.unwrap().unwrap();
                let mut value: serde_json::Value = serde_json::from_str(&state.value).unwrap();
                match corruption {
                    "root" if single => value["root_address"] = "1BitcoinEaterAddressDontSendf59kuE".into(),
                    "root" => {
                        value["account_xpub"] = bsv_account_xpub("shoot island position soft burden budget tooth cruel issue economy destroy above")
                            .unwrap()
                            .into()
                    }
                    "family" => value["kind"] = if single { "unsupported" } else { "singleAddress" }.into(),
                    "raw" => value["funding_reservations"][0]["signed"]["raw"] = "00".into(),
                    "payer" => value["funding_reservations"][0]["binding"]["payerWalletId"] = "privateKey_bitcoinsv_1BitcoinEaterAddressDontSendf59kuE".into(),
                    _ => unreachable!(),
                }
                state.value = serde_json::to_string(&value).unwrap();
                context.wallets.bsv_addresses.states.lock().unwrap().insert(original.id.clone(), state.clone());
                assert!(
                    context
                        .service
                        .import_wallet("Must not replace".into(), import(single), WalletSource::Import)
                        .await
                        .is_err()
                );
                assert!(context.service.wallets().await.unwrap().is_empty());
                assert!(!context.service.keystore.exists(keystore_id_for_wallet(original.id.id())));
                assert_eq!(context.wallets.bsv_addresses.get(original.id.clone()).await.unwrap(), Some(state));
            }
        }
    });
}

#[test]
fn test_delete_and_same_id_import_does_not_create_an_aba_window_for_a_late_cas() {
    block_on(async {
        for single in [false, true] {
            let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
            let original = wallet(context.service.import_wallet("Original".into(), import(single), WalletSource::Import).await.unwrap());
            let checkpoint = funded(&context, &original, true, 0).await;
            let before = context.wallets.bsv_addresses.get(original.id.clone()).await.unwrap().unwrap();
            context.service.delete_wallet(original.id.clone()).await.unwrap();
            let restored = wallet(context.service.import_wallet("Restored".into(), import(single), WalletSource::Import).await.unwrap());
            let mut accepted = checkpoint;
            accepted.status = BsvHomeBsvMultisigFundingStatus::Broadcast;
            context.service.bsv_addresses.record_funding_outcome(&restored, &accepted).await.unwrap();
            let after = context.wallets.bsv_addresses.get(restored.id.clone()).await.unwrap().unwrap();
            assert_eq!(after.revision, before.revision + 1);
            let stale = BsvHomeBsvAddressStoredState {
                revision: before.revision + 1,
                value: before.value,
            };
            assert!(!context.wallets.bsv_addresses.save(restored.id.clone(), Some(before.revision), stale).await.unwrap());
            assert_eq!(context.wallets.bsv_addresses.get(restored.id.clone()).await.unwrap(), Some(after));
            assert_eq!(context.service.bsv_addresses.funding_checkpoints(&restored).await.unwrap(), vec![accepted]);
        }
    });
}

#[test]
fn test_retained_import_never_runs_the_initial_factory_and_storage_errors_are_visible() {
    block_on(async {
        let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
        let original = wallet(context.service.import_wallet("Original".into(), import(false), WalletSource::Import).await.unwrap());
        context.service.delete_wallet(original.id.clone()).await.unwrap();
        let calls = AtomicUsize::new(0);
        let retained = context
            .service
            .bsv_addresses
            .import_record(&original, || {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(BsvHomeServiceError::Store {
                    msg: "initializer must not run".into(),
                })
            })
            .await
            .unwrap();
        assert!(retained.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        *context.wallets.bsv_addresses.read_error.lock().unwrap() = Some(BsvHomeServiceError::Store {
            msg: "retained read failed".into(),
        });
        assert!(context.service.import_wallet("Retry".into(), import(false), WalletSource::Import).await.is_err());
        assert!(context.service.wallets().await.unwrap().is_empty());
        assert!(!context.service.keystore.exists(keystore_id_for_wallet(original.id.id())));
        *context.wallets.bsv_addresses.read_error.lock().unwrap() = None;
        assert!(context.service.import_wallet("Recovered".into(), import(false), WalletSource::Import).await.is_ok());
    });
}

#[test]
fn test_non_bsv_wallet_import_preserves_its_existing_behavior_and_does_not_repair_unrelated_journals() {
    block_on(async {
        let context = TestContext::new();
        let input = BsvHomeWalletImportType::PrivateKey {
            value: ::signer::encode_private_key(&Chain::Ethereum, &[10; 32]).unwrap(),
            chain: Chain::Ethereum,
        };
        let original = wallet(context.service.import_wallet("Ethereum".into(), input.clone(), WalletSource::Import).await.unwrap());
        assert!(original.account(Chain::BitcoinSv).is_none());
        context.service.delete_wallet(original.id.clone()).await.unwrap();
        let unrelated = BsvHomeBsvAddressStoredState {
            revision: 7,
            value: "not a BSV journal for this chain".into(),
        };
        context.wallets.bsv_addresses.states.lock().unwrap().insert(original.id.clone(), unrelated.clone());
        let restored = wallet(
            context
                .service
                .import_wallet("Ethereum restored".into(), input.clone(), WalletSource::Import)
                .await
                .unwrap(),
        );
        assert_eq!(restored.id, original.id);
        assert_eq!(context.wallets.bsv_addresses.get(restored.id.clone()).await.unwrap(), Some(unrelated.clone()));
        context
            .service
            .bsv_addresses
            .initialize_import(&restored, primitives::testkit::ABANDON_PHRASE)
            .await
            .unwrap();
        let repeated = wallet(context.service.import_wallet("Ethereum existing".into(), input, WalletSource::Import).await.unwrap());
        assert_eq!(repeated.id, restored.id);
        assert_eq!(context.wallets.bsv_addresses.get(repeated.id).await.unwrap(), Some(unrelated));
    });
}

#[test]
fn test_hd_recovery_and_cold_observer_same_id_reimport_keep_the_actual_pool() {
    block_on(async {
        for observer in [false, true] {
            let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
            let words = primitives::testkit::ABANDON_PHRASE.to_string();
            let hot = wallet(context.service.bsv_hd_import("HD".into(), words.clone(), None, None, None, false).await.unwrap());
            let pairing = context.service.bsv_hd_pairing(hot.id.clone()).await.unwrap();
            let original = if observer {
                wallet(context.service.bsv_cold_wallet_import_pairing("Observer".into(), pairing.clone()).await.unwrap())
            } else {
                hot
            };
            let before = context.wallets.bsv_addresses.get(original.id.clone()).await.unwrap().unwrap();
            context.service.delete_wallet(original.id.clone()).await.unwrap();
            let restored = if observer {
                wallet(context.service.bsv_cold_wallet_import_pairing("Observer restored".into(), pairing).await.unwrap())
            } else {
                wallet(context.service.bsv_hd_import("HD restored".into(), words, None, None, None, false).await.unwrap())
            };
            assert_eq!(restored.id, original.id);
            assert_eq!(context.wallets.bsv_addresses.get(restored.id).await.unwrap(), Some(before));
        }
    });
}

async fn migration_record(context: &TestContext, wallet: &Wallet) -> serde_json::Value {
    let stored = context.wallets.bsv_addresses.get(wallet.id.clone()).await.unwrap().unwrap();
    serde_json::json!({
        "wallet": wallet,
        "storedRevision": stored.revision,
        "storedValue": stored.value,
    })
}

#[test]
fn test_typed_payer_migration_fixture_contains_sdk_validated_hd_and_single_address_journals() {
    block_on(async {
        let mut fixtures = Vec::new();
        for single in [false, true] {
            let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
            let wallet = wallet(
                context
                    .service
                    .import_wallet("Migration fixture".into(), import(single), WalletSource::Import)
                    .await
                    .unwrap(),
            );
            let checkpoint = funded(&context, &wallet, true, 0).await;
            assert_eq!(context.service.bsv_addresses.funding_checkpoints(&wallet).await.unwrap(), vec![checkpoint]);
            fixtures.push(migration_record(&context, &wallet).await);
            if !single {
                let pairing = context.service.bsv_hd_pairing(wallet.id.clone()).await.unwrap();
                let observer = match context.service.bsv_cold_wallet_import_pairing("Observer fixture".into(), pairing).await.unwrap() {
                    BsvHomeWalletImportResult::New { wallet } => wallet,
                    BsvHomeWalletImportResult::Existing { .. } => panic!("observer fixture must be a new wallet"),
                };
                assert!(context.service.bsv_addresses.snapshot(observer.id.clone()).await.unwrap().is_some());
                fixtures.push(migration_record(&context, &observer).await);
            }
        }
        let context = TestContext::new();
        let multicoin = wallet(
            context
                .service
                .import_wallet(
                    "Multicoin fixture".into(),
                    BsvHomeWalletImportType::MulticoinPhrase {
                        words: primitives::testkit::ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                        chains: vec![Chain::Ethereum, Chain::BitcoinSv],
                    },
                    WalletSource::Import,
                )
                .await
                .unwrap(),
        );
        let checkpoint = funded(&context, &multicoin, true, 0).await;
        assert_eq!(context.service.bsv_addresses.funding_checkpoints(&multicoin).await.unwrap(), vec![checkpoint]);
        fixtures.push(migration_record(&context, &multicoin).await);
        assert_eq!(fixtures.len(), 4);
        if std::env::var("WOWPAY_EXPORT_PAYER_MIGRATION_FIXTURE").ok().as_deref() == Some("1") {
            let output = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../ios/build/phase6-core/payer-retention-draft/ios/Packages/Store/Tests/StoreTests/Resources/bsv-payer-journal-migration.json");
            std::fs::create_dir_all(output.parent().unwrap()).unwrap();
            std::fs::write(output, serde_json::to_vec(&fixtures).unwrap()).unwrap();
        }
    });
}
