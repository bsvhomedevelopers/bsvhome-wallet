use std::future::Future;
use std::sync::Arc;
use std::task::Context as TaskContext;

use futures::executor::block_on;
use futures::join;
use primitives::{Account, Chain, Wallet, WalletType};
use serde_json::{Map, Value};

use super::*;
use crate::alien::AlienHttpMethod;
use crate::services::error::BsvHomeServiceError;
use crate::services::portal::client::PortalClient;
use crate::services::portal::tests::Context;
use crate::services::portal::transfer_password_testkit::PasswordProvider;
use crate::services::portal::{BsvHomePortalAuthError, SESSION};
use crate::testkit::TestAlienProvider;

const UNCONFIGURED: &str = r#"{"success":true,"data":{"configured":false}}"#;
const CONFIGURED: &str = r#"{"success":true,"data":{"configured":true}}"#;
const EMPTY_SUCCESS: &str = r#"{"success":true,"data":{}}"#;

fn fixed_auth(context: &Context, provider: Arc<PasswordProvider>) -> Arc<BsvHomePortalAuthService> {
    let ordinary = context.auth_service(provider.clone());
    Arc::new(
        BsvHomePortalAuthService::new_for_wallet(
            provider,
            ordinary.keystore.clone(),
            ordinary.password.clone(),
            ordinary.session.clone(),
            ordinary.device_key.clone(),
            ordinary.store.clone(),
            ordinary.client.base_url.clone(),
            "zh-CN".into(),
            "4".into(),
            context.wallet.clone(),
        )
        .unwrap(),
    )
}

#[test]
fn test_fixed_transfer_password_uses_reviewed_identity_without_changing_personal_current() {
    block_on(async {
        let context = Context::new();
        let provider = PasswordProvider::new(&[(200, UNCONFIGURED), (200, EMPTY_SUCCESS), (200, EMPTY_SUCCESS)]);
        let auth = fixed_auth(&context, provider.clone());
        let service = BsvHomePortalTransferPasswordService::new_for_wallet(auth.clone(), context.wallet.clone()).unwrap();
        context.switch_wallet();
        let current = auth.session.current_wallet_id().unwrap();
        let snapshot = service.status().await.unwrap();
        assert_eq!(snapshot.wallet_id, context.wallet.id.id());
        assert_eq!(snapshot.address, context.wallet.account(Chain::BitcoinSv).unwrap().address);
        let configured = service.set(snapshot, None, "fixed-fixture".into(), "fixed-fixture".into()).await.unwrap();
        service.verify(configured, "fixed-fixture".into()).await.unwrap();
        assert_eq!(auth.session.current_wallet_id().unwrap(), current);
        assert_eq!(
            provider.paths(),
            [
                "/wallet-auth/transfer-password/status",
                "/wallet-auth/transfer-password/set",
                "/wallet-auth/transfer-password/verify"
            ]
        );
        for request in provider.requests.lock().unwrap().iter() {
            let body: Map<String, Value> = serde_json::from_slice(request.body.as_ref().unwrap()).unwrap();
            assert_eq!(body.get("address"), Some(&Value::String(context.wallet.account(Chain::BitcoinSv).unwrap().address.clone())));
        }
    });
}

#[test]
fn test_fixed_transfer_password_constructor_requires_the_exact_fixed_identity_and_v1() {
    let context = Context::new();
    let provider = PasswordProvider::new(&[]);
    let ordinary = context.auth_service(provider.clone());
    assert_eq!(
        BsvHomePortalTransferPasswordService::new_for_wallet(ordinary, context.wallet.clone()).err(),
        Some(BsvHomeTransferPasswordError::InvalidConfiguration)
    );
    let auth = fixed_auth(&context, provider.clone());
    let mut wallet = context.wallet.clone();
    wallet.id = primitives::WalletId::Multicoin("different-fixed-fixture".into());
    assert_eq!(
        BsvHomePortalTransferPasswordService::new_for_wallet(auth.clone(), wallet).err(),
        Some(BsvHomeTransferPasswordError::InvalidConfiguration)
    );
    let mut changed = context.wallet.clone();
    changed.accounts[0].address = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5".into();
    assert_eq!(
        BsvHomePortalTransferPasswordService::new_for_wallet(auth.clone(), changed).err(),
        Some(BsvHomeTransferPasswordError::InvalidConfiguration)
    );
    assert_eq!(
        BsvHomePortalTransferPasswordService::new(auth.clone(), context.wallet.id.id()).err(),
        Some(BsvHomeTransferPasswordError::InvalidConfiguration)
    );
    let mut v2 = fixed_auth(&context, provider.clone());
    Arc::get_mut(&mut v2).unwrap().client = PortalClient::new(provider.clone(), "https://api-dev.hj0252.com/v2/api/portal".into(), "en".into()).unwrap();
    assert_eq!(
        BsvHomePortalTransferPasswordService::new_for_wallet(v2, context.wallet.clone()).err(),
        Some(BsvHomeTransferPasswordError::InvalidConfiguration)
    );
    auth.cancel();
    assert_eq!(
        BsvHomePortalTransferPasswordService::new_for_wallet(auth, context.wallet.clone()).err(),
        Some(BsvHomeTransferPasswordError::Cancelled)
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 0);
}

