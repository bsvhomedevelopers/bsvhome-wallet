use std::collections::{HashMap, VecDeque};
use std::fmt::{Debug, Formatter};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use futures::channel::oneshot;
use futures::executor::block_on;
use futures::join;
use num_bigint::{BigInt, BigUint};
use primitives::testkit::ABANDON_PHRASE;
use primitives::{Account, AssetId, AssetType, Chain, Wallet, WalletId, WalletType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::TempDir;

use super::model::{Challenge, IdentityRequest, Session};
use super::swap_model::{BsvHomePortalSwapDirection, OrderRequest, PendingOperation, SwapState, USDT_CONTRACT, WireOrder, WirePair, WireQuote};
use super::testkit::MemoryPortalSwapStore;
use super::{BsvHomePaymailError, BsvHomePaymailService, BsvHomePortalAuthError, BsvHomePortalAuthService, BsvMultisigTarget, SESSION, rules};
use super::{BsvHomePortalSwapError, BsvHomePortalSwapService, BsvHomePortalSwapStore, BsvHomePortalSwapSyncStatus, swap_rules};
use crate::alien::{AlienError, AlienHttpMethod, AlienProvider, AlienResponse, AlienTarget};
use crate::api::BsvHomeApiClient;
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore, decode_password};
use crate::message::signer::recover_bitcoin_message_address;
use crate::services::assets::{BsvHomeAssetStore, BsvHomeAssetsService, testkit::MemoryAssetStore};
use crate::services::confirm::BsvHomeTransactionCheckpointData;
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomeSecureStore;
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::price::{BsvHomePriceService, testkit::MemoryPriceStore};
use crate::services::wallet::addresses::testkit::pool_with_keystore;
use crate::services::wallet::testkit::{MemoryKeystorePassword, MemoryWalletStore, TEST_PASSWORD};
use crate::services::wallet::{BsvHomeKeystoreAuthentication, BsvHomeKeystorePassword, rules as wallet_rules};
use crate::services::wallet_session::BsvHomeWalletSessionService;
use crate::services::wallet_session::testkit::MemoryWalletSessionStore;

const BASE_URL: &str = "https://api-dev.hj0252.com/api/portal";
const V2_BASE_URL: &str = "https://api-dev.hj0252.com/v2/api/portal";
const TEST_TOKEN: &str = "portal-test-session";
const TEST_CHALLENGE_ID: &str = "C0123456789abcdef0123456789abcdef";
const CHALLENGE_PATH: &str = "/wallet-auth/challenge";
const LOGIN_PATH: &str = "/wallet-auth/login";
const PAIRS_PATH: &str = "/auth/swaps/pairs";
const MULTISIG_WALLETS_PATH: &str = "/auth/multisig/wallets";

#[derive(Clone, Copy, PartialEq)]
enum StoreOperation {
    Read,
    Write,
    Remove,
}

#[derive(Default)]
struct MemorySecureStore {
    values: Mutex<HashMap<String, String>>,
    failure: Mutex<Option<StoreOperation>>,
    writes: AtomicUsize,
    on_write: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

impl MemorySecureStore {
    fn check(&self, operation: StoreOperation) -> Result<(), BsvHomeServiceError> {
        if *self.failure.lock().unwrap() == Some(operation) {
            return Err(BsvHomeServiceError::Store {
                msg: "secure store unavailable".into(),
            });
        }
        Ok(())
    }
}

impl BsvHomeSecureStore for MemorySecureStore {
    fn get(&self, key: String) -> Result<Option<String>, BsvHomeServiceError> {
        self.check(StoreOperation::Read)?;
        Ok(self.values.lock().unwrap().get(&key).cloned())
    }

    fn set(&self, key: String, value: String) -> Result<(), BsvHomeServiceError> {
        self.check(StoreOperation::Write)?;
        self.values.lock().unwrap().insert(key, value);
        self.writes.fetch_add(1, Ordering::Relaxed);
        let on_write = self.on_write.lock().unwrap().take();
        if let Some(on_write) = on_write {
            on_write();
        }
        Ok(())
    }

    fn remove(&self, key: String) -> Result<(), BsvHomeServiceError> {
        self.check(StoreOperation::Remove)?;
        self.values.lock().unwrap().remove(&key);
        Ok(())
    }
}

struct UnavailablePassword;

impl BsvHomeKeystorePassword for UnavailablePassword {
    fn get_password(&self, _: bool) -> Result<String, BsvHomeServiceError> {
        Err(BsvHomeServiceError::Cancelled)
    }

    fn get_wallet_password(&self, _: WalletId) -> Result<Option<String>, BsvHomeServiceError> {
        Err(BsvHomeServiceError::Cancelled)
    }

    fn delete_wallet_password(&self, _: WalletId) -> Result<(), BsvHomeServiceError> {
        Err(BsvHomeServiceError::Cancelled)
    }

