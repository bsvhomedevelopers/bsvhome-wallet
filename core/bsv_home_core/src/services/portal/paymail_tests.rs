use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use bsv_home_client::sleep;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use futures::channel::oneshot;
use futures::executor::block_on;
use futures::join;
use primitives::{Account, Chain, Wallet};
use serde_json::{Map, Value, from_slice, from_str};

use crate::alien::{AlienHttpMethod, AlienResponse};
use crate::services::error::BsvHomeServiceError;

use super::model::{Challenge, IdentityRequest};
use super::paymail_destination::{destination_address, destination_url};
use super::paymail_model::{Binding, BindingChallenge, BsvHomePaymailBindingIntent, Configuration};
use super::rules;
use super::testkit::{MemoryPaymailProvider, MemoryPublicPaymailProvider, PaymailSubmission, TEST_PAYMAIL_PUBLIC_KEY as TEST_PUBLIC_KEY};
use super::tests::Context;
use super::{
    BsvHomePaymailBindingAvailability, BsvHomePaymailDestination, BsvHomePaymailDestinationService, BsvHomePaymailError, BsvHomePaymailMutationStatus, BsvHomePaymailVerification,
    BsvHomePortalAuthError, paymail_rules,
};

const PUBLIC_ORIGIN: &str = "https://api-dev.hj0252.com";

pub(super) fn configuration() -> Configuration {
    Configuration {
        domains: vec!["api-dev.hj0252.com".into()],
        public_base_url: PUBLIC_ORIGIN.into(),
        network: "BSV_MAINNET".into(),
        address_type: "P2PKH".into(),
        identity_key_rotation: false,
    }
}

#[test]
fn test_paymail_challenge_rejects_every_signed_field_change() {
    let now = Utc::now();
    let intent = BsvHomePaymailBindingIntent {
        wallet_id: "wallet".into(),
        address: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5".into(),
        public_origin: "https://api-dev.hj0252.com".into(),
        handle: "alice@api-dev.hj0252.com".into(),
        active: true,
        previous_version: 0,
    };
    let identity_public_key = "020000000000000000000000000000000000000000000000000000000000000001";
    let expires_at = (now + Duration::minutes(5)).to_rfc3339();
    let mut challenge = BindingChallenge {
        challenge_id: "11111111-1111-4111-8111-111111111111".into(),
        message: format!(
            "WowPay Paymail Binding\nOrigin: {}\nUser: 1\nHandle: {}\nNetwork: BSV_MAINNET\nAddress: {}\nIdentity Key: {}\nActive: true\nVersion: 0\nNonce: 11111111-1111-4111-8111-111111111111\nExpires: {expires_at}",
            intent.public_origin, intent.handle, intent.address, identity_public_key
        ),
        expires_at,
    };
    assert_eq!(paymail_rules::validate_challenge(&challenge, &intent, identity_public_key, now), Ok(()));
    let original = challenge.message.clone();
    for index in 0..11 {
        let mut lines = original.split('\n').map(String::from).collect::<Vec<_>>();
        lines[index].push('x');
        challenge.message = lines.join("\n");
        assert_eq!(
            paymail_rules::validate_challenge(&challenge, &intent, identity_public_key, now),
            Err(BsvHomePaymailError::InvalidChallenge)
        );
    }
    challenge.message = format!("{original}\n");
    assert_eq!(
        paymail_rules::validate_challenge(&challenge, &intent, identity_public_key, now),
        Err(BsvHomePaymailError::InvalidChallenge)
    );
    challenge.message = original.replace('\n', "\r\n");
    assert_eq!(
        paymail_rules::validate_challenge(&challenge, &intent, identity_public_key, now),
        Err(BsvHomePaymailError::InvalidChallenge)
    );
}

