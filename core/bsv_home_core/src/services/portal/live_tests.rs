#[path = "multisig_live_tests.rs"]
mod multisig_live_tests;
#[path = "transfer_password_live_tests.rs"]
mod transfer_password_live_tests;

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::process::{Command, Stdio};
use std::str::from_utf8;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use async_trait::async_trait;
use bsv_home_keystore::Mnemonic;
use futures::executor::block_on;
use primitives::{Account, Chain, Wallet, WalletId, WalletSource, WalletType};
use serde_json::{Value, json};
use tempfile::TempDir;
use zeroize::Zeroizing;

use super::paymail_model::PendingBinding;
use super::swap_model::WirePair;
use super::swap_rules;
use super::testkit::MemoryPortalSwapStore;
use super::{
    BsvHomePaymailService, BsvHomePaymailVerification, BsvHomePortalAuthError, BsvHomePortalAuthService, BsvHomePortalSwapDirection, BsvHomePortalSwapError,
    BsvHomePortalSwapService, BsvMultisigTarget,
};
use crate::alien::{AlienError, AlienHttpMethod, AlienProvider, AlienResponse, AlienTarget};
use crate::api::BsvHomeApiClient;
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore, decode_password};
use crate::services::assets::{BsvHomeAssetsService, testkit::MemoryAssetStore};
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomeSecureStore;
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::price::{BsvHomePriceService, testkit::MemoryPriceStore};
use crate::services::wallet::addresses::testkit::pool;
use crate::services::wallet::rules as wallet_rules;
use crate::services::wallet::testkit::{MemoryKeystorePassword, MemoryWalletStore, TEST_PASSWORD};
use crate::services::wallet_session::BsvHomeWalletSessionService;
use crate::services::wallet_session::testkit::MemoryWalletSessionStore;

pub(super) const BASE_URL: &str = "https://api-dev.hj0252.com/api/portal";
pub(super) const PUBLIC_ORIGIN: &str = "https://api-dev.hj0252.com";
const V2_BASE_URL: &str = "https://api-dev.hj0252.com/v2/api/portal";
const BSVHOME_BASE_URL: &str = "https://paymail.bsvhome.com/api/portal";
const BSVHOME_ORIGIN: &str = "https://paymail.bsvhome.com";

#[derive(Default)]
pub(super) struct TemporarySecureStore {
    values: Mutex<HashMap<String, Zeroizing<String>>>,
}

impl TemporarySecureStore {
    pub(super) fn pending_bindings(&self) -> Result<Vec<PendingBinding>, String> {
        self.values
            .lock()
            .unwrap()
            .iter()
            .filter(|(key, _)| key.starts_with("paymail_pending|"))
            .map(|(_, value)| serde_json::from_str(value).map_err(|_| "Temporary Paymail pending state invalid".into()))
            .collect()
    }
}

impl BsvHomeSecureStore for TemporarySecureStore {
    fn get(&self, key: String) -> Result<Option<String>, BsvHomeServiceError> {
        Ok(self.values.lock().unwrap().get(&key).map(|value| value.to_string()))
    }

    fn set(&self, key: String, value: String) -> Result<(), BsvHomeServiceError> {
        self.values.lock().unwrap().insert(key, Zeroizing::new(value));
        Ok(())
    }