    fn authentication(&self) -> Result<BsvHomeKeystoreAuthentication, BsvHomeServiceError> {
        Err(BsvHomeServiceError::Cancelled)
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestBody {
    chain: String,
    address: String,
    device_id: String,
    device_type: String,
    challenge_id: Option<String>,
    signature: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireResponse<T> {
    success: bool,
    force_logout: bool,
    code: &'static str,
    message: &'static str,
    data: Option<T>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireChallenge {
    challenge_id: String,
    address: String,
    chain: String,
    message: String,
    expires_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireLogin {
    token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chain: Option<String>,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct EmptyObject {}

#[derive(Clone, Copy)]
enum ChallengeFault {
    EmptyId,
    Chain,
    Address,
    Expired,
    Purpose,
    MissingAddress,
    DuplicateAddress,
    Device,
    DeviceType,
    UppercaseId,
    UppercaseNonce,
    ExtraContent,
}

#[derive(Clone, Copy)]
enum LoginFault {
    EmptyToken,
    WhitespaceToken,
    ControlToken,
    NonAsciiToken,
    Address,
    Chain,
    Expired,
}

enum Reply {
    Success,
    WalletChallenge(WireChallenge),
    TokenOnlyLogin,
    LoginExpiry(&'static str),
    Challenge(ChallengeFault),
    Login(LoginFault),
    Rejected {
        status: u16,
        success: bool,
        code: &'static str,
        force_logout: bool,
    },
    ObjectRejected {
        status: u16,
        code: &'static str,
        force_logout: bool,
    },
    Offline,
    Swap(serde_json::Value),
    Raw {
        status: u16,
        data: Vec<u8>,
    },
}

struct Step {
    reply: Reply,
    started: Option<oneshot::Sender<()>>,
    release: Option<oneshot::Receiver<()>>,
}

impl Step {
    fn reply(reply: Reply) -> Self {
        Self {
            reply,
            started: None,
            release: None,
        }
    }

    fn paused() -> (Self, oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (started, received) = oneshot::channel();
        let (release, waiting) = oneshot::channel();
        (
            Self {
                reply: Reply::Success,
                started: Some(started),
                release: Some(waiting),
            },
            received,
            release,
        )
    }
}

#[derive(Default)]
struct RecordingProvider {
    requests: Mutex<Vec<AlienTarget>>,
    messages: Mutex<Vec<String>>,
    steps: Mutex<VecDeque<Step>>,
}

impl Debug for RecordingProvider {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RecordingPortalProvider")
    }
}

impl RecordingProvider {
    fn paths(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.url.strip_prefix(BASE_URL).or_else(|| request.url.strip_prefix(V2_BASE_URL)).unwrap().to_string())
            .collect()
    }

    fn replies(&self, replies: Vec<Reply>) {
        *self.steps.lock().unwrap() = replies.into_iter().map(Step::reply).collect();
    }

    fn reset(&self) {
        self.requests.lock().unwrap().clear();
        self.messages.lock().unwrap().clear();
        self.steps.lock().unwrap().clear();
    }
}

fn response<T: Serialize>(status: u16, data: T) -> Arc<AlienResponse> {
    Arc::new(AlienResponse::new(
        Some(status),
        serde_json::to_vec(&WireResponse {
            success: true,
            force_logout: false,
            code: "OK",
            message: "OK",
            data: Some(data),
        })
        .unwrap(),
    ))
}

fn empty_optional_order_page(context: &Context) -> Value {
    let mut response: Value = serde_json::from_str(include_str!("../../../testdata/portal/order_page_usdt_bsv_empty_optionals.json")).unwrap();
    let (payout, refund) = swap_rules::wallet_addresses(&context.wallet, BsvHomePortalSwapDirection::UsdtToBsv).unwrap();
    response["records"][0]["payoutAddress"] = payout.into();
    response["records"][0]["refundAddress"] = refund.into();
    response
}

fn pending_usdt_to_bsv_operation(context: &Context) -> PendingOperation {
    let original: WirePair = serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap();
    let pair = WirePair {
        from: original.to,
        to: original.from,
        provider_code: "CHANGENOW".into(),
        provider_display_name: "ChangeNOW".into(),
        ..original
    };
    let mut quote: WireQuote = serde_json::from_str(include_str!("../../../testdata/portal/quote_usdt_bsv_empty_maximum.json")).unwrap();
    quote.expires_at = Utc::now() + Duration::minutes(1);
    let quote = swap_rules::quote(quote, &swap_rules::quote_request(&pair, BigUint::from(20_000_000u32)), &pair, Utc::now()).unwrap();
    let (payout_address, refund_address) = swap_rules::wallet_addresses(&context.wallet, quote.direction).unwrap();
    PendingOperation {
        request: OrderRequest {
            quote_id: quote.quote_id.clone(),
            client_operation_id: "11111111-1111-4111-8111-111111111111".into(),
            payout_address,
            refund_address,
        },
        quote,
        order_id: None,
    }
}

fn challenge(address: &str) -> WireChallenge {
    let now = Utc::now();
    let expires_at = now + Duration::minutes(5);
    WireChallenge {
        challenge_id: "Ctestchallenge".into(),
        address: address.into(),
        chain: "BSV".into(),
        message: format!(
            "WowPay Wallet Auth\nAddress: {address}\nNonce: 0123456789abcdef0123456789abcdef\nIssued At: {}\nExpiration Time: {}",
            now.to_rfc3339_opts(SecondsFormat::AutoSi, true),
            expires_at.to_rfc3339_opts(SecondsFormat::AutoSi, true)
        ),
        expires_at: expires_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
    }
}

fn device_challenge(address: &str, device_id: &str, device_type: &str) -> WireChallenge {
    let mut challenge = challenge(address);
    challenge.challenge_id = TEST_CHALLENGE_ID.into();
    challenge.message.push_str(&format!("\nDevice: {device_id}\nDevice Type: {device_type}"));
    challenge
}

fn login(address: &str) -> WireLogin {
    WireLogin {
        token: TEST_TOKEN.into(),
        address: Some(address.into()),
        chain: Some("BSV".into()),
        expires_at: None,
    }
}

#[async_trait]
impl AlienProvider for RecordingProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        self.requests.lock().unwrap().push(target.clone());
        let step = self.steps.lock().unwrap().pop_front().unwrap_or_else(|| Step::reply(Reply::Success));
        if let Some(started) = step.started {
            started.send(()).unwrap();
        }
        if let Some(release) = step.release {
            release.await.unwrap();
        }
        let path = target.url.strip_prefix(BASE_URL).or_else(|| target.url.strip_prefix(V2_BASE_URL)).unwrap();
        match step.reply {
            Reply::ObjectRejected { status, code, force_logout } => Ok(Arc::new(AlienResponse::new(
                Some(status),
                serde_json::to_vec(&WireResponse {
                    success: false,
                    force_logout,
                    code,
                    message: "session rejected",
                    data: Some(EmptyObject {}),
                })
                .unwrap(),
            ))),
            Reply::TokenOnlyLogin => {
                assert_eq!(path, LOGIN_PATH);
                Ok(response(
                    200,
                    WireLogin {
                        token: TEST_TOKEN.into(),
                        address: None,
                        chain: None,
                        expires_at: None,
                    },
                ))
            }
            Reply::LoginExpiry(expires_at) => {
                assert_eq!(path, LOGIN_PATH);
                let body: RequestBody = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                Ok(response(
                    200,
                    serde_json::json!({
                        "token": TEST_TOKEN,
                        "address": body.address,
                        "chain": "BSV",
                        "expiresAt": expires_at,
                    }),
                ))
            }
            Reply::Rejected {
                status,
                success,
                code,
                force_logout,
            } => Ok(Arc::new(AlienResponse::new(
                Some(status),
                serde_json::to_vec(&WireResponse::<Vec<()>> {
                    success,
                    force_logout,
                    code,
                    message: "session rejected",
                    data: None,
                })
                .unwrap(),
            ))),
            Reply::Offline => Err(AlienError::Offline),
            Reply::Swap(value) => Ok(response(200, value)),
            Reply::Raw { status, data } => Ok(Arc::new(AlienResponse::new(Some(status), data))),
            Reply::WalletChallenge(challenge) => Ok(response(200, challenge)),
            Reply::Challenge(fault) => {
                assert_eq!(path, CHALLENGE_PATH);
                let body: RequestBody = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                let mut data = device_challenge(&body.address, &body.device_id, &body.device_type);
                match fault {
                    ChallengeFault::EmptyId => data.challenge_id.clear(),
                    ChallengeFault::Chain => data.chain = "TRON".into(),
                    ChallengeFault::Address => data.address = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5".into(),
                    ChallengeFault::Expired => data.expires_at = (Utc::now() - Duration::seconds(1)).to_rfc3339(),
                    ChallengeFault::Purpose => data.message = format!("Pay Invoice\nAddress: {}", body.address),
                    ChallengeFault::MissingAddress => data.message = "WowPay Wallet Auth\nNonce: test-nonce".into(),
                    ChallengeFault::DuplicateAddress => data.message.push_str(&format!("\nAddress: {}", body.address)),
                    ChallengeFault::Device => data.message = data.message.replace(&format!("Device: {}", body.device_id), "Device: another-device"),
                    ChallengeFault::DeviceType => data.message = data.message.replace("Device Type: 4", "Device Type: 3"),
                    ChallengeFault::UppercaseId => data.challenge_id = data.challenge_id.to_ascii_uppercase(),
                    ChallengeFault::UppercaseNonce => data.message = data.message.replace("0123456789abcdef0123456789abcdef", "0123456789ABCDEF0123456789ABCDEF"),
                    ChallengeFault::ExtraContent => data.message.push_str("\nPurpose: transfer"),
                }
                Ok(response(200, data))
            }
            Reply::Login(fault) => {
                assert_eq!(path, LOGIN_PATH);
                let body: RequestBody = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                let mut data = login(&body.address);
                match fault {
                    LoginFault::EmptyToken => data.token.clear(),
                    LoginFault::WhitespaceToken => data.token = "test token".into(),
                    LoginFault::ControlToken => data.token = "test\r\ntoken".into(),
                    LoginFault::NonAsciiToken => data.token = "测试token".into(),
                    LoginFault::Address => data.address = Some("1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5".into()),
                    LoginFault::Chain => data.chain = Some("TRON".into()),
                    LoginFault::Expired => data.expires_at = Some(Utc::now() - Duration::seconds(1)),
                }
                Ok(response(200, data))
            }
            Reply::Success => match path {
                CHALLENGE_PATH => {
                    let body: RequestBody = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                    let data = if target.url.starts_with(V2_BASE_URL) {
                        challenge(&body.address)
                    } else {
                        device_challenge(&body.address, &body.device_id, &body.device_type)
                    };
                    self.messages.lock().unwrap().push(data.message.clone());
                    Ok(response(200, data))
                }
                LOGIN_PATH => {
                    let body: RequestBody = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                    Ok(response(200, login(&body.address)))
                }
                PAIRS_PATH | MULTISIG_WALLETS_PATH => Ok(response(200, Vec::<()>::new())),
                _ => panic!("unexpected Portal endpoint"),
            },
        }
    }
}

pub(super) struct Context {
    _directory: TempDir,
    keystore: Arc<BsvHomeKeystore>,
    pub(super) wallet: Wallet,
    wallets: Arc<MemoryWalletStore>,
    wallet_store: Arc<MemoryWalletSessionStore>,
    session: Arc<BsvHomeWalletSessionService>,
    device_key: Arc<BsvHomeDeviceKeyService>,
    store: Arc<MemorySecureStore>,
    pub(super) password: Arc<MemoryKeystorePassword>,
    provider: Arc<RecordingProvider>,
}

impl Context {
    pub(super) fn paymail_service(&self, provider: Arc<dyn AlienProvider>) -> BsvHomePaymailService {
        BsvHomePaymailService::new(self.auth_service(provider), vec!["https://api-dev.hj0252.com".into()], self.wallet.id.id()).unwrap()
    }

    pub(super) fn auth_service(&self, provider: Arc<dyn AlienProvider>) -> Arc<BsvHomePortalAuthService> {
        self.cache(self.cached_session(None));
        let auth = BsvHomePortalAuthService::new(
            provider,
            self.keystore.clone(),
            self.password.clone(),
            self.session.clone(),
            self.device_key.clone(),
            self.store.clone(),
            BASE_URL.into(),
            "zh-CN".into(),
            "4".into(),
        )
        .unwrap();
        Arc::new(auth)
    }

    pub(super) fn fail_secure_write(&self, enabled: bool) {
        *self.store.failure.lock().unwrap() = enabled.then_some(StoreOperation::Write);
    }
    fn swap_service(&self, store: Arc<MemoryPortalSwapStore>) -> Arc<BsvHomePortalSwapService> {
        self.swap_service_with_assets(store, Arc::new(MemoryAssetStore::default()))
    }

    fn swap_service_with_assets(&self, store: Arc<MemoryPortalSwapStore>, asset_store: Arc<MemoryAssetStore>) -> Arc<BsvHomePortalSwapService> {
        let preferences_store = Arc::new(MemoryPreferencesStore::default());
        let preferences = Arc::new(BsvHomePreferencesService::new(preferences_store.clone()));
        let gateway = Arc::new(BsvHomeGateway::new(self.provider.clone(), preferences_store, Arc::new(EmptyPreferences)));
        let assets = Arc::new(BsvHomeAssetsService::new(
            Arc::new(BsvHomeApiClient::new(self.provider.clone())),
            gateway.clone(),
            asset_store,
            Arc::new(BsvHomePriceService::new(Arc::new(MemoryPriceStore::default()), preferences.clone())),
            preferences,
            self.session.clone(),
        ));
        let address_pool = pool_with_keystore(
            gateway.clone(),
            self.wallets.clone(),
            self.keystore.clone(),
            self.password.clone(),
            self.wallets.bsv_addresses.clone(),
        );
        Arc::new(BsvHomePortalSwapService::new(
            Arc::new(self.service()),
            store,
            Arc::new(MemorySecureStore::default()),
            assets,
            gateway,
            address_pool,
        ))
    }

    pub(super) fn new() -> Self {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let stored = keystore
            .create_store(
                BsvHomeImportType::SinglePhrase {
                    words: ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                    chain: Chain::BitcoinSv,
                },
                decode_password(TEST_PASSWORD),
            )
            .unwrap();
        let mut wallet = Wallet {
            id: WalletId::from_id(&stored.wallet_id).unwrap(),
            wallet_type: stored.wallet_type,
            accounts: stored.accounts.into_iter().map(wallet_rules::account).collect(),
            ..Wallet::mock()
        };
        wallet.accounts.push(Account::mock(Chain::Tron, "TBKwjUtXVsX1r724C1V52nocBgtioDjx9u"));
        let wallets = Arc::new(MemoryWalletStore::default());
        wallets.wallets.lock().unwrap().push(wallet.clone());
        let wallet_store = Arc::new(MemoryWalletSessionStore::default());
        *wallet_store.current.lock().unwrap() = Some(wallet.id.clone());
        let session = Arc::new(BsvHomeWalletSessionService::new(wallet_store.clone(), wallets.clone()));
        Self {
            _directory: directory,
            keystore,
            wallet,
            wallets,
            wallet_store,
            session,
            device_key: Arc::new(BsvHomeDeviceKeyService::new(Arc::new(MemorySecureStore::default()))),
            store: Arc::new(MemorySecureStore::default()),
            password: Arc::new(MemoryKeystorePassword::default()),
            provider: Arc::new(RecordingProvider::default()),
        }
    }

    fn service(&self) -> BsvHomePortalAuthService {
        self.service_with_password(self.password.clone())
    }

    fn service_with_password(&self, password: Arc<dyn BsvHomeKeystorePassword>) -> BsvHomePortalAuthService {
        self.service_at(BASE_URL, password)
    }

    pub(super) fn service_at(&self, base_url: &str, password: Arc<dyn BsvHomeKeystorePassword>) -> BsvHomePortalAuthService {
        BsvHomePortalAuthService::new(
            self.provider.clone(),
            self.keystore.clone(),
            password,
            self.session.clone(),
            self.device_key.clone(),
            self.store.clone(),
            base_url.into(),
            "zh-CN".into(),
            "4".into(),
        )
        .unwrap()
    }

    pub(super) fn fixed_service(&self) -> BsvHomePortalAuthService {
        BsvHomePortalAuthService::new_for_wallet(
            self.provider.clone(),
            self.keystore.clone(),
            self.password.clone(),
            self.session.clone(),
            self.device_key.clone(),
            self.store.clone(),
            BASE_URL.into(),
            "zh-CN".into(),
            "4".into(),
            self.wallet.clone(),
        )
        .unwrap()
    }

    fn cached_session(&self, expires_at: Option<DateTime<Utc>>) -> Session {
        Session {
            base_url: BASE_URL.into(),
            wallet_id: self.wallet.id.id(),
            address: self.wallet.account(Chain::BitcoinSv).unwrap().address.clone(),
            token: TEST_TOKEN.into(),
            expires_at,
        }
    }

    fn cache(&self, session: Session) {
        self.store.values.lock().unwrap().insert(SESSION.into(), serde_json::to_string(&session).unwrap());
    }

    pub(super) fn replace_wallet(&self, wallet: Option<Wallet>) {
        let mut wallets = self.wallets.wallets.lock().unwrap();
        wallets.retain(|wallet| wallet.id != self.wallet.id);
        if let Some(wallet) = wallet {
            wallets.push(wallet);
        }
    }

    pub(super) fn switch_wallet(&self) {
        let wallet = Wallet {
            id: WalletId::Multicoin("second-test-wallet".into()),
            accounts: vec![Account::mock(Chain::BitcoinSv, "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5")],
            ..Wallet::mock()
        };
        self.wallets.wallets.lock().unwrap().push(wallet.clone());
        *self.wallet_store.current.lock().unwrap() = Some(wallet.id);
    }
}

#[test]
fn test_fixed_wallet_authentication_keeps_the_other_current_wallet_unchanged() {
    block_on(async {
        let context = Context::new();
        let auth = context.fixed_service();
        context.switch_wallet();
        let current = context.session.current_wallet_id().unwrap();
        assert_eq!(auth.multisig_wallet().await.unwrap().id, context.wallet.id);
        let result = auth.multisig_wallet_request::<Vec<()>>(&context.wallet, BsvMultisigTarget::Wallets).await;
        assert_eq!(result, Ok(Vec::new()));
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, MULTISIG_WALLETS_PATH]);
        assert_eq!(context.session.current_wallet_id().unwrap(), current);
    });
}

#[test]
fn test_fixed_wallet_scope_rejects_ordinary_paymail_swap_and_foreign_identities() {
    block_on(async {
        let context = Context::new();
        let auth = Arc::new(context.fixed_service());
        let paymail = BsvHomePaymailService::new(auth.clone(), vec!["https://api-dev.hj0252.com".into()], context.wallet.id.id()).unwrap();
        assert_eq!(
            paymail.load().await,
            Err(BsvHomePaymailError::Auth {
                error: BsvHomePortalAuthError::WalletChanged
            })
        );
        assert_eq!(auth.connect().await, Err(BsvHomePortalAuthError::WalletChanged));
        context.switch_wallet();
        let other = context.session.current_wallet().await.unwrap();
        assert_eq!(
            auth.multisig_wallet_request::<Vec<()>>(&other, BsvMultisigTarget::Wallets).await,
            Err(BsvHomePortalAuthError::WalletChanged)
        );
        assert_eq!(
            context.service().multisig_wallet_request::<Vec<()>>(&context.wallet, BsvMultisigTarget::Wallets).await,
            Err(BsvHomePortalAuthError::WalletChanged)
        );
        assert_eq!(context.provider.paths(), Vec::<String>::new());
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
    });
}

#[test]
fn test_fixed_wallet_deletion_during_a_read_discards_the_result_and_clears_its_session() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let auth = context.fixed_service();
        let (step, received, release) = Step::paused();
        context.provider.steps.lock().unwrap().push_back(step);
        let request = auth.multisig_wallet_request::<Vec<()>>(&context.wallet, BsvMultisigTarget::Wallets);
        let remove_wallet = async {
            received.await.unwrap();
            context.wallets.wallets.lock().unwrap().retain(|wallet| wallet.id != context.wallet.id);
            release.send(()).unwrap();
        };
        let (result, ()) = join!(request, remove_wallet);
        assert_eq!(result, Err(BsvHomePortalAuthError::WalletChanged));
        assert_eq!(context.provider.paths(), [MULTISIG_WALLETS_PATH]);
        assert_eq!(context.store.get(SESSION.into()).unwrap(), None);
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
    });
}