#[test]
fn test_paymail_configuration_trust_and_normalized_inputs() {
    assert_eq!(paymail_rules::alias("  Alice_1  "), Ok("alice_1".into()));
    assert_eq!(paymail_rules::domain(" BÜCHER.EXAMPLE "), Ok("xn--bcher-kva.example".into()));
    for name in ["", "_alice", "alice.", "alice@bob", "a/b", "爱丽丝"] {
        assert_eq!(paymail_rules::alias(name), Err(BsvHomePaymailError::InvalidAlias));
    }
    for domain in ["localhost", "127.0.0.1", "[::1]", "example.com/path", "example.com:443", "-example.com", "example..com"] {
        assert_eq!(paymail_rules::domain(domain), Err(BsvHomePaymailError::InvalidDomain));
    }
    for origin in [
        "http://example.com",
        "https://user@example.com",
        "https://example.com:444",
        "https://example.com/path",
        "https://example.com?q=1",
        "https://example.com#x",
    ] {
        assert_eq!(paymail_rules::origin(origin), Err(BsvHomePaymailError::InvalidConfiguration));
    }
    assert_eq!(paymail_rules::origin("https://example.com:443"), Ok("https://example.com".into()));
    let mut config = configuration();
    config.public_base_url = "https://api-dev.hj0252.com:443".into();
    assert_eq!(
        paymail_rules::configuration(config, &[PUBLIC_ORIGIN.into()]).unwrap().public_base_url,
        "https://api-dev.hj0252.com:443"
    );
    assert_eq!(
        paymail_rules::configuration(configuration(), &["https://untrusted.example".into()]).err(),
        Some(BsvHomePaymailError::InvalidConfiguration)
    );
    let mut config = configuration();
    config.network = "BSV_TESTNET".into();
    assert_eq!(
        paymail_rules::configuration(config, &[PUBLIC_ORIGIN.into()]).err(),
        Some(BsvHomePaymailError::InvalidConfiguration)
    );
    let mut config = configuration();
    config.address_type = "P2SH".into();
    assert_eq!(
        paymail_rules::configuration(config, &[PUBLIC_ORIGIN.into()]).err(),
        Some(BsvHomePaymailError::InvalidConfiguration)
    );
}

#[test]
fn test_paymail_snapshot_only_selects_confirmed_current_receiving_handle() {
    let address = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5";
    let wallet = Wallet {
        accounts: vec![Account::mock(Chain::BitcoinSv, address)],
        ..Wallet::mock()
    };
    let binding = Binding {
        handle: "alice@api-dev.hj0252.com".into(),
        address: address.into(),
        identity_public_key: TEST_PUBLIC_KEY.into(),
        active: true,
        version: 1,
    };
    let snapshot = paymail_rules::snapshot(&wallet, &configuration(), vec![binding.clone()]).unwrap();
    assert_eq!(snapshot.active_handle, Some(binding.handle.clone()));
    assert_eq!(snapshot.registration_domains, Vec::<String>::new());
    let mut disabled = binding.clone();
    disabled.active = false;
    assert_eq!(paymail_rules::snapshot(&wallet, &configuration(), vec![disabled]).unwrap().active_handle, None);
    let mut removed = binding.clone();
    removed.handle = "alice@removed.example".into();
    let snapshot = paymail_rules::snapshot(&wallet, &configuration(), vec![removed]).unwrap();
    assert_eq!(snapshot.active_handle, None);
    assert_eq!(snapshot.bindings[0].availability, BsvHomePaymailBindingAvailability::DomainRemoved);
    assert_eq!(snapshot.registration_domains, vec!["api-dev.hj0252.com"]);
    let mut wrong_address = binding.clone();
    wrong_address.address = "1111111111111111111114oLvT2".into();
    let snapshot = paymail_rules::snapshot(&wallet, &configuration(), vec![wrong_address]).unwrap();
    assert_eq!(snapshot.active_handle, None);
    assert_eq!(snapshot.bindings[0].availability, BsvHomePaymailBindingAvailability::AddressMismatch);
    assert_eq!(
        paymail_rules::snapshot(&wallet, &configuration(), vec![binding.clone(), binding]).err(),
        Some(BsvHomePaymailError::InvalidResponse)
    );
}

