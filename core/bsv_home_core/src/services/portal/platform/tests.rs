use std::sync::Arc;

use futures::channel::oneshot;
use futures::executor::block_on;
use futures::join;
use serde_json::Value;

use crate::alien::AlienHttpMethod;

use super::client::parse_response;
use super::rules;
use super::testkit::{MemoryPlatformAuthenticator, MemoryPlatformProvider, MemoryPlatformSecureStore, PlatformReply};
use super::{
    BsvHomePlatformAccountError, BsvHomePlatformAccountService, BsvHomePlatformAuthMethod, BsvHomePlatformClosureStatus, BsvHomePlatformPasskeyOperation, BsvHomePlatformSession,
};

const BASE: &str = "https://api-dev.hj0252.com/v2/api/";
const EMAIL_CHALLENGE: &str = r#"{"success":true,"code":"OK","data":{"challengeId":"11111111111111111111111111111111","expiresAt":"2099-01-01T00:00:00Z","retryAfterSeconds":60}}"#;
const EMAIL_SESSION: &str = r#"{"success":true,"code":"OK","data":{"token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","userId":"42","expiresAt":"2099-01-01T00:00:00Z","authMethod":"EMAIL_OTP","walletAuthority":false}}"#;
const SECOND_SESSION: &str = r#"{"success":true,"code":"OK","data":{"token":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","userId":"43","expiresAt":"2099-01-01T00:00:00Z","authMethod":"EMAIL_OTP","walletAuthority":false}}"#;
const PASSKEY_START: &str = r#"{"success":true,"code":"OK","data":{"challengeId":"22222222222222222222222222222222","expiresAt":"2099-01-01T00:00:00Z","options":{"publicKey":{"challenge":"eA","rpId":"test.example"}}}}"#;
const STEP_UP: &str = r#"{"success":true,"code":"OK","data":{"grant":"single-use-test-grant","actionHash":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}}"#;

fn service(provider: Arc<MemoryPlatformProvider>, store: Arc<MemoryPlatformSecureStore>, auth: Arc<MemoryPlatformAuthenticator>) -> BsvHomePlatformAccountService {
    BsvHomePlatformAccountService::new(provider, store, auth, BASE.into(), "zh-CN".into(), "ios-test-installation".into()).unwrap()
}

async fn login(service: &BsvHomePlatformAccountService, provider: &MemoryPlatformProvider, session: &str) {
    provider.reply(200, EMAIL_CHALLENGE);
    provider.reply(200, session);
    let challenge = service.email_start("Person+tag@Example.com".into()).await.unwrap();
    service.email_finish(challenge.id, "001234".into()).await.unwrap();
}