#[test]
fn test_v1_wallet_authentication_accepts_only_the_ios_device_bound_challenge() {
    block_on(async {
        let context = Context::new();
        let auth = context.service();
        let address = context.wallet.account(Chain::BitcoinSv).unwrap().address.clone();
        let device_challenge = device_challenge(&address, &context.device_key.device_id().unwrap(), "4");
        let message = device_challenge.message.clone();
        context.provider.replies(vec![Reply::WalletChallenge(device_challenge)]);
        let wallets = auth.multisig_wallet_request::<Vec<()>>(&context.wallet, BsvMultisigTarget::Wallets).await;
        assert_eq!(wallets, Ok(Vec::new()));
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, MULTISIG_WALLETS_PATH]);
        let requests = context.provider.requests.lock().unwrap();
        let signed: RequestBody = serde_json::from_slice(requests[1].body.as_ref().unwrap()).unwrap();
        assert_eq!(signed.challenge_id.as_deref(), Some(TEST_CHALLENGE_ID));
        assert_eq!(signed.device_id, context.device_key.device_id().unwrap());
        assert_eq!(signed.device_type, "4");
        assert_eq!(recover_bitcoin_message_address(message.as_bytes(), signed.signature.as_ref().unwrap()).unwrap(), address);
    });
}

#[test]
fn test_v1_wallet_authentication_rejects_legacy_challenge_before_key_access() {
    block_on(async {
        let context = Context::new();
        let address = context.wallet.account(Chain::BitcoinSv).unwrap().address.clone();
        context.provider.replies(vec![Reply::WalletChallenge(challenge(&address))]);
        let result = context.service().multisig_wallet_request::<Vec<()>>(&context.wallet, BsvMultisigTarget::Wallets).await;
        assert_eq!(result, Err(BsvHomePortalAuthError::InvalidChallenge));
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH]);
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        assert_eq!(context.store.writes.load(Ordering::Relaxed), 0);
    });
}

#[test]
fn test_v1_challenge_enforces_exact_lifetime_and_bounded_clock_skew() {
    let now = DateTime::parse_from_rfc3339("2026-11-02T12:00:00Z").unwrap().with_timezone(&Utc);
    let identity = IdentityRequest {
        chain: "BSV",
        address: "test-address",
        device_id: "ios-device",
        device_type: "4",
    };
    for (issued_offset, lifetime, accepted) in [
        (0, 300_000, true),
        (60_000, 300_000, true),
        (60_001, 300_000, false),
        (-299_999, 300_000, true),
        (-300_000, 300_000, false),
        (0, 299_999, false),
        (0, 300_001, false),
    ] {
        let issued_at = (now + Duration::milliseconds(issued_offset)).to_rfc3339_opts(SecondsFormat::AutoSi, true);
        let expires_at = (now + Duration::milliseconds(issued_offset + lifetime)).to_rfc3339_opts(SecondsFormat::AutoSi, true);
        let challenge = Challenge {
            challenge_id: TEST_CHALLENGE_ID.into(),
            address: identity.address.into(),
            chain: "BSV".into(),
            message: format!(
                "WowPay Wallet Auth\nAddress: test-address\nNonce: 0123456789abcdef0123456789abcdef\nIssued At: {issued_at}\nExpiration Time: {expires_at}\nDevice: ios-device\nDevice Type: 4"
            ),
            expires_at,
        };
        assert_eq!(
            rules::validate_challenge(&challenge, &identity, BASE_URL, now),
            if accepted { Ok(()) } else { Err(BsvHomePortalAuthError::InvalidChallenge) },
            "issued offset {issued_offset}, lifetime {lifetime}"
        );
    }
}

#[test]
fn test_v1_challenge_rejects_noncanonical_identifiers_nonces_and_timestamps() {
    let now = DateTime::parse_from_rfc3339("2026-11-02T12:00:00Z").unwrap().with_timezone(&Utc);
    let identity = IdentityRequest {
        chain: "BSV",
        address: "test-address",
        device_id: "ios-device",
        device_type: "4",
    };
    for (id, nonce, issued_at) in [
        ("C0123456789abcdef0123456789abcde", "0123456789abcdef0123456789abcdef", "2026-11-02T12:00:00Z"),
        ("C0123456789abcdef0123456789abcdef0", "0123456789abcdef0123456789abcdef", "2026-11-02T12:00:00Z"),
        ("C0123456789ABCDEF0123456789ABCDEF", "0123456789abcdef0123456789abcdef", "2026-11-02T12:00:00Z"),
        (TEST_CHALLENGE_ID, "0123456789abcdef0123456789abcde", "2026-11-02T12:00:00Z"),
        (TEST_CHALLENGE_ID, "0123456789abcdef0123456789abcdef0", "2026-11-02T12:00:00Z"),
        (TEST_CHALLENGE_ID, "0123456789ABCDEF0123456789ABCDEF", "2026-11-02T12:00:00Z"),
        (TEST_CHALLENGE_ID, "0123456789abcdef0123456789abcdef", "2026-11-02T12:00:00+00:00"),
    ] {
        let challenge = Challenge {
            challenge_id: id.into(),
            address: identity.address.into(),
            chain: "BSV".into(),
            message: format!(
                "WowPay Wallet Auth\nAddress: test-address\nNonce: {nonce}\nIssued At: {issued_at}\nExpiration Time: 2026-11-02T12:05:00Z\nDevice: ios-device\nDevice Type: 4"
            ),
            expires_at: "2026-11-02T12:05:00Z".into(),
        };
        assert_eq!(
            rules::validate_challenge(&challenge, &identity, BASE_URL, now),
            Err(BsvHomePortalAuthError::InvalidChallenge)
        );
    }
}

#[test]
fn test_v2_wallet_authentication_accepts_only_the_legacy_protocol() {
    block_on(async {
        for device_bound in [false, true] {
            let context = Context::new();
            let auth = context.service_at(V2_BASE_URL, context.password.clone());
            let address = context.wallet.account(Chain::BitcoinSv).unwrap().address.clone();
            let challenge = if device_bound {
                device_challenge(&address, &context.device_key.device_id().unwrap(), "4")
            } else {
                challenge(&address)
            };
            context.provider.replies(vec![Reply::WalletChallenge(challenge)]);
            let result = auth.multisig_wallet_request::<Vec<()>>(&context.wallet, BsvMultisigTarget::Wallets).await;
            if device_bound {
                assert_eq!(result, Err(BsvHomePortalAuthError::InvalidChallenge));
                assert_eq!(context.provider.paths(), [CHALLENGE_PATH]);
                assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
                assert_eq!(context.store.writes.load(Ordering::Relaxed), 0);
            } else {
                assert_eq!(result, Ok(Vec::new()));
                assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, MULTISIG_WALLETS_PATH]);
            }
        }
    });
}

#[test]
fn test_portal_protocol_switch_reauthenticates_and_keeps_multisig_origin_canonical() {
    block_on(async {
        let context = Context::new();
        for base_url in [BASE_URL, V2_BASE_URL, BASE_URL] {
            context.provider.reset();
            let auth = context.service_at(base_url, context.password.clone());
            assert_eq!(auth.multisig_origin(), "https://api-dev.hj0252.com");
            let result = auth.multisig_wallet_request::<Vec<()>>(&context.wallet, BsvMultisigTarget::Wallets).await;
            assert_eq!(result, Ok(Vec::new()));
            assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, MULTISIG_WALLETS_PATH]);
            let cached: Session = serde_json::from_str(&context.store.get(SESSION.into()).unwrap().unwrap()).unwrap();
            assert_eq!(cached.base_url, base_url);
        }
        assert_eq!(*context.password.create_requests.lock().unwrap(), [false, false, false]);
    });
}

#[test]
fn test_connect_sends_wallet_proof_and_exact_portal_headers() {
    block_on(async {
        let context = Context::new();
        context.service().connect().await.unwrap();
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH]);
        let requests = context.provider.requests.lock().unwrap();
        let unauthenticated_headers = HashMap::from([
            ("Content-Type".into(), "application/json".into()),
            ("Cache-Control".into(), "no-store".into()),
            ("language".into(), "zh-CN".into()),
        ]);
        assert_eq!(requests[0].method, AlienHttpMethod::Post);
        assert_eq!(requests[1].method, AlienHttpMethod::Post);
        assert_eq!(requests[2].method, AlienHttpMethod::Get);
        assert_eq!(requests[0].headers, Some(unauthenticated_headers.clone()));
        assert_eq!(requests[1].headers, Some(unauthenticated_headers.clone()));
        let mut authenticated_headers = unauthenticated_headers;
        authenticated_headers.insert("Authorization".into(), TEST_TOKEN.into());
        assert_eq!(requests[2].headers, Some(authenticated_headers));
        assert_eq!(requests[2].body, None);
        let address = context.wallet.account(Chain::BitcoinSv).unwrap().address.clone();
        let body: RequestBody = serde_json::from_slice(requests[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(
            body,
            RequestBody {
                chain: "BSV".into(),
                address: address.clone(),
                device_id: context.device_key.device_id().unwrap(),
                device_type: "4".into(),
                challenge_id: None,
                signature: None
            },
        );
        let signed: RequestBody = serde_json::from_slice(requests[1].body.as_ref().unwrap()).unwrap();
        assert_eq!(signed.chain, "BSV");
        assert_eq!(signed.address, address);
        assert_eq!(signed.device_id, body.device_id);
        assert_eq!(signed.device_type, "4");
        assert_eq!(signed.challenge_id.as_deref(), Some(TEST_CHALLENGE_ID));
        let encoded_signature = signed.signature.unwrap();
        assert_eq!(
            recover_bitcoin_message_address(context.provider.messages.lock().unwrap()[0].as_bytes(), &encoded_signature).unwrap(),
            address
        );
        let signature = BASE64.decode(encoded_signature).unwrap();
        assert_eq!(signature.len(), 65);
        assert!(matches!(signature[0], 31..=34));
        assert_eq!(*context.password.create_requests.lock().unwrap(), [false]);
        assert_eq!(context.store.writes.load(Ordering::Relaxed), 1);
    });
}

#[test]
fn test_multisig_first_fetch_authenticates_without_swap_or_paymail_requests() {
    block_on(async {
        let context = Context::new();
        let auth = context.service();
        let wallet = auth.multisig_wallet().await.unwrap();
        assert_eq!(wallet.id.id(), context.wallet.id.id());
        assert_eq!(auth.multisig_origin(), "https://api-dev.hj0252.com");
        let response = auth.multisig_wallet_request::<Vec<Value>>(&wallet, BsvMultisigTarget::Wallets).await.unwrap();
        assert_eq!(response, Vec::<Value>::new());
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, MULTISIG_WALLETS_PATH]);
        let requests = context.provider.requests.lock().unwrap();
        assert_eq!(requests[2].method, AlienHttpMethod::Get);
        assert_eq!(requests[2].body, None);
        assert_eq!(requests[2].headers.as_ref().unwrap().get("Authorization").unwrap(), TEST_TOKEN);
        assert_eq!(requests[2].headers.as_ref().unwrap().get("Cache-Control").unwrap(), "no-store");
    });
}

#[test]
fn test_multisig_get_reauthenticates_once_for_bare_login_required() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let rejection = || Reply::Raw {
            status: 200,
            data: br#"{"code":"LOGIN_REQUIRED","message":"login required"}"#.to_vec(),
        };
        context.provider.replies(vec![rejection()]);
        let auth = context.service();
        assert_eq!(
            auth.multisig_wallet_request::<Vec<Value>>(&context.wallet, BsvMultisigTarget::Wallets).await.unwrap(),
            Vec::<Value>::new()
        );
        assert_eq!(context.provider.paths(), [MULTISIG_WALLETS_PATH, CHALLENGE_PATH, LOGIN_PATH, MULTISIG_WALLETS_PATH]);
        context.provider.reset();
        context.provider.replies(vec![rejection(), Reply::Success, Reply::Success, rejection()]);
        assert_eq!(
            auth.multisig_wallet_request::<Vec<Value>>(&context.wallet, BsvMultisigTarget::Wallets).await,
            Err(BsvHomePortalAuthError::Api {
                code: "LOGIN_REQUIRED".into(),
                msg: "login required".into(),
            })
        );
        assert_eq!(context.provider.paths(), [MULTISIG_WALLETS_PATH, CHALLENGE_PATH, LOGIN_PATH, MULTISIG_WALLETS_PATH]);
        assert_eq!(context.store.get(SESSION.into()).unwrap(), None);
    });
}

