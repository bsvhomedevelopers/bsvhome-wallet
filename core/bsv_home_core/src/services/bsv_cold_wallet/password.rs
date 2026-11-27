use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use base64::{Engine, engine::general_purpose::STANDARD};
use bsv_home_bsv::BsvAddress;
use bsv_home_crypto::{compare::constant_time_eq, pbkdf::pbkdf2_hmac_sha256, random};
use bsv_home_hash::sha2::sha256;
use chrono::Utc;
use futures::lock::Mutex as AsyncMutex;
use primitives::{Address, Chain, Wallet, WalletId, WalletType};
use serde::{Deserialize, Deserializer, Serialize};
use zeroize::{Zeroize, Zeroizing};

use super::{BsvHomeBsvColdWalletActionScope, BsvHomeOfflineSigningPasswordDomain};
use crate::services::portal::{BsvHomeTransferPasswordError as Error, BsvHomeTransferPasswordSnapshot as Snapshot};
use crate::services::preferences::BsvHomeSecureStore;
use crate::services::transfer_password::{validate_current_password, validate_new_password, validate_verification_password};
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[cfg(test)]
#[path = "password_testkit.rs"]
pub(crate) mod testkit;
#[cfg(test)]
#[path = "password_tests.rs"]
mod tests;

const ITERATIONS: u32 = 600_000;
const SALT_BYTES: usize = 16;
const VERIFIER_BYTES: usize = 32;
const MAX_RECORD_BYTES: usize = 4096;
const MAX_FAILURES: u8 = 5;
const WINDOW_MS: i64 = 300_000;
const KDF: &str = "PBKDF2-HMAC-SHA256";

type Gate = Arc<AsyncMutex<()>>;
type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;
static GATES: OnceLock<Mutex<HashMap<String, Weak<AsyncMutex<()>>>>> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    version: u32,
    domain: String,
    wallet_id: WalletId,
    address: String,
    kdf: String,
    iterations: u32,
    salt: String,
    verifier: String,
    failure_count: u8,
    #[serde(deserialize_with = "deserialize_last_failure")]
    last_failure_ms: Option<i64>,
}

impl Drop for Record {
    fn drop(&mut self) {
        self.salt.zeroize();
        self.verifier.zeroize();
    }
}

impl Record {
    fn new(service: &BsvHomeOfflineSigningPasswordService, password: &str) -> Result<Self, Error> {
        let salt = Zeroizing::new(random::bytes::<SALT_BYTES>().map_err(unavailable)?);
        let verifier = pbkdf2_hmac_sha256(password.as_bytes(), salt.as_slice(), ITERATIONS as usize, VERIFIER_BYTES).map_err(unavailable)?;
        Ok(Self {
            version: 1,
            domain: domain_tag(service.domain).into(),
            wallet_id: service.wallet.id.clone(),
            address: service.address.clone(),
            kdf: KDF.into(),
            iterations: ITERATIONS,
            salt: STANDARD.encode(salt.as_slice()),
            verifier: STANDARD.encode(verifier.as_slice()),
            failure_count: 0,
            last_failure_ms: None,
        })
    }

    fn validate(&self, service: &BsvHomeOfflineSigningPasswordService) -> Result<(), Error> {
        if self.version != 1
            || self.domain != domain_tag(service.domain)
            || self.wallet_id != service.wallet.id
            || self.address != service.address
            || self.kdf != KDF
            || self.iterations != ITERATIONS
            || self.failure_count > MAX_FAILURES
        {
            return Err(Error::Unavailable);
        }
        match (self.failure_count, self.last_failure_ms) {
            (0, None) => {}
            (1..=MAX_FAILURES, Some(time)) if time >= 0 => {}
            _ => return Err(Error::Unavailable),
        }
        decode_fixed(&self.salt, SALT_BYTES)?;
        decode_fixed(&self.verifier, VERIFIER_BYTES)?;
        Ok(())
    }