#[test]
fn test_paymail_binding_registration_disable_restore_and_exact_authenticated_requests() {
    block_on(async {
        let context = Context::new();
        let provider = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::Confirmed));
        let service = context.paymail_service(provider.clone());
        let snapshot = service.load().await.unwrap();
        assert_eq!(snapshot.bindings, Vec::new());
        for (active, previous_version, expected_version) in [(true, 0, 1), (false, 1, 2), (true, 2, 3)] {
            let snapshot = service.load().await.unwrap();
            let intent = service.binding_intent(snapshot, " Alice ".into(), " API-DEV.HJ0252.COM ".into(), active).await.unwrap();
            assert_eq!(intent.previous_version, previous_version);
            let result = service.submit_binding(intent).await.unwrap();
            assert_eq!(result.status, BsvHomePaymailMutationStatus::Confirmed);
            let snapshot = result.snapshot.unwrap();
            assert_eq!(snapshot.bindings[0].version, expected_version);
            assert_eq!(snapshot.bindings[0].active, active);
            assert_eq!(snapshot.active_handle, active.then(|| "alice@api-dev.hj0252.com".into()));
        }
        assert_eq!(*context.password.create_requests.lock().unwrap(), vec![false, false, false]);
        for request in provider.requests.lock().unwrap().iter() {
            assert_eq!(
                request.headers,
                Some(HashMap::from([
                    ("Content-Type".into(), "application/json".into()),
                    ("Cache-Control".into(), "no-store".into()),
                    ("language".into(), "zh-CN".into()),
                    ("Authorization".into(), "portal-test-session".into()),
                ]))
            );
            let method = if request.url.ends_with("/config") || request.url.ends_with("/me") {
                AlienHttpMethod::Get
            } else {
                AlienHttpMethod::Post
            };
            assert_eq!(request.method, method);
        }
    });
}

#[test]
fn test_paymail_submission_reconciles_without_replaying_a_consumed_challenge() {
    block_on(async {
        for (submission, expected_status, expected_snapshot) in [
            (PaymailSubmission::OfflineCommitted, BsvHomePaymailMutationStatus::Reconciled, true),
            (PaymailSubmission::InvalidSuccess, BsvHomePaymailMutationStatus::Reconciled, true),
            (PaymailSubmission::OfflineMissing, BsvHomePaymailMutationStatus::PendingVerification, true),
            (PaymailSubmission::OfflineUnverified, BsvHomePaymailMutationStatus::PendingVerification, false),
            (PaymailSubmission::OfflineAdvanced, BsvHomePaymailMutationStatus::Changed, true),
        ] {
            let context = Context::new();
            let provider = Arc::new(MemoryPaymailProvider::new(submission));
            let service = context.paymail_service(provider.clone());
            let snapshot = service.load().await.unwrap();
            let intent = service.binding_intent(snapshot, "alice".into(), "api-dev.hj0252.com".into(), true).await.unwrap();
            let result = service.submit_binding(intent).await.unwrap();
            assert_eq!(result.status, expected_status);
            assert_eq!(result.snapshot.is_some(), expected_snapshot);
            let paths = provider.paths();
            assert_eq!(paths.iter().filter(|path| *path == "/auth/paymail/bindings").count(), 1);
            if expected_snapshot {
                assert_eq!(paths[paths.len() - 2..], ["/auth/paymail/me", "/auth/paymail/config"]);
            } else {
                assert_eq!(paths.last().unwrap(), "/auth/paymail/me");
            }
        }
    });
}

#[test]
fn test_paymail_binding_auth_failure_reauthenticates_read_only_reconciliation() {
    block_on(async {
        let context = Context::new();
        let provider = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::Unauthorized));
        let service = context.paymail_service(provider.clone());
        let snapshot = service.load().await.unwrap();
        let intent = service.binding_intent(snapshot, "alice".into(), "api-dev.hj0252.com".into(), true).await.unwrap();
        assert_eq!(
            service.submit_binding(intent).await,
            Err(BsvHomePaymailError::Api {
                code: "USER_SESSION_EXPIRED".into(),
                msg: "expired".into()
            })
        );
        let paths = provider.paths();
        assert_eq!(paths.iter().filter(|path| *path == "/auth/paymail/bindings").count(), 1);
        assert_eq!(
            paths[paths.len() - 4..],
            ["/wallet-auth/challenge", "/wallet-auth/login", "/auth/paymail/me", "/auth/paymail/config"]
        );
    });
}

