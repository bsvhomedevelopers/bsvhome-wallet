use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bsv_home_crypto::random;
use bsv_home_keystore::Mnemonic;
use futures::executor::block_on;
use primitives::{Chain, Wallet, WalletId, WalletSource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::TempDir;
use zeroize::Zeroizing;

use super::{BSVHOME_BASE_URL, CurlPortalProvider, TemporarySecureStore, curl_response, json_type, request_error, schema_fields};
use crate::alien::{AlienError, AlienHttpMethod, AlienProvider, AlienResponse, AlienTarget};
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore, decode_password};
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::error::BsvHomeServiceError;
use crate::services::portal::{BsvHomePortalAuthService, BsvHomePortalTransferPasswordService, BsvHomeTransferPasswordError};
use crate::services::wallet::rules as wallet_rules;
use crate::services::wallet::testkit::MemoryWalletStore;
use crate::services::wallet::{BsvHomeKeystoreAuthentication, BsvHomeKeystorePassword};
use crate::services::wallet_session::BsvHomeWalletSessionService;
use crate::services::wallet_session::testkit::MemoryWalletSessionStore;

const WRITE_OPT_IN: &str = "BSVHOME_TRANSFER_PASSWORD_LIVE_WRITE";

struct PasswordWriteApproval;

impl PasswordWriteApproval {
    fn from_environment() -> Result<Self, &'static str> {
        Self::from_value(std::env::var(WRITE_OPT_IN).ok().as_deref())
    }

    fn from_value(value: Option<&str>) -> Result<Self, &'static str> {
        match value {
            Some("1") => Ok(Self),
            Some(_) | None => Err("explicit temporary transfer-password write approval and opt-in are required"),
        }
    }
}

struct EphemeralKeystorePassword {
    encoded: Zeroizing<String>,
}

impl BsvHomeKeystorePassword for EphemeralKeystorePassword {
    fn get_password(&self, _: bool) -> Result<String, BsvHomeServiceError> {
        Ok(self.encoded.to_string())
    }

    fn get_wallet_password(&self, _: WalletId) -> Result<Option<String>, BsvHomeServiceError> {
        Ok(None)
    }

    fn delete_wallet_password(&self, _: WalletId) -> Result<(), BsvHomeServiceError> {
        Err(BsvHomeServiceError::Unsupported {
            msg: "temporary password deletion is outside this test".into(),
        })
    }

    fn authentication(&self) -> Result<BsvHomeKeystoreAuthentication, BsvHomeServiceError> {
        Ok(BsvHomeKeystoreAuthentication::None)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PasswordLiveAction {
    Challenge,
    Login,
    Status,
    Set,
    Verify,
}

impl PasswordLiveAction {
    fn endpoint(self) -> &'static str {
        match self {
            Self::Challenge => "/wallet-auth/challenge",
            Self::Login => "/wallet-auth/login",
            Self::Status => "/wallet-auth/transfer-password/status",
            Self::Set => "/wallet-auth/transfer-password/set",
            Self::Verify => "/wallet-auth/transfer-password/verify",
        }
    }

    fn requires_authentication(self) -> bool {
        match self {
            Self::Challenge | Self::Login => false,
            Self::Status | Self::Set | Self::Verify => true,
        }
    }
}

const MAX_AUTHENTICATION_PAIRS: usize = 3;
const MAX_PASSWORD_HTTP_REQUESTS: usize = 13;
const PASSWORD_BUSINESS_LIFECYCLE: [PasswordLiveAction; 7] = [
    PasswordLiveAction::Status,
    PasswordLiveAction::Set,
    PasswordLiveAction::Verify,
    PasswordLiveAction::Verify,
    PasswordLiveAction::Set,
    PasswordLiveAction::Verify,
    PasswordLiveAction::Verify,
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthenticationBody<'a> {
    chain: &'a str,
    address: &'a str,
    device_id: &'a str,
    device_type: &'a str,
    challenge_id: Option<&'a str>,
    signature: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusBody<'a> {
    address: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetBody<'a> {
    address: &'a str,
    current_password: Option<&'a str>,
    new_password: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyBody<'a> {
    address: &'a str,
    password: &'a str,
}

#[derive(Serialize)]
struct PasswordResponseSchema {
    endpoint: &'static str,
    http_status: u16,
    envelope_fields: BTreeMap<String, &'static str>,
    data_type: &'static str,
    data_fields: BTreeMap<String, &'static str>,
    data_object_field_count: Option<usize>,
}

#[derive(Default, PartialEq, Eq)]
enum PasswordAuthentication {
    #[default]
    Unauthenticated,
    AwaitingLogin,
    Ready,
}

#[derive(Default)]
struct PasswordRequestBudget {
    business_next: usize,
    challenges: usize,
    logins: usize,
    authentication: PasswordAuthentication,
}

impl PasswordRequestBudget {
    fn total(&self) -> usize {
        self.business_next + self.challenges + self.logins
    }
}

struct PasswordLifecycleProvider {
    _approval: PasswordWriteApproval,
    address: String,
    budget: Mutex<PasswordRequestBudget>,
}

impl Debug for PasswordLifecycleProvider {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PasswordLifecycleProvider")
    }
}