    fn remove(&self, key: String) -> Result<(), BsvHomeServiceError> {
        self.values.lock().unwrap().remove(&key);
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct CurlPortalProvider {
    base_url: &'static str,
    challenges: AtomicUsize,
    logins: AtomicUsize,
    pairs: AtomicUsize,
    quotes: AtomicUsize,
    orders: AtomicUsize,
    schema_metadata: Mutex<Vec<Value>>,
}

impl Default for CurlPortalProvider {
    fn default() -> Self {
        Self::new(BASE_URL)
    }
}

impl CurlPortalProvider {
    fn new(base_url: &'static str) -> Self {
        Self {
            base_url,
            challenges: AtomicUsize::new(0),
            logins: AtomicUsize::new(0),
            pairs: AtomicUsize::new(0),
            quotes: AtomicUsize::new(0),
            orders: AtomicUsize::new(0),
            schema_metadata: Mutex::new(Vec::new()),
        }
    }

    fn request_target(&self, path: &str, method: AlienHttpMethod) -> Result<(&'static str, Option<&AtomicUsize>), AlienError> {
        Ok(match (self.base_url, path, method) {
            (BASE_URL | BSVHOME_BASE_URL, "/api/portal/wallet-auth/challenge", AlienHttpMethod::Post)
            | (V2_BASE_URL, "/v2/api/portal/wallet-auth/challenge", AlienHttpMethod::Post) => ("POST", Some(&self.challenges)),
            (BASE_URL | BSVHOME_BASE_URL, "/api/portal/wallet-auth/login", AlienHttpMethod::Post) | (V2_BASE_URL, "/v2/api/portal/wallet-auth/login", AlienHttpMethod::Post) => {
                ("POST", Some(&self.logins))
            }
            (BASE_URL, "/api/portal/auth/swaps/pairs", AlienHttpMethod::Get) | (V2_BASE_URL, "/v2/api/portal/auth/swaps/pairs", AlienHttpMethod::Get) => ("GET", Some(&self.pairs)),
            (BASE_URL, "/api/portal/auth/swaps/quotes", AlienHttpMethod::Post) => ("POST", Some(&self.quotes)),
            (BASE_URL, "/api/portal/auth/swaps/orders?status=active&page=1&size=20", AlienHttpMethod::Get) => ("GET", Some(&self.orders)),
            (BASE_URL, "/api/portal/auth/paymail/config" | "/api/portal/auth/paymail/me", AlienHttpMethod::Get) => ("GET", None),
            (BASE_URL, "/api/portal/auth/paymail/challenges" | "/api/portal/auth/paymail/bindings", AlienHttpMethod::Post) => ("POST", None),
            (BASE_URL, "/.well-known/bsvalias", AlienHttpMethod::Get) => ("GET", None),
            (BASE_URL, path, AlienHttpMethod::Get) if path.starts_with("/api/paymail/id/") => ("GET", None),
            (BASE_URL, path, AlienHttpMethod::Post) if path.starts_with("/api/paymail/address/") => ("POST", None),
            (V2_BASE_URL, "/v2/api/portal/auth/multisig/wallets" | "/v2/api/portal/auth/paymail/config" | "/v2/api/portal/auth/paymail/me", AlienHttpMethod::Get) => ("GET", None),
            (BSVHOME_BASE_URL, "/api/portal/auth/multisig/wallets" | "/api/portal/auth/paymail/config" | "/api/portal/auth/paymail/me", AlienHttpMethod::Get) => ("GET", None),
            _ => return Err(request_error("live authentication test endpoint rejected")),
        })
    }

    pub(super) fn schema_metadata(&self) -> Vec<Value> {
        self.schema_metadata.lock().unwrap().clone()
    }
    fn counts(&self) -> (usize, usize, usize) {
        (
            self.challenges.load(Ordering::Relaxed),
            self.logins.load(Ordering::Relaxed),
            self.pairs.load(Ordering::Relaxed),
        )
    }
}

fn request_error(message: &str) -> AlienError {
    AlienError::RequestError { msg: message.into() }
}

fn config_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn curl_response(url: &str, method: &str, headers: Option<HashMap<String, String>>, body: Option<Vec<u8>>) -> Result<(u16, Vec<u8>), AlienError> {
    let mut config = Zeroizing::new(format!(
        "silent\nshow-error\nmax-time = 20\nconnect-timeout = 10\nmax-redirs = 0\nproto = \"=https\"\nurl = \"{}\"\nrequest = \"{method}\"\nwrite-out = \"\\n%{{http_code}}\"\n",
        config_value(url),
    ));
    for (name, value) in headers.into_iter().flatten() {
        config.push_str(&format!("header = \"{}\"\n", config_value(&format!("{name}: {value}"))));
    }
    if let Some(body) = body {
        let body = Zeroizing::new(String::from_utf8(body).map_err(|_| request_error("invalid authentication request encoding"))?);
        config.push_str(&format!("data-binary = \"{}\"\n", config_value(&body)));
    }
    let mut child = Command::new("curl")
        .args(["--disable", "--config", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| request_error("curl transport unavailable"))?;
    let mut stdin = child.stdin.take().ok_or_else(|| request_error("curl input unavailable"))?;
    stdin.write_all(config.as_bytes()).map_err(|_| request_error("curl input failed"))?;
    drop(stdin);
    let output = child.wait_with_output().map_err(|_| request_error("curl transport failed"))?;
    if !output.status.success() {
        return Err(request_error("curl authentication request failed"));
    }
    let separator = output
        .stdout
        .iter()
        .rposition(|byte| *byte == b'\n')
        .ok_or_else(|| request_error("invalid curl status response"))?;
    let status = from_utf8(&output.stdout[separator + 1..])
        .ok()
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| request_error("invalid curl HTTP status"))?;
    let data = output.stdout[..separator].to_vec();
    Ok((status, data))
}

#[async_trait]
impl AlienProvider for CurlPortalProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        let origin = if self.base_url == BSVHOME_BASE_URL { BSVHOME_ORIGIN } else { PUBLIC_ORIGIN };
        let path = target.url.strip_prefix(origin).ok_or_else(|| request_error("live authentication test origin rejected"))?;
        let (method, count) = self.request_target(path, target.method)?;
        if let Some(count) = count {
            count.fetch_add(1, Ordering::Relaxed);
        }
        let authenticated = target
            .headers
            .as_ref()
            .is_some_and(|headers| headers.keys().any(|name| name.eq_ignore_ascii_case("authorization")));
        if self.base_url == V2_BASE_URL && authenticated != path.starts_with("/v2/api/portal/auth/") {
            return Err(request_error("V2 live test authentication boundary rejected"));
        }
        if self.base_url == BSVHOME_BASE_URL && authenticated != path.starts_with("/api/portal/auth/") {
            return Err(request_error("BSVHome live test authentication boundary rejected"));
        }
        if self.base_url == BASE_URL && !path.starts_with("/api/portal/") && authenticated {
            return Err(request_error("public Paymail request carried authentication"));
        }
        let (status, data) = curl_response(&target.url, method, target.headers, target.body)?;
        if self.base_url == V2_BASE_URL || self.base_url == BSVHOME_BASE_URL {
            let response = serde_json::from_slice::<Value>(&data).ok();
            let payload = response.as_ref().and_then(|value| value.get("data"));
            let code = match response.as_ref().and_then(|value| value.get("code")) {
                Some(Value::String(code)) if code == "OK" || code == "LOGIN_REQUIRED" || code == "40015" => Some(code.clone()),
                Some(Value::Number(code)) if code.is_i64() || code.is_u64() => Some(code.to_string()),
                Some(_) | None => None,
            };
            self.schema_metadata.lock().unwrap().push(json!({
                "endpoint": path,
                "httpStatus": status,
                "code": code,
                "envelopeFields": schema_fields(response.as_ref(), &["success", "forceLogout", "code", "message", "data"]),
                "dataType": json_type(payload),
                "dataFields": schema_fields(payload, &["challengeId", "address", "chain", "message", "expiresAt", "token", "domains", "publicBaseUrl", "network", "addressType", "identityKeyRotation"]),
                "dataObjectFieldCount": payload.and_then(Value::as_object).map(|value| value.len()),
            }));
        }
        if ["/api/portal/auth/paymail/config", "/api/portal/auth/paymail/me", "/api/portal/auth/paymail/bindings"].contains(&path)
            && let Ok(response) = serde_json::from_slice::<Value>(&data)
        {
            let payload = response.get("data");
            let record = payload.and_then(|value| match value {
                Value::Array(records) => records.first(),
                Value::Object(_) => Some(value),
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
            });
            let envelope_fields = schema_fields(Some(&response), &["success", "forceLogout", "code", "message", "data"]);
            let payload_fields = schema_fields(
                record,
                &[
                    "domains",
                    "publicBaseUrl",
                    "network",
                    "addressType",
                    "identityKeyRotation",
                    "handle",
                    "address",
                    "identityPublicKey",
                    "active",
                    "version",
                ],
            );
            self.schema_metadata.lock().unwrap().push(json!({
                "endpoint": path,
                "httpStatus": status,
                "success": response.get("success").and_then(Value::as_bool),
                "envelopeFields": envelope_fields,
                "dataFields": payload_fields,
            }));
        }
        Ok(Arc::new(AlienResponse::new(Some(status), data)))
    }
}

