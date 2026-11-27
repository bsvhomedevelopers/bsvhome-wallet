use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bsv_home_crypto::random;
use bsv_home_keystore::Mnemonic;
use futures::executor::block_on;
use primitives::{Chain, Wallet, WalletId, WalletSource};
use serde::Serialize;
use serde_json::{Value, json};
use tempfile::TempDir;
use zeroize::Zeroizing;

use super::{BSVHOME_BASE_URL, CurlPortalProvider, TemporarySecureStore, V2_BASE_URL, curl_response, json_type, request_error, schema_fields};
use crate::alien::{AlienError, AlienHttpMethod, AlienProvider, AlienResponse, AlienTarget};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore, decode_password};
use crate::services::bsv_cold_wallet::testkit::MemoryColdWalletStore;
use crate::services::bsv_multisig::testkit::MemoryBsvMultisigStore;
use crate::services::bsv_multisig::{
    BsvHomeBsvMultisigActionScope, BsvHomeBsvMultisigConfigurationStatus, BsvHomeBsvMultisigService, BsvHomeBsvMultisigSigner, BsvHomeBsvMultisigWallet,
};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::error::BsvHomeServiceError;
use crate::services::portal::BsvHomePortalAuthService;
use crate::services::wallet::addresses::testkit::pool_with_keystore;
use crate::services::wallet::rules as wallet_rules;
use crate::services::wallet::testkit::MemoryWalletStore;
use crate::services::wallet::{BsvHomeKeystoreAuthentication, BsvHomeKeystorePassword};
use crate::services::wallet_session::BsvHomeWalletSessionService;
use crate::services::wallet_session::testkit::MemoryWalletSessionStore;

const WRITE_OPT_IN: &str = "BSVHOME_MULTISIG_LIVE_WRITE";
const GROUP_NAME: &str = "Temporary phase5 verification";
const MAX_HTTP_REQUESTS: usize = 16;

struct MultisigWriteApproval;

