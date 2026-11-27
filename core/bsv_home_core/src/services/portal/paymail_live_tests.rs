use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use bsv_home_keystore::Mnemonic;
use futures::executor::block_on;
use primitives::testkit::ABANDON_PHRASE;
use primitives::{Chain, Wallet, WalletId, WalletSource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::{NamedTempFile, TempDir};
use uuid::Uuid;

use super::live_tests::{BASE_URL, CurlPortalProvider, PUBLIC_ORIGIN, TemporarySecureStore, authentication_failure};
use super::paymail_model::{BsvHomePaymailBindingIntent, PendingBinding};
use super::{
    BsvHomePaymailDestinationService, BsvHomePaymailError, BsvHomePaymailMutationStatus, BsvHomePaymailService, BsvHomePaymailSnapshot, BsvHomePaymailVerification,
    BsvHomePortalAuthService,
};
use crate::alien::{AlienHttpMethod, AlienProvider, AlienResponse, AlienTarget};
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore, decode_password, keystore_id_for_wallet};
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::preferences::BsvHomeSecureStore;
use crate::services::wallet::rules as wallet_rules;
use crate::services::wallet::testkit::{MemoryKeystorePassword, MemoryWalletStore, TEST_PASSWORD};
use crate::services::wallet_session::BsvHomeWalletSessionService;
use crate::services::wallet_session::testkit::MemoryWalletSessionStore;

#[derive(Deserialize)]
struct PublicIdentity {
    bsvalias: String,
    handle: String,
    pubkey: String,
}

const FIXTURE_MANIFEST: &str = "paymail-live.fixture";
const FIXTURE_NAME: &str = "Temporary Paymail verification";
const FIXTURE_ALIAS_PREFIX: &str = "ios-paymail-test-";

fn temporary_directory() -> Result<TempDir, String> {
    let directory = TempDir::new().map_err(|_| "Temporary Paymail keystore directory unavailable")?;
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).map_err(|_| "Temporary Paymail keystore directory protection failed")?;
    Ok(directory)
}

enum FixtureDirectory {
    Temporary(TempDir),
    Retained(PathBuf),
}

impl FixtureDirectory {
    fn path(&self) -> &Path {
        match self {
            Self::Temporary(directory) => directory.path(),
            Self::Retained(directory) => directory,
        }
    }