#[test]
fn test_platform_envelope_rejects_unknown_errors_and_incomplete_success() {
    for (status, body) in [
        (200, r#"{"success":true,"data":{}}"#),
        (200, r#"{"success":true,"code":"OK"}"#),
        (200, r#"{"success":false,"code":"OK","data":{}}"#),
        (500, r#"{"success":true,"code":"OK","data":{}}"#),
        (200, "<html>upstream failure</html>"),
    ] {
        assert!(parse_response(status, body.as_bytes()).is_err());
    }
    assert_eq!(
        parse_response(200, br#"{"code":"INTERNAL_SECRET_PATH","message":"do not display"}"#),
        Err(BsvHomePlatformAccountError::Api { code: "SERVICE_ERROR".into() })
    );
    assert_eq!(
        parse_response(503, br#"{"code":"PORTAL_AUTH_UNAVAILABLE"}"#),
        Err(BsvHomePlatformAccountError::Api {
            code: "PORTAL_AUTH_UNAVAILABLE".into()
        })
    );
    assert_eq!(
        parse_response(200, br#"{"success":true,"code":"OK","forceLogout":true,"data":{}}"#),
        Err(BsvHomePlatformAccountError::LoginRequired)
    );
    assert_eq!(parse_response(200, &vec![b' '; 4_194_305]), Err(BsvHomePlatformAccountError::InvalidResponse));
    assert_eq!(parse_response(200, br#"{"code":"LOGIN_REQUIRED"}"#), Err(BsvHomePlatformAccountError::LoginRequired));
}

#[test]
fn test_platform_environment_and_email_boundaries() {
    for url in [
        "http://api-dev.hj0252.com/v2/api/",
        "https://user@api-dev.hj0252.com/v2/api/",
        "https://api-dev.hj0252.com/api/",
        "https://api-dev.hj0252.com/v2/api/?x=1",
        "https://api-dev.hj0252.com/v2/api/%2e%2e/",
    ] {
        assert_eq!(rules::base_url(url), Err(BsvHomePlatformAccountError::InvalidConfiguration));
    }
    assert_eq!(rules::email(" Person.Name+Tag@Example.com ").unwrap(), "person.name+tag@example.com");
    for email in ["a@", "a@@example.com", "a@-example.com", "a..b@example.com", "中文@example.com"] {
        assert_eq!(rules::email(email), Err(BsvHomePlatformAccountError::InvalidInput));
    }
    assert_eq!(rules::code("001234"), Ok(()));
    assert_eq!(rules::code("12345"), Err(BsvHomePlatformAccountError::InvalidInput));
}

#[test]
fn test_email_login_has_isolated_v2_session_and_exact_request_contract() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let store = Arc::new(MemoryPlatformSecureStore::default());
        let account = service(provider.clone(), store.clone(), Arc::new(MemoryPlatformAuthenticator::default()));
        login(&account, &provider, EMAIL_SESSION).await;
        let session = account.session().unwrap().unwrap();
        assert_eq!(
            (session.user_id, session.auth_method, session.wallet_authority),
            ("42".into(), BsvHomePlatformAuthMethod::EmailOtp, false)
        );
        let requests = provider.requests.lock().unwrap();
        assert_eq!(
            requests.iter().map(|target| target.url.as_str()).collect::<Vec<_>>(),
            vec![
                "https://api-dev.hj0252.com/v2/api/portal/identity/email/start",
                "https://api-dev.hj0252.com/v2/api/portal/identity/email/finish"
            ]
        );
        assert_eq!(requests[0].method, AlienHttpMethod::Post);
        assert_eq!(requests[0].headers.as_ref().unwrap().get("Authorization"), None);
        let expected: Value = serde_json::from_str(r#"{"deviceId":"ios-test-installation","deviceType":"ios","email":"person+tag@example.com"}"#).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(requests[0].body.as_ref().unwrap()).unwrap(), expected);
        let expected: Value =
            serde_json::from_str(r#"{"deviceId":"ios-test-installation","deviceType":"ios","challengeId":"11111111111111111111111111111111","code":"001234"}"#).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(requests[1].body.as_ref().unwrap()).unwrap(), expected);
        drop(requests);
        let other = BsvHomePlatformAccountService::new(
            provider,
            store,
            Arc::new(MemoryPlatformAuthenticator::default()),
            "https://isolated.example/v2/api/".into(),
            "en".into(),
            "ios-test-installation".into(),
        )
        .unwrap();
        assert_eq!(other.session().unwrap(), None);
    });
}

#[test]
fn test_late_unauthorized_does_not_remove_replacement_account_session() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let store = Arc::new(MemoryPlatformSecureStore::default());
        let account = service(provider.clone(), store, Arc::new(MemoryPlatformAuthenticator::default()));
        login(&account, &provider, EMAIL_SESSION).await;
        let (started, waiting) = oneshot::channel();
        let (release, resumed) = oneshot::channel();
        provider
            .replies
            .lock()
            .unwrap()
            .push_back(PlatformReply::Paused(started, resumed, 401, b"not JSON".to_vec()));
        let old = account.profile();
        let replace = async {
            waiting.await.unwrap();
            login(&account, &provider, SECOND_SESSION).await;
            release.send(()).unwrap();
        };
        let (old, ()) = join!(old, replace);
        assert_eq!(old, Err(BsvHomePlatformAccountError::LoginRequired));
        assert_eq!(account.session().unwrap().unwrap().user_id, "43");
        let request = &provider.requests.lock().unwrap()[2];
        assert_eq!(
            request.headers.as_ref().unwrap().get("Authorization").unwrap(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    });
}

#[test]
fn test_passkey_cancel_and_changed_action_never_reach_authorized_write() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let auth = Arc::new(MemoryPlatformAuthenticator::default());
        let account = service(provider.clone(), Arc::new(MemoryPlatformSecureStore::default()), auth.clone());
        provider.reply(200, PASSKEY_START);
        *auth.result.lock().unwrap() = Err(BsvHomePlatformAccountError::Cancelled);
        assert_eq!(account.passkey_login().await, Err(BsvHomePlatformAccountError::Cancelled));
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
        assert_eq!(account.session().unwrap(), None);
        assert_eq!(*auth.calls.lock().unwrap(), vec![BsvHomePlatformPasskeyOperation::Login]);
        *auth.result.lock().unwrap() = MemoryPlatformAuthenticator::default().result.into_inner().unwrap();
        login(&account, &provider, EMAIL_SESSION).await;
        provider.reply(200, PASSKEY_START);
        provider.reply(200, STEP_UP);
        assert_eq!(account.authorize(&"d".repeat(64)).await, Err(BsvHomePlatformAccountError::InvalidResponse));
        assert_eq!(provider.requests.lock().unwrap().len(), 5);
    });
}

#[test]
fn test_closed_account_only_clears_session_after_valid_closed_response() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let account = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            Arc::new(MemoryPlatformAuthenticator::default()),
        );
        login(&account, &provider, EMAIL_SESSION).await;
        assert_eq!(
            account.closure_request(false, true).await,
            Err(BsvHomePlatformAccountError::Api {
                code: "CLOSURE_ACKNOWLEDGEMENT_REQUIRED".into()
            })
        );
        provider.reply(200, r#"{"success":true,"code":"OK","data":{"policy":{"version":"ACCOUNT_CLOSURE_P1_V1","chainAddressesRemainValid":true,"walletOwnershipRetained":true,"recoveryMaterialsRetained":true,"deletionScheduled":false},"actionHash":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}}"#);
        provider.reply(200, PASSKEY_START);
        provider.reply(200, STEP_UP);
        provider.reply(
            200,
            r#"{"success":true,"code":"OK","data":{"id":"11111111111111111111111111111111","status":"CLOSED","readyAt":null,"policyVersion":"ACCOUNT_CLOSURE_P1_V1"}}"#,
        );
        let result = account.closure_finish("1".repeat(32), true, true).await.unwrap();
        assert_eq!(result.status, BsvHomePlatformClosureStatus::Closed);
        assert_eq!(account.session().unwrap(), None);
    });
}

#[test]
fn test_reopen_requires_email_factor_and_does_not_create_intermediate_session() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let account = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            Arc::new(MemoryPlatformAuthenticator::default()),
        );
        provider.reply(200, PASSKEY_START);
        provider.reply(200, r#"{"success":true,"code":"OK","data":{"nextFactor":"VERIFIED_EMAIL_OTP","sessionIssued":false,"challengeId":"11111111111111111111111111111111","expiresAt":"2099-01-01T00:00:00Z"}}"#);
        let challenge = account.reopen_start().await.unwrap();
        assert_eq!(challenge.retry_after_seconds, None);
        assert_eq!(account.session().unwrap(), None);
        assert_eq!(
            account.email_finish(challenge.id.clone(), "001234".into()).await,
            Err(BsvHomePlatformAccountError::SessionChanged)
        );
        provider.reply(200, r#"{"success":true,"code":"OK","data":{"token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","userId":"42","expiresAt":"2099-01-01T00:00:00Z","authMethod":"ACCOUNT_REOPEN_PASSKEY_EMAIL","walletAuthority":true}}"#);
        assert_eq!(
            account.reopen_finish(challenge.id, "001234".into()).await.unwrap().auth_method,
            BsvHomePlatformAuthMethod::AccountReopenPasskeyEmail
        );
    });
}