impl MultisigWriteApproval {
    fn from_environment() -> Result<Self, &'static str> {
        Self::from_value(std::env::var(WRITE_OPT_IN).ok().as_deref())
    }

    fn from_value(value: Option<&str>) -> Result<Self, &'static str> {
        match value {
            Some("1") => Ok(Self),
            Some(_) | None => Err("fresh approval and exact multisig live-write opt-in are required"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Identity {
    Creator,
    Joiner,
}

impl Identity {
    fn index(self) -> usize {
        match self {
            Self::Creator => 0,
            Self::Joiner => 1,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Challenge,
    Login,
    List,
    Create,
    Wallet,
    Invite,
    Preview,
    Join,
    Confirm,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Challenge => "challenge",
            Self::Login => "login",
            Self::List => "wallet_list",
            Self::Create => "create",
            Self::Wallet => "wallet_read",
            Self::Invite => "invite",
            Self::Preview => "preview",
            Self::Join => "join",
            Self::Confirm => "confirm",
        }
    }

    fn method(self) -> AlienHttpMethod {
        match self {
            Self::List | Self::Wallet => AlienHttpMethod::Get,
            Self::Challenge | Self::Login | Self::Create | Self::Invite | Self::Preview | Self::Join | Self::Confirm => AlienHttpMethod::Post,
        }
    }

    fn authenticated(self) -> bool {
        match self {
            Self::Challenge | Self::Login => false,
            Self::List | Self::Create | Self::Wallet | Self::Invite | Self::Preview | Self::Join | Self::Confirm => true,
        }
    }

    fn path(self, wallet_id: Option<&str>) -> Option<String> {
        let path = match self {
            Self::Challenge => "/wallet-auth/challenge".into(),
            Self::Login => "/wallet-auth/login".into(),
            Self::List | Self::Create => "/auth/multisig/wallets".into(),
            Self::Wallet => format!("/auth/multisig/wallets/{}", wallet_id?),
            Self::Invite => format!("/auth/multisig/wallets/{}/invites", wallet_id?),
            Self::Preview => "/auth/multisig/invites/preview".into(),
            Self::Join => "/auth/multisig/join".into(),
            Self::Confirm => format!("/auth/multisig/wallets/{}/confirm", wallet_id?),
        };
        Some(path)
    }
}

const BUSINESS: [(Identity, Action); 12] = [
    (Identity::Creator, Action::List),
    (Identity::Creator, Action::Create),
    (Identity::Creator, Action::Wallet),
    (Identity::Creator, Action::Invite),
    (Identity::Joiner, Action::Preview),
    (Identity::Joiner, Action::Join),
    (Identity::Creator, Action::Wallet),
    (Identity::Creator, Action::Wallet),
    (Identity::Creator, Action::Confirm),
    (Identity::Joiner, Action::Wallet),
    (Identity::Joiner, Action::Confirm),
    (Identity::Creator, Action::Wallet),
];

#[derive(Default)]
struct RequestBudget {
    next: usize,
    authentication: [u8; 2],
    total: usize,
    failed: bool,
    wallet_id: Option<String>,
    invitation: Option<Zeroizing<String>>,
}

impl RequestBudget {
    fn reserve(&mut self, identity: Identity, action: Action) -> Result<(), AlienError> {
        if self.failed || self.total >= MAX_HTTP_REQUESTS || self.next >= BUSINESS.len() {
            return Err(request_error("multisig live request budget is closed"));
        }
        let index = identity.index();
        let first = match identity {
            Identity::Creator => 0,
            Identity::Joiner => 4,
        };
        match action {
            Action::Challenge if self.next == first && self.authentication[index] == 0 => self.authentication[index] = 1,
            Action::Login if self.next == first && self.authentication[index] == 1 => self.authentication[index] = 2,
            Action::Challenge | Action::Login => return Err(request_error("multisig live authentication order rejected")),
            _ if self.authentication[index] == 2 && BUSINESS[self.next] == (identity, action) => self.next += 1,
            _ => return Err(request_error("multisig live business order rejected")),
        }
        self.total += 1;
        Ok(())
    }
}

#[derive(Serialize)]
struct ResponseSchema {
    stage: &'static str,
    http_status: u16,
    envelope_fields: BTreeMap<String, &'static str>,
    data_type: &'static str,
    wallet_fields: BTreeMap<String, &'static str>,
    member_fields: Vec<BTreeMap<String, &'static str>>,
    member_count: Option<usize>,
    invitation_fields: BTreeMap<String, &'static str>,
}

struct MultisigLifecycleProvider {
    identity: Identity,
    address: String,
    public_key: String,
    budget: Arc<Mutex<RequestBudget>>,
    _approval: Arc<MultisigWriteApproval>,
}

impl Debug for MultisigLifecycleProvider {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("MultisigLifecycleProvider")
    }
}

impl MultisigLifecycleProvider {
    fn action(&self, target: &AlienTarget, budget: &RequestBudget) -> Result<Action, AlienError> {
        let path = target.url.strip_prefix(BSVHOME_BASE_URL).ok_or_else(|| request_error("multisig live origin rejected"))?;
        let action = [Action::Challenge, Action::Login]
            .into_iter()
            .chain(BUSINESS.iter().map(|(_, action)| *action))
            .find(|action| action.method() == target.method && action.path(budget.wallet_id.as_deref()).as_deref() == Some(path))
            .ok_or_else(|| request_error("multisig live endpoint rejected"))?;
        let credentials = target
            .headers
            .iter()
            .flat_map(|headers| headers.iter())
            .filter(|(name, _)| name.eq_ignore_ascii_case("authorization"))
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        if credentials.len() != usize::from(action.authenticated()) || credentials.iter().any(|value| value.is_empty() || value.starts_with("Bearer ")) {
            return Err(request_error("multisig live authorization boundary rejected"));
        }
        if target.method == AlienHttpMethod::Get {
            if target.body.is_some() {
                return Err(request_error("multisig live GET body rejected"));
            }
            return Ok(action);
        }
        let body: Value =
            serde_json::from_slice(target.body.as_deref().ok_or_else(|| request_error("multisig live body missing"))?).map_err(|_| request_error("multisig live body rejected"))?;
        let object = body.as_object().ok_or_else(|| request_error("multisig live object body required"))?;
        let exact = |keys: &[&str]| object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key));
        let version = || body.get("expectedVersion").and_then(Value::as_u64).is_some();
        let token = || body.get("token").and_then(Value::as_str) == budget.invitation.as_deref().map(String::as_str) && budget.invitation.is_some();
        let valid = match action {
            Action::Challenge | Action::Login => {
                let signing = match action {
                    Action::Challenge => exact(&["chain", "address", "deviceId", "deviceType"]),
                    Action::Login => {
                        exact(&["chain", "address", "deviceId", "deviceType", "challengeId", "signature"])
                            && body.get("challengeId").and_then(Value::as_str).is_some_and(|value| !value.is_empty())
                            && body.get("signature").and_then(Value::as_str).is_some_and(|value| !value.is_empty())
                    }
                    _ => false,
                };
                signing
                    && body.get("chain").and_then(Value::as_str) == Some("BSV")
                    && body.get("address").and_then(Value::as_str) == Some(self.address.as_str())
                    && body.get("deviceType").and_then(Value::as_str) == Some("4")
                    && body.get("deviceId").and_then(Value::as_str).is_some_and(|value| !value.is_empty())
            }
            Action::Create => {
                exact(&["name", "threshold", "memberCount", "publicKey"])
                    && body.get("name").and_then(Value::as_str) == Some(GROUP_NAME)
                    && body.get("threshold").and_then(Value::as_u64) == Some(2)
                    && body.get("memberCount").and_then(Value::as_u64) == Some(2)
                    && body.get("publicKey").and_then(Value::as_str) == Some(self.public_key.as_str())
            }
            Action::Invite => exact(&["expectedVersion"]) && version(),
            Action::Preview => exact(&["token"]) && token(),
            Action::Join => {
                exact(&["token", "publicKey", "walletId", "expectedVersion"])
                    && token()
                    && version()
                    && body.get("walletId").and_then(Value::as_str) == budget.wallet_id.as_deref()
                    && body.get("publicKey").and_then(Value::as_str) == Some(self.public_key.as_str())
            }
            Action::Confirm => {
                exact(&["expectedVersion", "configurationHash"]) && version() && body.get("configurationHash").and_then(Value::as_str).is_some_and(|value| !value.is_empty())
            }
            Action::List | Action::Wallet => false,
        };
        if !valid {
            return Err(request_error("multisig live request identity or shape rejected"));
        }
        Ok(action)
    }

    fn record_response(&self, action: Action, status: u16, data: &[u8]) -> Result<(), AlienError> {
        let response: Value = serde_json::from_slice(data).map_err(|_| request_error("multisig live response JSON rejected"))?;
        let payload = response.get("data");
        let wallet = match action {
            Action::Create | Action::Wallet | Action::Join | Action::Confirm => payload,
            Action::Preview => payload.and_then(|data| data.get("wallet")),
            _ => None,
        };
        let members = wallet.and_then(|wallet| wallet.get("members")).and_then(Value::as_array);
        let schema = ResponseSchema {
            stage: action.label(),
            http_status: status,
            envelope_fields: schema_fields(Some(&response), &["success", "forceLogout", "code", "message", "data"]),
            data_type: json_type(payload),
            wallet_fields: schema_fields(
                wallet,
                &[
                    "id",
                    "name",
                    "status",
                    "threshold",
                    "memberCount",
                    "version",
                    "members",
                    "lockingScript",
                    "configurationHash",
                    "scriptHash",
                ],
            ),
            member_fields: members
                .into_iter()
                .flatten()
                .take(2)
                .map(|member| schema_fields(Some(member), &["userId", "address", "publicKey", "confirmed"]))
                .collect(),
            member_count: members.map(Vec::len),
            invitation_fields: schema_fields(
                if matches!(action, Action::Invite | Action::Preview) { payload } else { None },
                &["wallet", "walletId", "token", "expiresAt"],
            ),
        };
        println!(
            "Temporary multisig response schema: {}",
            serde_json::to_string(&schema).map_err(|_| request_error("safe multisig schema encoding failed"))?
        );
        if !(200..300).contains(&status) || response.get("success").and_then(Value::as_bool) != Some(true) || response.get("forceLogout").and_then(Value::as_bool) == Some(true) {
            return Err(request_error("multisig live HTTP or envelope rejected"));
        }
        let mut budget = self.budget.lock().map_err(|_| request_error("multisig live budget unavailable"))?;
        if action == Action::List && payload.and_then(Value::as_array).is_none_or(|wallets| !wallets.is_empty()) {
            return Err(request_error("new temporary identity must have no existing collective wallets"));
        }
        if action == Action::Create {
            let id = wallet
                .and_then(|wallet| wallet.get("id"))
                .and_then(Value::as_str)
                .ok_or_else(|| request_error("multisig live created ID missing"))?;
            if budget.wallet_id.is_some() || id.is_empty() || !id.bytes().all(|value| value.is_ascii_alphanumeric() || value == b'-' || value == b'_') {
                return Err(request_error("multisig live created ID rejected"));
            }
            budget.wallet_id = Some(id.into());
        }
        if let Some(wallet) = wallet
            && wallet.get("id").and_then(Value::as_str) != budget.wallet_id.as_deref()
        {
            return Err(request_error("multisig live response wallet scope changed"));
        }
        if action == Action::Invite {
            let token = payload
                .and_then(|data| data.get("token"))
                .and_then(Value::as_str)
                .ok_or_else(|| request_error("multisig live invitation missing"))?;
            budget.invitation = Some(Zeroizing::new(token.into()));
        }
        Ok(())
    }
}