    fn retain(self) -> PathBuf {
        match self {
            Self::Temporary(directory) => directory.keep(),
            Self::Retained(directory) => directory,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PendingPaymailFixture {
    wallet: Wallet,
    alias: String,
    domain: String,
    pending_bindings: Vec<PendingBinding>,
    schema_metadata: Vec<Value>,
}

fn save_fixture(directory: &Path, fixture: &PendingPaymailFixture) -> Result<(), String> {
    let mut file = NamedTempFile::new_in(directory).map_err(|_| "Temporary Paymail fixture manifest cannot be saved")?;
    let data = serde_json::to_vec(fixture).map_err(|_| "Temporary Paymail fixture metadata invalid")?;
    file.write_all(&data).map_err(|_| "Temporary Paymail fixture metadata write failed")?;
    file.as_file().sync_all().map_err(|_| "Temporary Paymail fixture metadata sync failed")?;
    file.persist(directory.join(FIXTURE_MANIFEST))
        .map_err(|_| "Temporary Paymail fixture metadata commit failed")?;
    Ok(())
}

fn open_fixture(directory: PathBuf) -> Result<(FixtureDirectory, PendingPaymailFixture), String> {
    let metadata = fs::symlink_metadata(&directory).map_err(|_| "Retained Paymail fixture unavailable")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata.permissions().mode() & 0o777 != 0o700 {
        return Err("Retained Paymail fixture must be an owner-only directory".into());
    }
    let directory = directory.canonicalize().map_err(|_| "Retained Paymail fixture path invalid")?;
    if !directory.starts_with(env::temp_dir().canonicalize().map_err(|_| "Temporary directory unavailable")?) {
        return Err("Retained Paymail fixture must remain in the temporary directory".into());
    }
    let fixture: PendingPaymailFixture = serde_json::from_slice(&fs::read(directory.join(FIXTURE_MANIFEST)).map_err(|_| "Retained Paymail manifest unavailable")?)
        .map_err(|_| "Retained Paymail manifest invalid")?;
    if fixture.wallet.name != FIXTURE_NAME
        || fixture.alias.strip_prefix(FIXTURE_ALIAS_PREFIX).and_then(|suffix| Uuid::parse_str(suffix).ok()).is_none()
        || fixture.domain != "api-dev.hj0252.com"
    {
        return Err("Retained Paymail fixture ownership marker invalid".into());
    }
    let keystore_file = format!("{}.json", keystore_id_for_wallet(fixture.wallet.id.id()));
    for entry in fs::read_dir(&directory).map_err(|_| "Retained Paymail fixture listing failed")? {
        let entry = entry.map_err(|_| "Retained Paymail fixture entry invalid")?;
        let metadata = entry.path().symlink_metadata().map_err(|_| "Retained Paymail fixture file unavailable")?;
        let name = entry.file_name();
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.permissions().mode() & 0o777 != 0o600
            || (name != FIXTURE_MANIFEST && name != keystore_file.as_str())
        {
            return Err("Retained Paymail fixture contains an unexpected or unprotected file".into());
        }
    }
    Ok((FixtureDirectory::Retained(directory), fixture))
}

async fn cleanup_binding(service: &BsvHomePaymailService, provider: &CurlPortalProvider, alias: &str, domain: &str, address: &str) -> Result<(), String> {
    let handle = format!("{alias}@{domain}");
    let current = service.load().await.map_err(failure)?;
    if current.verification == BsvHomePaymailVerification::Pending {
        return Err("Temporary Paymail cleanup remains pending verification".into());
    }
    if let Some(binding) = current.bindings.iter().find(|binding| binding.handle == handle && binding.active) {
        let expected_version = binding.version + 1;
        let identity_public_key = binding.identity_public_key.clone();
        let disabled = change_binding(service, alias, domain, false).await?;
        let binding = disabled
            .bindings
            .iter()
            .find(|binding| binding.handle == handle)
            .ok_or("Temporary Paymail cleanup lost the reserved binding")?;
        if binding.active || binding.version != expected_version || binding.identity_public_key != identity_public_key || binding.address != address {
            return Err("Temporary Paymail cleanup failed the disabled binding and version contract".into());
        }
    }
    sleep(Duration::from_secs(1));
    verify_disabled_public_lookup(provider, &handle).await
}

#[test]
fn test_retained_paymail_fixture_restores_encrypted_identity_without_session_tokens() {
    let directory = temporary_directory().unwrap();
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
    let wallet = Wallet {
        id: WalletId::from_id(&stored.wallet_id).unwrap(),
        external_id: None,
        name: FIXTURE_NAME.into(),
        index: 0,
        wallet_type: stored.wallet_type,
        accounts: stored.accounts.into_iter().map(wallet_rules::account).collect(),
        is_pinned: false,
        image_url: None,
        source: WalletSource::Create,
    };
    let address = wallet.accounts[0].address.clone();
    let identity_public_key = keystore.paymail_identity_public_key(stored.keystore_id, &address, decode_password(TEST_PASSWORD)).unwrap();
    let store = TemporarySecureStore::default();
    store.set("session".into(), "synthetic-session-token-must-not-persist".into()).unwrap();
    let alias = format!("{FIXTURE_ALIAS_PREFIX}{}", Uuid::new_v4().simple());
    let pending = PendingBinding {
        intent: BsvHomePaymailBindingIntent {
            wallet_id: wallet.id.id(),
            address: address.clone(),
            public_origin: PUBLIC_ORIGIN.into(),
            handle: format!("{alias}@api-dev.hj0252.com"),
            active: true,
            previous_version: 0,
        },
        identity_public_key: identity_public_key.clone(),
    };
    store
        .set(format!("paymail_pending|{BASE_URL}|{}|{address}", wallet.id.id()), serde_json::to_string(&pending).unwrap())
        .unwrap();
    let fixture = PendingPaymailFixture {
        wallet,
        alias,
        domain: "api-dev.hj0252.com".into(),
        pending_bindings: store.pending_bindings().unwrap(),
        schema_metadata: Vec::new(),
    };
    save_fixture(directory.path(), &fixture).unwrap();
    let metadata = fs::read_to_string(directory.path().join(FIXTURE_MANIFEST)).unwrap();
    assert_eq!(metadata.find("synthetic-session-token-must-not-persist"), None);
    assert_eq!(metadata.find(ABANDON_PHRASE), None);
    let directory = directory.keep();
    let (_, recovered) = open_fixture(directory.clone()).unwrap();
    assert_eq!(recovered.wallet.id, fixture.wallet.id);
    assert_eq!(recovered.alias, fixture.alias);
    assert_eq!(recovered.pending_bindings.len(), 1);
    assert_eq!(recovered.pending_bindings[0].intent, pending.intent);
    let recovered_keystore = BsvHomeKeystore::new(directory.to_string_lossy().into_owned()).unwrap();
    assert_eq!(
        recovered_keystore
            .paymail_identity_public_key(keystore_id_for_wallet(recovered.wallet.id.id()), &address, decode_password(TEST_PASSWORD))
            .unwrap(),
        identity_public_key
    );
    fs::remove_dir_all(directory).unwrap();
}

fn failure(error: BsvHomePaymailError) -> String {
    match error {
        BsvHomePaymailError::Api { code, .. } if code.len() <= 64 && code.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_') => {
            format!("Paymail live API rejected the operation: {code}")
        }
        BsvHomePaymailError::Api { .. } => "Paymail live API returned an unexpected error code".into(),
        BsvHomePaymailError::Auth { error } => authentication_failure(error),
        BsvHomePaymailError::InvalidAlias => "Paymail live alias rejected".into(),
        BsvHomePaymailError::InvalidDomain => "Paymail live domain rejected".into(),
        BsvHomePaymailError::InvalidHandle => "Paymail live handle rejected".into(),
        BsvHomePaymailError::InvalidConfiguration => "Paymail live configuration failed the trusted service contract".into(),
        BsvHomePaymailError::InvalidChallenge => "Paymail live challenge failed the documented signing contract".into(),
        BsvHomePaymailError::InvalidResponse => "Paymail live response failed the documented contract".into(),
        BsvHomePaymailError::IdentityMismatch => "Paymail live identity changed".into(),
        BsvHomePaymailError::BindingChanged => "Paymail live binding changed unexpectedly".into(),
        BsvHomePaymailError::PendingVerification => "Paymail live operation pending verification".into(),
        BsvHomePaymailError::UnsupportedCapability => "Paymail live payment destination capability unsupported".into(),
        BsvHomePaymailError::UnsupportedOutput => "Paymail live payment script unsupported".into(),
        BsvHomePaymailError::InvalidAmount => "Paymail live amount rejected".into(),
        BsvHomePaymailError::SenderRequired => "Paymail live sender registration required".into(),
    }
}

async fn change_binding(service: &BsvHomePaymailService, alias: &str, domain: &str, active: bool) -> Result<BsvHomePaymailSnapshot, String> {
    let snapshot = service.load().await.map_err(failure)?;
    let intent = service.binding_intent(snapshot, alias.into(), domain.into(), active).await.map_err(failure)?;
    let mutation = service.submit_binding(intent).await.map_err(failure)?;
    match mutation.status {
        BsvHomePaymailMutationStatus::Confirmed | BsvHomePaymailMutationStatus::Reconciled => {}
        BsvHomePaymailMutationStatus::PendingVerification => return Err("Paymail live mutation result is pending verification".into()),
        BsvHomePaymailMutationStatus::Changed => return Err("Paymail live mutation was superseded".into()),
    }
    service.load().await.map_err(failure)
}

async fn public_request(provider: &CurlPortalProvider, path: String, method: AlienHttpMethod, body: Option<Vec<u8>>) -> Result<Arc<AlienResponse>, String> {
    provider
        .request(AlienTarget {
            url: format!("{PUBLIC_ORIGIN}{path}"),
            method,
            headers: Some(HashMap::from([
                ("Content-Type".into(), "application/json".into()),
                ("Cache-Control".into(), "no-store".into()),
            ])),
            body,
        })
        .await
        .map_err(|_| "Paymail public transport failed".into())
}

async fn verify_public_identity(provider: &CurlPortalProvider, handle: &str, public_key: &str) -> Result<(), String> {
    let response = public_request(provider, format!("/api/paymail/id/{handle}"), AlienHttpMethod::Get, None)
        .await?
        .to_rpc_response();
    if response.status != Some(200) {
        return Err("Paymail public identity lookup did not return HTTP 200".into());
    }
    let identity: PublicIdentity = serde_json::from_slice(&response.data).map_err(|_| "Paymail public identity response invalid")?;
    if identity.bsvalias != "1.0" || identity.handle != handle || identity.pubkey != public_key {
        return Err("Paymail public identity differs from the registered binding".into());
    }
    Ok(())
}

async fn verify_disabled_public_lookup(provider: &CurlPortalProvider, handle: &str) -> Result<(), String> {
    for (path, method, body) in [
        (format!("/api/paymail/id/{handle}"), AlienHttpMethod::Get, None),
        (format!("/api/paymail/address/{handle}"), AlienHttpMethod::Post, Some(b"{}".to_vec())),
    ] {
        let response = public_request(provider, path, method, body).await?.to_rpc_response();
        if response.status != Some(404) {
            return Err("Disabled Paymail public lookup did not return HTTP 404".into());
        }
        sleep(Duration::from_secs(1));
    }
    Ok(())
}

#[test]
#[ignore = "requires PAYMAIL_LIVE_TESTS=1; registers, disables and restores only an unfunded temporary Paymail identity on the approved test deployment"]
fn test_live_paymail_register_lookup_disable_restore_and_cleanup() -> Result<(), String> {
    if env::var("PAYMAIL_LIVE_TESTS").as_deref() != Ok("1") {
        return Err("Set PAYMAIL_LIVE_TESTS=1 to authorize the temporary Paymail lifecycle test".into());
    }
    block_on(async {
        let (directory, recovered) = match env::var_os("PAYMAIL_LIVE_FIXTURE_DIR") {
            Some(path) => {
                let (directory, fixture) = open_fixture(PathBuf::from(path))?;
                (directory, Some(fixture))
            }
            None => (FixtureDirectory::Temporary(temporary_directory()?), None),
        };
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).map_err(|_| "Temporary Paymail keystore unavailable")?;
        let wallet = match &recovered {
            Some(fixture) => fixture.wallet.clone(),
            None => {
                let stored = keystore
                    .create_store(
                        BsvHomeImportType::SinglePhrase {
                            words: Mnemonic::generate(12).map_err(|_| "Temporary Paymail phrase generation failed")?,
                            chain: Chain::BitcoinSv,
                        },
                        decode_password(TEST_PASSWORD),
                    )
                    .map_err(|_| "Temporary Paymail wallet creation failed")?;
                Wallet {
                    id: WalletId::from_id(&stored.wallet_id).ok_or("Temporary Paymail wallet id invalid")?,
                    external_id: None,
                    name: FIXTURE_NAME.into(),
                    index: 0,
                    wallet_type: stored.wallet_type,
                    accounts: stored.accounts.into_iter().map(wallet_rules::account).collect(),
                    is_pinned: false,
                    image_url: None,
                    source: WalletSource::Create,
                }
            }
        };
        let public_wallet = wallet.clone();
        let portal_store = Arc::new(TemporarySecureStore::default());
        if let Some(fixture) = &recovered {
            for pending in &fixture.pending_bindings {
                if pending.intent.wallet_id != wallet.id.id() || pending.intent.public_origin != PUBLIC_ORIGIN {
                    return Err("Retained Paymail pending state does not match the fixed test identity".into());
                }
                portal_store
                    .set(
                        format!("paymail_pending|{BASE_URL}|{}|{}", pending.intent.wallet_id, pending.intent.address),
                        serde_json::to_string(pending).map_err(|_| "Retained Paymail pending state invalid")?,
                    )
                    .map_err(|_| "Retained Paymail pending state restoration failed")?;
            }
        }
        let wallets = Arc::new(MemoryWalletStore::default());
        wallets.wallets.lock().unwrap().push(wallet.clone());
        let wallet_id = wallet.id.id();
        let wallet_store = Arc::new(MemoryWalletSessionStore::default());
        *wallet_store.current.lock().unwrap() = Some(wallet.id);
        let session = Arc::new(BsvHomeWalletSessionService::new(wallet_store, wallets));
        let provider = Arc::new(CurlPortalProvider::default());
        let auth = Arc::new(
            BsvHomePortalAuthService::new(
                provider.clone(),
                keystore.clone(),
                Arc::new(MemoryKeystorePassword::default()),
                session,
                Arc::new(BsvHomeDeviceKeyService::new(Arc::new(TemporarySecureStore::default()))),
                portal_store.clone(),
                BASE_URL.into(),
                "zh-CN".into(),
                "4".into(),
            )
            .map_err(|_| "Temporary Paymail authentication setup failed")?,
        );
        let service = BsvHomePaymailService::new(auth, vec![PUBLIC_ORIGIN.into()], wallet_id).map_err(failure)?;
        let destinations = BsvHomePaymailDestinationService::new(provider.clone());
        if env::var("PAYMAIL_LIVE_READ_ONLY").as_deref() == Ok("1") {
            let outcome = service.load().await.map_err(failure);
            println!(
                "Paymail read-only schema: {}",
                serde_json::to_string(&provider.schema_metadata()).map_err(|_| "Paymail schema metadata encoding failed")?
            );
            return outcome.map(|_| ());
        }
        let initial = service.load().await.map_err(failure)?;
        let cleanup_only = recovered.is_some();
        let mut fixture = match recovered {
            Some(fixture) => fixture,
            None => {
                if !initial.bindings.is_empty() {
                    return Err("Fresh temporary Paymail wallet already has a binding".into());
                }
                let domain = initial.registration_domains.first().cloned().ok_or("Paymail test deployment has no registration domain")?;
                let alias = format!("{FIXTURE_ALIAS_PREFIX}{}", Uuid::new_v4().simple());
                let identity_public_key = keystore
                    .paymail_identity_public_key(keystore_id_for_wallet(initial.wallet_id.clone()), &initial.address, decode_password(TEST_PASSWORD))
                    .map_err(|_| "Temporary Paymail identity derivation failed")?;
                PendingPaymailFixture {
                    wallet: public_wallet,
                    alias: alias.clone(),
                    domain: domain.clone(),
                    pending_bindings: vec![PendingBinding {
                        intent: BsvHomePaymailBindingIntent {
                            wallet_id: initial.wallet_id.clone(),
                            address: initial.address.clone(),
                            public_origin: initial.public_origin.clone(),
                            handle: format!("{alias}@{domain}"),
                            active: true,
                            previous_version: 0,
                        },
                        identity_public_key,
                    }],
                    schema_metadata: provider.schema_metadata(),
                }
            }
        };
        save_fixture(directory.path(), &fixture)?;
        let directory = directory.retain();
        let alias = fixture.alias.clone();
        let domain = fixture.domain.clone();
        let handle = format!("{alias}@{domain}");
        let outcome: Result<(), String> = async {
            if cleanup_only {
                return Ok(());
            }
            let registered = change_binding(&service, &alias, &domain, true).await?;
            let binding = registered
                .bindings
                .iter()
                .find(|binding| binding.handle == handle)
                .ok_or("Registered Paymail binding missing from /me")?;
            if !binding.active || binding.version != 1 || binding.address != initial.address || registered.active_handle.as_deref() != Some(handle.as_str()) {
                return Err("Registered Paymail state failed the expected fields and version".into());
            }
            let public_key = binding.identity_public_key.clone();
            sleep(Duration::from_secs(1));
            verify_public_identity(&provider, &handle, &public_key).await?;
            sleep(Duration::from_secs(1));
            let destination = destinations.destination(handle.clone(), Some(30_000_000)).await.map_err(failure)?;
            if destination.handle != handle || destination.address != initial.address {
                return Err("Paymail resolved receiving address does not match the registered wallet".into());
            }
            let disabled = change_binding(&service, &alias, &domain, false).await?;
            let binding = disabled
                .bindings
                .iter()
                .find(|binding| binding.handle == handle)
                .ok_or("Disabled Paymail binding disappeared from /me")?;
            if binding.active || binding.version != 2 || binding.identity_public_key != public_key || disabled.active_handle.is_some() {
                return Err("Disabled Paymail state failed the retained identity and version contract".into());
            }
            sleep(Duration::from_secs(1));
            verify_disabled_public_lookup(&provider, &handle).await?;
            let restored = change_binding(&service, &alias, &domain, true).await?;
            let binding = restored
                .bindings
                .iter()
                .find(|binding| binding.handle == handle)
                .ok_or("Restored Paymail binding missing from /me")?;
            if !binding.active || binding.version != 3 || binding.identity_public_key != public_key || restored.active_handle.as_deref() != Some(handle.as_str()) {
                return Err("Restored Paymail state failed the stable identity and version contract".into());
            }
            sleep(Duration::from_secs(1));
            verify_public_identity(&provider, &handle, &public_key).await?;
            sleep(Duration::from_secs(1));
            let destination = destinations.destination(handle.clone(), None).await.map_err(failure)?;
            if destination.address != initial.address {
                return Err("Restored Paymail did not resolve to its receiving address".into());
            }
            Ok(())
        }
        .await;
        let cleanup = cleanup_binding(&service, &provider, &alias, &domain, &initial.address).await;
        fixture.schema_metadata = provider.schema_metadata();
        println!(
            "Paymail live schema: {}",
            serde_json::to_string(&fixture.schema_metadata).map_err(|_| "Paymail schema metadata encoding failed")?
        );
        if cleanup.is_ok() {
            fs::remove_dir_all(&directory).map_err(|_| "Verified temporary Paymail fixture removal failed")?;
        } else {
            let pending = portal_store.pending_bindings()?;
            if !pending.is_empty() {
                fixture.pending_bindings = pending;
            }
            let saved = save_fixture(&directory, &fixture);
            println!("Paymail cleanup pending; encrypted fixture retained at {} for {}", directory.display(), handle);
            saved?;
        }
        match (outcome, cleanup) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(format!("Temporary Paymail cleanup failed: {error}")),
            (Err(error), Err(cleanup)) => Err(format!("{error}; temporary Paymail cleanup failed: {cleanup}")),
        }
    })
}