#[test]
fn test_paymail_binding_rejects_wallet_switch_during_challenge_and_serial_stale_intents() {
    block_on(async {
        let context = Context::new();
        let provider = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::Confirmed));
        let service = context.paymail_service(provider.clone());
        let snapshot = service.load().await.unwrap();
        let intent = service.binding_intent(snapshot, "alice".into(), "api-dev.hj0252.com".into(), true).await.unwrap();
        let (started, waiting) = oneshot::channel();
        let (release, paused) = oneshot::channel();
        *provider.challenge_pause.lock().unwrap() = Some((started, paused));
        let submit = service.submit_binding(intent);
        let switch = async {
            waiting.await.unwrap();
            context.switch_wallet();
            release.send(()).unwrap();
        };
        let (result, ()) = join!(submit, switch);
        assert_eq!(
            result,
            Err(BsvHomePaymailError::Auth {
                error: BsvHomePortalAuthError::WalletChanged
            })
        );
        assert_eq!(provider.paths().last().unwrap(), "/auth/paymail/challenges");

        let context = Context::new();
        let provider = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::Confirmed));
        let service = context.paymail_service(provider.clone());
        let snapshot = service.load().await.unwrap();
        let intent = service.binding_intent(snapshot, "alice".into(), "api-dev.hj0252.com".into(), true).await.unwrap();
        let (first, second) = join!(service.submit_binding(intent.clone()), service.submit_binding(intent));
        assert_eq!(first.unwrap().status, BsvHomePaymailMutationStatus::Confirmed);
        assert_eq!(second, Err(BsvHomePaymailError::BindingChanged));
        assert_eq!(provider.paths().iter().filter(|path| *path == "/auth/paymail/bindings").count(), 1);
    });
}

#[test]
fn test_paymail_public_destination_uses_delegated_https_without_portal_credentials() {
    block_on(async {
        let provider = Arc::new(MemoryPublicPaymailProvider::default());
        provider.replies.lock().unwrap().extend([
            Arc::new(AlienResponse::new(
                Some(200),
                br#"{"bsvalias":"1.0","capabilities":{"paymentDestination":"https://delegate.example/address/{alias}@{domain.tld}","6745385c3fc0":false}}"#.to_vec(),
            )),
            Arc::new(AlienResponse::new(
                Some(200),
                br#"{"output":"76a914000000000000000000000000000000000000000088ac"}"#.to_vec(),
            )),
        ]);
        let service = BsvHomePaymailDestinationService::new(provider.clone());
        assert_eq!(
            service.destination(" Alice@ThirdParty.Example ".into(), Some(30_000_000)).await.unwrap(),
            BsvHomePaymailDestination {
                handle: "alice@thirdparty.example".into(),
                address: "1111111111111111111114oLvT2".into(),
                reference: None,
            }
        );
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests[0].url, "https://thirdparty.example/.well-known/bsvalias");
        assert_eq!(requests[1].url, "https://delegate.example/address/alice@thirdparty.example");
        assert_eq!(requests[1].body, Some(br#"{"amount":30000000}"#.to_vec()));
        for request in requests.iter() {
            assert_eq!(
                request.headers,
                Some(HashMap::from([
                    ("Content-Type".into(), "application/json".into()),
                    ("Cache-Control".into(), "no-store".into())
                ]))
            );
        }
    });
}

#[test]
fn test_wallet_paymail_destination_requires_verified_sender_and_sends_metadata_without_credentials() {
    block_on(async {
        let context = Context::new();
        let management = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::Confirmed));
        let sender = Arc::new(context.paymail_service(management.clone()));
        let provider = Arc::new(MemoryPublicPaymailProvider::default());
        let service = BsvHomePaymailDestinationService::new_for_wallet(provider.clone(), sender);
        assert_eq!(service.destination("alice@example.com".into(), None).await, Err(BsvHomePaymailError::SenderRequired));
        assert_eq!(provider.requests.lock().unwrap().len(), 0);
        management.bindings.lock().unwrap().push(Binding {
            handle: "sender@api-dev.hj0252.com".into(),
            address: context.wallet.account(Chain::BitcoinSv).unwrap().address.clone(),
            identity_public_key: TEST_PUBLIC_KEY.into(),
            active: true,
            version: 1,
        });
        provider.replies.lock().unwrap().extend([
            Arc::new(AlienResponse::new(
                Some(200),
                br#"{"bsvalias":"1.0","capabilities":{"paymentDestination":"https://delegate.example/{alias}@{domain.tld}"}}"#.to_vec(),
            )),
            Arc::new(AlienResponse::new(
                Some(200),
                br#"{"output":"76a914000000000000000000000000000000000000000088ac","reference":"recipient-reference"}"#.to_vec(),
            )),
        ]);
        assert_eq!(
            service.destination("alice@example.com".into(), None).await.unwrap(),
            BsvHomePaymailDestination {
                handle: "alice@example.com".into(),
                address: "1111111111111111111114oLvT2".into(),
                reference: Some("recipient-reference".into()),
            }
        );
        let requests = provider.requests.lock().unwrap();
        let mut body: Map<String, Value> = from_slice(requests[1].body.as_ref().unwrap()).unwrap();
        let dt = body.remove("dt").unwrap().as_str().unwrap().to_owned();
        assert_eq!(DateTime::parse_from_rfc3339(&dt).unwrap().to_rfc3339_opts(SecondsFormat::Millis, true), dt);
        assert_eq!(body, from_str(r#"{"senderHandle":"sender@api-dev.hj0252.com"}"#).unwrap());
        for request in requests.iter() {
            assert_eq!(request.headers.as_ref().unwrap().get("Authorization"), None);
            assert_eq!(request.headers.as_ref().unwrap().get("Cookie"), None);
        }
    });
}

#[test]
fn test_wallet_paymail_discovery_and_destination_drop_changed_wallet() {
    block_on(async {
        for pause_at in [1, 2] {
            let context = Context::new();
            let management = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::Confirmed));
            management.bindings.lock().unwrap().push(Binding {
                handle: "sender@api-dev.hj0252.com".into(),
                address: context.wallet.account(Chain::BitcoinSv).unwrap().address.clone(),
                identity_public_key: TEST_PUBLIC_KEY.into(),
                active: true,
                version: 1,
            });
            let sender = Arc::new(context.paymail_service(management));
            let provider = Arc::new(MemoryPublicPaymailProvider::default());
            provider.replies.lock().unwrap().push_back(Arc::new(AlienResponse::new(
                Some(200),
                br#"{"bsvalias":"1.0","capabilities":{"paymentDestination":"https://delegate.example/{alias}"}}"#.to_vec(),
            )));
            provider.replies.lock().unwrap().push_back(Arc::new(AlienResponse::new(
                Some(200),
                br#"{"output":"76a914000000000000000000000000000000000000000088ac"}"#.to_vec(),
            )));
            let (reached, waiting) = oneshot::channel();
            let (resume, resumed) = oneshot::channel();
            *provider.request_pause.lock().unwrap() = Some((pause_at, reached, resumed));
            let service = BsvHomePaymailDestinationService::new_for_wallet(provider.clone(), sender);
            let switching = async {
                waiting.await.unwrap();
                context.switch_wallet();
                resume.send(()).unwrap();
            };
            let (result, ()) = join!(service.destination("alice@example.com".into(), None), switching);
            assert_eq!(
                result,
                Err(BsvHomePaymailError::Auth {
                    error: BsvHomePortalAuthError::WalletChanged
                })
            );
            assert_eq!(provider.requests.lock().unwrap().len(), pause_at);
        }
    });
}