#[async_trait]
impl AlienProvider for MultisigLifecycleProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        let action = {
            let mut budget = self.budget.lock().map_err(|_| request_error("multisig live budget unavailable"))?;
            let action = self.action(&target, &budget)?;
            budget.reserve(self.identity, action)?;
            action
        };
        let method = if target.method == AlienHttpMethod::Get { "GET" } else { "POST" };
        let result = curl_response(&target.url, method, target.headers, target.body).and_then(|(status, data)| {
            self.record_response(action, status, &data)?;
            Ok(Arc::new(AlienResponse::new(Some(status), data)))
        });
        if result.is_err() {
            self.budget.lock().map_err(|_| request_error("multisig live budget unavailable"))?.failed = true;
        }
        result
    }
}

struct EphemeralSigner {
    keystore: Arc<BsvHomeKeystore>,
    wallet_id: WalletId,
    password: Zeroizing<String>,
}

impl BsvHomeKeystorePassword for EphemeralSigner {
    fn get_password(&self, _: bool) -> Result<String, BsvHomeServiceError> {
        Ok(self.password.to_string())
    }
    fn get_wallet_password(&self, _: WalletId) -> Result<Option<String>, BsvHomeServiceError> {
        Ok(None)
    }
    fn delete_wallet_password(&self, _: WalletId) -> Result<(), BsvHomeServiceError> {
        Err(outside_scope())
    }
    fn authentication(&self) -> Result<BsvHomeKeystoreAuthentication, BsvHomeServiceError> {
        Ok(BsvHomeKeystoreAuthentication::None)
    }
}