fn schema_fields(value: Option<&Value>, names: &[&str]) -> BTreeMap<String, &'static str> {
    names.iter().map(|name| (name.to_string(), json_type(value.and_then(|value| value.get(name))))).collect()
}

fn json_type(value: Option<&Value>) -> &'static str {
    match value {
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
        None => "missing",
    }
}

fn swap_failure(error: BsvHomePortalSwapError) -> String {
    match error {
        BsvHomePortalSwapError::Auth { error } => authentication_failure(error),
        _ => "Portal swap response failed the documented contract".into(),
    }
}

#[test]
#[ignore = "requires the approved Portal test deployment; only authenticates an unfunded temporary identity and reads swap data"]
fn test_live_portal_pairs_quotes_and_orders_without_creating_or_paying_orders() {
    block_on(async {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let stored = keystore
            .create_store(
                BsvHomeImportType::MulticoinPhrase {
                    words: Mnemonic::generate(12).unwrap(),
                    chains: vec![Chain::BitcoinSv, Chain::Tron],
                },
                decode_password(TEST_PASSWORD),
            )
            .unwrap();
        let wallet = Wallet {
            id: WalletId::from_id(&stored.wallet_id).unwrap(),
            external_id: None,
            name: "Temporary Portal swap verification".into(),
            index: 0,
            wallet_type: stored.wallet_type,
            accounts: stored.accounts.into_iter().map(wallet_rules::account).collect(),
            is_pinned: false,
            image_url: None,
            source: WalletSource::Create,
        };
        let wallets = Arc::new(MemoryWalletStore::default());
        wallets.wallets.lock().unwrap().push(wallet.clone());
        let wallet_store = Arc::new(MemoryWalletSessionStore::default());
        *wallet_store.current.lock().unwrap() = Some(wallet.id);
        let session = Arc::new(BsvHomeWalletSessionService::new(wallet_store, wallets.clone()));
        let provider = Arc::new(CurlPortalProvider::default());
        let auth = Arc::new(
            BsvHomePortalAuthService::new(
                provider.clone(),
                keystore,
                Arc::new(MemoryKeystorePassword::default()),
                session.clone(),
                Arc::new(BsvHomeDeviceKeyService::new(Arc::new(TemporarySecureStore::default()))),
                Arc::new(TemporarySecureStore::default()),
                BASE_URL.into(),
                "zh-CN".into(),
                "4".into(),
            )
            .unwrap(),
        );
        let preferences_store = Arc::new(MemoryPreferencesStore::default());
        let preferences = Arc::new(BsvHomePreferencesService::new(preferences_store.clone()));
        let gateway = Arc::new(BsvHomeGateway::new(provider.clone(), preferences_store, Arc::new(EmptyPreferences)));
        let assets = Arc::new(BsvHomeAssetsService::new(
            Arc::new(BsvHomeApiClient::new(provider.clone())),
            gateway.clone(),
            Arc::new(MemoryAssetStore::default()),
            Arc::new(BsvHomePriceService::new(Arc::new(MemoryPriceStore::default()), preferences.clone())),
            preferences,
            session,
        ));
        let address_pool = pool(gateway.clone(), wallets);
        let swaps = BsvHomePortalSwapService::new(
            auth,
            Arc::new(MemoryPortalSwapStore::default()),
            Arc::new(TemporarySecureStore::default()),
            assets,
            gateway,
            address_pool,
        );
        let pairs = swaps.pairs().await.map_err(swap_failure).unwrap();
        assert_eq!(pairs.len(), 4);
        for direction in [
            BsvHomePortalSwapDirection::BsvToUsdt,
            BsvHomePortalSwapDirection::UsdtToBsv,
            BsvHomePortalSwapDirection::UsdtToTrx,
            BsvHomePortalSwapDirection::TrxToUsdt,
        ] {
            assert_eq!(pairs.iter().filter(|pair| pair.direction == direction).count(), 1);
        }
        println!("Portal pairs: OK; four documented directions, asset identities and decimal scales validated");
        let mut failures = Vec::new();
        for (direction, amount) in [
            (BsvHomePortalSwapDirection::BsvToUsdt, "3"),
            (BsvHomePortalSwapDirection::UsdtToBsv, "20"),
            (BsvHomePortalSwapDirection::UsdtToTrx, "20"),
            (BsvHomePortalSwapDirection::TrxToUsdt, "100"),
        ] {
            sleep(Duration::from_secs(2));
            let outcome = match swaps.quote(direction, amount.into()).await {
                Ok(quote) => format!("OK; estimated payout {}", quote.estimated_to_amount_display),
                Err(BsvHomePortalSwapError::Auth {
                    error: BsvHomePortalAuthError::Api { code, .. },
                }) if matches!(
                    code.as_str(),
                    "SWAP_PROVIDER_UNAVAILABLE" | "SWAP_AMOUNT_OUT_OF_RANGE" | "SWAP_RATE_LIMITED" | "SYS_REQUEST_LIMITED" | "SWAP_PAIR_UNAVAILABLE"
                ) =>
                {
                    code
                }
                Err(error) => {
                    let failure = swap_failure(error);
                    failures.push(format!("{direction:?}: {failure}"));
                    failure
                }
            };
            println!("Portal quote {direction:?}: {outcome}");
        }
        let orders = swaps.orders(true, 1, 20).await.map_err(swap_failure).unwrap();
        assert_eq!(orders.total, 0);
        assert_eq!(orders.records, Vec::new());
        assert_eq!(provider.counts(), (1, 1, 5));
        assert_eq!(provider.quotes.load(Ordering::Relaxed), 4);
        assert_eq!(provider.orders.load(Ordering::Relaxed), 1);
        println!("Portal orders: OK; total/records decoded; no order created and no payment broadcast");
        assert!(failures.is_empty(), "{}", failures.join("; "));
    });
}