impl PasswordLifecycleProvider {
    fn reserve(&self, action: PasswordLiveAction) -> Result<(), AlienError> {
        let mut budget = self.budget.lock().map_err(|_| request_error("temporary password request counter unavailable"))?;
        if budget.total() >= MAX_PASSWORD_HTTP_REQUESTS || budget.business_next >= PASSWORD_BUSINESS_LIFECYCLE.len() {
            return Err(request_error("temporary password request budget exceeded"));
        }
        match action {
            PasswordLiveAction::Challenge => {
                if budget.authentication == PasswordAuthentication::AwaitingLogin || budget.challenges >= MAX_AUTHENTICATION_PAIRS {
                    return Err(request_error("temporary password authentication budget exceeded"));
                }
                budget.challenges += 1;
                budget.authentication = PasswordAuthentication::AwaitingLogin;
            }
            PasswordLiveAction::Login => {
                if budget.authentication != PasswordAuthentication::AwaitingLogin {
                    return Err(request_error("temporary password login order rejected"));
                }
                budget.logins += 1;
                budget.authentication = PasswordAuthentication::Ready;
            }
            PasswordLiveAction::Status | PasswordLiveAction::Set | PasswordLiveAction::Verify => {
                if budget.authentication != PasswordAuthentication::Ready || PASSWORD_BUSINESS_LIFECYCLE.get(budget.business_next) != Some(&action) {
                    return Err(request_error("temporary password business request order rejected"));
                }
                budget.business_next += 1;
            }
        }
        Ok(())
    }

    fn action(&self, target: &AlienTarget) -> Result<PasswordLiveAction, AlienError> {
        if target.method != AlienHttpMethod::Post {
            return Err(request_error("temporary password test method rejected"));
        }
        let path = target
            .url
            .strip_prefix(BSVHOME_BASE_URL)
            .ok_or_else(|| request_error("temporary password test origin rejected"))?;
        let action = [
            PasswordLiveAction::Challenge,
            PasswordLiveAction::Login,
            PasswordLiveAction::Status,
            PasswordLiveAction::Set,
            PasswordLiveAction::Verify,
        ]
        .into_iter()
        .find(|action| action.endpoint() == path)
        .ok_or_else(|| request_error("temporary password test endpoint rejected"))?;
        let mut authorization = target
            .headers
            .iter()
            .flat_map(|headers| headers.iter())
            .filter(|(name, _)| name.eq_ignore_ascii_case("authorization"))
            .map(|(_, value)| value);
        let credential = authorization.next();
        if authorization.next().is_some()
            || action.requires_authentication() != credential.is_some()
            || credential.is_some_and(|value| value.is_empty() || value.starts_with("Bearer "))
        {
            return Err(request_error("temporary password test authorization boundary rejected"));
        }
        let bytes = target.body.as_deref().ok_or_else(|| request_error("temporary password test body missing"))?;
        match action {
            PasswordLiveAction::Challenge | PasswordLiveAction::Login => {
                let body: AuthenticationBody<'_> = serde_json::from_slice(bytes).map_err(|_| request_error("temporary authentication body rejected"))?;
                let signing = match action {
                    PasswordLiveAction::Challenge => body.challenge_id.is_none() && body.signature.is_none(),
                    PasswordLiveAction::Login => body.challenge_id.is_some_and(|value| !value.is_empty()) && body.signature.is_some_and(|value| !value.is_empty()),
                    PasswordLiveAction::Status | PasswordLiveAction::Set | PasswordLiveAction::Verify => false,
                };
                if body.address != self.address || body.chain != "BSV" || body.device_id.is_empty() || body.device_type != "4" || !signing {
                    return Err(request_error("temporary authentication identity rejected"));
                }
            }
            PasswordLiveAction::Status => {
                let body: StatusBody<'_> = serde_json::from_slice(bytes).map_err(|_| request_error("temporary password status body rejected"))?;
                if body.address != self.address {
                    return Err(request_error("temporary password status identity rejected"));
                }
            }
            PasswordLiveAction::Set => {
                let body: SetBody<'_> = serde_json::from_slice(bytes).map_err(|_| request_error("temporary password set body rejected"))?;
                if body.address != self.address || !is_ephemeral_password(body.new_password) || body.current_password.is_some_and(|value| !is_ephemeral_password(value)) {
                    return Err(request_error("temporary password set identity or format rejected"));
                }
            }
            PasswordLiveAction::Verify => {
                let body: VerifyBody<'_> = serde_json::from_slice(bytes).map_err(|_| request_error("temporary password verify body rejected"))?;
                if body.address != self.address || !is_ephemeral_password(body.password) {
                    return Err(request_error("temporary password verify identity or format rejected"));
                }
            }
        }
        Ok(action)
    }
}