#[test]
fn test_multisig_mutations_never_replay_after_expired_authentication() {
    block_on(async {
        let context = Context::new();
        let body = || serde_json::from_str::<Value>(r#"{"expectedVersion":3}"#).unwrap();
        for target in [
            BsvMultisigTarget::Create { body: body() },
            BsvMultisigTarget::Invites {
                wallet_id: "W-test".into(),
                body: body(),
            },
            BsvMultisigTarget::PreviewInvite { body: body() },
            BsvMultisigTarget::Join { body: body() },
            BsvMultisigTarget::Confirm {
                wallet_id: "W-test".into(),
                body: body(),
            },
            BsvMultisigTarget::Prepare {
                wallet_id: "W-test".into(),
                body: body(),
            },
            BsvMultisigTarget::Signatures {
                proposal_id: "P-test".into(),
                body: body(),
            },
            BsvMultisigTarget::Withdraw {
                proposal_id: "P-test".into(),
                body: body(),
            },
            BsvMultisigTarget::Refresh { proposal_id: "P-test".into() },
            BsvMultisigTarget::Broadcast {
                proposal_id: "P-test".into(),
                body: body(),
            },
        ] {
            context.provider.reset();
            context.cache(context.cached_session(None));
            context.provider.replies(vec![Reply::Raw {
                status: 200,
                data: br#"{"forceLogout":true,"message":"session expired"}"#.to_vec(),
            }]);
            assert_eq!(
                context.service().multisig_wallet_request::<Value>(&context.wallet, target).await,
                Err(BsvHomePortalAuthError::Api {
                    code: "HTTP_200".into(),
                    msg: "session expired".into(),
                })
            );
            let requests = context.provider.requests.lock().unwrap();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].method, AlienHttpMethod::Post);
            assert_eq!(context.store.get(SESSION.into()).unwrap(), None);
        }
    });
}

#[test]
fn test_multisig_bare_errors_preserve_code_message_and_do_not_reauthenticate() {
    block_on(async {
        let context = Context::new();
        for (status, body, expected) in [
            (200, br#"{"code":"STALE_VERSION","message":"STALE_VERSION"}"#.to_vec(), ("STALE_VERSION", "STALE_VERSION")),
            (200, br#"{"message":"service unavailable"}"#.to_vec(), ("HTTP_200", "service unavailable")),
            (200, br#"{"success":false,"code":"NOT_A_MEMBER"}"#.to_vec(), ("NOT_A_MEMBER", "Portal request failed")),
            (429, br#"{"message":"limited"}"#.to_vec(), ("HTTP_429", "limited")),
            (503, b"offline".to_vec(), ("HTTP_503", "HTTP 503")),
        ] {
            context.provider.reset();
            context.cache(context.cached_session(None));
            context.provider.replies(vec![Reply::Raw { status, data: body }]);
            assert_eq!(
                context
                    .service()
                    .multisig_wallet_request::<Value>(&context.wallet, BsvMultisigTarget::Wallet { wallet_id: "W-test".into() })
                    .await,
                Err(BsvHomePortalAuthError::Api {
                    code: expected.0.into(),
                    msg: expected.1.into(),
                })
            );
            assert_eq!(context.provider.paths(), ["/auth/multisig/wallets/W-test"]);
            assert_eq!(
                context.store.get(SESSION.into()).unwrap(),
                Some(serde_json::to_string(&context.cached_session(None)).unwrap())
            );
        }
    });
}

#[test]
fn test_multisig_rate_limit_blocks_reads_and_writes_before_login_without_replaying() {
    block_on(async {
        for (status, body, code, logout) in [
            (429, br#"{"code":"CUSTOM_QUOTA","message":"limited","forceLogout":true}"#.to_vec(), "CUSTOM_QUOTA", true),
            (429, br#"{"message":"limited"}"#.to_vec(), "HTTP_429", false),
            (200, br#"{"code":"RATE_LIMITED","message":"limited"}"#.to_vec(), "RATE_LIMITED", false),
            (200, br#"{"code":"MULTISIG_RATE_LIMITED","message":"limited"}"#.to_vec(), "MULTISIG_RATE_LIMITED", false),
        ] {
            let context = Context::new();
            context.cache(context.cached_session(None));
            context.provider.replies(vec![Reply::Raw { status, data: body }]);
            let auth = context.service();
            assert_eq!(
                auth.multisig_wallet_request::<Value>(&context.wallet, BsvMultisigTarget::Wallets).await,
                Err(BsvHomePortalAuthError::Api {
                    code: code.into(),
                    msg: "limited".into()
                })
            );
            if logout {
                assert_eq!(context.store.get(SESSION.into()).unwrap(), None);
            }
            context.store.remove(SESSION.into()).unwrap();
            for target in [
                BsvMultisigTarget::Wallets,
                BsvMultisigTarget::Join {
                    body: serde_json::json!({"token":"public"}),
                },
            ] {
                assert_eq!(
                    auth.multisig_wallet_request::<Value>(&context.wallet, target).await,
                    Err(BsvHomePortalAuthError::Api {
                        code: "MULTISIG_RATE_LIMITED".into(),
                        msg: "multisig requests are temporarily rate limited".into(),
                    })
                );
            }
            assert_eq!(context.provider.paths(), [MULTISIG_WALLETS_PATH]);
            assert!(context.password.create_requests.lock().unwrap().is_empty());
            // The shared auth owner continues to serve unrelated capabilities.
            auth.connect().await.unwrap();
            assert_eq!(context.provider.paths(), [MULTISIG_WALLETS_PATH, CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH]);
        }
    });
}

#[test]
fn test_multisig_queued_request_checks_cooldown_after_connection_lock() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let (mut step, started, release) = Step::paused();
        step.reply = Reply::Raw {
            status: 429,
            data: br#"{"code":"CUSTOM_QUOTA","message":"limited","forceLogout":true}"#.to_vec(),
        };
        context.provider.steps.lock().unwrap().push_back(step);
        let auth = context.service();
        let first = auth.multisig_wallet_request::<Value>(&context.wallet, BsvMultisigTarget::Wallets);
        let queued = async {
            started.await.unwrap();
            release.send(()).unwrap();
            auth.multisig_wallet_request::<Value>(&context.wallet, BsvMultisigTarget::Wallets).await
        };
        let (first, queued) = futures::join!(first, queued);
        assert!(matches!(first, Err(BsvHomePortalAuthError::Api { code, .. }) if code == "CUSTOM_QUOTA"));
        assert!(matches!(queued, Err(BsvHomePortalAuthError::Api { code, .. }) if code == "MULTISIG_RATE_LIMITED"));
        assert_eq!(context.provider.paths(), [MULTISIG_WALLETS_PATH]);
        assert_eq!(context.store.get(SESSION.into()).unwrap(), None);
        assert!(context.password.create_requests.lock().unwrap().is_empty());
    });
}

#[test]
fn test_multisig_challenge_rate_limit_also_stops_subsequent_authentication() {
    block_on(async {
        let context = Context::new();
        context.provider.replies(vec![Reply::Raw {
            status: 429,
            data: br#"{"code":"CUSTOM_AUTH_QUOTA","message":"limited"}"#.to_vec(),
        }]);
        let auth = context.service();
        assert!(matches!(
            auth.multisig_wallet_request::<Value>(&context.wallet, BsvMultisigTarget::Wallets).await,
            Err(BsvHomePortalAuthError::Api { code, .. }) if code == "CUSTOM_AUTH_QUOTA"
        ));
        assert!(matches!(
            auth.multisig_wallet_request::<Value>(&context.wallet, BsvMultisigTarget::Wallets).await,
            Err(BsvHomePortalAuthError::Api { code, .. }) if code == "MULTISIG_RATE_LIMITED"
        ));
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH]);
        assert!(context.password.create_requests.lock().unwrap().is_empty());
    });
}

#[test]
fn test_multisig_requests_reject_path_injection_and_invalid_pagination_before_network() {
    block_on(async {
        let context = Context::new();
        let auth = context.service();
        for identifier in ["", "../paymail/me", "W/another", "W?limit=1", "W#part", "W%2Fpart", "W part", "钱包"] {
            assert_eq!(
                auth.multisig_wallet_request::<Value>(&context.wallet, BsvMultisigTarget::Wallet { wallet_id: identifier.into() })
                    .await,
                Err(BsvHomePortalAuthError::InvalidSession)
            );
        }
        for limit in [0, 201] {
            assert_eq!(
                auth.multisig_wallet_request::<Value>(
                    &context.wallet,
                    BsvMultisigTarget::ProposalPage {
                        wallet_id: "W-test".into(),
                        offset: 0,
                        limit
                    }
                )
                .await,
                Err(BsvHomePortalAuthError::InvalidSession)
            );
        }
        assert_eq!(context.provider.paths(), Vec::<String>::new());
    });
}

#[test]
fn test_multisig_generation_ticket_rejects_cancel_before_first_poll_or_auth_queue() {
    block_on(async {
        for queued in [false, true] {
            let context = Context::new();
            let auth = context.service();
            auth.store.remove(SESSION.into()).unwrap();
            let generation = auth.multisig_version();
            let lock = auth.connection.lock().await;
            let request = auth.multisig_wallet_request_at_generation::<Value>(
                &context.wallet,
                BsvMultisigTarget::Create {
                    body: Value::Object(Default::default()),
                },
                generation,
            );
            let result = if queued {
                let (result, ()) = join!(request, async {
                    auth.cancel();
                    drop(lock);
                });
                result
            } else {
                auth.cancel();
                drop(lock);
                request.await
            };
            assert_eq!(
                result,
                Err(BsvHomePortalAuthError::Service {
                    error: BsvHomeServiceError::Cancelled
                })
            );
            assert_eq!(context.provider.paths(), Vec::<String>::new());
            assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        }
    });
}

#[test]
fn test_multisig_rejects_invalid_or_oversized_mutation_payloads_before_login() {
    block_on(async {
        let context = Context::new();
        let auth = context.service();
        assert_eq!(
            auth.multisig_wallet_request::<Value>(&context.wallet, BsvMultisigTarget::Join { body: Value::Null }).await,
            Err(BsvHomePortalAuthError::InvalidSession)
        );
        let mut body = serde_json::from_str::<Value>(r#"{"rawTransaction":""}"#).unwrap();
        body["rawTransaction"] = "a".repeat(2_100_000).into();
        assert_eq!(
            auth.multisig_wallet_request::<Value>(
                &context.wallet,
                BsvMultisigTarget::Broadcast {
                    proposal_id: "P-test".into(),
                    body
                }
            )
            .await,
            Err(BsvHomePortalAuthError::Api {
                code: "MULTISIG_REQUEST_TOO_LARGE".into(),
                msg: "multisig request exceeds the size limit".into(),
            })
        );
        assert_eq!(context.provider.paths(), Vec::<String>::new());
    });
}

#[test]
fn test_multisig_rejects_wallet_changes_and_cancellation_after_requests() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let auth = context.service();
        let version = auth.multisig_version();
        let (step, started, release) = Step::paused();
        context.provider.steps.lock().unwrap().push_back(step);
        let request = auth.multisig_wallet_request::<Vec<Value>>(&context.wallet, BsvMultisigTarget::Wallets);
        let switch = async {
            started.await.unwrap();
            context.switch_wallet();
            release.send(()).unwrap();
        };
        let (result, ()) = futures::join!(request, switch);
        assert_eq!(result, Err(BsvHomePortalAuthError::WalletChanged));
        assert_eq!(auth.multisig_check_wallet(&context.wallet, version).await, Err(BsvHomePortalAuthError::WalletChanged));
        assert_eq!(context.provider.paths(), [MULTISIG_WALLETS_PATH]);
        assert_eq!(context.store.get(SESSION.into()).unwrap(), None);

        let context = Context::new();
        let auth = context.service();
        let version = auth.multisig_version();
        auth.cancel();
        assert_eq!(auth.multisig_check_wallet(&context.wallet, version).await, Err(BsvHomeServiceError::Cancelled.into()));
        assert_eq!(context.provider.paths(), Vec::<String>::new());
    });
}

#[test]
fn test_connect_reuses_a_secure_session_across_service_recreation() {
    block_on(async {
        let context = Context::new();
        let service = context.service();
        service.connect().await.unwrap();
        service.connect().await.unwrap();
        context.service().connect().await.unwrap();
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH, PAIRS_PATH, PAIRS_PATH]);
        assert_eq!(*context.password.create_requests.lock().unwrap(), [false]);
        assert_eq!(context.store.writes.load(Ordering::Relaxed), 1);
    });
}

#[test]
fn test_connect_rejects_login_without_wallet_identity() {
    block_on(async {
        let context = Context::new();
        context.provider.replies(vec![Reply::Success, Reply::TokenOnlyLogin]);
        assert_eq!(context.service().connect().await, Err(BsvHomePortalAuthError::InvalidSession));
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH]);
        assert_eq!(context.store.get(SESSION.into()).unwrap(), None);
    });
}

#[test]
fn test_connect_force_logout_without_code_reauthenticates_once_before_shape_error() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        context.provider.replies(vec![
            Reply::Raw {
                status: 200,
                data: br#"{"success":true,"forceLogout":true,"data":{}}"#.to_vec(),
            },
            Reply::Success,
            Reply::Success,
            Reply::Success,
        ]);
        assert_eq!(context.service().connect().await, Ok(()));
        assert_eq!(context.provider.paths(), [PAIRS_PATH, CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH]);
    });
}