pub(super) fn authentication_failure(error: BsvHomePortalAuthError) -> String {
    match error {
        BsvHomePortalAuthError::Api { code, .. }
            if code.starts_with("WALLET_AUTH_") || code.starts_with("USER_SESSION_") || code.starts_with("SWAP_") || code.starts_with("SYS_") || code.starts_with("HTTP_") =>
        {
            format!("Portal API rejected authentication: {code}")
        }
        BsvHomePortalAuthError::Api { .. } => "Portal API returned an unexpected error code".into(),
        BsvHomePortalAuthError::InvalidChallenge => "Portal challenge failed the documented identity or login-purpose contract".into(),
        BsvHomePortalAuthError::InvalidSession => "Portal response failed the documented session contract".into(),
        BsvHomePortalAuthError::InvalidResponse => "Portal response failed the documented response contract".into(),
        BsvHomePortalAuthError::Service { .. } => "Portal transport or local signing failed".into(),
        BsvHomePortalAuthError::MissingBsvAccount => "temporary BSV account missing".into(),
        BsvHomePortalAuthError::WatchOnly => "temporary signing wallet unavailable".into(),
        BsvHomePortalAuthError::WalletChanged => "temporary wallet identity changed".into(),
    }
}

fn wallet_authentication_fixture(base_url: &'static str) -> (TempDir, Wallet, BsvHomePortalAuthService, Arc<CurlPortalProvider>, Arc<MemoryWalletStore>) {
    let directory = TempDir::new().unwrap();
    let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
    let stored = keystore
        .create_store(
            BsvHomeImportType::SinglePhrase {
                words: Mnemonic::generate(12).unwrap(),
                chain: Chain::BitcoinSv,
            },
            decode_password(TEST_PASSWORD),
        )
        .unwrap();
    let wallet = Wallet {
        id: WalletId::from_id(&stored.wallet_id).unwrap(),
        external_id: None,
        name: "Temporary Portal verification".into(),
        index: 0,
        wallet_type: stored.wallet_type,
        accounts: stored.accounts.into_iter().map(wallet_rules::account).collect(),
        is_pinned: false,
        image_url: None,
        source: WalletSource::Create,
    };
    let wallets = Arc::new(MemoryWalletStore::default());
    wallets.wallets.lock().unwrap().push(wallet.clone());
    let wallet_store = Arc::new(MemoryWalletSessionStore::default());
    *wallet_store.current.lock().unwrap() = Some(wallet.id.clone());
    let session = Arc::new(BsvHomeWalletSessionService::new(wallet_store, wallets.clone()));
    let provider = Arc::new(CurlPortalProvider::new(base_url));
    let service = BsvHomePortalAuthService::new(
        provider.clone(),
        keystore,
        Arc::new(MemoryKeystorePassword::default()),
        session,
        Arc::new(BsvHomeDeviceKeyService::new(Arc::new(TemporarySecureStore::default()))),
        Arc::new(TemporarySecureStore::default()),
        base_url.into(),
        "zh-CN".into(),
        "4".into(),
    )
    .unwrap();
    (directory, wallet, service, provider, wallets)
}