#[async_trait]
impl AlienProvider for PasswordLifecycleProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        let action = self.action(&target)?;
        self.reserve(action)?;
        let (status, data) = curl_response(&target.url, "POST", target.headers, target.body)?;
        let response = serde_json::from_slice::<Value>(&data).ok();
        let payload = response.as_ref().and_then(|value| value.get("data"));
        let schema = PasswordResponseSchema {
            endpoint: action.endpoint(),
            http_status: status,
            envelope_fields: schema_fields(response.as_ref(), &["success", "forceLogout", "code", "message", "data"]),
            data_type: json_type(payload),
            data_fields: schema_fields(payload, &["configured"]),
            data_object_field_count: payload.and_then(Value::as_object).map(|value| value.len()),
        };
        println!(
            "Temporary password response schema: {}",
            serde_json::to_string(&schema).map_err(|_| request_error("safe password schema encoding failed"))?
        );
        Ok(Arc::new(AlienResponse::new(Some(status), data)))
    }
}

fn is_ephemeral_password(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn random_password() -> Result<Zeroizing<String>, &'static str> {
    let bytes = Zeroizing::new(random::bytes::<32>().map_err(|_| "temporary randomness unavailable")?);
    Ok(Zeroizing::new(hex::encode(bytes.as_slice())))
}

struct PasswordLiveFixture {
    _directory: TempDir,
    service: BsvHomePortalTransferPasswordService,
    provider: Arc<PasswordLifecycleProvider>,
}

impl PasswordLiveFixture {
    fn new(approval: PasswordWriteApproval) -> Result<Self, &'static str> {
        let directory = TempDir::new().map_err(|_| "temporary encrypted keystore directory unavailable")?;
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).map_err(|_| "temporary encrypted keystore unavailable")?;
        let password = Arc::new(EphemeralKeystorePassword { encoded: random_password()? });
        let stored = keystore
            .create_store(
                BsvHomeImportType::SinglePhrase {
                    words: Mnemonic::generate(12).map_err(|_| "temporary mnemonic generation failed")?,
                    chain: Chain::BitcoinSv,
                },
                decode_password(&password.encoded),
            )
            .map_err(|_| "temporary encrypted identity creation failed")?;
        let wallet = Wallet {
            id: WalletId::from_id(&stored.wallet_id).ok_or("temporary wallet identity invalid")?,
            external_id: None,
            name: "Unfunded temporary password verification".into(),
            index: 0,
            wallet_type: stored.wallet_type,
            accounts: stored.accounts.into_iter().map(wallet_rules::account).collect(),
            is_pinned: false,
            image_url: None,
            source: WalletSource::Create,
        };
        let provider = Arc::new(PasswordLifecycleProvider {
            _approval: approval,
            address: wallet.account(Chain::BitcoinSv).ok_or("temporary BSV account unavailable")?.address.clone(),
            budget: Mutex::new(PasswordRequestBudget::default()),
        });
        let wallets = Arc::new(MemoryWalletStore::default());
        wallets.wallets.lock().map_err(|_| "temporary wallet store unavailable")?.push(wallet.clone());
        let current = Arc::new(MemoryWalletSessionStore::default());
        *current.current.lock().map_err(|_| "temporary session store unavailable")? = Some(wallet.id.clone());
        let auth = BsvHomePortalAuthService::new(
            provider.clone(),
            keystore,
            password,
            Arc::new(BsvHomeWalletSessionService::new(current, wallets)),
            Arc::new(BsvHomeDeviceKeyService::new(Arc::new(TemporarySecureStore::default()))),
            Arc::new(TemporarySecureStore::default()),
            BSVHOME_BASE_URL.into(),
            "en".into(),
            "4".into(),
        )
        .map_err(|_| "temporary password authentication configuration invalid")?;
        let service = BsvHomePortalTransferPasswordService::new(Arc::new(auth), wallet.id.id()).map_err(|_| "temporary password service configuration invalid")?;
        Ok(Self {
            _directory: directory,
            service,
            provider,
        })
    }
}