fn outside_scope() -> BsvHomeServiceError {
    BsvHomeServiceError::Unsupported {
        msg: "transaction signing and secret deletion are outside this live test".into(),
    }
}

#[async_trait]
impl BsvHomeBsvMultisigSigner for EphemeralSigner {
    async fn public_key(&self, wallet: Wallet) -> Result<String, BsvHomeServiceError> {
        if wallet.id != self.wallet_id {
            return Err(outside_scope());
        }
        self.keystore
            .bsv_multisig_public_key(self.keystore.keystore_id(wallet.id.id()), decode_password(&self.password))
            .map_err(|_| BsvHomeServiceError::Platform {
                msg: "temporary encrypted identity could not be unlocked".into(),
            })
    }
    async fn authenticate(&self, wallet: Wallet, scope: Option<Arc<BsvHomeBsvMultisigActionScope>>) -> Result<(), BsvHomeServiceError> {
        if let Some(scope) = scope {
            scope.check().map_err(|_| BsvHomeServiceError::Cancelled)?;
        }
        self.public_key(wallet).await.map(|_| ())
    }
    async fn sign_proposal(&self, _: Wallet, _: String, _: String, _: String, _: u64, _: u64, _: Arc<BsvHomeBsvMultisigActionScope>) -> Result<String, BsvHomeServiceError> {
        Err(outside_scope())
    }
    async fn sign_funding(&self, _: Wallet, _: String, _: String, _: Arc<BsvHomeBsvMultisigActionScope>) -> Result<String, BsvHomeServiceError> {
        Err(outside_scope())
    }
}

