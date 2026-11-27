use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use futures::{executor::block_on, join, pin_mut, poll};
use primitives::{Account, Chain, Wallet, WalletId, WalletType};
use serde_json::{Value, json};

use super::testkit::MemoryOfflinePasswordStore;
use super::{BsvHomeOfflineSigningPasswordDomain as Domain, BsvHomeOfflineSigningPasswordService as Service, Error, MAX_FAILURES, Record, Snapshot, WINDOW_MS, gate};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::transfer_password::{validate_current_password, validate_new_password, validate_verification_password};
use crate::services::wallet::testkit::{MemoryWalletStore, TEST_PASSWORD};
use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};

const TEST_ROOT: &str = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT";
const TEST_OTHER_ROOT: &str = "1BitcoinEaterAddressDontSendf59kuE";
const TEST_REPLACEMENT: &str = "replacement-for-local-only-tests";
const TEST_WRONG: &str = "incorrect-local-only-test";

struct Context {
    wallet: Wallet,
    wallets: Arc<MemoryWalletStore>,
    session: Arc<BsvHomeWalletSessionService>,
    secure: Arc<MemoryOfflinePasswordStore>,
    now: Arc<AtomicI64>,
}

impl Context {
    fn new() -> Self {
        let wallet = wallet(TEST_ROOT);
        let wallets = Arc::new(MemoryWalletStore {
            wallets: Mutex::new(vec![wallet.clone()]),
            ..Default::default()
        });
        let selection = Arc::new(MemoryWalletSessionStore {
            current: Mutex::new(Some(wallet.id.clone())),
        });
        let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
            selection,
            wallets.clone(),
            BsvHomeRuntimeEnvironment::WowPay,
        ));
        Self {
            wallet,
            wallets,
            session,
            secure: Arc::new(MemoryOfflinePasswordStore::default()),
            now: Arc::new(AtomicI64::new(1_000_000)),
        }
    }

    fn service(&self, domain: Domain) -> Service {
        let mut service = Service::new(self.session.clone(), self.secure.clone(), self.wallet.clone(), domain).unwrap();
        let now = self.now.clone();
        service.clock = Arc::new(move || now.load(Ordering::SeqCst));
        service
    }

    async fn configured(&self) -> (Service, Snapshot) {
        let service = self.service(Domain::UnifiedCold);
        let missing = service.status().await.unwrap();
        let configured = service.set(missing, None, TEST_PASSWORD.into(), TEST_PASSWORD.into()).await.unwrap();
        (service, configured)
    }
}

fn wallet(root: &str) -> Wallet {
    Wallet {
        id: WalletId::Single(Chain::BitcoinSv, root.into()),
        wallet_type: WalletType::Single,
        ..Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, root)])
    }
}