#[test]
fn test_base_url_keeps_authentication_on_the_approved_portal_endpoint() {
    assert_eq!(rules::base_url(&format!("{BASE_URL}/")), Ok(BASE_URL.into()));
    assert_eq!(rules::base_url("https://portal.test/api/portal"), Ok("https://portal.test/api/portal".into()));
    assert_eq!(rules::base_url("https://portal.test/v2/api/portal/"), Ok("https://portal.test/v2/api/portal".into()));
    for value in [
        "https://portal.test/v2/v2/api/portal",
        "https://portal.test/v2/api/portal/../admin",
        "http://api-dev.hj0252.com/api/portal",
        "http://47.84.135.53/api/portal",
        "http://47.84.135.53:80/api/portal",
        "http://portal.test/api/portal",
        "http://47.84.135.53:7001/api/portal",
        "https://user:password@portal.test/api/portal",
        "https://portal.test/api/portal?token=test",
        "https://portal.test/api/portal#login",
        "https://portal.test/api/admin",
        "ftp://portal.test/api/portal",
        "not a URL",
    ] {
        assert_eq!(rules::base_url(value), Err(BsvHomePortalAuthError::InvalidSession));
    }
}

#[test]
fn test_login_expiry_accepts_empty_without_inventing_a_ttl_and_rejects_invalid_dates() {
    block_on(async {
        for (expiry, accepted) in [
            ("", true),
            ("2099-01-01T00:00:00Z", true),
            ("tomorrow", false),
            (" ", false),
            ("2099-01-01 00:00:00", false),
        ] {
            let context = Context::new();
            context.provider.replies(vec![Reply::Success, Reply::LoginExpiry(expiry)]);
            let result = context.service().connect().await;
            if !accepted {
                assert_eq!(result, Err(BsvHomePortalAuthError::InvalidResponse));
                assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH]);
                assert!(context.store.get(SESSION.into()).unwrap().is_none());
                continue;
            }
            assert_eq!(result, Ok(()));
            let value = context.store.get(SESSION.into()).unwrap().unwrap();
            let stored: Session = serde_json::from_str(&value).unwrap();
            if expiry.is_empty() {
                assert_eq!(stored.expires_at, None);
            } else {
                assert_eq!(stored.expires_at.unwrap().to_rfc3339(), "2099-01-01T00:00:00+00:00");
            }
            context.service().connect().await.unwrap();
            assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH, PAIRS_PATH]);
        }
    });
}

#[test]
fn test_connect_renews_expired_or_mismatched_cached_sessions() {
    block_on(async {
        let context = Context::new();
        for session in [
            context.cached_session(Some(Utc::now() - Duration::seconds(1))),
            Session {
                base_url: "http://47.84.135.53/api/portal".into(),
                ..context.cached_session(None)
            },
            Session {
                base_url: "https://other.test/api/portal".into(),
                ..context.cached_session(None)
            },
            Session {
                address: "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5".into(),
                ..context.cached_session(None)
            },
            Session {
                wallet_id: "another-wallet".into(),
                ..context.cached_session(None)
            },
        ] {
            context.provider.reset();
            context.cache(session);
            context.service().connect().await.unwrap();
            assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH]);
        }
    });
}

#[test]
fn test_connect_reauthenticates_once_for_http_or_portal_session_expiration() {
    block_on(async {
        let context = Context::new();
        for (status, success, code, force_logout) in [
            (401, false, "SYS_NO_PERMISSION", false),
            (200, false, "USER_SESSION_PLEASE_LOGIN_FIRST", false),
            (200, false, "USER_SESSION_EXPIRED", false),
            (200, false, "USER_SESSION_INVALID", false),
            (200, false, "SYS_NO_PERMISSION", true),
            (200, true, "OK", true),
        ] {
            context.provider.reset();
            context.cache(context.cached_session(None));
            context.provider.replies(vec![Reply::Rejected {
                status,
                success,
                code,
                force_logout,
            }]);
            context.service().connect().await.unwrap();
            assert_eq!(context.provider.paths(), [PAIRS_PATH, CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH]);
        }
    });
}

#[test]
fn test_connect_stops_after_a_second_authentication_failure() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let rejected = || Reply::Rejected {
            status: 401,
            success: false,
            code: "USER_SESSION_EXPIRED",
            force_logout: false,
        };
        context.provider.replies(vec![rejected(), Reply::Success, Reply::Success, rejected()]);
        assert_eq!(
            context.service().connect().await,
            Err(BsvHomePortalAuthError::Api {
                code: "USER_SESSION_EXPIRED".into(),
                msg: "session rejected".into()
            })
        );
        assert_eq!(context.provider.paths(), [PAIRS_PATH, CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH]);
        assert_eq!(context.store.values.lock().unwrap().get(SESSION), None);
    });
}

#[test]
fn test_connect_reads_authentication_and_business_errors_before_success_data_shape() {
    block_on(async {
        let context = Context::new();
        for (status, code, force_logout) in [(401, "SYS_NO_PERMISSION", false), (200, "USER_SESSION_EXPIRED", true)] {
            context.provider.reset();
            context.cache(context.cached_session(None));
            context.provider.replies(vec![Reply::ObjectRejected { status, code, force_logout }]);
            assert_eq!(context.service().connect().await, Ok(()));
            assert_eq!(context.provider.paths(), [PAIRS_PATH, CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH]);
        }
        context.provider.reset();
        context.provider.replies(vec![Reply::ObjectRejected {
            status: 200,
            code: "SWAP_RATE_LIMITED",
            force_logout: false,
        }]);
        assert_eq!(
            context.service().connect().await,
            Err(BsvHomePortalAuthError::Api {
                code: "SWAP_RATE_LIMITED".into(),
                msg: "session rejected".into()
            })
        );
        assert_eq!(context.provider.paths(), [PAIRS_PATH]);
    });
}

#[test]
fn test_v2_connect_preserves_gateway_errors_without_reauthenticating() {
    block_on(async {
        let context = Context::new();
        let auth = BsvHomePortalAuthService::new(
            context.provider.clone(),
            context.keystore.clone(),
            context.password.clone(),
            context.session.clone(),
            context.device_key.clone(),
            context.store.clone(),
            V2_BASE_URL.into(),
            "zh-CN".into(),
            "4".into(),
        )
        .unwrap();
        let session = Session {
            base_url: V2_BASE_URL.into(),
            ..context.cached_session(None)
        };
        context.cache(session);
        let cached = context.store.get(SESSION.into()).unwrap();
        for (status, body, expected) in [
            (
                200,
                r#"{"code":40015,"message":"访问路径错误，请检查URL和METHOD TYPE是否正确"}"#,
                Err(BsvHomePortalAuthError::Api {
                    code: "40015".into(),
                    msg: "访问路径错误，请检查URL和METHOD TYPE是否正确".into(),
                }),
            ),
            (
                200,
                r#"{"success":true,"code":40015,"data":[]}"#,
                Err(BsvHomePortalAuthError::Api {
                    code: "40015".into(),
                    msg: "Portal request failed".into(),
                }),
            ),
            (200, r#"{"success":true,"code":40015.5,"data":[]}"#, Err(BsvHomePortalAuthError::InvalidResponse)),
            (
                502,
                "<html>Bad Gateway</html>",
                Err(BsvHomePortalAuthError::Api {
                    code: "HTTP_502".into(),
                    msg: "HTTP 502".into(),
                }),
            ),
            (200, r#"{"success":true,"code":"OK","data":[]}"#, Ok(())),
        ] {
            context.provider.reset();
            context.provider.replies(vec![Reply::Raw {
                status,
                data: body.as_bytes().to_vec(),
            }]);
            assert_eq!(auth.connect().await, expected);
            let requests = context.provider.requests.lock().unwrap();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].url, format!("{V2_BASE_URL}{PAIRS_PATH}"));
            assert_eq!(requests[0].method, AlienHttpMethod::Get);
            assert_eq!(context.store.get(SESSION.into()).unwrap(), cached);
        }
    });
}

#[test]
fn test_connect_rejects_invalid_challenges_before_unlocking_the_keystore() {
    block_on(async {
        let context = Context::new();
        for fault in [
            ChallengeFault::EmptyId,
            ChallengeFault::Chain,
            ChallengeFault::Address,
            ChallengeFault::Expired,
            ChallengeFault::Purpose,
            ChallengeFault::MissingAddress,
            ChallengeFault::DuplicateAddress,
            ChallengeFault::Device,
            ChallengeFault::DeviceType,
            ChallengeFault::UppercaseId,
            ChallengeFault::UppercaseNonce,
            ChallengeFault::ExtraContent,
        ] {
            context.provider.reset();
            context.provider.replies(vec![Reply::Challenge(fault)]);
            assert_eq!(context.service().connect().await, Err(BsvHomePortalAuthError::InvalidChallenge));
            assert_eq!(context.provider.paths(), [CHALLENGE_PATH]);
            assert_eq!(context.store.values.lock().unwrap().get(SESSION), None);
        }
        assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        assert_eq!(context.store.writes.load(Ordering::Relaxed), 0);
    });
}

#[test]
fn test_connect_rejects_invalid_tokens_and_login_identity_echoes() {
    block_on(async {
        let context = Context::new();
        for fault in [
            LoginFault::EmptyToken,
            LoginFault::WhitespaceToken,
            LoginFault::ControlToken,
            LoginFault::NonAsciiToken,
            LoginFault::Address,
            LoginFault::Chain,
            LoginFault::Expired,
        ] {
            context.provider.reset();
            context.provider.replies(vec![Reply::Success, Reply::Login(fault)]);
            assert_eq!(context.service().connect().await, Err(BsvHomePortalAuthError::InvalidSession));
            assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH]);
            assert_eq!(context.store.values.lock().unwrap().get(SESSION), None);
        }
        assert_eq!(context.store.writes.load(Ordering::Relaxed), 0);
    });
}

#[test]
fn test_connect_does_not_keep_a_session_when_the_wallet_changes_during_a_request() {
    block_on(async {
        let context = Context::new();
        for paused_index in 0..3 {
            context.provider.reset();
            context.store.values.lock().unwrap().clear();
            *context.wallet_store.current.lock().unwrap() = Some(context.wallet.id.clone());
            let (step, started, release) = Step::paused();
            let steps = (0..paused_index).map(|_| Step::reply(Reply::Success)).chain([step]).collect();
            *context.provider.steps.lock().unwrap() = steps;
            let service = context.service();
            let switching = async {
                started.await.unwrap();
                context.switch_wallet();
                release.send(()).unwrap();
            };
            let (result, ()) = futures::join!(service.connect(), switching);
            assert_eq!(result, Err(BsvHomePortalAuthError::WalletChanged));
            assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH][..=paused_index]);
            assert_eq!(context.store.values.lock().unwrap().get(SESSION), None);
        }
    });
}

#[test]
fn test_connect_blocks_watch_only_and_missing_bsv_wallets_before_any_request() {
    block_on(async {
        let context = Context::new();
        for (wallet_type, accounts, expected) in [
            (WalletType::View, context.wallet.accounts.clone(), BsvHomePortalAuthError::WatchOnly),
            (
                WalletType::Multicoin,
                vec![Account::mock(Chain::Tron, "TJRyWwFs9wTFGZg3JbrVriFbNfCug5tDeC")],
                BsvHomePortalAuthError::MissingBsvAccount,
            ),
            (
                WalletType::Multicoin,
                vec![Account::mock(Chain::BitcoinSv, "invalid-address")],
                BsvHomePortalAuthError::MissingBsvAccount,
            ),
        ] {
            let wallet = Wallet {
                wallet_type,
                accounts,
                ..context.wallet.clone()
            };
            *context.wallets.wallets.lock().unwrap() = vec![wallet];
            assert_eq!(context.service().connect().await, Err(expected));
            assert_eq!(context.provider.paths(), Vec::<String>::new());
        }
    });
}

#[test]
fn test_connect_propagates_keystore_and_password_failures_without_login() {
    block_on(async {
        let context = Context::new();
        assert_eq!(
            context.service_with_password(Arc::new(UnavailablePassword)).connect().await,
            Err(BsvHomePortalAuthError::Service {
                error: BsvHomeServiceError::Cancelled
            })
        );
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH]);
        context.provider.reset();
        context.keystore.delete(context.keystore.keystore_id(context.wallet.id.id())).unwrap();
        assert!(matches!(
            context.service().connect().await,
            Err(BsvHomePortalAuthError::Service {
                error: BsvHomeServiceError::Core { .. }
            })
        ));
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH]);
        assert_eq!(context.store.values.lock().unwrap().get(SESSION), None);
    });
}