struct LiveIdentity {
    service: BsvHomeBsvMultisigService,
    wallet: Wallet,
    _directory: TempDir,
}

impl Drop for LiveIdentity {
    fn drop(&mut self) {
        self.service.cancel();
    }
}

impl LiveIdentity {
    fn new(identity: Identity, budget: Arc<Mutex<RequestBudget>>, approval: Arc<MultisigWriteApproval>) -> Result<Self, &'static str> {
        let directory = TempDir::new().map_err(|_| "temporary multisig directory unavailable")?;
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).map_err(|_| "temporary multisig keystore unavailable")?;
        let random = Zeroizing::new(random::bytes::<32>().map_err(|_| "temporary multisig randomness unavailable")?);
        let password = Zeroizing::new(hex::encode(random.as_slice()));
        let stored = keystore
            .create_store(
                BsvHomeImportType::SinglePhrase {
                    words: Mnemonic::generate(12).map_err(|_| "temporary multisig mnemonic generation failed")?,
                    chain: Chain::BitcoinSv,
                },
                decode_password(&password),
            )
            .map_err(|_| "temporary encrypted multisig identity creation failed")?;
        let wallet = Wallet {
            id: WalletId::from_id(&stored.wallet_id).ok_or("temporary multisig wallet ID invalid")?,
            external_id: None,
            name: "Unfunded temporary multisig identity".into(),
            index: 0,
            wallet_type: stored.wallet_type,
            accounts: stored.accounts.into_iter().map(wallet_rules::account).collect(),
            is_pinned: false,
            image_url: None,
            source: WalletSource::Create,
        };
        let signer = Arc::new(EphemeralSigner {
            keystore: keystore.clone(),
            wallet_id: wallet.id.clone(),
            password,
        });
        let public_key = keystore
            .bsv_multisig_public_key(stored.keystore_id, decode_password(&signer.password))
            .map_err(|_| "temporary multisig public identity unavailable")?;
        let provider = Arc::new(MultisigLifecycleProvider {
            identity,
            address: wallet.account(Chain::BitcoinSv).ok_or("temporary multisig BSV account missing")?.address.clone(),
            public_key,
            budget,
            _approval: approval,
        });
        let wallets = Arc::new(MemoryWalletStore::default());
        wallets.wallets.lock().map_err(|_| "temporary multisig wallet store unavailable")?.push(wallet.clone());
        let current = Arc::new(MemoryWalletSessionStore::default());
        *current.current.lock().map_err(|_| "temporary multisig session store unavailable")? = Some(wallet.id.clone());
        let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
            current,
            wallets.clone(),
            BsvHomeRuntimeEnvironment::WowPay,
        ));
        let auth = Arc::new(
            BsvHomePortalAuthService::new_for_wallet(
                provider.clone(),
                keystore.clone(),
                signer.clone(),
                session.clone(),
                Arc::new(BsvHomeDeviceKeyService::new(Arc::new(TemporarySecureStore::default()))),
                Arc::new(TemporarySecureStore::default()),
                BSVHOME_BASE_URL.into(),
                "en".into(),
                "4".into(),
                wallet.clone(),
            )
            .map_err(|_| "temporary multisig authentication configuration invalid")?,
        );
        let gateway = Arc::new(BsvHomeGateway::new(provider, Arc::new(EmptyPreferences), Arc::new(EmptyPreferences)));
        let addresses = pool_with_keystore(gateway.clone(), wallets.clone(), keystore, signer.clone(), wallets.bsv_addresses.clone());
        let service = BsvHomeBsvMultisigService::new(
            Arc::new(MemoryBsvMultisigStore::default()),
            session,
            gateway,
            signer,
            auth,
            addresses,
            wallet.id.id(),
            Arc::new(MemoryColdWalletStore::default()),
        );
        Ok(Self {
            service,
            wallet,
            _directory: directory,
        })
    }
}

fn check_members(wallet: &BsvHomeBsvMultisigWallet, count: usize) -> Result<(), &'static str> {
    if wallet.threshold != 2 || wallet.member_count != 2 || wallet.members.iter().filter(|member| member.public_key.is_some()).count() != count {
        return Err("temporary multisig member or signing-rule count mismatch");
    }
    Ok(())
}