#[test]
fn test_storage_failures_are_not_cache_misses_and_cancel_discards_old_challenges() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let store = Arc::new(MemoryPlatformSecureStore::default());
        let account = service(provider.clone(), store.clone(), Arc::new(MemoryPlatformAuthenticator::default()));
        *store.fail_reads.lock().unwrap() = true;
        assert_eq!(account.session(), Err(BsvHomePlatformAccountError::SecureStorageUnavailable));
        *store.fail_reads.lock().unwrap() = false;
        provider.reply(200, EMAIL_CHALLENGE);
        let challenge = account.email_start("person@example.com".into()).await.unwrap();
        account.cancel().unwrap();
        assert_eq!(account.email_finish(challenge.id, "001234".into()).await, Err(BsvHomePlatformAccountError::SessionChanged));
        assert_eq!(provider.requests.lock().unwrap().len(), 1);
    });
}

#[test]
fn test_paymail_gets_use_the_platform_session_and_request_targets_reject_other_routes() {
    block_on(async {
        let provider = Arc::new(MemoryPlatformProvider::default());
        let account = service(
            provider.clone(),
            Arc::new(MemoryPlatformSecureStore::default()),
            Arc::new(MemoryPlatformAuthenticator::default()),
        );
        login(&account, &provider, EMAIL_SESSION).await;
        let lease = account.session_lease().unwrap();
        provider.reply(200, r#"{"success":true,"code":"OK","data":{}}"#);
        provider.reply(200, r#"{"success":true,"code":"OK","data":[]}"#);
        account.authenticated_get_json(&lease, "portal/auth/paymail/config").await.unwrap();
        account.authenticated_get_json(&lease, "portal/auth/paymail/me").await.unwrap();
        assert_eq!(
            account.authenticated_get_json(&lease, "https://receiver.test/.well-known/bsvalias").await,
            Err(BsvHomePlatformAccountError::InvalidInput)
        );
        let body: Value = serde_json::from_str("{}").unwrap();
        assert_eq!(
            account.authenticated_json("portal/auth/paymail/config", body.clone()).await,
            Err(BsvHomePlatformAccountError::InvalidInput)
        );
        assert_eq!(
            account.authenticated_json("portal/admin/profile", body).await,
            Err(BsvHomePlatformAccountError::InvalidInput)
        );
        account.cancel().unwrap();
        assert_eq!(
            account.authenticated_get_json(&lease, "portal/auth/paymail/config").await,
            Err(BsvHomePlatformAccountError::SessionChanged)
        );
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert!(requests[2].url.ends_with("/v2/api/portal/auth/paymail/config"));
        assert!(requests[3].url.ends_with("/v2/api/portal/auth/paymail/me"));
        for request in &requests[2..] {
            assert_eq!(request.method, AlienHttpMethod::Get);
            assert_eq!(request.body, None);
            assert_eq!(
                request.headers.as_ref().unwrap().get("Authorization").unwrap(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            );
        }
    });
}

#[test]
fn test_user_identifier_preserves_integer_precision_and_local_timestamp_is_not_rewritten() {
    let session: BsvHomePlatformSession =
        serde_json::from_str(r#"{"userId":9007199254740993,"expiresAt":"2026-11-03 11:27:19","authMethod":"EMAIL_OTP","walletAuthority":false}"#).unwrap();
    assert_eq!(session.user_id, "9007199254740993");
    assert_eq!(session.expires_at, "2026-11-03 11:27:19");
    assert_eq!(rules::unexpired(&session.expires_at), Ok(()));
    assert_eq!(rules::unexpired("2026-11-03T11:27:19"), Ok(()));
    assert_eq!(rules::unexpired("2000-01-01T00:00:00Z"), Err(BsvHomePlatformAccountError::LoginRequired));
    assert!(serde_json::from_str::<BsvHomePlatformSession>(r#"{"userId":42.5,"expiresAt":"2099-01-01T00:00:00","authMethod":"EMAIL_OTP","walletAuthority":false}"#).is_err());
    assert_eq!(rules::timestamp("sometime tomorrow"), Err(BsvHomePlatformAccountError::InvalidResponse));
}