#[test]
#[ignore = "requires the approved Portal test deployment and creates an unfunded temporary wallet session"]
fn test_live_portal_wallet_authentication_and_cached_session() {
    block_on(async {
        let (_directory, _, service, provider, _) = wallet_authentication_fixture(BASE_URL);
        service
            .connect()
            .await
            .map_err(|error| format!("{}; challenge/login/pairs counts: {:?}", authentication_failure(error), provider.counts()))
            .unwrap();
        assert_eq!(provider.counts(), (1, 1, 1));
        service
            .connect()
            .await
            .map_err(|error| format!("{}; challenge/login/pairs counts: {:?}", authentication_failure(error), provider.counts()))
            .unwrap();
        assert_eq!(provider.counts(), (1, 1, 2));
    });
}

fn v2_response<T>(provider: &CurlPortalProvider, stage: &str, result: Result<T, BsvHomePortalAuthError>) -> T {
    let metadata = provider.schema_metadata();
    println!(
        "V2 live {stage}; challenge/login/pairs attempts {:?}; HTTP response count {}",
        provider.counts(),
        metadata.len()
    );
    println!("V2 live response metadata: {}", Value::Array(metadata));
    let result = result.map_err(|error| match error {
        BsvHomePortalAuthError::Service { .. } => "Service",
        BsvHomePortalAuthError::MissingBsvAccount => "MissingBsvAccount",
        BsvHomePortalAuthError::WatchOnly => "WatchOnly",
        BsvHomePortalAuthError::WalletChanged => "WalletChanged",
        BsvHomePortalAuthError::InvalidChallenge => "InvalidChallenge",
        BsvHomePortalAuthError::InvalidSession => "InvalidSession",
        BsvHomePortalAuthError::Api { .. } => "Api",
        BsvHomePortalAuthError::InvalidResponse => "InvalidResponse",
    });
    assert_eq!(result.as_ref().err(), None, "V2 live {stage} must return valid authenticated data");
    result.unwrap()
}