#[test]
fn test_fixed_password_bridge_does_not_expand_other_fixed_wallet_requests() {
    block_on(async {
        let context = Context::new();
        let provider = PasswordProvider::new(&[(200, CONFIGURED)]);
        let auth = fixed_auth(&context, provider.clone());
        for path in [
            "/wallet-auth/transfer-password/status",
            "/wallet-auth/transfer-password/set",
            "/wallet-auth/transfer-password/verify",
            "/auth/paymail/config",
            "/auth/swaps/pairs",
        ] {
            assert_eq!(
                auth.wallet_request_at_generation::<Value>(&context.wallet, path, AlienHttpMethod::Post, Some(b"{}".to_vec()), SessionRetry::Never, 0)
                    .await,
                Err(BsvHomePortalAuthError::WalletChanged)
            );
        }
        assert_eq!(auth.connect().await, Err(BsvHomePortalAuthError::WalletChanged));
        assert_eq!(provider.requests.lock().unwrap().len(), 0);
        let service = BsvHomePortalTransferPasswordService::new_for_wallet(auth, context.wallet.clone()).unwrap();
        assert!(service.status().await.unwrap().configured);
        assert_eq!(provider.paths(), ["/wallet-auth/transfer-password/status"]);
    });
}

#[derive(Clone, Copy)]
enum FixedChange {
    Deleted,
    Address,
    Watch,
    Cancelled,
}

impl FixedChange {
    fn apply(self, context: &Context, service: &BsvHomePortalTransferPasswordService) -> BsvHomeTransferPasswordError {
        match self {
            Self::Deleted => context.replace_wallet(None),
            Self::Address => context.replace_wallet(Some(Wallet {
                accounts: vec![Account::mock(Chain::BitcoinSv, "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5")],
                ..context.wallet.clone()
            })),
            Self::Watch => context.replace_wallet(Some(Wallet {
                wallet_type: WalletType::View,
                ..context.wallet.clone()
            })),
            Self::Cancelled => service.cancel(),
        }
        match self {
            Self::Deleted | Self::Address => BsvHomeTransferPasswordError::WalletChanged,
            Self::Watch => BsvHomeTransferPasswordError::WatchOnly,
            Self::Cancelled => BsvHomeTransferPasswordError::Cancelled,
        }
    }
}

#[test]
fn test_fixed_password_rechecks_existence_identity_and_cancellation_before_network() {
    block_on(async {
        for change in [FixedChange::Deleted, FixedChange::Address, FixedChange::Watch, FixedChange::Cancelled] {
            let context = Context::new();
            let provider = PasswordProvider::new(&[]);
            let auth = fixed_auth(&context, provider.clone());
            let service = BsvHomePortalTransferPasswordService::new_for_wallet(auth, context.wallet.clone()).unwrap();
            context.switch_wallet();
            let expected = change.apply(&context, &service);
            assert_eq!(service.status().await, Err(expected));
            assert_eq!(provider.requests.lock().unwrap().len(), 0);
        }
    });
}