#[test]
fn test_paymail_public_destination_validates_capabilities_outputs_and_empty_json_body() {
    assert_eq!(
        destination_address("76a914000000000000000000000000000000000000000088ac"),
        Ok("1111111111111111111114oLvT2".into())
    );
    for output in [
        "",
        "6a00",
        "76a915000000000000000000000000000000000000000088ac",
        "76a914000000000000000000000000000000000000000088ab",
        "76a914gg0000000000000000000000000000000000000088ac",
    ] {
        assert_eq!(destination_address(output), Err(BsvHomePaymailError::UnsupportedOutput));
    }
    for template in [
        "http://example.com/{alias}",
        "https://user@example.com/{alias}",
        "https://example.com/{unsupported}",
        "https://example.com/{alias}#fragment",
    ] {
        assert_eq!(destination_url(template, "alice", "example.com"), Err(BsvHomePaymailError::UnsupportedCapability));
    }
    block_on(async {
        let provider = Arc::new(MemoryPublicPaymailProvider::default());
        provider.replies.lock().unwrap().extend([
            Arc::new(AlienResponse::new(
                Some(200),
                br#"{"bsvalias":"1.0","capabilities":{"paymentDestination":"https://example.com/{alias}@{domain}"}}"#.to_vec(),
            )),
            Arc::new(AlienResponse::new(Some(404), br#"{"code":"PAYMAIL_NOT_FOUND","message":"unavailable"}"#.to_vec())),
        ]);
        let service = BsvHomePaymailDestinationService::new(provider.clone());
        assert_eq!(
            service.destination("alice@example.com".into(), None).await,
            Err(BsvHomePaymailError::Api {
                code: "PAYMAIL_NOT_FOUND".into(),
                msg: "unavailable".into()
            })
        );
        assert_eq!(provider.requests.lock().unwrap()[1].body, Some(b"{}".to_vec()));
        assert_eq!(
            service.destination("alice@example.com".into(), Some(2_100_000_000_000_001)).await,
            Err(BsvHomePaymailError::InvalidAmount)
        );
    });
}

#[test]
fn test_paymail_pending_change_survives_restart_and_old_me_until_exact_or_newer_version() {
    block_on(async {
        for (version_increment, expected_verification) in [(0, BsvHomePaymailVerification::Reconciled), (1, BsvHomePaymailVerification::Changed)] {
            let context = Context::new();
            let confirmed_provider = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::Confirmed));
            let service = context.paymail_service(confirmed_provider.clone());
            let snapshot = service.load().await.unwrap();
            let registration = service.binding_intent(snapshot, "alice".into(), "api-dev.hj0252.com".into(), true).await.unwrap();
            let registered = service.submit_binding(registration).await.unwrap().snapshot.unwrap();
            let provider = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::OfflineMissing));
            *provider.bindings.lock().unwrap() = confirmed_provider.bindings.lock().unwrap().clone();
            let service = context.paymail_service(provider.clone());
            let intent = service.binding_intent(registered, "alice".into(), "api-dev.hj0252.com".into(), false).await.unwrap();
            let result = service.submit_binding(intent.clone()).await.unwrap();
            assert_eq!(result.status, BsvHomePaymailMutationStatus::PendingVerification);
            assert_eq!(result.snapshot.unwrap().verification, BsvHomePaymailVerification::Pending);
            let restarted = context.paymail_service(provider.clone());
            let snapshot = restarted.load().await.unwrap();
            assert_eq!(snapshot.verification, BsvHomePaymailVerification::Pending);
            assert_eq!(snapshot.active_handle, None);
            assert!(snapshot.bindings[0].active);
            assert_eq!(
                restarted.binding_intent(snapshot, "alice".into(), "api-dev.hj0252.com".into(), false).await,
                Err(BsvHomePaymailError::PendingVerification)
            );
            assert_eq!(restarted.submit_binding(intent).await, Err(BsvHomePaymailError::PendingVerification));
            assert_eq!(provider.paths().iter().filter(|path| *path == "/auth/paymail/challenges").count(), 1);
            assert_eq!(provider.paths().iter().filter(|path| *path == "/auth/paymail/bindings").count(), 1);
            let mut observed = provider.attempted_binding.lock().unwrap().clone().unwrap();
            observed.version += version_increment;
            if version_increment > 0 {
                observed.active = true;
            }
            *provider.bindings.lock().unwrap() = vec![observed];
            let resolved = restarted.load().await.unwrap();
            assert_eq!(resolved.verification, expected_verification);
            assert_eq!(restarted.load().await.unwrap().verification, BsvHomePaymailVerification::Clear);
        }
    });
}