#[test]
#[ignore = "requires the approved V2 Portal deployment; authenticates an unfunded temporary wallet and reads multisig and Paymail data"]
fn test_live_v2_wallet_authentication_multisig_and_paymail_reads() {
    block_on(async {
        let (_directory, wallet, service, provider, _) = wallet_authentication_fixture(V2_BASE_URL);
        let response = service.multisig_wallet_request::<Vec<Value>>(&wallet, BsvMultisigTarget::Wallets).await;
        let wallets = v2_response(&provider, "multisig wallets", response);
        println!("V2 live multisig wallet count {}", wallets.len());
        let response = service.swap_wallet_request::<Value>(&wallet, "/auth/paymail/config", AlienHttpMethod::Get, None).await;
        let configuration = v2_response(&provider, "Paymail configuration", response);
        assert!(configuration.is_object());
        let response = service.swap_wallet_request::<Vec<Value>>(&wallet, "/auth/paymail/me", AlienHttpMethod::Get, None).await;
        let bindings = v2_response(&provider, "Paymail bindings", response);
        println!("V2 live Paymail binding count {}", bindings.len());
        assert_eq!(provider.counts(), (1, 1, 0));
    });
}

#[test]
#[ignore = "requires the approved V2 Portal deployment; authenticates an unfunded temporary wallet and reads swap pairs"]
fn test_live_v2_wallet_authentication_and_swap_pairs() {
    block_on(async {
        let (_directory, wallet, service, provider, _) = wallet_authentication_fixture(V2_BASE_URL);
        let response = service.swap_wallet_request::<Vec<WirePair>>(&wallet, "/auth/swaps/pairs", AlienHttpMethod::Get, None).await;
        let pairs = v2_response(&provider, "swap pairs", response);
        let pairs = swap_rules::pairs(pairs)
            .map_err(|_| "V2 live pairs failed the documented asset and direction contract")
            .unwrap();
        assert!(!pairs.is_empty(), "V2 live pairs must include a supported direction");
        println!("V2 live swap pair count {}", pairs.len());
        assert_eq!(provider.counts(), (1, 1, 1));
    });
}