#[test]
fn test_fixed_password_late_status_set_and_verify_cannot_authorize_a_changed_identity() {
    block_on(async {
        for action in [TransferPasswordAction::Status, TransferPasswordAction::Set, TransferPasswordAction::Verify] {
            for change in [FixedChange::Deleted, FixedChange::Address, FixedChange::Watch, FixedChange::Cancelled] {
                let context = Context::new();
                let initial = match action {
                    TransferPasswordAction::Status | TransferPasswordAction::Verify => CONFIGURED,
                    TransferPasswordAction::Set => UNCONFIGURED,
                };
                let provider = PasswordProvider::new(&[(200, initial), (200, EMPTY_SUCCESS)]);
                let auth = fixed_auth(&context, provider.clone());
                let service = BsvHomePortalTransferPasswordService::new_for_wallet(auth, context.wallet.clone()).unwrap();
                context.switch_wallet();
                let snapshot = match action {
                    TransferPasswordAction::Status => None,
                    TransferPasswordAction::Set | TransferPasswordAction::Verify => Some(service.status().await.unwrap()),
                };
                let (started, release) = provider.pause_next();
                let mut expected = BsvHomeTransferPasswordError::Unavailable;
                let request = async {
                    match action {
                        TransferPasswordAction::Status => service.status().await.map(|_| ()),
                        TransferPasswordAction::Set => service
                            .set(snapshot.clone().unwrap(), None, "fixed-fixture".into(), "fixed-fixture".into())
                            .await
                            .map(|_| ()),
                        TransferPasswordAction::Verify => service.verify(snapshot.clone().unwrap(), "fixed-fixture".into()).await,
                    }
                };
                let (result, ()) = join!(request, async {
                    started.await.unwrap();
                    expected = change.apply(&context, &service);
                    release.send(()).unwrap();
                });
                assert_eq!(result, Err(expected));
                assert_eq!(service.state.lock().unwrap().snapshot, snapshot);
                let requests = match action {
                    TransferPasswordAction::Status => 1,
                    TransferPasswordAction::Set | TransferPasswordAction::Verify => 2,
                };
                assert_eq!(provider.requests.lock().unwrap().len(), requests);
            }
        }
    });
}

#[test]
fn test_fixed_password_waiting_for_auth_or_first_poll_cannot_login_after_cancel() {
    block_on(async {
        for queued in [false, true] {
            let context = Context::new();
            let provider = PasswordProvider::new(&[]);
            let auth = fixed_auth(&context, provider.clone());
            auth.store.remove(SESSION.into()).unwrap();
            let service = BsvHomePortalTransferPasswordService::new_for_wallet(auth.clone(), context.wallet.clone()).unwrap();
            context.switch_wallet();
            let lock = auth.connection.lock().await;
            let mut request = Box::pin(service.status());
            if queued {
                let waker = futures::task::noop_waker();
                assert!(request.as_mut().poll(&mut TaskContext::from_waker(&waker)).is_pending());
            }
            service.cancel();
            drop(lock);
            assert_eq!(request.await, Err(BsvHomeTransferPasswordError::Cancelled));
            assert_eq!(service.status().await, Err(BsvHomeTransferPasswordError::Cancelled));
            assert_eq!(provider.requests.lock().unwrap().len(), 0);
            assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        }
    });
}

#[test]
fn test_fixed_password_queued_mutation_and_later_status_remain_cancelled() {
    block_on(async {
        let context = Context::new();
        let provider = PasswordProvider::new(&[(200, CONFIGURED)]);
        let auth = fixed_auth(&context, provider.clone());
        let service = BsvHomePortalTransferPasswordService::new_for_wallet(auth, context.wallet.clone()).unwrap();
        context.switch_wallet();
        let snapshot = service.status().await.unwrap();
        let lock = service.operation.lock().await;
        let mut request = Box::pin(service.verify(snapshot.clone(), "fixed-fixture".into()));
        let waker = futures::task::noop_waker();
        assert!(request.as_mut().poll(&mut TaskContext::from_waker(&waker)).is_pending());
        service.cancel();
        drop(lock);
        assert_eq!(request.await, Err(BsvHomeTransferPasswordError::Cancelled));
        assert_eq!(service.verify(snapshot, "fixed-fixture".into()).await, Err(BsvHomeTransferPasswordError::Cancelled));
        assert_eq!(service.status().await, Err(BsvHomeTransferPasswordError::Cancelled));
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    });
}