    fn stamp(&self) -> Result<[u8; 32], Error> {
        let encoded =
            Zeroizing::new(serde_json::to_vec(&(&self.domain, &self.wallet_id, &self.address, &self.kdf, self.iterations, &self.salt, &self.verifier)).map_err(unavailable)?);
        Ok(sha256(encoded.as_slice()))
    }

    fn failures(&self, now: i64) -> u8 {
        match self.last_failure_ms {
            Some(last) if now < last || now - last <= WINDOW_MS => self.failure_count,
            Some(_) | None => 0,
        }
    }

    fn verify(&self, password: &str) -> Result<bool, Error> {
        let salt = decode_fixed(&self.salt, SALT_BYTES)?;
        let expected = decode_fixed(&self.verifier, VERIFIER_BYTES)?;
        let actual = pbkdf2_hmac_sha256(password.as_bytes(), salt.as_slice(), ITERATIONS as usize, VERIFIER_BYTES).map_err(unavailable)?;
        Ok(constant_time_eq(actual.as_slice(), expected.as_slice()))
    }
}

struct CachedSnapshot {
    snapshot: Snapshot,
    credential: Option<[u8; 32]>,
}

enum Binding {
    Management(Arc<BsvHomeWalletSessionService>),
    Scope(Arc<BsvHomeBsvColdWalletActionScope>),
}

#[derive(uniffi::Object)]
pub struct BsvHomeOfflineSigningPasswordService {
    store: Arc<dyn BsvHomeSecureStore>,
    wallet: Wallet,
    address: String,
    domain: BsvHomeOfflineSigningPasswordDomain,
    key: String,
    binding: Binding,
    cancelled: AtomicBool,
    snapshot: Mutex<Option<CachedSnapshot>>,
    clock: Clock,
}

#[uniffi::export]
impl BsvHomeOfflineSigningPasswordService {
    #[uniffi::constructor]
    pub fn new(session: Arc<BsvHomeWalletSessionService>, store: Arc<dyn BsvHomeSecureStore>, wallet: Wallet, domain: BsvHomeOfflineSigningPasswordDomain) -> Result<Self, Error> {
        Self::build(store, wallet, domain, Binding::Management(session))
    }

    #[uniffi::constructor]
    pub fn for_scope(store: Arc<dyn BsvHomeSecureStore>, scope: Arc<BsvHomeBsvColdWalletActionScope>) -> Result<Self, Error> {
        let wallet = scope.password_wallet().map_err(Error::from)?;
        let domain = scope.password_domain();
        Self::build(store, wallet, domain, Binding::Scope(scope))
    }

    pub async fn status(&self) -> Result<Snapshot, Error> {
        self.check()?;
        let gate = gate(&self.key)?;
        let _guard = gate.lock().await;
        self.check()?;
        *self.snapshot.lock().map_err(unavailable)? = None;
        self.validate_wallet().await?;
        let record = self.read()?;
        self.validate_wallet().await?;
        self.cache(record.as_ref())
    }

    pub async fn set(&self, snapshot: Snapshot, current_password: Option<String>, new_password: String, confirmation: String) -> Result<Snapshot, Error> {
        let current_password = current_password.map(Zeroizing::new);
        let new_password = Zeroizing::new(new_password);
        let confirmation = Zeroizing::new(confirmation);
        self.require_snapshot(&snapshot)?;
        validate_new_password(&new_password, &confirmation)?;
        validate_current_password(snapshot.configured, current_password.as_deref().map(String::as_str))?;
        let gate = gate(&self.key)?;
        let _guard = gate.lock().await;
        self.require_snapshot(&snapshot)?;
        self.validate_wallet().await?;
        let previous = self.read()?;
        self.require_credential(&snapshot, previous.as_ref())?;
        if let Some(mut record) = previous {
            self.verify_record(&mut record, current_password.as_deref().map(String::as_str).ok_or(Error::CurrentPasswordRequired)?)
                .await?;
        }
        self.validate_wallet().await?;
        let record = Record::new(self, &new_password)?;
        self.validate_wallet().await?;
        self.write(&record)?;
        self.validate_wallet().await?;
        let updated = self.cache(Some(&record))?;
        self.authorize().await?;
        Ok(updated)
    }