#[test]
#[ignore = "requires new explicit approval and BSVHOME_TRANSFER_PASSWORD_LIVE_WRITE=1; seven business requests for one new unfunded temporary identity, at most three authentication pairs and thirteen HTTP requests"]
fn test_live_bsvhome_temporary_transfer_password_lifecycle_after_explicit_write_approval() -> Result<(), &'static str> {
    let approval = PasswordWriteApproval::from_environment()?;
    block_on(async {
        let fixture = PasswordLiveFixture::new(approval)?;
        let original = random_password()?;
        let replacement = random_password()?;
        let incorrect = random_password()?;
        if original == replacement || original == incorrect || replacement == incorrect {
            return Err("temporary password randomness collided");
        }
        let status = fixture.service.status().await.map_err(|_| "temporary password status failed")?;
        if status.configured {
            return Err("new temporary identity unexpectedly has a configured password");
        }
        let configured = fixture
            .service
            .set(status, None, original.to_string(), original.to_string())
            .await
            .map_err(|_| "temporary initial password setting failed")?;
        fixture
            .service
            .verify(configured.clone(), original.to_string())
            .await
            .map_err(|_| "temporary correct-password verification failed")?;
        if fixture.service.verify(configured.clone(), incorrect.to_string()).await != Err(BsvHomeTransferPasswordError::IncorrectPassword) {
            return Err("temporary incorrect-password rejection mismatch");
        }
        let changed = fixture
            .service
            .set(configured, Some(original.to_string()), replacement.to_string(), replacement.to_string())
            .await
            .map_err(|_| "temporary password change failed")?;
        if fixture.service.verify(changed.clone(), original.to_string()).await != Err(BsvHomeTransferPasswordError::IncorrectPassword) {
            return Err("temporary previous-password rejection mismatch");
        }
        fixture
            .service
            .verify(changed, replacement.to_string())
            .await
            .map_err(|_| "temporary replacement-password verification failed")?;
        fixture.service.cancel();
        let budget = fixture.provider.budget.lock().map_err(|_| "temporary request counter unavailable")?;
        if budget.business_next != PASSWORD_BUSINESS_LIFECYCLE.len()
            || budget.authentication != PasswordAuthentication::Ready
            || budget.challenges != budget.logins
            || !(1..=MAX_AUTHENTICATION_PAIRS).contains(&budget.logins)
            || budget.total() > MAX_PASSWORD_HTTP_REQUESTS
        {
            return Err("temporary password request count mismatch");
        }
        println!(
            "Temporary password lifecycle passed: challenge={} login={} status=1 set=2 verify=4 incorrect_attempts=2 total_http={}; no funds, binding, Swap or broadcast action",
            budget.challenges,
            budget.logins,
            budget.total()
        );
        Ok(())
    })
}