#[test]
fn test_connect_requires_secure_storage_to_succeed_at_every_session_boundary() {
    block_on(async {
        let context = Context::new();
        for (operation, expires_at, expected_paths) in [
            (StoreOperation::Read, None, vec![]),
            (StoreOperation::Remove, Some(Utc::now() - Duration::seconds(1)), vec![]),
            (StoreOperation::Write, Some(Utc::now() - Duration::seconds(1)), vec![CHALLENGE_PATH, LOGIN_PATH]),
        ] {
            context.provider.reset();
            context.cache(context.cached_session(expires_at));
            *context.store.failure.lock().unwrap() = Some(operation);
            assert_eq!(
                context.service().connect().await,
                Err(BsvHomePortalAuthError::Service {
                    error: BsvHomeServiceError::Store {
                        msg: "secure store unavailable".into()
                    }
                })
            );
            assert_eq!(context.provider.paths(), expected_paths);
        }
    });
}

#[test]
fn test_connect_does_not_reauthenticate_for_provider_business_or_network_failures() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        context.provider.replies(vec![Reply::Rejected {
            status: 200,
            success: false,
            code: "SWAP_PROVIDER_UNAVAILABLE",
            force_logout: false,
        }]);
        assert_eq!(
            context.service().connect().await,
            Err(BsvHomePortalAuthError::Api {
                code: "SWAP_PROVIDER_UNAVAILABLE".into(),
                msg: "session rejected".into()
            })
        );
        assert_eq!(context.provider.paths(), [PAIRS_PATH]);
        context.provider.reset();
        context.provider.replies(vec![Reply::Offline]);
        assert!(matches!(
            context.service().connect().await,
            Err(BsvHomePortalAuthError::Service {
                error: BsvHomeServiceError::Api { .. }
            })
        ));
        assert_eq!(context.provider.paths(), [PAIRS_PATH]);
    });
}

#[test]
fn test_concurrent_connects_share_one_wallet_authentication() {
    block_on(async {
        let context = Context::new();
        let service = context.service();
        let (step, started, release) = Step::paused();
        context.provider.steps.lock().unwrap().push_back(step);
        let releasing = async {
            started.await.unwrap();
            release.send(()).unwrap();
        };
        let (first, second, ()) = futures::join!(service.connect(), service.connect(), releasing);
        assert_eq!(first, Ok(()));
        assert_eq!(second, Ok(()));
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, PAIRS_PATH, PAIRS_PATH]);
        assert_eq!(*context.password.create_requests.lock().unwrap(), [false]);
        assert_eq!(context.store.writes.load(Ordering::Relaxed), 1);
    });
}

#[test]
fn test_cancel_stops_delayed_authentication_and_pairs_without_new_session_writes() {
    block_on(async {
        let context = Context::new();
        for paused_index in 0..3 {
            context.provider.reset();
            context.store.values.lock().unwrap().clear();
            if paused_index == 2 {
                context.cache(context.cached_session(None));
            }
            let saved = context.store.values.lock().unwrap().get(SESSION).cloned();
            let password_requests = context.password.create_requests.lock().unwrap().len();
            let (step, started, release) = Step::paused();
            let completed_requests = if paused_index == 2 { 0 } else { paused_index };
            *context.provider.steps.lock().unwrap() = (0..completed_requests).map(|_| Step::reply(Reply::Success)).chain([step]).collect();
            let service = context.service();
            let cancelling = async {
                started.await.unwrap();
                service.cancel();
                release.send(()).unwrap();
            };
            let (result, ()) = futures::join!(service.connect(), cancelling);
            assert_eq!(
                result,
                Err(BsvHomePortalAuthError::Service {
                    error: BsvHomeServiceError::Cancelled
                })
            );
            let expected_paths = if paused_index == 2 {
                vec![PAIRS_PATH]
            } else {
                vec![CHALLENGE_PATH, LOGIN_PATH][..=paused_index].to_vec()
            };
            assert_eq!(context.provider.paths(), expected_paths);
            assert_eq!(context.store.values.lock().unwrap().get(SESSION).cloned(), saved);
            assert_eq!(context.store.writes.load(Ordering::Relaxed), 0);
            assert_eq!(context.password.create_requests.lock().unwrap().len(), password_requests + usize::from(paused_index == 1));
        }
    });
}

#[test]
fn test_swap_orders_refresh_expired_auth_without_exposing_a_token() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        context.provider.replies(vec![
            Reply::ObjectRejected {
                status: 401,
                code: "USER_SESSION_EXPIRED",
                force_logout: true,
            },
            Reply::Success,
            Reply::Success,
            Reply::Swap(serde_json::from_str(r#"{"total":"0","records":[]}"#).unwrap()),
        ]);
        let service = context.swap_service(Arc::new(MemoryPortalSwapStore::default()));
        assert_eq!(service.orders(true, 1, 20).await.unwrap().total, 0);
        assert_eq!(
            context.provider.paths(),
            [
                "/auth/swaps/orders?status=active&page=1&size=20",
                CHALLENGE_PATH,
                LOGIN_PATH,
                "/auth/swaps/orders?status=active&page=1&size=20"
            ]
        );
        let requests = context.provider.requests.lock().unwrap();
        assert_eq!(requests[0].headers.as_ref().unwrap().get("Authorization"), Some(&TEST_TOKEN.to_string()));
        assert_eq!(requests[3].headers.as_ref().unwrap().get("Authorization"), Some(&TEST_TOKEN.to_string()));
    });
}

#[test]
fn test_orders_empty_optional_values_persist_without_guessing_pending_operation_identity() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let store = Arc::new(MemoryPortalSwapStore::default());
        let service = context.swap_service(store.clone());
        let operation = pending_usdt_to_bsv_operation(&context);
        let original_request = operation.request.clone();
        let scope = service.scope(&context.wallet).unwrap();
        service
            .write_state(
                &scope,
                &SwapState {
                    operations: vec![operation],
                    orders: vec![],
                },
            )
            .unwrap();
        context.provider.replies(vec![Reply::Swap(empty_optional_order_page(&context))]);
        let page = service.orders(true, 1, 20).await.unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.records.len(), 1);
        assert_eq!(page.records[0].client_operation_id, None);
        assert_eq!(page.records[0].actual_to_amount, None);
        assert_eq!(page.records[0].source_txid, None);
        assert_eq!(page.records[0].deposit_expires_at, None);
        assert!(!page.records[0].payout_verified);
        let persisted = service.read_state(&scope).unwrap();
        assert_eq!(persisted.orders, page.records);
        assert_eq!(persisted.operations[0].request, original_request);
        assert_eq!(persisted.operations[0].order_id, None);
        let recreated = context.swap_service(store);
        assert!(recreated.has_pending_order().await.unwrap());
        let mut expected = page.records;
        expected[0].sync_status = BsvHomePortalSwapSyncStatus::Stale;
        expected[0].can_pay = false;
        assert_eq!(recreated.cached_orders().await.unwrap(), expected);
        assert_eq!(recreated.read_state(&scope).unwrap().operations[0].request, original_request);
        assert_eq!(context.provider.paths(), ["/auth/swaps/orders?status=active&page=1&size=20"]);
    });
}

#[test]
fn test_malformed_order_responses_do_not_reauthenticate_or_change_persisted_operations() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let service = context.swap_service(Arc::new(MemoryPortalSwapStore::default()));
        let scope = service.scope(&context.wallet).unwrap();
        let operation = pending_usdt_to_bsv_operation(&context);
        service
            .write_state(
                &scope,
                &SwapState {
                    operations: vec![operation],
                    orders: vec![],
                },
            )
            .unwrap();
        let saved_state = service.store.get(scope.clone()).unwrap();
        let saved_session = context.store.get(SESSION.into()).unwrap();
        let mut unknown_status = empty_optional_order_page(&context);
        unknown_status["records"][0]["orderStatus"] = "UNRECOGNIZED".into();
        let mut malformed_expiry = empty_optional_order_page(&context);
        malformed_expiry["records"][0]["depositExpiresAt"] = "tomorrow".into();
        let mut missing_actions = empty_optional_order_page(&context);
        missing_actions["records"][0].as_object_mut().unwrap().remove("requiredActions");
        let mut incorrect_amount_type = empty_optional_order_page(&context);
        incorrect_amount_type["records"][0]["actualToAmount"] = 1.into();
        for reply in [
            Reply::Raw { status: 200, data: b"{".to_vec() },
            Reply::Raw {
                status: 200,
                data: b"{}".to_vec(),
            },
            Reply::Swap(Value::Null),
            Reply::Swap(unknown_status),
            Reply::Swap(malformed_expiry),
            Reply::Swap(missing_actions),
            Reply::Swap(incorrect_amount_type),
        ] {
            context.provider.reset();
            context.provider.replies(vec![reply]);
            assert_eq!(
                service.orders(true, 1, 20).await,
                Err(BsvHomePortalSwapError::Auth {
                    error: BsvHomePortalAuthError::InvalidResponse
                })
            );
            assert_eq!(context.provider.paths(), ["/auth/swaps/orders?status=active&page=1&size=20"]);
            assert_eq!(context.store.get(SESSION.into()).unwrap(), saved_session);
            assert_eq!(service.store.get(scope.clone()).unwrap(), saved_state);
            assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
        }
    });
}

#[test]
fn test_empty_remote_source_hash_preserves_payment_and_reports_only_saved_hash() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let service = context.swap_service(Arc::new(MemoryPortalSwapStore::default()));
        let response = empty_optional_order_page(&context);
        let wire = response["records"][0].clone();
        let mut local = swap_rules::order(serde_json::from_value(wire.clone()).unwrap(), None, None, None, Utc::now()).unwrap();
        let hash = "aa".repeat(32);
        local.source_txid = Some(hash.clone());
        local.broadcast_report_pending = true;
        let scope = service.scope(&context.wallet).unwrap();
        service
            .write_state(
                &scope,
                &SwapState {
                    operations: vec![],
                    orders: vec![local.clone()],
                },
            )
            .unwrap();
        let saved = BsvHomeTransactionCheckpointData {
            version: 0,
            bsv_spend: None,
            original_value: None,
            use_max_amount: false,
            gas_price: None,
            wallet_id: context.wallet.id.clone(),
            asset_id: local.from.as_ref().unwrap().asset_id.clone(),
            sender_address: local.refund_address.clone(),
            recipient_address: local.deposit_address.clone().unwrap(),
            value: BigInt::from(local.from_amount.clone()),
            network_fee: BigUint::from(1u32),
            fee_asset_id: Chain::Tron.as_asset_id(),
            transaction_id: hash.clone(),
            signed_data: "test-signed-payment".into(),
        };
        service
            .payment_store
            .set(format!("{scope}|{}", local.order_id), serde_json::to_string(&saved).unwrap())
            .unwrap();
        context.provider.replies(vec![Reply::Swap(response), Reply::Swap(wire.clone())]);
        let page = service.orders(true, 1, 20).await.unwrap();
        assert_eq!(page.records[0].source_txid, Some(hash.clone()));
        assert!(page.records[0].broadcast_report_pending);
        assert!(!page.records[0].can_pay);
        assert_eq!(
            service.clone().prepare_payment(local.order_id.clone()).await.map(|_| ()),
            Err(BsvHomePortalSwapError::PaymentUnavailable)
        );
        assert_eq!(
            context.provider.paths(),
            ["/auth/swaps/orders?status=active&page=1&size=20", "/auth/swaps/orders/O-usdt-bsv-test"]
        );
        context.provider.reset();
        context.provider.replies(vec![Reply::Swap(wire.clone()), Reply::Offline]);
        assert!(matches!(
            service.refresh_order(local.order_id.clone()).await,
            Err(BsvHomePortalSwapError::Auth {
                error: BsvHomePortalAuthError::Service { .. }
            })
        ));
        assert_eq!(service.read_state(&scope).unwrap().orders[0].source_txid, Some(hash.clone()));
        assert!(service.read_state(&scope).unwrap().orders[0].broadcast_report_pending);
        context.provider.reset();
        let mut reported = wire.clone();
        reported["sourceTxid"] = hash.clone().into();
        context.provider.replies(vec![Reply::Swap(wire), Reply::Swap(Value::Null), Reply::Swap(reported)]);
        let refreshed = service.refresh_order(local.order_id.clone()).await.unwrap();
        assert_eq!(refreshed.source_txid, Some(hash.clone()));
        assert!(!refreshed.broadcast_report_pending);
        assert!(!refreshed.can_pay);
        assert_eq!(
            context.provider.paths(),
            [
                "/auth/swaps/orders/O-usdt-bsv-test/refresh",
                "/auth/swaps/orders/O-usdt-bsv-test/broadcast",
                "/auth/swaps/orders/O-usdt-bsv-test"
            ]
        );
        let requests = context.provider.requests.lock().unwrap();
        assert_eq!(requests[0].method, AlienHttpMethod::Post);
        assert_eq!(requests[1].method, AlienHttpMethod::Post);
        assert_eq!(requests[2].method, AlienHttpMethod::Get);
        assert_eq!(
            serde_json::from_slice::<Value>(requests[1].body.as_ref().unwrap()).unwrap(),
            serde_json::from_str::<Value>(&format!(r#"{{"sourceTxid":"{hash}"}}"#)).unwrap()
        );
        assert_eq!(service.read_payment(&context.wallet, &local.order_id).unwrap(), Some(saved));
    });
}