#[test]
#[ignore = "requires the approved bsvhome unfunded temporary identity login; only reads multisig wallets, Paymail configuration and empty bindings"]
fn test_live_bsvhome_ios_authentication_and_read_contracts() {
    block_on(async {
        let (_directory, wallet, service, provider, _) = wallet_authentication_fixture(BSVHOME_BASE_URL);
        let service = Arc::new(service);
        let wallets = service
            .multisig_wallet_request::<Vec<Value>>(&wallet, BsvMultisigTarget::Wallets)
            .await
            .map_err(|_| "BSVHome authentication or wallet read failed");
        println!("BSVHome authentication response field types: {}", Value::Array(provider.schema_metadata()));
        assert!(wallets.unwrap().is_empty(), "new temporary identity must have no collective wallets");
        let paymail = BsvHomePaymailService::new(service.clone(), vec![BSVHOME_ORIGIN.into()], wallet.id.id())
            .map_err(|_| "BSVHome Paymail configuration rejected")
            .unwrap();
        let snapshot = paymail.load().await.map_err(|_| "BSVHome Paymail configuration or empty bindings read failed").unwrap();
        println!("BSVHome read response field types: {}", Value::Array(provider.schema_metadata()));
        assert!(snapshot.wallet_id == wallet.id.id(), "Paymail snapshot must retain the authenticated wallet");
        assert!(snapshot.public_origin.trim_end_matches('/') == BSVHOME_ORIGIN, "untrusted public Paymail origin");
        assert!(!snapshot.domains.is_empty(), "Paymail registration domains must be supplied");
        assert!(snapshot.registration_domains == snapshot.domains, "new temporary identity must have no registered domains");
        assert!(snapshot.bindings.is_empty(), "new temporary identity must have no Paymail bindings");
        assert!(snapshot.active_handle.is_none(), "new temporary identity must have no active Paymail handle");
        assert_eq!(snapshot.verification, BsvHomePaymailVerification::Clear);
        println!("BSVHome Paymail empty bindings: OK; configuration and me decoded through BsvHomePaymailService.load; no registration or binding lifecycle asserted");
        let cached = service
            .multisig_wallet_request::<Vec<Value>>(&wallet, BsvMultisigTarget::Wallets)
            .await
            .map_err(|_| "BSVHome cached-session wallet read failed")
            .unwrap();
        assert!(cached.is_empty(), "new temporary identity must have no collective wallets");
        assert_eq!(provider.counts(), (1, 1, 0));
        assert_eq!(provider.quotes.load(Ordering::Relaxed), 0);
        assert_eq!(provider.orders.load(Ordering::Relaxed), 0);
    });
}

#[test]
#[ignore = "requires the approved bsvhome unfunded temporary identity login; only reads fixed-scope multisig wallets without changing the local selection"]
fn test_live_bsvhome_fixed_wallet_authentication_preserves_current_selection_and_reuses_session() {
    block_on(async {
        let (_directory, wallet, service, provider, wallets) = wallet_authentication_fixture(BSVHOME_BASE_URL);
        let fixed = BsvHomePortalAuthService::new_for_wallet(
            provider.clone(),
            service.keystore.clone(),
            service.password.clone(),
            service.session.clone(),
            service.device_key.clone(),
            service.store.clone(),
            BSVHOME_BASE_URL.into(),
            "zh-CN".into(),
            "4".into(),
            wallet.clone(),
        )
        .map_err(|_| "BSVHome fixed-wallet configuration failed")
        .unwrap();
        let observation_address = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5";
        let observation = Wallet {
            id: WalletId::View(Chain::BitcoinSv, observation_address.into()),
            name: "Temporary local observation wallet".into(),
            wallet_type: WalletType::View,
            ..Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, observation_address)])
        };
        wallets.wallets.lock().unwrap().push(observation.clone());
        fixed.session.set_current_wallet_id(Some(observation.id.clone())).unwrap();
        let bound = fixed.multisig_wallet().await.map_err(|_| "BSVHome fixed signer lookup failed").unwrap();
        assert!(bound.id == wallet.id, "fixed signer must remain the temporary signing identity");

        for _ in 0..2 {
            let response = fixed
                .multisig_wallet_request::<Vec<Value>>(&wallet, BsvMultisigTarget::Wallets)
                .await
                .map_err(|_| "BSVHome fixed-scope authentication or wallet read failed");
            println!("BSVHome fixed-scope response field types: {}", Value::Array(provider.schema_metadata()));
            assert!(response.unwrap().is_empty(), "new temporary identity must have no collective wallets");
            assert!(
                fixed.session.current_wallet_id().unwrap() == observation.id,
                "fixed-scope reads must preserve the selected local observation wallet"
            );
            assert_eq!(provider.counts(), (1, 1, 0));
        }
        assert_eq!(provider.schema_metadata().len(), 4);
        assert_eq!(provider.quotes.load(Ordering::Relaxed), 0);
        assert_eq!(provider.orders.load(Ordering::Relaxed), 0);
    });
}