#[test]
fn test_default_portal_live_provider_rejects_all_transfer_password_actions_without_transport() {
    for provider in [CurlPortalProvider::default(), CurlPortalProvider::new(BSVHOME_BASE_URL)] {
        for action in [PasswordLiveAction::Status, PasswordLiveAction::Set, PasswordLiveAction::Verify] {
            let path = format!("/api/portal{}", action.endpoint());
            assert!(provider.request_target(&path, AlienHttpMethod::Post).is_err());
        }
        assert_eq!(provider.counts(), (0, 0, 0));
        assert!(provider.schema_metadata().is_empty());
    }
}

#[test]
fn test_temporary_password_write_scope_is_disabled_without_the_exact_opt_in() {
    for value in [None, Some(""), Some("0"), Some("true")] {
        assert!(PasswordWriteApproval::from_value(value).is_err());
    }
}

#[test]
fn test_password_write_provider_refuses_unknown_origin_method_scope_and_request_budget() {
    let provider = PasswordLifecycleProvider {
        _approval: PasswordWriteApproval,
        address: "public-fixture".into(),
        budget: Mutex::new(PasswordRequestBudget::default()),
    };
    for (url, method) in [
        ("https://foreign.example/api/portal/wallet-auth/transfer-password/set", AlienHttpMethod::Post),
        ("https://paymail.bsvhome.com/api/portal/wallet-auth/transfer-password/set", AlienHttpMethod::Get),
        ("https://paymail.bsvhome.com/api/portal/auth/paymail/bindings", AlienHttpMethod::Post),
        ("https://paymail.bsvhome.com/api/portal/auth/swaps/orders", AlienHttpMethod::Post),
        ("https://paymail.bsvhome.com/api/portal/wallet-auth/transfer-password/reset", AlienHttpMethod::Post),
    ] {
        assert!(
            provider
                .action(&AlienTarget {
                    url: url.into(),
                    method,
                    headers: None,
                    body: None
                })
                .is_err()
        );
    }
    assert_eq!(provider.budget.lock().unwrap().total(), 0);
    let challenge_body = br#"{"chain":"BSV","address":"public-fixture","deviceId":"public-device","deviceType":"4"}"#.to_vec();
    assert!(matches!(
        provider.action(&AlienTarget {
            url: format!("{BSVHOME_BASE_URL}/wallet-auth/challenge"),
            method: AlienHttpMethod::Post,
            headers: None,
            body: Some(challenge_body.clone()),
        }),
        Ok(PasswordLiveAction::Challenge)
    ));
    for headers in [
        [("authorization".into(), "public-fixture".into())].into_iter().collect(),
        [("Authorization".into(), "public-fixture".into()), ("authorization".into(), "public-fixture".into())]
            .into_iter()
            .collect(),
    ] {
        assert!(
            provider
                .action(&AlienTarget {
                    url: format!("{BSVHOME_BASE_URL}/wallet-auth/challenge"),
                    method: AlienHttpMethod::Post,
                    headers: Some(headers),
                    body: Some(challenge_body.clone()),
                })
                .is_err()
        );
    }
    assert!(provider.reserve(PasswordLiveAction::Verify).is_err());
    provider.reserve(PasswordLiveAction::Challenge).unwrap();
    assert!(provider.reserve(PasswordLiveAction::Challenge).is_err());
    provider.reserve(PasswordLiveAction::Login).unwrap();
    assert!(provider.reserve(PasswordLiveAction::Login).is_err());
    for action in &PASSWORD_BUSINESS_LIFECYCLE[..4] {
        provider.reserve(*action).unwrap();
    }
    provider.reserve(PasswordLiveAction::Challenge).unwrap();
    provider.reserve(PasswordLiveAction::Login).unwrap();
    for action in &PASSWORD_BUSINESS_LIFECYCLE[4..6] {
        provider.reserve(*action).unwrap();
    }
    provider.reserve(PasswordLiveAction::Challenge).unwrap();
    provider.reserve(PasswordLiveAction::Login).unwrap();
    assert!(provider.reserve(PasswordLiveAction::Challenge).is_err());
    for action in &PASSWORD_BUSINESS_LIFECYCLE[6..] {
        provider.reserve(*action).unwrap();
    }
    assert_eq!(provider.budget.lock().unwrap().total(), MAX_PASSWORD_HTTP_REQUESTS);
    assert!(provider.reserve(PasswordLiveAction::Status).is_err());
    assert!(provider.reserve(PasswordLiveAction::Challenge).is_err());
}