#[test]
fn test_pending_creation_preserves_operation_identity_after_malformed_order_response() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let store = Arc::new(MemoryPortalSwapStore::default());
        let service = context.swap_service(store.clone());
        let operation = pending_usdt_to_bsv_operation(&context);
        let request = operation.request.clone();
        let scope = service.scope(&context.wallet).unwrap();
        service
            .write_state(
                &scope,
                &SwapState {
                    operations: vec![operation],
                    orders: vec![],
                },
            )
            .unwrap();
        let response = empty_optional_order_page(&context)["records"][0].clone();
        let mut malformed = response.clone();
        malformed.as_object_mut().unwrap().remove("requiredActions");
        context.provider.replies(vec![Reply::Swap(malformed), Reply::Swap(response)]);
        assert_eq!(
            service.retry_pending_order().await,
            Err(BsvHomePortalSwapError::Auth {
                error: BsvHomePortalAuthError::InvalidResponse
            })
        );
        let persisted = service.read_state(&scope).unwrap();
        assert_eq!(persisted.operations[0].request, request);
        assert_eq!(persisted.operations[0].order_id, None);
        let recreated = context.swap_service(store);
        let completed = recreated.retry_pending_order().await.unwrap();
        assert_eq!(completed.client_operation_id, Some(request.client_operation_id.clone()));
        assert!(!recreated.has_pending_order().await.unwrap());
        let requests = context.provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(serde_json::from_slice::<OrderRequest>(requests[0].body.as_ref().unwrap()).unwrap(), request);
        assert_eq!(requests[0].body, requests[1].body);
    });
}

#[test]
fn test_pending_creation_preserves_operation_and_payload_across_timeout_and_service_recreation() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let store = Arc::new(MemoryPortalSwapStore::default());
        let service = context.swap_service(store.clone());
        let pair: WirePair = serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap();
        let mut wire_quote: WireQuote = serde_json::from_str(include_str!("../../../testdata/portal/quote.json")).unwrap();
        wire_quote.expires_at = Utc::now() + Duration::minutes(1);
        let quote = swap_rules::quote(wire_quote, &swap_rules::quote_request(&pair, BigUint::from(150_000_000u32)), &pair, Utc::now()).unwrap();
        let (payout, refund) = swap_rules::wallet_addresses(&context.wallet, quote.direction).unwrap();
        let request = OrderRequest {
            quote_id: quote.quote_id.clone(),
            client_operation_id: "11111111-1111-4111-8111-111111111111".into(),
            payout_address: payout.clone(),
            refund_address: refund.clone(),
        };
        let operation = PendingOperation {
            request: request.clone(),
            quote,
            order_id: None,
        };
        let scope = service.scope(&context.wallet).unwrap();
        store
            .set(
                scope.clone(),
                serde_json::to_string(&SwapState {
                    operations: vec![operation],
                    orders: vec![],
                })
                .unwrap(),
            )
            .unwrap();
        let mut order: serde_json::Value = serde_json::from_str(include_str!("../../../testdata/portal/order.json")).unwrap();
        order["payoutAddress"] = payout.into();
        order["refundAddress"] = refund.into();
        order["depositExpiresAt"] = (Utc::now() + Duration::minutes(30)).to_rfc3339().into();
        context.provider.replies(vec![Reply::Offline, Reply::Swap(order)]);
        assert!(matches!(
            service.retry_pending_order().await,
            Err(BsvHomePortalSwapError::Auth {
                error: BsvHomePortalAuthError::Service { .. }
            })
        ));
        let persisted: SwapState = serde_json::from_str(&store.get(scope.clone()).unwrap().unwrap()).unwrap();
        assert_eq!(persisted.operations[0].request, request);
        assert_eq!(persisted.operations[0].order_id, None);
        let recreated = context.swap_service(store.clone());
        let completed = recreated.retry_pending_order().await.unwrap();
        assert_eq!(completed.order_id, "O-test");
        assert!(!recreated.has_pending_order().await.unwrap());
        let replayed = recreated.create_order(request.quote_id.clone()).await.unwrap();
        assert_eq!(replayed, completed);
        {
            let requests = context.provider.requests.lock().unwrap();
            assert_eq!(requests.len(), 2);
            assert_eq!(serde_json::from_slice::<OrderRequest>(requests[0].body.as_ref().unwrap()).unwrap(), request);
            assert_eq!(requests[0].body, requests[1].body);
        }
        let persisted: SwapState = serde_json::from_str(&store.get(scope).unwrap().unwrap()).unwrap();
        assert_eq!(persisted.orders, vec![completed]);
        let cached = recreated.cached_orders().await.unwrap();
        assert_eq!(cached[0].sync_status, BsvHomePortalSwapSyncStatus::Stale);
        assert!(!cached[0].can_pay);
        assert_eq!(persisted.orders[0].sync_status, BsvHomePortalSwapSyncStatus::Fresh);
    });
}

#[test]
fn test_new_quote_cannot_create_another_order_while_result_is_unknown() {
    block_on(async {
        let context = Context::new();
        let store = Arc::new(MemoryPortalSwapStore::default());
        let service = context.swap_service(store.clone());
        let pair: WirePair = serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap();
        let mut wire_quote: WireQuote = serde_json::from_str(include_str!("../../../testdata/portal/quote.json")).unwrap();
        wire_quote.expires_at = Utc::now() + Duration::minutes(1);
        let quote = swap_rules::quote(wire_quote, &swap_rules::quote_request(&pair, BigUint::from(150_000_000u32)), &pair, Utc::now()).unwrap();
        let (payout, refund) = swap_rules::wallet_addresses(&context.wallet, quote.direction).unwrap();
        let operation = PendingOperation {
            request: OrderRequest {
                quote_id: quote.quote_id.clone(),
                client_operation_id: "11111111-1111-4111-8111-111111111111".into(),
                payout_address: payout,
                refund_address: refund,
            },
            quote,
            order_id: None,
        };
        let scope = service.scope(&context.wallet).unwrap();
        store
            .set(
                scope,
                serde_json::to_string(&SwapState {
                    operations: vec![operation],
                    orders: vec![],
                })
                .unwrap(),
            )
            .unwrap();
        assert_eq!(service.create_order("new-quote".into()).await, Err(BsvHomePortalSwapError::PendingCreation));
        assert_eq!(context.provider.requests.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_expired_quote_during_recovery_preserves_unknown_operation_id() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let store = Arc::new(MemoryPortalSwapStore::default());
        let service = context.swap_service(store.clone());
        let pair: WirePair = serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap();
        let mut wire_quote: WireQuote = serde_json::from_str(include_str!("../../../testdata/portal/quote.json")).unwrap();
        wire_quote.expires_at = Utc::now() + Duration::minutes(1);
        let quote = swap_rules::quote(wire_quote, &swap_rules::quote_request(&pair, BigUint::from(150_000_000u32)), &pair, Utc::now()).unwrap();
        let (payout, refund) = swap_rules::wallet_addresses(&context.wallet, quote.direction).unwrap();
        let request = OrderRequest {
            quote_id: quote.quote_id.clone(),
            client_operation_id: "11111111-1111-4111-8111-111111111111".into(),
            payout_address: payout,
            refund_address: refund,
        };
        let scope = service.scope(&context.wallet).unwrap();
        service
            .write_state(
                &scope,
                &SwapState {
                    operations: vec![PendingOperation {
                        request: request.clone(),
                        quote,
                        order_id: None,
                    }],
                    orders: vec![],
                },
            )
            .unwrap();
        context.provider.replies(vec![Reply::Rejected {
            status: 200,
            success: false,
            code: "SWAP_QUOTE_EXPIRED",
            force_logout: false,
        }]);
        assert!(matches!(
            service.retry_pending_order().await,
            Err(BsvHomePortalSwapError::Auth { error: BsvHomePortalAuthError::Api { code, .. } }) if code == "SWAP_QUOTE_EXPIRED"
        ));
        assert!(service.has_pending_order().await.unwrap());
        assert_eq!(service.read_state(&scope).unwrap().operations[0].request, request);
        assert_eq!(service.create_order("new-quote".into()).await, Err(BsvHomePortalSwapError::PendingCreation));
        let requests = context.provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(serde_json::from_slice::<OrderRequest>(requests[0].body.as_ref().unwrap()).unwrap(), request);
    });
}

#[test]
fn test_payment_preparation_assembles_matching_context_with_one_order_request() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let assets = Arc::new(MemoryAssetStore::default());
        assets
            .save_assets(vec![Chain::BitcoinSv.new_asset("Bitcoin SV", "BSV", 8, AssetType::NATIVE).as_basic_primitive()])
            .await
            .unwrap();
        let service = context.swap_service_with_assets(Arc::new(MemoryPortalSwapStore::default()), assets);
        let (payout, refund) = swap_rules::wallet_addresses(&context.wallet, super::BsvHomePortalSwapDirection::BsvToUsdt).unwrap();
        let mut order: serde_json::Value = serde_json::from_str(include_str!("../../../testdata/portal/order.json")).unwrap();
        order["payoutAddress"] = payout.into();
        order["refundAddress"] = refund.into();
        order["depositExpiresAt"] = (Utc::now() + Duration::minutes(30)).to_rfc3339().into();
        context.provider.replies(vec![Reply::Swap(order)]);
        let preparation = service.clone().prepare_payment("O-test".into()).await.unwrap();
        assert_eq!(context.provider.paths(), ["/auth/swaps/orders/O-test"]);
        assert_eq!(preparation.payment.order_id, preparation.order.order_id);
        assert_eq!(preparation.payment.wallet_id, context.wallet.id);
        assert_eq!(preparation.transfer.value, num_bigint::BigInt::from(preparation.order.from_amount));
        assert_eq!(preparation.transfer.recipient.address, preparation.payment.deposit_address);
        assert_eq!(preparation.transfer.input_type.get_asset().id, preparation.payment.from_asset_id);
        assert!(!preparation.transfer.use_max_amount);
        assert_eq!(preparation.checkpoint.load().await.unwrap(), None);
        assert_eq!(context.provider.paths(), ["/auth/swaps/orders/O-test"]);
        service.cancel();
        let saved = crate::services::confirm::BsvHomeTransactionCheckpointData {
            version: 0,
            bsv_spend: None,
            original_value: None,
            use_max_amount: false,
            gas_price: None,
            wallet_id: context.wallet.id.clone(),
            asset_id: preparation.payment.from_asset_id.clone(),
            sender_address: context.wallet.account(Chain::BitcoinSv).unwrap().address.clone(),
            recipient_address: preparation.transfer.recipient.address.clone(),
            value: preparation.transfer.value.clone(),
            network_fee: BigUint::from(1u32),
            fee_asset_id: Chain::BitcoinSv.as_asset_id(),
            transaction_id: "0".repeat(64),
            signed_data: String::new(),
        };
        assert!(matches!(preparation.checkpoint.authorize_broadcast(saved).await, Err(crate::BsvHomeCoreError::Cancelled)));
        assert_eq!(context.provider.paths(), ["/auth/swaps/orders/O-test"]);
    });
}

#[test]
fn test_pairs_filter_unsupported_directions_without_hiding_invalid_responses() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let service = context.swap_service(Arc::new(MemoryPortalSwapStore::default()));
        let pair: Value = serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap();
        let bsv = pair["from"].clone();
        let usdt = pair["to"].clone();
        let mut trx = usdt.clone();
        trx["kind"] = "NATIVE".into();
        trx["contract"] = "".into();
        trx["symbol"] = "TRX".into();
        trx["network"] = "TRON".into();
        let pairs = [
            (bsv.clone(), usdt.clone()),
            (usdt.clone(), bsv.clone()),
            (bsv.clone(), trx.clone()),
            (trx.clone(), bsv),
            (usdt.clone(), trx.clone()),
            (trx, usdt),
        ]
        .into_iter()
        .map(|(from, to)| {
            let mut pair = pair.clone();
            pair["from"] = from;
            pair["to"] = to;
            pair
        })
        .collect();
        context.provider.replies(vec![Reply::Swap(Value::Array(pairs))]);
        assert_eq!(
            service.pairs().await.unwrap().into_iter().map(|pair| pair.direction).collect::<Vec<_>>(),
            vec![
                BsvHomePortalSwapDirection::BsvToUsdt,
                BsvHomePortalSwapDirection::UsdtToBsv,
                BsvHomePortalSwapDirection::UsdtToTrx,
                BsvHomePortalSwapDirection::TrxToUsdt,
            ]
        );
        for (field, value) in [("scale", Value::from(6)), ("symbol", Value::from("TRX")), ("network", Value::from(""))] {
            let mut invalid = pair.clone();
            invalid["from"][field] = value;
            context.provider.replies(vec![Reply::Swap(Value::Array(vec![pair.clone(), invalid]))]);
            assert_eq!(service.pairs().await, Err(BsvHomePortalSwapError::InvalidResponse));
        }
        let mut invalid = pair.clone();
        invalid["providerCode"] = "".into();
        context.provider.replies(vec![Reply::Swap(Value::Array(vec![pair, invalid]))]);
        assert_eq!(service.pairs().await, Err(BsvHomePortalSwapError::InvalidResponse));
    });
}