#[test]
fn test_paymail_public_business_rate_limit_at_http_200_preserves_code_and_cooldown() {
    block_on(async {
        for discovery_rejection in [true, false] {
            let provider = Arc::new(MemoryPublicPaymailProvider::default());
            let discovery = Arc::new(AlienResponse::new(
                Some(200),
                br#"{"bsvalias":"1.0","capabilities":{"paymentDestination":"https://example.com/{alias}@{domain}"}}"#.to_vec(),
            ));
            let rejected = Arc::new(AlienResponse::new(Some(200), br#"{"success":false,"code":"RATE_LIMITED","message":"wait"}"#.to_vec()));
            if !discovery_rejection {
                provider.replies.lock().unwrap().push_back(discovery.clone());
            }
            provider.replies.lock().unwrap().push_back(rejected);
            provider.replies.lock().unwrap().extend([
                discovery,
                Arc::new(AlienResponse::new(
                    Some(200),
                    br#"{"output":"76a914000000000000000000000000000000000000000088ac"}"#.to_vec(),
                )),
            ]);
            let service = BsvHomePaymailDestinationService::new(provider.clone());
            assert_eq!(
                service.destination("alice@example.com".into(), None).await,
                Err(BsvHomePaymailError::Api {
                    code: "RATE_LIMITED".into(),
                    msg: "wait".into()
                })
            );
            assert_eq!(
                service.destination("alice@example.com".into(), None).await,
                Err(BsvHomePaymailError::Api {
                    code: "RATE_LIMITED".into(),
                    msg: "RATE_LIMITED".into()
                })
            );
            assert_eq!(provider.requests.lock().unwrap().len(), if discovery_rejection { 1 } else { 2 });
            sleep(StdDuration::from_secs(1)).await;
            service.destination("alice@example.com".into(), None).await.unwrap();
            let times = provider.request_times.lock().unwrap();
            let rejection_index = if discovery_rejection { 0 } else { 1 };
            assert!(times[rejection_index + 1].duration_since(times[rejection_index]) >= StdDuration::from_secs(1));
            assert_eq!(provider.requests.lock().unwrap().len(), if discovery_rejection { 3 } else { 4 });
        }
    });
}

#[test]
fn test_paymail_pending_checkpoint_write_failure_stops_before_binding_submission() {
    block_on(async {
        let context = Context::new();
        let provider = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::Confirmed));
        let service = context.paymail_service(provider.clone());
        let snapshot = service.load().await.unwrap();
        let intent = service.binding_intent(snapshot, "alice".into(), "api-dev.hj0252.com".into(), true).await.unwrap();
        context.fail_secure_write(true);
        assert_eq!(
            service.submit_binding(intent).await,
            Err(BsvHomePaymailError::Auth {
                error: BsvHomePortalAuthError::Service {
                    error: BsvHomeServiceError::Store {
                        msg: "secure store unavailable".into()
                    }
                },
            })
        );
        assert_eq!(provider.paths().last().unwrap(), "/auth/paymail/challenges");
        context.fail_secure_write(false);
        assert_eq!(service.load().await.unwrap().verification, BsvHomePaymailVerification::Clear);
    });
}