#[test]
#[ignore = "requires fresh server-write approval and BSVHOME_MULTISIG_LIVE_WRITE=1; two new unfunded identities, one 2-of-2 group, one invite/join, two confirmations, at most sixteen HTTP requests"]
fn test_live_bsvhome_two_temporary_identities_multisig_lifecycle_after_explicit_write_approval() -> Result<(), &'static str> {
    let approval = Arc::new(MultisigWriteApproval::from_environment()?);
    block_on(async {
        let budget = Arc::new(Mutex::new(RequestBudget::default()));
        let creator = LiveIdentity::new(Identity::Creator, budget.clone(), approval.clone())?;
        let joiner = LiveIdentity::new(Identity::Joiner, budget.clone(), approval)?;
        if creator.wallet.id == joiner.wallet.id {
            return Err("temporary multisig identities collided");
        }
        let draft = creator
            .service
            .begin_creation(GROUP_NAME.into(), 2, vec!["Creator".into(), "Joiner".into()], creator.wallet.id.id())
            .await
            .map_err(|_| "temporary multisig creation review failed")?;
        let created = creator
            .service
            .submit_creation(draft.id.clone())
            .await
            .map_err(|_| "temporary multisig creation result failed; do not repeat creation")?;
        check_members(&created, 1)?;
        let invitation = creator.service.invitation(created.id.clone()).await.map_err(|_| "temporary multisig invitation failed")?;
        let preview = joiner
            .service
            .preview_invitation(invitation.token)
            .await
            .map_err(|_| "temporary multisig invitation preview failed")?;
        check_members(&preview.wallet, 1)?;
        let joined = joiner.service.join_invitation(preview).await.map_err(|_| "temporary multisig join failed")?;
        check_members(&joined, 2)?;
        let reviewed = creator
            .service
            .reconcile_creation(draft.id.clone())
            .await
            .map_err(|_| "temporary multisig creator fresh review failed")?;
        let reviewed = reviewed.first().ok_or("temporary multisig creator review missing")?;
        check_members(reviewed, 2)?;
        let first = creator
            .service
            .approve_configuration(created.id.clone(), reviewed.version)
            .await
            .map_err(|_| "temporary multisig creator confirmation failed")?;
        let second = joiner
            .service
            .approve_configuration(created.id.clone(), first.version)
            .await
            .map_err(|_| "temporary multisig joiner confirmation failed")?;
        let final_wallets = creator
            .service
            .reconcile_creation(draft.id)
            .await
            .map_err(|_| "temporary multisig final creator refresh failed")?;
        let final_wallet = final_wallets.first().ok_or("temporary multisig final wallet missing")?;
        for wallet in [final_wallet, &second] {
            check_members(wallet, 2)?;
            if wallet.configuration_status != BsvHomeBsvMultisigConfigurationStatus::Confirmed || !wallet.can_transact {
                return Err("temporary multisig local configuration trust missing");
            }
        }
        let creator_backup = creator
            .service
            .export_configuration(created.id.clone())
            .map_err(|_| "temporary multisig creator public backup failed")?;
        let joiner_backup = joiner
            .service
            .export_configuration(created.id)
            .map_err(|_| "temporary multisig joiner public backup failed")?;
        if creator_backup != joiner_backup {
            return Err("temporary multisig public backup mismatch");
        }
        creator.service.cancel();
        joiner.service.cancel();
        let budget = budget.lock().map_err(|_| "temporary multisig request budget unavailable")?;
        if budget.failed || budget.next != BUSINESS.len() || budget.authentication != [2, 2] || budget.total != MAX_HTTP_REQUESTS {
            return Err("temporary multisig request budget mismatch");
        }
        println!(
            "Temporary multisig lifecycle passed: identities=2 groups=1 challenge=2 login=2 business_get=6 create=1 invite=1 preview=1 join=1 confirm=2 local_public_backups=2 total_http=16; no funds, proposals, payment, password or broadcast requests"
        );
        Ok(())
    })
}