    pub async fn verify(&self, snapshot: Snapshot, password: String) -> Result<(), Error> {
        let password = Zeroizing::new(password);
        self.require_snapshot(&snapshot)?;
        if !snapshot.configured {
            return Err(Error::NotConfigured);
        }
        validate_verification_password(&password)?;
        let gate = gate(&self.key)?;
        let _guard = gate.lock().await;
        self.require_snapshot(&snapshot)?;
        self.validate_wallet().await?;
        let mut record = self.read()?.ok_or(Error::InvalidSnapshot)?;
        self.require_credential(&snapshot, Some(&record))?;
        self.verify_record(&mut record, &password).await?;
        self.authorize().await
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut snapshot) = self.snapshot.lock() {
            *snapshot = None;
        }
        match &self.binding {
            Binding::Management(_) => {}
            Binding::Scope(scope) => scope.cancel(),
        }
    }
}

impl BsvHomeOfflineSigningPasswordService {
    fn build(store: Arc<dyn BsvHomeSecureStore>, wallet: Wallet, domain: BsvHomeOfflineSigningPasswordDomain, binding: Binding) -> Result<Self, Error> {
        let address = signing_address(&wallet)?.to_string();
        Ok(Self {
            key: record_key(domain, &wallet.id),
            store,
            wallet,
            address,
            domain,
            binding,
            cancelled: AtomicBool::new(false),
            snapshot: Mutex::new(None),
            clock: Arc::new(|| Utc::now().timestamp_millis()),
        })
    }

    fn check(&self) -> Result<(), Error> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(Error::Cancelled);
        }
        match &self.binding {
            Binding::Management(_) => Ok(()),
            Binding::Scope(scope) => scope.check().map_err(Error::from),
        }
    }

    async fn validate_wallet(&self) -> Result<(), Error> {
        self.check()?;
        let current = match &self.binding {
            Binding::Management(session) => session.current_wallet().await.map_err(Error::from)?,
            Binding::Scope(scope) => {
                scope.validate().await.map_err(Error::from)?;
                scope.password_wallet().map_err(Error::from)?
            }
        };
        self.check()?;
        if current.id != self.wallet.id || current.wallet_type != self.wallet.wallet_type || current.account(Chain::BitcoinSv) != self.wallet.account(Chain::BitcoinSv) {
            return Err(Error::WalletChanged);
        }
        signing_address(&current)?;
        Ok(())
    }

    fn read(&self) -> Result<Option<Record>, Error> {
        self.check()?;
        let Some(json) = self.store.get(self.key.clone()).map_err(unavailable)?.map(Zeroizing::new) else {
            return Ok(None);
        };
        if json.len() > MAX_RECORD_BYTES {
            return Err(Error::Unavailable);
        }
        let record: Record = serde_json::from_str(&json).map_err(unavailable)?;
        record.validate(self)?;
        Ok(Some(record))
    }

    fn write(&self, record: &Record) -> Result<(), Error> {
        self.check()?;
        let value = serde_json::to_string(record).map_err(unavailable)?;
        self.store.set(self.key.clone(), value).map_err(unavailable)?;
        self.check()
    }

    fn cache(&self, record: Option<&Record>) -> Result<Snapshot, Error> {
        self.check()?;
        let generation = GENERATION
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| value.checked_add(1))
            .map_err(unavailable)?;
        let snapshot = Snapshot {
            wallet_id: self.wallet.id.id(),
            address: self.address.clone(),
            configured: record.is_some(),
            generation,
        };
        let credential = record.map(Record::stamp).transpose()?;
        *self.snapshot.lock().map_err(unavailable)? = Some(CachedSnapshot {
            snapshot: snapshot.clone(),
            credential,
        });
        self.check()?;
        Ok(snapshot)
    }

    fn require_snapshot(&self, expected: &Snapshot) -> Result<(), Error> {
        self.check()?;
        let state = self.snapshot.lock().map_err(unavailable)?;
        if state.as_ref().is_none_or(|cached| &cached.snapshot != expected) {
            return Err(Error::InvalidSnapshot);
        }
        Ok(())
    }

    fn require_credential(&self, expected: &Snapshot, record: Option<&Record>) -> Result<(), Error> {
        self.require_snapshot(expected)?;
        let actual = record.map(Record::stamp).transpose()?;
        let state = self.snapshot.lock().map_err(unavailable)?;
        if state.as_ref().is_none_or(|cached| cached.credential != actual) {
            return Err(Error::InvalidSnapshot);
        }
        Ok(())
    }

    fn now(&self) -> Result<i64, Error> {
        let now = (self.clock)();
        if now < 0 {
            return Err(Error::Unavailable);
        }
        Ok(now)
    }

    async fn verify_record(&self, record: &mut Record, password: &str) -> Result<(), Error> {
        let now = self.now()?;
        let failures = record.failures(now);
        if failures >= MAX_FAILURES {
            return Err(Error::RateLimited);
        }
        let valid = record.verify(password)?;
        self.validate_wallet().await?;
        self.check()?;
        if valid {
            if record.failure_count != 0 {
                record.failure_count = 0;
                record.last_failure_ms = None;
                self.write(record)?;
            }
            return Ok(());
        }
        record.failure_count = failures + 1;
        let now = self.now()?;
        record.last_failure_ms = Some(match record.last_failure_ms {
            Some(previous) => now.max(previous),
            None => now,
        });
        self.write(record)?;
        Err(Error::IncorrectPassword)
    }

    async fn authorize(&self) -> Result<(), Error> {
        self.validate_wallet().await?;
        match &self.binding {
            Binding::Management(_) => Ok(()),
            Binding::Scope(scope) => {
                scope.grant_password().map_err(Error::from)?;
                self.check()
            }
        }
    }
}