#[test]
fn test_portal_login_challenge_allows_bounded_clock_skew_and_rejects_excess() {
    let now = Utc::now();
    let identity = IdentityRequest {
        chain: "BSV",
        address: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5",
        device_id: "device",
        device_type: "4",
    };
    for (offset, expected) in [(0, Ok(())), (6, Ok(())), (5000, Ok(())), (5001, Err(BsvHomePortalAuthError::InvalidChallenge))] {
        let issued_at = (now + Duration::milliseconds(offset)).to_rfc3339();
        let expires_at = (now + Duration::minutes(5)).to_rfc3339();
        let challenge = Challenge {
            challenge_id: "Cclocktest".into(),
            chain: "BSV".into(),
            address: identity.address.into(),
            message: format!(
                "WowPay Wallet Auth\nAddress: {}\nNonce: 0123456789abcdef0123456789abcdef\nIssued At: {issued_at}\nExpiration Time: {expires_at}",
                identity.address
            ),
            expires_at,
        };
        assert_eq!(rules::validate_challenge(&challenge, &identity, "https://portal.test/v2/api/portal", now), expected);
    }
}

#[test]
fn test_paymail_service_scope_rejects_other_current_wallet_before_authentication() {
    block_on(async {
        let context = Context::new();
        let provider = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::Confirmed));
        let service = context.paymail_service(provider.clone());
        context.switch_wallet();
        assert_eq!(
            service.load().await,
            Err(BsvHomePaymailError::Auth {
                error: BsvHomePortalAuthError::WalletChanged
            })
        );
        assert_eq!(provider.paths(), Vec::<String>::new());
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
    });
}

#[test]
fn test_paymail_binding_version_uses_portal_exact_integer_wire_decoder() {
    for version in ["1", "\"1\""] {
        let response = format!(
            r#"{{"handle":"alice@api-dev.hj0252.com","address":"1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5","identityPublicKey":"{TEST_PUBLIC_KEY}","active":true,"version":{version}}}"#
        );
        assert_eq!(serde_json::from_str::<Binding>(&response).unwrap().version, 1);
    }
    for version in [
        "1.0",
        "1e0",
        "-1",
        "\"-1\"",
        "\"+1\"",
        "\"1.0\"",
        "\"0x1\"",
        "\"\"",
        "\"18446744073709551616\"",
        "18446744073709551616",
    ] {
        let response = format!(
            r#"{{"handle":"alice@api-dev.hj0252.com","address":"1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5","identityPublicKey":"{TEST_PUBLIC_KEY}","active":true,"version":{version}}}"#
        );
        assert!(serde_json::from_str::<Binding>(&response).is_err());
    }
}