#[test]
fn test_quote_rejects_malformed_pair_list_before_requesting_quote() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let pair: Value = serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap();
        let mut invalid = pair.clone();
        invalid["from"]["scale"] = 6.into();
        let mut quote: Value = serde_json::from_str(include_str!("../../../testdata/portal/quote.json")).unwrap();
        quote["expiresAt"] = (Utc::now() + Duration::minutes(1)).to_rfc3339().into();
        context.provider.replies(vec![Reply::Swap(Value::Array(vec![invalid, pair])), Reply::Swap(quote)]);
        let service = context.swap_service(Arc::new(MemoryPortalSwapStore::default()));
        assert_eq!(
            service.quote(BsvHomePortalSwapDirection::BsvToUsdt, "1.5".into()).await,
            Err(BsvHomePortalSwapError::InvalidResponse)
        );
        assert_eq!(context.provider.paths(), [PAIRS_PATH]);
    });
}

#[test]
fn test_bsv_trx_quote_and_pending_creation_are_blocked_without_discarding_operation() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let store = Arc::new(MemoryPortalSwapStore::default());
        let service = context.swap_service(store.clone());
        let pair: WirePair = serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap();
        let mut wire_quote: WireQuote = serde_json::from_str(include_str!("../../../testdata/portal/quote.json")).unwrap();
        wire_quote.expires_at = Utc::now() + Duration::minutes(1);
        let mut quote = swap_rules::quote(wire_quote, &swap_rules::quote_request(&pair, BigUint::from(150_000_000u32)), &pair, Utc::now()).unwrap();
        quote.direction = BsvHomePortalSwapDirection::BsvToTrx;
        quote.to = swap_rules::asset("TRON", "NATIVE", None).unwrap();
        let (payout_address, refund_address) = swap_rules::wallet_addresses(&context.wallet, quote.direction).unwrap();
        let request = OrderRequest {
            quote_id: quote.quote_id.clone(),
            client_operation_id: "11111111-1111-4111-8111-111111111111".into(),
            payout_address,
            refund_address,
        };
        let scope = service.scope(&context.wallet).unwrap();
        service
            .write_state(
                &scope,
                &SwapState {
                    operations: vec![PendingOperation {
                        request: request.clone(),
                        quote,
                        order_id: None,
                    }],
                    orders: vec![],
                },
            )
            .unwrap();
        let saved = store.get(scope.clone()).unwrap();
        let remote_pair: Value = serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap();
        context.provider.replies(vec![Reply::Swap(Value::Array(vec![remote_pair]))]);
        for direction in [BsvHomePortalSwapDirection::BsvToTrx, BsvHomePortalSwapDirection::TrxToBsv] {
            assert_eq!(service.quote(direction, "1.5".into()).await, Err(BsvHomePortalSwapError::UnsupportedPair));
        }
        assert_eq!(service.retry_pending_order().await, Err(BsvHomePortalSwapError::UnsupportedPair));
        assert_eq!(service.create_order(request.quote_id.clone()).await, Err(BsvHomePortalSwapError::UnsupportedPair));
        assert_eq!(store.get(scope.clone()).unwrap(), saved);
        assert!(service.has_pending_order().await.unwrap());
        assert_eq!(context.provider.paths(), Vec::<String>::new());
        let mut state = service.read_state(&scope).unwrap();
        let mut wire: WireOrder = serde_json::from_str(include_str!("../../../testdata/portal/order.json")).unwrap();
        wire.client_operation_id = Some(request.client_operation_id.clone());
        wire.to_kind = Some("NATIVE".into());
        wire.to_contract = Some(String::new());
        wire.payout_address = request.payout_address.clone();
        wire.refund_address = request.refund_address.clone();
        let order = swap_rules::order(wire, None, Some(&state.operations[0].quote), Some(&request), Utc::now()).unwrap();
        state.operations[0].order_id = Some(order.order_id.clone());
        state.orders.push(order.clone());
        service.write_state(&scope, &state).unwrap();
        let recreated = context.swap_service(store);
        assert_eq!(recreated.create_order(request.quote_id).await.unwrap(), order);
        assert_eq!(recreated.read_state(&scope).unwrap().operations[0].request.client_operation_id, request.client_operation_id);
        assert_eq!(context.provider.paths(), Vec::<String>::new());
    });
}

#[test]
fn test_trx_quote_missing_remote_pair_does_not_send_quote_or_create_request() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let pair: Value = serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap();
        context.provider.replies(vec![Reply::Swap(Value::Array(vec![pair]))]);
        let store = Arc::new(MemoryPortalSwapStore::default());
        let service = context.swap_service(store.clone());

        assert_eq!(
            service.quote(BsvHomePortalSwapDirection::TrxToUsdt, "1.123456".into()).await,
            Err(BsvHomePortalSwapError::UnsupportedPair)
        );
        assert_eq!(context.provider.paths(), [PAIRS_PATH]);
        assert_eq!(store.values.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_trx_quote_sends_native_identity_and_exact_minor_units_through_portal_service() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let mut pair: Value = serde_json::from_str(include_str!("../../../testdata/portal/pair.json")).unwrap();
        pair["from"]["chain"] = "TRON".into();
        pair["from"]["kind"] = "NATIVE".into();
        pair["from"]["contract"] = "".into();
        pair["from"]["symbol"] = "TRX".into();
        pair["from"]["network"] = "TRON".into();
        pair["from"]["scale"] = 6.into();
        let mut quote: Value = serde_json::from_str(include_str!("../../../testdata/portal/quote.json")).unwrap();
        quote["fromChain"] = "TRON".into();
        quote["fromKind"] = "NATIVE".into();
        quote["fromContract"] = "".into();
        quote["fromAmount"] = "1123456".into();
        quote["minFromAmount"] = "1000000".into();
        quote["expiresAt"] = (Utc::now() + Duration::minutes(1)).to_rfc3339().into();
        context.provider.replies(vec![Reply::Swap(Value::Array(vec![pair])), Reply::Swap(quote)]);
        let service = context.swap_service(Arc::new(MemoryPortalSwapStore::default()));

        let quote = service.quote(BsvHomePortalSwapDirection::TrxToUsdt, "1.123456".into()).await.unwrap();

        assert_eq!(quote.direction, BsvHomePortalSwapDirection::TrxToUsdt);
        assert_eq!(quote.from.asset_id, Chain::Tron.as_asset_id());
        assert_eq!(quote.from.scale, 6);
        assert_eq!(quote.from_amount, BigUint::from(1_123_456u32));
        assert_eq!(quote.from_amount_display, "1.123456 TRX");
        assert_eq!(context.provider.paths(), [PAIRS_PATH, "/auth/swaps/quotes"]);
        let requests = context.provider.requests.lock().unwrap();
        assert_eq!(requests[0].method, AlienHttpMethod::Get);
        assert_eq!(requests[1].method, AlienHttpMethod::Post);
        assert_eq!(
            serde_json::from_slice::<Value>(requests[1].body.as_ref().unwrap()).unwrap(),
            serde_json::from_str::<Value>(
                r#"{"fromChain":"TRON","fromKind":"NATIVE","fromContract":"","toChain":"TRON","toKind":"TOKEN","toContract":"TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t","fromAmount":"1123456"}"#
            )
            .unwrap()
        );
    });
}

#[test]
fn test_trx_payment_preparation_uses_native_asset_and_same_chain_wallet_addresses() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let assets = Arc::new(MemoryAssetStore::default());
        let native = Chain::Tron.new_asset("TRON", "TRX", 6, AssetType::NATIVE);
        assets.save_assets(vec![native.as_basic_primitive()]).await.unwrap();
        let service = context.swap_service_with_assets(Arc::new(MemoryPortalSwapStore::default()), assets);
        let address = context.wallet.account(Chain::Tron).unwrap().address.clone();
        let mut order: Value = serde_json::from_str(include_str!("../../../testdata/portal/order.json")).unwrap();
        order["fromChain"] = "TRON".into();
        order["fromKind"] = "NATIVE".into();
        order["fromContract"] = "".into();
        order["fromAmount"] = "1123456".into();
        order["depositAddress"] = "TEB39Rt69QkgD1BKhqaRNqGxfQzCarkRCb".into();
        order["payoutAddress"] = address.clone().into();
        order["refundAddress"] = address.clone().into();
        order["depositExpiresAt"] = (Utc::now() + Duration::minutes(30)).to_rfc3339().into();
        context.provider.replies(vec![Reply::Swap(order)]);

        let preparation = service.prepare_payment("O-test".into()).await.unwrap();

        assert_eq!(preparation.order.direction, Some(BsvHomePortalSwapDirection::TrxToUsdt));
        assert_eq!(preparation.transfer.input_type.get_asset(), &native);
        assert_eq!(preparation.transfer.value, BigInt::from(1_123_456u32));
        assert_eq!(preparation.transfer.recipient.address, "TEB39Rt69QkgD1BKhqaRNqGxfQzCarkRCb");
        assert_eq!(preparation.payment.from_asset_id, Chain::Tron.as_asset_id());
        assert_eq!(preparation.payment.payout_asset_id, AssetId::from_token(Chain::Tron, USDT_CONTRACT));
        assert_eq!(preparation.payment.payout_address, address);
        assert_eq!(preparation.payment.refund_address, address);
        assert!(!preparation.transfer.use_max_amount);
        assert_eq!(preparation.checkpoint.load().await.unwrap(), None);
        assert_eq!(context.provider.paths(), ["/auth/swaps/orders/O-test"]);
    });
}

#[test]
fn test_trx_extension_preserves_cached_legacy_bsv_usdt_order_state_after_service_recreation() {
    block_on(async {
        let context = Context::new();
        context.cache(context.cached_session(None));
        let store = Arc::new(MemoryPortalSwapStore::default());
        let service = context.swap_service(store.clone());
        let scope = service.scope(&context.wallet).unwrap();
        let legacy = include_str!("../../../testdata/portal/state_legacy.json");
        store.set(scope.clone(), legacy.into()).unwrap();

        let recreated = context.swap_service(store.clone());
        let orders = recreated.cached_orders().await.unwrap();

        assert_eq!(
            orders.iter().map(|order| order.direction).collect::<Vec<_>>(),
            vec![Some(BsvHomePortalSwapDirection::BsvToUsdt), Some(BsvHomePortalSwapDirection::UsdtToBsv)]
        );
        assert_eq!(
            orders.iter().map(|order| order.from_amount.clone()).collect::<Vec<_>>(),
            vec![BigUint::from(150_000_000u32), BigUint::from(12_345_678u32)]
        );
        assert_eq!(orders.iter().map(|order| order.can_pay).collect::<Vec<_>>(), vec![false, false]);
        assert_eq!(
            orders.iter().map(|order| order.sync_status).collect::<Vec<_>>(),
            vec![BsvHomePortalSwapSyncStatus::Stale, BsvHomePortalSwapSyncStatus::Stale]
        );
        let persisted = recreated.read_state(&scope).unwrap();
        assert_eq!(
            serde_json::to_value(&persisted.orders)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|order| order["direction"].clone())
                .collect::<Vec<_>>(),
            vec![Value::String("BsvToUsdt".into()), Value::String("UsdtToBsv".into())]
        );
        assert_eq!(store.get(scope).unwrap(), Some(legacy.into()));
        assert_eq!(context.provider.paths(), Vec::<String>::new());
    });
}

#[test]
fn test_wallet_change_prevents_saved_pending_order_from_using_other_wallet_auth() {
    block_on(async {
        let context = Context::new();
        let service = context.service();
        context.switch_wallet();
        assert_eq!(
            service
                .swap_wallet_request::<serde_json::Value>(&context.wallet, "/auth/swaps/orders", AlienHttpMethod::Post, None)
                .await,
            Err(BsvHomePortalAuthError::WalletChanged)
        );
        assert_eq!(context.provider.requests.lock().unwrap().len(), 0);
    });
}

#[path = "action_checkpoint_tests.rs"]
mod action_checkpoint;