fn signing_address(wallet: &Wallet) -> Result<&str, Error> {
    match wallet.wallet_type {
        WalletType::View => return Err(Error::WatchOnly),
        WalletType::Single | WalletType::Multicoin | WalletType::PrivateKey => {}
    }
    let account = wallet.account(Chain::BitcoinSv).ok_or(Error::MissingBsvAccount)?;
    BsvAddress::from_str(&account.address).map_err(|_| Error::MissingBsvAccount)?;
    Ok(&account.address)
}

fn domain_tag(domain: BsvHomeOfflineSigningPasswordDomain) -> &'static str {
    match domain {
        BsvHomeOfflineSigningPasswordDomain::LegacyCold => "legacyCold",
        BsvHomeOfflineSigningPasswordDomain::UnifiedCold => "unifiedCold",
    }
}

fn record_key(domain: BsvHomeOfflineSigningPasswordDomain, wallet: &WalletId) -> String {
    format!("offlinesigningpassword:v1:{}:{}", domain_tag(domain), hex::encode(sha256(wallet.id().as_bytes())))
}

fn gate(key: &str) -> Result<Gate, Error> {
    let mut gates = GATES.get_or_init(|| Mutex::new(HashMap::new())).lock().map_err(unavailable)?;
    gates.retain(|_, weak| weak.strong_count() > 0);
    if let Some(gate) = gates.get(key).and_then(Weak::upgrade) {
        return Ok(gate);
    }
    let gate = Arc::new(AsyncMutex::new(()));
    gates.insert(key.into(), Arc::downgrade(&gate));
    Ok(gate)
}

fn decode_fixed(value: &str, length: usize) -> Result<Zeroizing<Vec<u8>>, Error> {
    let bytes = Zeroizing::new(STANDARD.decode(value).map_err(unavailable)?);
    if bytes.len() != length || STANDARD.encode(bytes.as_slice()) != value {
        return Err(Error::Unavailable);
    }
    Ok(bytes)
}

fn unavailable<T>(_: T) -> Error {
    Error::Unavailable
}

fn deserialize_last_failure<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    Option::<i64>::deserialize(deserializer)
}