#[test]
fn test_default_portal_live_provider_rejects_all_multisig_writes_without_transport() {
    for provider in [
        CurlPortalProvider::default(),
        CurlPortalProvider::new(BSVHOME_BASE_URL),
        CurlPortalProvider::new(V2_BASE_URL),
    ] {
        for path in [
            "/wallets",
            "/wallets/public-fixture/invites",
            "/invites/preview",
            "/join",
            "/wallets/public-fixture/confirm",
        ] {
            let prefix = if provider.base_url == V2_BASE_URL { "/v2/api/portal" } else { "/api/portal" };
            assert!(provider.request_target(&format!("{prefix}/auth/multisig{path}"), AlienHttpMethod::Post).is_err());
        }
        assert_eq!(provider.counts(), (0, 0, 0));
        assert!(provider.schema_metadata().is_empty());
    }
}

#[test]
fn test_multisig_live_write_requires_exact_opt_in_and_one_ordered_lifecycle() {
    for value in [None, Some(""), Some("0"), Some("true")] {
        assert!(MultisigWriteApproval::from_value(value).is_err());
    }
    let mut budget = RequestBudget::default();
    assert!(budget.reserve(Identity::Creator, Action::Create).is_err());
    assert!(budget.reserve(Identity::Joiner, Action::Challenge).is_err());
    for (index, (identity, action)) in BUSINESS.iter().enumerate() {
        if index == 0 || index == 4 {
            budget.reserve(*identity, Action::Challenge).unwrap();
            assert!(budget.reserve(*identity, Action::Challenge).is_err());
            budget.reserve(*identity, Action::Login).unwrap();
            assert!(budget.reserve(*identity, Action::Login).is_err());
        }
        budget.reserve(*identity, *action).unwrap();
    }
    assert_eq!(budget.total, MAX_HTTP_REQUESTS);
    assert!(budget.reserve(Identity::Creator, Action::Create).is_err());
    assert!(budget.reserve(Identity::Joiner, Action::Challenge).is_err());
    let mut failed = RequestBudget {
        failed: true,
        ..Default::default()
    };
    assert!(failed.reserve(Identity::Creator, Action::Challenge).is_err());
    assert_eq!(failed.total, 0);
}

#[test]
fn test_multisig_live_provider_rejects_foreign_origins_other_methods_and_extra_fields_without_transport() {
    let provider = MultisigLifecycleProvider {
        identity: Identity::Creator,
        address: "public-fixture".into(),
        public_key: "public-key-fixture".into(),
        budget: Arc::new(Mutex::new(RequestBudget::default())),
        _approval: Arc::new(MultisigWriteApproval),
    };
    let budget = RequestBudget::default();
    for (url, method) in [
        ("https://foreign.example/api/portal/wallet-auth/challenge", AlienHttpMethod::Post),
        ("https://paymail.bsvhome.com/api/portal/wallet-auth/challenge", AlienHttpMethod::Get),
        (
            "https://paymail.bsvhome.com/api/portal/auth/multisig/proposals/public-fixture/broadcast",
            AlienHttpMethod::Post,
        ),
        ("https://paymail.bsvhome.com/api/portal/auth/multisig/wallets/public-fixture/utxos", AlienHttpMethod::Get),
        ("https://paymail.bsvhome.com/api/portal/auth/paymail/bindings", AlienHttpMethod::Post),
        ("https://paymail.bsvhome.com/api/portal/wallet-auth/transfer-password/set", AlienHttpMethod::Post),
        ("https://paymail.bsvhome.com/api/portal/auth/swaps/orders", AlienHttpMethod::Post),
    ] {
        assert!(
            block_on(provider.request(AlienTarget {
                url: url.into(),
                method,
                headers: None,
                body: None
            }))
            .is_err()
        );
    }
    let mut body = json!({"chain":"BSV","address":"public-fixture","deviceId":"public-device","deviceType":"4"});
    let mut target = AlienTarget {
        url: format!("{BSVHOME_BASE_URL}/wallet-auth/challenge"),
        method: AlienHttpMethod::Post,
        headers: None,
        body: Some(serde_json::to_vec(&body).unwrap()),
    };
    assert!(matches!(provider.action(&target, &budget), Ok(Action::Challenge)));
    target.headers = Some([("Authorization".into(), "public-test-credential".into())].into_iter().collect());
    assert!(provider.action(&target, &budget).is_err());
    target.headers = None;
    body["unexpected"] = Value::Bool(true);
    target.body = Some(serde_json::to_vec(&body).unwrap());
    assert!(provider.action(&target, &budget).is_err());
    assert_eq!(provider.budget.lock().unwrap().total, 0);
}