#[test]
fn test_offline_password_real_set_verify_change_and_rejected_old_password() {
    block_on(async {
        let context = Context::new();
        let (service, snapshot) = context.configured().await;
        assert!(snapshot.configured);
        assert_eq!(service.verify(snapshot.clone(), TEST_PASSWORD.into()).await, Ok(()));
        assert_eq!(
            service
                .set(snapshot.clone(), Some(TEST_WRONG.into()), TEST_REPLACEMENT.into(), TEST_REPLACEMENT.into())
                .await,
            Err(Error::IncorrectPassword)
        );
        let changed = service
            .set(snapshot, Some(TEST_PASSWORD.into()), TEST_REPLACEMENT.into(), TEST_REPLACEMENT.into())
            .await
            .unwrap();
        assert_eq!(service.verify(changed.clone(), TEST_PASSWORD.into()).await, Err(Error::IncorrectPassword));
        assert_eq!(service.verify(changed, TEST_REPLACEMENT.into()).await, Ok(()));
        let record = service.read().unwrap().unwrap();
        assert_eq!((record.failure_count, record.last_failure_ms), (0, None));
        assert_eq!(context.secure.removes.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn test_offline_password_utf16_blank_confirmation_and_current_password_rules_match_online() {
    for password in ["🔐🔐🔐🔐".to_string(), "a".repeat(128), "\u{85}".repeat(8)] {
        assert_eq!(validate_new_password(&password, &password), Ok(()));
    }
    for password in ["🔐🔐🔐".to_string(), "🔐".repeat(65), " ".repeat(8), "\u{1c}".repeat(8)] {
        assert_eq!(validate_new_password(&password, &password), Err(Error::InvalidPassword));
    }
    assert_eq!(validate_new_password(TEST_PASSWORD, TEST_WRONG), Err(Error::ConfirmationMismatch));
    assert_eq!(validate_current_password(true, None), Err(Error::CurrentPasswordRequired));
    assert_eq!(validate_current_password(false, Some(TEST_PASSWORD)), Err(Error::InvalidPassword));
    assert_eq!(validate_current_password(false, None), Ok(()));
    assert_eq!(validate_verification_password(""), Err(Error::InvalidPassword));
    assert_eq!(validate_verification_password(&"🔐".repeat(65)), Err(Error::InvalidPassword));
    assert_eq!(validate_verification_password(" "), Ok(()));
}

#[test]
fn test_offline_password_actual_set_rejects_invalid_input_before_secure_write() {
    block_on(async {
        let context = Context::new();
        let service = context.service(Domain::UnifiedCold);
        let snapshot = service.status().await.unwrap();
        assert_eq!(service.verify(snapshot.clone(), TEST_PASSWORD.into()).await, Err(Error::NotConfigured));
        assert_eq!(service.set(snapshot.clone(), None, "short".into(), "short".into()).await, Err(Error::InvalidPassword));
        assert_eq!(
            service.set(snapshot.clone(), None, TEST_PASSWORD.into(), TEST_WRONG.into()).await,
            Err(Error::ConfirmationMismatch)
        );
        assert_eq!(
            service.set(snapshot, Some(TEST_PASSWORD.into()), TEST_PASSWORD.into(), TEST_PASSWORD.into()).await,
            Err(Error::InvalidPassword)
        );
        assert_eq!(context.secure.writes.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn test_offline_password_five_failures_persist_across_instances_and_clock_rollback_does_not_unlock() {
    block_on(async {
        let context = Context::new();
        let (service, snapshot) = context.configured().await;
        for _ in 0..MAX_FAILURES {
            assert_eq!(service.verify(snapshot.clone(), TEST_WRONG.into()).await, Err(Error::IncorrectPassword));
        }
        let restarted = context.service(Domain::UnifiedCold);
        let snapshot = restarted.status().await.unwrap();
        let writes = context.secure.writes.load(Ordering::SeqCst);
        assert_eq!(restarted.verify(snapshot.clone(), TEST_PASSWORD.into()).await, Err(Error::RateLimited));
        context.now.store(1, Ordering::SeqCst);
        assert_eq!(restarted.verify(snapshot.clone(), TEST_PASSWORD.into()).await, Err(Error::RateLimited));
        context.now.store(1_000_000 + WINDOW_MS, Ordering::SeqCst);
        assert_eq!(restarted.verify(snapshot.clone(), TEST_PASSWORD.into()).await, Err(Error::RateLimited));
        assert_eq!(context.secure.writes.load(Ordering::SeqCst), writes);
        context.now.store(1_000_000 + WINDOW_MS + 1, Ordering::SeqCst);
        assert_eq!(restarted.verify(snapshot, TEST_PASSWORD.into()).await, Ok(()));
        assert_eq!(restarted.read().unwrap().unwrap().failure_count, 0);
    });
}

#[test]
fn test_offline_password_expired_attempt_window_resets_count_without_backdating_record() {
    block_on(async {
        let context = Context::new();
        let (service, snapshot) = context.configured().await;
        assert_eq!(service.verify(snapshot.clone(), TEST_WRONG.into()).await, Err(Error::IncorrectPassword));
        context.now.fetch_add(WINDOW_MS + 1, Ordering::SeqCst);
        assert_eq!(service.verify(snapshot.clone(), TEST_WRONG.into()).await, Err(Error::IncorrectPassword));
        let record = service.read().unwrap().unwrap();
        assert_eq!((record.failure_count, record.last_failure_ms), (1, Some(1_000_000 + WINDOW_MS + 1)));
        assert_eq!(service.verify(snapshot, TEST_PASSWORD.into()).await, Ok(()));
    });
}

#[test]
fn test_offline_password_same_key_global_gate_serializes_two_real_service_instances() {
    block_on(async {
        let context = Context::new();
        let (first, first_snapshot) = context.configured().await;
        let second = context.service(Domain::UnifiedCold);
        let second_snapshot = second.status().await.unwrap();
        let mut record = first.read().unwrap().unwrap();
        record.failure_count = MAX_FAILURES - 1;
        record.last_failure_ms = Some(context.now.load(Ordering::SeqCst));
        first.write(&record).unwrap();
        let shared = gate(&first.key).unwrap();
        let held = shared.lock().await;
        let first_result = first.verify(first_snapshot, TEST_WRONG.into());
        let second_result = second.verify(second_snapshot, TEST_WRONG.into());
        pin_mut!(first_result, second_result);
        assert!(poll!(&mut first_result).is_pending());
        assert!(poll!(&mut second_result).is_pending());
        drop(held);
        let results = join!(first_result, second_result);
        assert_eq!(results, (Err(Error::IncorrectPassword), Err(Error::RateLimited)));
        assert_eq!(first.read().unwrap().unwrap().failure_count, MAX_FAILURES);
    });
}

#[test]
fn test_offline_password_credential_stamp_rejects_stale_setup_and_verification_across_instances() {
    block_on(async {
        let context = Context::new();
        let first = context.service(Domain::UnifiedCold);
        let second = context.service(Domain::UnifiedCold);
        let first_missing = first.status().await.unwrap();
        let stale_missing = second.status().await.unwrap();
        let configured = first.set(first_missing, None, TEST_PASSWORD.into(), TEST_PASSWORD.into()).await.unwrap();
        assert_eq!(
            second.set(stale_missing, None, TEST_REPLACEMENT.into(), TEST_REPLACEMENT.into()).await,
            Err(Error::InvalidSnapshot)
        );
        let stale_configured = second.status().await.unwrap();
        first
            .set(configured, Some(TEST_PASSWORD.into()), TEST_REPLACEMENT.into(), TEST_REPLACEMENT.into())
            .await
            .unwrap();
        assert_eq!(second.verify(stale_configured, TEST_PASSWORD.into()).await, Err(Error::InvalidSnapshot));
        let fresh = second.status().await.unwrap();
        assert_eq!(second.verify(fresh, TEST_REPLACEMENT.into()).await, Ok(()));
    });
}

#[test]
fn test_offline_password_snapshots_are_instance_owned_and_refresh_invalidates_old_snapshot() {
    block_on(async {
        let context = Context::new();
        let (first, snapshot) = context.configured().await;
        let second = context.service(Domain::UnifiedCold);
        second.status().await.unwrap();
        assert_eq!(second.verify(snapshot.clone(), TEST_PASSWORD.into()).await, Err(Error::InvalidSnapshot));
        let latest = first.status().await.unwrap();
        assert_ne!(snapshot.generation, latest.generation);
        assert_eq!(first.verify(snapshot, TEST_PASSWORD.into()).await, Err(Error::InvalidSnapshot));
        assert_eq!(first.verify(latest, TEST_PASSWORD.into()).await, Ok(()));
    });
}

#[test]
fn test_offline_password_cancelled_waiter_cannot_read_or_write_after_gate_is_released() {
    block_on(async {
        let context = Context::new();
        let (service, snapshot) = context.configured().await;
        let shared = gate(&service.key).unwrap();
        let held = shared.lock().await;
        let result = service.verify(snapshot, TEST_PASSWORD.into());
        pin_mut!(result);
        assert!(poll!(&mut result).is_pending());
        service.cancel();
        let reads = context.secure.reads.load(Ordering::SeqCst);
        let writes = context.secure.writes.load(Ordering::SeqCst);
        drop(held);
        assert_eq!(result.await, Err(Error::Cancelled));
        assert_eq!((context.secure.reads.load(Ordering::SeqCst), context.secure.writes.load(Ordering::SeqCst)), (reads, writes));
    });
}

#[test]
fn test_offline_password_current_wallet_root_change_and_deletion_reject_without_password_reset() {
    block_on(async {
        let context = Context::new();
        let (service, snapshot) = context.configured().await;
        let before = context.secure.values.lock().unwrap().clone();
        let other = wallet(TEST_OTHER_ROOT);
        context.wallets.wallets.lock().unwrap().push(other.clone());
        context.session.set_current_wallet_id(Some(other.id)).unwrap();
        assert_eq!(service.verify(snapshot.clone(), TEST_PASSWORD.into()).await, Err(Error::WalletChanged));
        context.session.set_current_wallet_id(Some(context.wallet.id.clone())).unwrap();
        context.wallets.wallets.lock().unwrap()[0].accounts[0].address = TEST_OTHER_ROOT.into();
        assert_eq!(service.verify(snapshot, TEST_PASSWORD.into()).await, Err(Error::WalletChanged));
        context.wallets.wallets.lock().unwrap().clear();
        assert_eq!(context.service(Domain::UnifiedCold).status().await, Err(Error::WalletChanged));
        assert!(*context.secure.values.lock().unwrap() == before);
    });
}

#[test]
fn test_offline_password_same_id_reimport_preserves_verifier_failure_count_and_other_namespaces() {
    block_on(async {
        let context = Context::new();
        let (service, snapshot) = context.configured().await;
        assert_eq!(service.verify(snapshot, TEST_WRONG.into()).await, Err(Error::IncorrectPassword));
        context
            .secure
            .values
            .lock()
            .unwrap()
            .insert("existing-keystore-password".into(), "existing-secure-sentinel".into());
        let before = context.secure.values.lock().unwrap().clone();
        context.wallets.wallets.lock().unwrap().clear();
        assert_eq!(context.service(Domain::UnifiedCold).status().await, Err(Error::WalletChanged));
        context.wallets.wallets.lock().unwrap().push(context.wallet.clone());
        let restarted = context.service(Domain::UnifiedCold);
        let configured = restarted.status().await.unwrap();
        assert!(configured.configured);
        assert_eq!(restarted.read().unwrap().unwrap().failure_count, 1);
        assert!(*context.secure.values.lock().unwrap() == before);
        assert_eq!(context.secure.removes.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn test_offline_password_domains_and_wallet_ids_do_not_share_credentials() {
    block_on(async {
        let context = Context::new();
        let (unified, snapshot) = context.configured().await;
        let legacy = context.service(Domain::LegacyCold);
        assert!(!legacy.status().await.unwrap().configured);
        assert_ne!(legacy.key, unified.key);
        let other = wallet(TEST_OTHER_ROOT);
        context.wallets.wallets.lock().unwrap().push(other.clone());
        context.session.set_current_wallet_id(Some(other.id.clone())).unwrap();
        let second = Service::new(context.session.clone(), context.secure.clone(), other, Domain::UnifiedCold).unwrap();
        assert!(!second.status().await.unwrap().configured);
        assert_ne!(second.key, unified.key);
        context.session.set_current_wallet_id(Some(context.wallet.id.clone())).unwrap();
        assert_eq!(unified.verify(snapshot, TEST_PASSWORD.into()).await, Ok(()));
    });
}

#[test]
fn test_offline_password_invalid_secure_record_fails_closed_without_replacement() {
    block_on(async {
        let context = Context::new();
        let (service, _) = context.configured().await;
        let original = context.secure.values.lock().unwrap().get(&service.key).unwrap().clone();
        let valid: Value = serde_json::from_str(&original).unwrap();
        let mut invalid = vec!["{".to_string(), format!("{{\"version\":1,{}", &original[1..]), "x".repeat(4097)];
        for (field, replacement) in [
            ("version", json!(2)),
            ("domain", json!("legacyCold")),
            ("walletId", json!("invalid")),
            ("address", json!(TEST_OTHER_ROOT)),
            ("kdf", json!("other")),
            ("iterations", json!(1)),
            ("salt", json!("AA==")),
            ("verifier", json!("AA==")),
            ("failureCount", json!(6)),
            ("lastFailureMs", json!(1)),
            ("unexpected", json!(true)),
        ] {
            let mut changed = valid.clone();
            changed[field] = replacement;
            invalid.push(serde_json::to_string(&changed).unwrap());
        }
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove("lastFailureMs");
        invalid.push(serde_json::to_string(&missing).unwrap());
        for encoded in invalid {
            context.secure.values.lock().unwrap().insert(service.key.clone(), encoded.clone());
            assert_eq!(service.status().await, Err(Error::Unavailable));
            assert!(context.secure.values.lock().unwrap().get(&service.key) == Some(&encoded));
        }
        context.secure.values.lock().unwrap().insert(service.key.clone(), original);
        assert!(service.status().await.unwrap().configured);
    });
}

#[test]
fn test_offline_password_secure_store_read_and_write_failures_are_visible_and_do_not_reset() {
    block_on(async {
        let context = Context::new();
        let service = context.service(Domain::UnifiedCold);
        context.secure.fail_get.store(true, Ordering::SeqCst);
        assert_eq!(service.status().await, Err(Error::Unavailable));
        context.secure.fail_get.store(false, Ordering::SeqCst);
        let missing = service.status().await.unwrap();
        context.secure.fail_set.store(true, Ordering::SeqCst);
        assert_eq!(service.set(missing, None, TEST_PASSWORD.into(), TEST_PASSWORD.into()).await, Err(Error::Unavailable));
        assert!(context.secure.values.lock().unwrap().is_empty());
        context.secure.fail_set.store(false, Ordering::SeqCst);
        let (configured, snapshot) = context.configured().await;
        let before = context.secure.values.lock().unwrap().clone();
        context.secure.fail_set.store(true, Ordering::SeqCst);
        assert_eq!(configured.verify(snapshot, TEST_WRONG.into()).await, Err(Error::Unavailable));
        assert!(*context.secure.values.lock().unwrap() == before);
    });
}

#[test]
fn test_offline_password_watch_only_or_missing_bsv_account_has_no_management_service() {
    let context = Context::new();
    let mut watched = context.wallet.clone();
    watched.wallet_type = WalletType::View;
    assert_eq!(
        Service::new(context.session.clone(), context.secure.clone(), watched, Domain::UnifiedCold).err(),
        Some(Error::WatchOnly)
    );
    let mut missing = context.wallet;
    missing.accounts.clear();
    assert_eq!(
        Service::new(context.session, context.secure, missing, Domain::UnifiedCold).err(),
        Some(Error::MissingBsvAccount)
    );
}

#[test]
fn test_offline_password_strict_record_rejects_failure_count_without_time_and_negative_time() {
    block_on(async {
        let context = Context::new();
        let (service, _) = context.configured().await;
        let mut record: Record = service.read().unwrap().unwrap();
        record.failure_count = 1;
        assert_eq!(record.validate(&service), Err(Error::Unavailable));
        record.last_failure_ms = Some(-1);
        assert_eq!(record.validate(&service), Err(Error::Unavailable));
        record.last_failure_ms = Some(0);
        assert_eq!(record.validate(&service), Ok(()));
        context.now.store(-1, Ordering::SeqCst);
        let snapshot = service.status().await.unwrap();
        assert_eq!(service.verify(snapshot, TEST_PASSWORD.into()).await, Err(Error::Unavailable));
    });
}

#[test]
fn test_offline_password_actual_set_uses_utf16_minimum() {
    block_on(async {
        let context = Context::new();
        let service = context.service(Domain::UnifiedCold);
        let missing = service.status().await.unwrap();
        let password = "🔐🔐🔐🔐";
        let configured = service.set(missing, None, password.into(), password.into()).await.unwrap();
        assert_eq!(service.verify(configured, password.into()).await, Ok(()));
    });
}