#[test]
fn test_fixed_password_typed_bridge_rejects_mismatched_path_method_and_v2() {
    block_on(async {
        let context = Context::new();
        let provider = PasswordProvider::new(&[]);
        let auth = fixed_auth(&context, provider.clone());
        for (action, path, method) in [
            (TransferPasswordAction::Status, "/wallet-auth/transfer-password/set", AlienHttpMethod::Post),
            (TransferPasswordAction::Status, "/wallet-auth/transfer-password/status", AlienHttpMethod::Get),
            (TransferPasswordAction::Verify, "/wallet-auth/transfer-password/verify?address=other", AlienHttpMethod::Post),
            (TransferPasswordAction::Set, "/wallet-auth/transfer-password/reset", AlienHttpMethod::Post),
        ] {
            assert_eq!(
                auth.wallet_scoped_request::<Value>(
                    &context.wallet,
                    path,
                    method,
                    Some(b"{}".to_vec()),
                    SessionRetry::Never,
                    super::super::WalletRequestCheckpoint {
                        generation: 0,
                        check: &|| Ok(()),
                        before_request: &|| Ok(()),
                        kind: super::super::WalletRequestKind::TransferPassword(action),
                    },
                )
                .await,
                Err(BsvHomePortalAuthError::WalletChanged)
            );
        }
        let mut v2 = fixed_auth(&context, provider.clone());
        Arc::get_mut(&mut v2).unwrap().client = PortalClient::new(provider.clone(), "https://api-dev.hj0252.com/v2/api/portal".into(), "en".into()).unwrap();
        assert_eq!(
            v2.transfer_password_request_at_generation::<Value>(&context.wallet, TransferPasswordAction::Status, b"{}".to_vec(), SessionRetry::Never, 0)
                .await,
            Err(BsvHomePortalAuthError::WalletChanged)
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_fixed_password_current_change_during_response_is_allowed_and_status_reauth_is_read_only() {
    block_on(async {
        let context = Context::new();
        let expired = r#"{"success":false,"code":"LOGIN_REQUIRED"}"#;
        let provider = PasswordProvider::new(&[(401, expired), (200, CONFIGURED)]);
        let auth = fixed_auth(&context, provider.clone());
        let service = BsvHomePortalTransferPasswordService::new_for_wallet(auth.clone(), context.wallet.clone()).unwrap();
        let (started, release) = provider.pause_next();
        let (result, ()) = join!(service.status(), async {
            started.await.unwrap();
            context.switch_wallet();
            release.send(()).unwrap();
        });
        assert!(result.unwrap().configured);
        assert_ne!(auth.session.current_wallet_id().unwrap(), context.wallet.id);
        assert_eq!(
            provider.paths(),
            [
                "/wallet-auth/transfer-password/status",
                "/wallet-auth/challenge",
                "/wallet-auth/login",
                "/wallet-auth/transfer-password/status"
            ]
        );
        for setting in [false, true] {
            let provider = PasswordProvider::new(&[(200, if setting { UNCONFIGURED } else { CONFIGURED }), (401, expired)]);
            let auth = fixed_auth(&context, provider.clone());
            let service = BsvHomePortalTransferPasswordService::new_for_wallet(auth, context.wallet.clone()).unwrap();
            let snapshot = service.status().await.unwrap();
            let result = if setting {
                service.set(snapshot, None, "fixed-fixture".into(), "fixed-fixture".into()).await.map(|_| ())
            } else {
                service.verify(snapshot, "fixed-fixture".into()).await
            };
            assert_eq!(result, Err(BsvHomeTransferPasswordError::Unavailable));
            assert_eq!(provider.requests.lock().unwrap().len(), 2);
        }
    });
}

#[test]
fn test_transfer_password_numeric_success_codes_preserve_other_v1_and_v2_envelopes() {
    block_on(async {
        let context = Context::new();
        let provider = PasswordProvider::new(&[
            (200, r#"{"success":true,"code":200,"data":{"configured":false}}"#),
            (200, r#"{"success":true,"code":0,"data":{}}"#),
            (200, r#"{"success":true,"code":-1,"data":{}}"#),
        ]);
        let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
        let snapshot = service.status().await.unwrap();
        let configured = service.set(snapshot, None, "first-fixture".into(), "first-fixture".into()).await.unwrap();
        service.verify(configured, "first-fixture".into()).await.unwrap();
        assert_eq!(provider.requests.lock().unwrap().len(), 3);

        for (base, path, method, status, data) in [
            (
                "https://portal.test/api/portal",
                "/auth/paymail/config",
                AlienHttpMethod::Get,
                200,
                r#"{"success":true,"code":200,"data":{}}"#,
            ),
            (
                "https://portal.test/v2/api/portal",
                "/auth/multisig/wallets",
                AlienHttpMethod::Get,
                200,
                r#"{"success":true,"code":200,"data":{}}"#,
            ),
            (
                "https://portal.test/api/portal",
                "/wallet-auth/transfer-password/verify",
                AlienHttpMethod::Post,
                500,
                r#"{"success":true,"code":200,"data":{}}"#,
            ),
            (
                "https://portal.test/api/portal",
                "/wallet-auth/transfer-password/verify",
                AlienHttpMethod::Post,
                200,
                r#"{"success":true,"forceLogout":true,"code":200,"data":{}}"#,
            ),
        ] {
            let provider = Arc::new(TestAlienProvider::with_json(status, data));
            let client = PortalClient::new(provider.clone(), base.into(), "en".into()).unwrap();
            assert!(
                client
                    .authenticated::<Map<String, Value>>("synthetic-session", path, method, Some(b"{}".to_vec()))
                    .await
                    .is_err()
            );
        }
    });
}

#[test]
fn test_transfer_password_failed_verification_reloads_status_before_a_fresh_attempt() {
    block_on(async {
        for (status, data) in [(500, r#"{"success":false,"code":"UNAVAILABLE"}"#), (200, r#"{"success":true,"data":[]}"#)] {
            let context = Context::new();
            let provider = PasswordProvider::new(&[(200, CONFIGURED), (status, data), (200, CONFIGURED), (200, EMPTY_SUCCESS)]);
            let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
            let snapshot = service.status().await.unwrap();
            let error = service.verify(snapshot.clone(), "fixture".into()).await.unwrap_err();
            assert!(error.requires_status_reload());
            let stale = service.verify(snapshot, "fixture".into()).await.unwrap_err();
            assert_eq!(stale, BsvHomeTransferPasswordError::InvalidSnapshot);
            assert!(stale.requires_status_reload());
            let refreshed = service.status().await.unwrap();
            service.verify(refreshed, "fixture".into()).await.unwrap();
            assert_eq!(provider.requests.lock().unwrap().len(), 4);
        }
        let context = Context::new();
        let provider = PasswordProvider::new(&[(200, UNCONFIGURED), (200, CONFIGURED), (200, EMPTY_SUCCESS)]);
        let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
        let snapshot = service.status().await.unwrap();
        let error = service.verify(snapshot, "fixture".into()).await.unwrap_err();
        assert_eq!(error, BsvHomeTransferPasswordError::NotConfigured);
        assert!(error.requires_status_reload());
        service.verify(service.status().await.unwrap(), "fixture".into()).await.unwrap();
        assert_eq!(provider.requests.lock().unwrap().len(), 3);
    });
}

#[test]
fn test_transfer_password_known_rejections_preserve_status_for_explicit_user_retry() {
    block_on(async {
        for (status, response) in [
            (200, r#"{"success":false,"code":"TRANSFER_PASSWORD_INCORRECT"}"#),
            (429, r#"{"success":false,"code":"TRANSFER_PASSWORD_RATE_LIMITED"}"#),
        ] {
            let context = Context::new();
            let provider = PasswordProvider::new(&[(200, CONFIGURED), (status, response), (200, EMPTY_SUCCESS)]);
            let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
            let snapshot = service.status().await.unwrap();
            let error = service.verify(snapshot.clone(), "fixture".into()).await.unwrap_err();
            assert!(!error.requires_status_reload());
            service.verify(snapshot, "fixture".into()).await.unwrap();
            assert_eq!(provider.requests.lock().unwrap().len(), 3);
        }
    });
}

#[test]
fn test_transfer_password_new_status_after_external_auth_cancel_invalidates_an_equal_old_snapshot() {
    block_on(async {
        let context = Context::new();
        let provider = PasswordProvider::new(&[(200, CONFIGURED), (200, CONFIGURED), (200, EMPTY_SUCCESS)]);
        let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
        let previous = service.status().await.unwrap();
        service.auth.cancel();
        let next = service.status().await.unwrap();
        assert_eq!(previous.wallet_id, next.wallet_id);
        assert_eq!(previous.address, next.address);
        assert_eq!(previous.configured, next.configured);
        assert_ne!(previous.generation, next.generation);
        assert_eq!(service.verify(previous, "fixture".into()).await, Err(BsvHomeTransferPasswordError::Cancelled));
        service.verify(next, "fixture".into()).await.unwrap();
        assert_eq!(provider.requests.lock().unwrap().len(), 3);
    });
}

#[test]
fn test_transfer_password_requests_use_exact_objects_and_cached_current_identity() {
    block_on(async {
        let context = Context::new();
        let provider = PasswordProvider::new(&[(200, UNCONFIGURED), (200, EMPTY_SUCCESS), (200, EMPTY_SUCCESS), (200, EMPTY_SUCCESS)]);
        let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
        let snapshot = service.status().await.unwrap();
        let configured = service.set(snapshot, None, "first-fixture".into(), "first-fixture".into()).await.unwrap();
        let updated = service
            .set(configured, Some("first-fixture".into()), "second-fixture".into(), "second-fixture".into())
            .await
            .unwrap();
        service.verify(updated.clone(), "second-fixture".into()).await.unwrap();
        assert!(updated.configured);
        assert_eq!(updated.wallet_id, context.wallet.id.id());
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        for (request, action) in requests.iter().zip(["status", "set", "set", "verify"]) {
            assert_eq!(request.url, format!("https://api-dev.hj0252.com/api/portal/wallet-auth/transfer-password/{action}"));
            assert_eq!(request.method, AlienHttpMethod::Post);
            let headers = request.headers.as_ref().unwrap();
            assert_eq!(headers.get("language").map(String::as_str), Some("zh-CN"));
            assert_eq!(headers.get("Cache-Control").map(String::as_str), Some("no-store"));
            assert!(headers.get("Authorization").is_some_and(|value| !value.starts_with("Bearer ")));
            let body: Map<String, Value> = serde_json::from_slice(request.body.as_ref().unwrap()).unwrap();
            assert_eq!(body.get("address"), Some(&Value::String(updated.address.clone())));
            assert!(!body.contains_key("confirmation"));
        }
        let initial: Map<String, Value> = serde_json::from_slice(requests[1].body.as_ref().unwrap()).unwrap();
        assert_eq!(initial.len(), 2);
        assert!(!initial.contains_key("currentPassword"));
        assert_eq!(initial.get("newPassword"), Some(&Value::String("first-fixture".into())));
        let changed: Map<String, Value> = serde_json::from_slice(requests[2].body.as_ref().unwrap()).unwrap();
        assert_eq!(changed.len(), 3);
        assert_eq!(changed.get("currentPassword"), Some(&Value::String("first-fixture".into())));
        let verified: Map<String, Value> = serde_json::from_slice(requests[3].body.as_ref().unwrap()).unwrap();
        assert_eq!(verified.len(), 2);
        assert_eq!(verified.get("password"), Some(&Value::String("second-fixture".into())));
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
    });
}

#[test]
fn test_transfer_password_requires_the_stored_snapshot_and_validates_before_network() {
    block_on(async {
        let context = Context::new();
        let provider = PasswordProvider::new(&[(200, UNCONFIGURED), (200, EMPTY_SUCCESS)]);
        let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
        let snapshot = service.status().await.unwrap();
        let mut invalid = snapshot.clone();
        invalid.wallet_id.push('x');
        assert_eq!(
            service.set(invalid, None, "first-fixture".into(), "first-fixture".into()).await,
            Err(BsvHomeTransferPasswordError::InvalidSnapshot)
        );
        let mut stale = snapshot.clone();
        stale.generation += 1;
        assert_eq!(service.verify(stale, "fixture".into()).await, Err(BsvHomeTransferPasswordError::Cancelled));
        let mut address = snapshot.clone();
        address.address.push('x');
        assert_eq!(service.verify(address, "fixture".into()).await, Err(BsvHomeTransferPasswordError::InvalidSnapshot));
        let mut configured = snapshot.clone();
        configured.configured = true;
        assert_eq!(service.verify(configured, "fixture".into()).await, Err(BsvHomeTransferPasswordError::InvalidSnapshot));
        assert_eq!(service.verify(snapshot.clone(), "fixture".into()).await, Err(BsvHomeTransferPasswordError::NotConfigured));
        for password in ["".into(), "short".into(), " ".repeat(8), "\u{1c}".repeat(8), "🔐".repeat(65)] {
            assert_eq!(
                service.set(snapshot.clone(), None, password.clone(), password).await,
                Err(BsvHomeTransferPasswordError::InvalidPassword)
            );
        }
        assert_eq!(
            service.set(snapshot.clone(), None, "first-fixture".into(), "other-fixture".into()).await,
            Err(BsvHomeTransferPasswordError::ConfirmationMismatch)
        );
        assert_eq!(
            service.set(snapshot.clone(), Some("unused".into()), "first-fixture".into(), "first-fixture".into()).await,
            Err(BsvHomeTransferPasswordError::InvalidPassword)
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
        let password = "🔐".repeat(4);
        let configured = service.set(snapshot, None, password.clone(), password).await.unwrap();
        assert_eq!(
            service.set(configured, None, "next-fixture".into(), "next-fixture".into()).await,
            Err(BsvHomeTransferPasswordError::CurrentPasswordRequired)
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 2);
    });
}

#[test]
fn test_transfer_password_status_reauthenticates_once_but_mutations_never_replay() {
    block_on(async {
        let context = Context::new();
        let expired = r#"{"success":false,"code":"LOGIN_REQUIRED","message":"expired"}"#;
        let provider = PasswordProvider::new(&[(401, expired), (200, CONFIGURED)]);
        let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
        assert!(service.status().await.unwrap().configured);
        assert_eq!(
            provider.paths(),
            vec![
                "/wallet-auth/transfer-password/status",
                "/wallet-auth/challenge",
                "/wallet-auth/login",
                "/wallet-auth/transfer-password/status"
            ]
        );
        for setting in [false, true] {
            let provider = PasswordProvider::new(&[(200, if setting { UNCONFIGURED } else { CONFIGURED }), (401, expired)]);
            let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
            let snapshot = service.status().await.unwrap();
            let result = if setting {
                service.set(snapshot, None, "first-fixture".into(), "first-fixture".into()).await.map(|_| ())
            } else {
                service.verify(snapshot, "fixture".into()).await
            };
            assert_eq!(result, Err(BsvHomeTransferPasswordError::Unavailable));
            assert_eq!(provider.requests.lock().unwrap().len(), 2);
        }
    });
}

#[test]
fn test_transfer_password_rejects_non_object_success_and_non_boolean_configuration() {
    block_on(async {
        for body in [
            r#"{"success":true,"data":[]}"#,
            r#"{"success":true,"data":null}"#,
            EMPTY_SUCCESS,
            r#"{"success":true,"data":{"configured":"true"}}"#,
        ] {
            let context = Context::new();
            let provider = PasswordProvider::new(&[(200, body)]);
            let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider), context.wallet.id.id()).unwrap();
            assert_eq!(service.status().await, Err(BsvHomeTransferPasswordError::InvalidResponse));
        }
        for setting in [false, true] {
            for body in [r#"{"success":true,"data":[]}"#, r#"{"success":true,"data":null}"#] {
                let context = Context::new();
                let provider = PasswordProvider::new(&[(200, if setting { UNCONFIGURED } else { CONFIGURED }), (200, body)]);
                let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
                let snapshot = service.status().await.unwrap();
                let result = if setting {
                    service.set(snapshot.clone(), None, "first-fixture".into(), "first-fixture".into()).await.map(|_| ())
                } else {
                    service.verify(snapshot.clone(), "fixture".into()).await
                };
                assert_eq!(result, Err(BsvHomeTransferPasswordError::InvalidResponse));
                assert_eq!(service.verify(snapshot, "fixture".into()).await, Err(BsvHomeTransferPasswordError::InvalidSnapshot));
                assert_eq!(provider.requests.lock().unwrap().len(), 2);
            }
        }
    });
}

#[test]
fn test_transfer_password_errors_never_expose_backend_messages_or_credentials() {
    block_on(async {
        for (status, code, expected) in [
            (200, "TRANSFER_PASSWORD_INCORRECT", BsvHomeTransferPasswordError::IncorrectPassword),
            (429, "TRANSFER_PASSWORD_RATE_LIMITED", BsvHomeTransferPasswordError::RateLimited),
            (500, "unknown-sensitive-code", BsvHomeTransferPasswordError::Unavailable),
        ] {
            let context = Context::new();
            let body = format!(r#"{{"success":false,"code":"{code}","message":"backend-sensitive-message"}}"#);
            let provider = PasswordProvider::new(&[(200, CONFIGURED), (status, &body)]);
            let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
            let snapshot = service.status().await.unwrap();
            let error = service.verify(snapshot, "fixture".into()).await.unwrap_err();
            assert_eq!(error, expected);
            assert!(!format!("{error:?} {error}").contains("sensitive"));
            assert_eq!(provider.requests.lock().unwrap().len(), 2);
        }
    });
}

#[test]
fn test_transfer_password_cancel_is_permanent_and_late_status_cannot_publish() {
    block_on(async {
        let context = Context::new();
        let provider = PasswordProvider::new(&[(200, CONFIGURED)]);
        let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
        let (started, release) = provider.pause_next();
        let (result, ()) = join!(service.status(), async {
            started.await.unwrap();
            service.cancel();
            release.send(()).unwrap();
        });
        assert_eq!(result, Err(BsvHomeTransferPasswordError::Cancelled));
        assert_eq!(service.status().await, Err(BsvHomeTransferPasswordError::Cancelled));
        assert!(service.state.lock().unwrap().snapshot.is_none());
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    });
}

#[test]
fn test_transfer_password_late_mutations_reject_switched_deleted_or_modified_wallets() {
    block_on(async {
        for modification in 0..3 {
            let context = Context::new();
            let provider = PasswordProvider::new(&[(200, CONFIGURED), (200, EMPTY_SUCCESS)]);
            let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
            let snapshot = service.status().await.unwrap();
            let (started, release) = provider.pause_next();
            let (result, ()) = join!(service.verify(snapshot, "fixture".into()), async {
                started.await.unwrap();
                match modification {
                    0 => context.switch_wallet(),
                    1 => context.replace_wallet(None),
                    2 => context.replace_wallet(Some(Wallet {
                        accounts: vec![Account::mock(Chain::BitcoinSv, "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5")],
                        ..context.wallet.clone()
                    })),
                    _ => unreachable!(),
                }
                release.send(()).unwrap();
            });
            assert_eq!(result, Err(BsvHomeTransferPasswordError::WalletChanged));
            assert_eq!(provider.requests.lock().unwrap().len(), 2);
        }
    });
}

#[test]
fn test_transfer_password_scope_rejects_fixed_v2_and_watch_only_wallets() {
    block_on(async {
        let context = Context::new();
        assert!(BsvHomePortalTransferPasswordService::new(Arc::new(context.fixed_service()), context.wallet.id.id()).is_err());
        assert!(
            BsvHomePortalTransferPasswordService::new(
                Arc::new(context.service_at("https://api-dev.hj0252.com/v2/api/portal", context.password.clone())),
                context.wallet.id.id()
            )
            .is_err()
        );
        let provider = PasswordProvider::new(&[]);
        let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
        context.replace_wallet(Some(Wallet {
            wallet_type: WalletType::View,
            ..context.wallet.clone()
        }));
        assert_eq!(service.status().await, Err(BsvHomeTransferPasswordError::WatchOnly));
        assert_eq!(provider.requests.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_transfer_password_expired_generation_cannot_login_after_auth_lock_queue_or_first_poll() {
    block_on(async {
        for queued in [true, false] {
            let context = Context::new();
            let provider = PasswordProvider::new(&[]);
            let auth = context.auth_service(provider.clone());
            auth.store.remove(SESSION.into()).unwrap();
            let generation = auth.connection_version.load(Ordering::SeqCst);
            let lock = auth.connection.lock().await;
            let mut request = Box::pin(auth.wallet_request_at_generation::<Map<String, Value>>(
                &context.wallet,
                TransferPasswordAction::Status.path(),
                AlienHttpMethod::Post,
                Some(b"{}".to_vec()),
                SessionRetry::Authenticate,
                generation,
            ));
            if queued {
                let waker = futures::task::noop_waker();
                assert!(request.as_mut().poll(&mut TaskContext::from_waker(&waker)).is_pending());
            }
            auth.cancel();
            drop(lock);
            assert_eq!(
                request.await,
                Err(BsvHomePortalAuthError::Service {
                    error: BsvHomeServiceError::Cancelled
                })
            );
            assert_eq!(provider.requests.lock().unwrap().len(), 0);
            assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        }
    });
}

#[test]
fn test_transfer_password_old_snapshot_and_queued_operation_cannot_survive_cancel() {
    block_on(async {
        let context = Context::new();
        let provider = PasswordProvider::new(&[(200, CONFIGURED)]);
        let service = BsvHomePortalTransferPasswordService::new(context.auth_service(provider.clone()), context.wallet.id.id()).unwrap();
        let snapshot = service.status().await.unwrap();
        let lock = service.operation.lock().await;
        let mut request = Box::pin(service.verify(snapshot.clone(), "fixture".into()));
        let waker = futures::task::noop_waker();
        assert!(request.as_mut().poll(&mut TaskContext::from_waker(&waker)).is_pending());
        service.cancel();
        drop(lock);
        assert_eq!(request.await, Err(BsvHomeTransferPasswordError::Cancelled));
        assert_eq!(service.verify(snapshot, "fixture".into()).await, Err(BsvHomeTransferPasswordError::Cancelled));
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    });
}

#[test]
fn test_transfer_password_transport_rejects_get_v2_query_suffix_and_unknown_action() {
    block_on(async {
        let provider = Arc::new(TestAlienProvider::with_status(500));
        for (base, path, method) in [
            ("https://portal.test/api/portal", "/wallet-auth/transfer-password/status", AlienHttpMethod::Get),
            ("https://portal.test/api/portal", "/wallet-auth/transfer-password/set?address=other", AlienHttpMethod::Post),
            ("https://portal.test/api/portal", "/wallet-auth/transfer-password/verify/", AlienHttpMethod::Post),
            ("https://portal.test/api/portal", "/wallet-auth/transfer-password/reset", AlienHttpMethod::Post),
            ("https://portal.test/v2/api/portal", "/wallet-auth/transfer-password/status", AlienHttpMethod::Post),
        ] {
            let client = PortalClient::new(provider.clone(), base.into(), "en".into()).unwrap();
            assert!(client.authenticated::<Map<String, Value>>("synthetic-session", path, method, None).await.is_err());
        }
        assert_eq!(provider.requested_paths(), Vec::<String>::new());
    });
}
