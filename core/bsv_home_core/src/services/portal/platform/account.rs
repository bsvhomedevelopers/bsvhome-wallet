use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use bsv_home_hash::sha2::sha256;
use futures::{lock::Mutex as AsyncMutex, try_join};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::alien::AlienProvider;
use crate::services::preferences::BsvHomeSecureStore;

use super::client::PlatformClient;
use super::rules;
use super::{
    BsvHomePlatformAccountError, BsvHomePlatformAccountSnapshot, BsvHomePlatformAuthenticator, BsvHomePlatformDevice, BsvHomePlatformEmailChallenge, BsvHomePlatformPasskey,
    BsvHomePlatformPasskeyOperation, BsvHomePlatformProfile, BsvHomePlatformSession,
};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PlatformSessionLease {
    token: String,
    pub user_id: String,
    generation: u64,
}

#[derive(Serialize, Deserialize)]
pub(super) struct StoredSession {
    pub environment: String,
    pub token: String,
    pub session: BsvHomePlatformSession,
}

pub(super) struct PendingOperation {
    pub lease: PlatformSessionLease,
    pub value: Value,
}

#[derive(Default)]
pub(super) struct PlatformState {
    pub generation: u64,
    pub email: HashMap<String, (u64, bool)>,
    pub operations: HashMap<String, PendingOperation>,
}

#[derive(uniffi::Object)]
pub struct BsvHomePlatformAccountService {
    pub(super) client: PlatformClient,
    pub(super) store: Arc<dyn BsvHomeSecureStore>,
    pub(super) authenticator: Arc<dyn BsvHomePlatformAuthenticator>,
    device_id: String,
    namespace: String,
    state: Mutex<PlatformState>,
    authentication: AsyncMutex<()>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionResponse {
    token: String,
    #[serde(flatten)]
    session: BsvHomePlatformSession,
}

#[uniffi::export]
impl BsvHomePlatformAccountService {
    #[uniffi::constructor]
    pub fn new(
        provider: Arc<dyn AlienProvider>,
        store: Arc<dyn BsvHomeSecureStore>,
        authenticator: Arc<dyn BsvHomePlatformAuthenticator>,
        base_url: String,
        language: String,
        device_id: String,
    ) -> Result<Self, BsvHomePlatformAccountError> {
        if device_id.is_empty() || device_id.len() > 128 || !device_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') {
            return Err(BsvHomePlatformAccountError::InvalidConfiguration);
        }
        let client = PlatformClient::new(provider, base_url, language)?;
        let namespace = format!("platform.{}", hex::encode(sha256(client.base_url.as_bytes())));
        Ok(Self {
            client,
            store,
            authenticator,
            device_id,
            namespace,
            state: Mutex::new(PlatformState::default()),
            authentication: AsyncMutex::new(()),
        })
    }

    pub fn session(&self) -> Result<Option<BsvHomePlatformSession>, BsvHomePlatformAccountError> {
        let _state = self.state()?;
        Ok(self.stored_session()?.map(|session| session.session))
    }

    pub async fn snapshot(&self) -> Result<Option<BsvHomePlatformAccountSnapshot>, BsvHomePlatformAccountError> {
        let generation = self.generation()?;
        let Some(session) = self.session()? else { return Ok(None) };
        let lease = self.session_lease()?;
        if lease.generation != generation || session.user_id != lease.user_id {
            return Err(BsvHomePlatformAccountError::SessionChanged);
        }
        let (profile, passkeys, devices) = try_join!(self.profile(), self.passkeys(), self.devices())?;
        self.require_lease(&lease)?;
        Ok(Some(BsvHomePlatformAccountSnapshot {
            session,
            profile,
            can_revoke_passkey: passkeys.len() > 1,
            passkeys,
            devices,
        }))
    }

    pub async fn email_start(&self, email: String) -> Result<BsvHomePlatformEmailChallenge, BsvHomePlatformAccountError> {
        let email = rules::email(&email)?;
        let generation = self.generation()?;
        let mut body = self.device();
        body["email"] = email.into();
        let challenge: BsvHomePlatformEmailChallenge = serde_json::from_value(self.client.request("portal/identity/email/start", body, None).await?)?;
        rules::email_challenge(&challenge)?;
        if challenge.retry_after_seconds.is_none() {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        let mut state = self.state()?;
        self.require_generation(&state, generation)?;
        state.email.clear();
        state.email.insert(challenge.id.clone(), (generation, false));
        Ok(challenge)
    }

    pub async fn email_finish(&self, challenge_id: String, code: String) -> Result<BsvHomePlatformSession, BsvHomePlatformAccountError> {
        self.email_finish_kind(challenge_id, code, false).await
    }

    pub async fn passkey_login(&self) -> Result<BsvHomePlatformSession, BsvHomePlatformAccountError> {
        let generation = self.generation()?;
        let start = self.client.request("portal/identity/passkey/login/start", self.device(), None).await?;
        let finish = self.passkey_response(BsvHomePlatformPasskeyOperation::Login, start).await?;
        if self.generation()? != generation {
            return Err(BsvHomePlatformAccountError::SessionChanged);
        }
        let result = self.client.request("portal/identity/passkey/login/finish", finish, None).await?;
        let session: SessionResponse = serde_json::from_value(result)?;
        if session.session.auth_method != super::BsvHomePlatformAuthMethod::PasskeyUv {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        self.save_session(session, generation)
    }

    pub async fn register_passkey(&self, label: String) -> Result<(), BsvHomePlatformAccountError> {
        rules::label(&label)?;
        let lease = self.session_lease()?;
        let existing = self.passkeys().await?;
        let mut body = self.device();
        body["label"] = label.into();
        if !existing.is_empty() {
            body["grant"] = self.authorize(&hex::encode(sha256(b"PASSKEY_REGISTER"))).await?.into();
        }
        self.require_lease(&lease)?;
        let start = self.authenticated_json("portal/identity/passkey/register/start", body).await?;
        let finish = self.passkey_response(BsvHomePlatformPasskeyOperation::Register, start).await?;
        self.require_lease(&lease)?;
        self.authenticated_json("portal/identity/passkey/register/finish", finish).await?;
        Ok(())
    }

    pub async fn passkeys(&self) -> Result<Vec<BsvHomePlatformPasskey>, BsvHomePlatformAccountError> {
        let keys: Vec<BsvHomePlatformPasskey> = serde_json::from_value(self.authenticated_json("portal/identity/passkey/list", json!({})).await?)?;
        for key in &keys {
            rules::identifier(&key.id).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
        }
        Ok(keys)
    }

    pub async fn profile(&self) -> Result<BsvHomePlatformProfile, BsvHomePlatformAccountError> {
        let lease = self.session_lease()?;
        let profile: BsvHomePlatformProfile = serde_json::from_value(self.authenticated_json("portal/account/profile", json!({})).await?)?;
        if profile.user_id != lease.user_id {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(profile)
    }

    pub async fn update_profile(&self, nickname: String, language: String, timezone: String) -> Result<BsvHomePlatformProfile, BsvHomePlatformAccountError> {
        rules::label(&nickname)?;
        if language.is_empty() || language.len() > 35 || timezone.is_empty() || timezone.len() > 80 || timezone.chars().any(char::is_control) {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        self.authenticated_json("portal/account/profile/update", json!({ "nickname": nickname, "language": language, "timezone": timezone }))
            .await?;
        self.profile().await
    }

    pub async fn devices(&self) -> Result<Vec<BsvHomePlatformDevice>, BsvHomePlatformAccountError> {
        let lease = self.session_lease()?;
        let mut devices = Vec::new();
        for current in 1..=100 {
            self.require_lease(&lease)?;
            let result = self
                .authenticated_json("portal/account/devices/page", json!({ "current": current, "pageSize": 100 }))
                .await?;
            self.require_lease(&lease)?;
            let rows: Vec<BsvHomePlatformDevice> = serde_json::from_value(result.get("rows").cloned().ok_or(BsvHomePlatformAccountError::InvalidResponse)?)?;
            let total = result.get("total").and_then(Value::as_u64).ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
            if rows
                .iter()
                .any(|row| rules::identifier(&row.id).is_err() || devices.iter().any(|previous: &BsvHomePlatformDevice| previous.id == row.id))
            {
                return Err(BsvHomePlatformAccountError::InvalidResponse);
            }
            let empty = rows.is_empty();
            devices.extend(rows);
            if devices.len() as u64 >= total {
                return Ok(devices);
            }
            if empty {
                return Err(BsvHomePlatformAccountError::InvalidResponse);
            }
        }
        Err(BsvHomePlatformAccountError::InvalidResponse)
    }

    pub async fn revoke_device(&self, id: String, reason: String) -> Result<(), BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        if reason.trim().is_empty() || reason.chars().count() > 500 {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        self.authenticated_json("portal/account/devices/revoke-sessions", json!({ "id": id, "reason": reason }))
            .await?;
        Ok(())
    }

    pub async fn renew_session(&self) -> Result<BsvHomePlatformSession, BsvHomePlatformAccountError> {
        let lease = self.session_lease()?;
        let action = self.authenticated_json("portal/account/actions/challenge", json!({ "kind": "SESSION_RENEW" })).await?;
        let grant = self.authorize(rules::string(&action, "actionHash")?).await?;
        self.require_lease(&lease)?;
        let response = self.authenticated_json("portal/account/session/renew", json!({ "grant": grant })).await?;
        self.require_lease(&lease)?;
        let current = self.session()?.ok_or(BsvHomePlatformAccountError::LoginRequired)?;
        let replacement = SessionResponse {
            token: rules::string(&response, "token")?.into(),
            session: BsvHomePlatformSession {
                user_id: current.user_id,
                expires_at: rules::string(&response, "expiresAt")?.into(),
                auth_method: serde_json::from_value(response.get("authMethod").cloned().ok_or(BsvHomePlatformAccountError::InvalidResponse)?)?,
                wallet_authority: current.wallet_authority,
            },
        };
        self.save_session(replacement, lease.generation)
    }

    pub async fn revoke_passkey(&self, id: String) -> Result<(), BsvHomePlatformAccountError> {
        rules::identifier(&id)?;
        let lease = self.session_lease()?;
        let action = self
            .authenticated_json("portal/account/actions/challenge", json!({ "kind": "PASSKEY_REVOKE", "id": id }))
            .await?;
        let grant = self.authorize(rules::string(&action, "actionHash")?).await?;
        self.require_lease(&lease)?;
        self.authenticated_json("portal/account/passkeys/revoke", json!({ "id": id, "grant": grant })).await?;
        Ok(())
    }

    pub async fn logout(&self) -> Result<(), BsvHomePlatformAccountError> {
        let lease = {
            let state = self.state()?;
            let Some(stored) = self.stored_session()? else { return Ok(()) };
            PlatformSessionLease {
                token: stored.token,
                user_id: stored.session.user_id,
                generation: state.generation,
            }
        };
        let result = self.client.request("portal/wallet-auth/logout", json!({}), Some(&lease.token)).await;
        self.clear_for(&lease)?;
        result.map(|_| ())
    }

    pub fn cancel(&self) -> Result<(), BsvHomePlatformAccountError> {
        let mut state = self.state()?;
        state.generation = state.generation.wrapping_add(1);
        state.email.clear();
        state.operations.clear();
        Ok(())
    }
}

impl BsvHomePlatformAccountService {
    pub(crate) fn scope(&self) -> &str {
        &self.client.base_url
    }

    pub(crate) fn session_lease(&self) -> Result<PlatformSessionLease, BsvHomePlatformAccountError> {
        let state = self.state()?;
        let stored = self.stored_session()?.ok_or(BsvHomePlatformAccountError::LoginRequired)?;
        rules::unexpired(&stored.session.expires_at)?;
        Ok(PlatformSessionLease {
            token: stored.token,
            user_id: stored.session.user_id,
            generation: state.generation,
        })
    }

    pub(crate) fn require_lease(&self, lease: &PlatformSessionLease) -> Result<(), BsvHomePlatformAccountError> {
        if &self.session_lease()? != lease {
            return Err(BsvHomePlatformAccountError::SessionChanged);
        }
        Ok(())
    }

    pub(crate) async fn authenticated_json(&self, path: &str, body: Value) -> Result<Value, BsvHomePlatformAccountError> {
        let lease = self.session_lease()?;
        let result = self.client.request(path, body, Some(&lease.token)).await;
        if result == Err(BsvHomePlatformAccountError::LoginRequired) {
            self.clear_for(&lease)?;
            return result;
        }
        self.require_lease(&lease)?;
        result
    }

    pub(crate) async fn authenticated_get_json(&self, lease: &PlatformSessionLease, path: &str) -> Result<Value, BsvHomePlatformAccountError> {
        self.require_lease(lease)?;
        let result = self.client.authenticated_get(path, &lease.token).await;
        if result == Err(BsvHomePlatformAccountError::LoginRequired) {
            self.clear_for(lease)?;
            return result;
        }
        self.require_lease(lease)?;
        result
    }

    pub(crate) async fn authorize(&self, action_hash: &str) -> Result<String, BsvHomePlatformAccountError> {
        rules::action_hash(action_hash)?;
        let lease = self.session_lease()?;
        let mut body = self.device();
        body["actionHash"] = action_hash.into();
        let start = self.authenticated_json("portal/identity/passkey/step-up/start", body).await?;
        let finish = self.passkey_response(BsvHomePlatformPasskeyOperation::StepUp, start).await?;
        self.require_lease(&lease)?;
        let result = self.authenticated_json("portal/identity/passkey/step-up/finish", finish).await?;
        if rules::string(&result, "actionHash")? != action_hash {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        let grant = rules::string(&result, "grant")?;
        if grant.is_empty() || grant.len() > 512 || !grant.is_ascii() {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        Ok(grant.into())
    }

    pub(super) fn state(&self) -> Result<MutexGuard<'_, PlatformState>, BsvHomePlatformAccountError> {
        self.state.lock().map_err(|_| BsvHomePlatformAccountError::SecureStorageUnavailable)
    }

    pub(super) fn generation(&self) -> Result<u64, BsvHomePlatformAccountError> {
        Ok(self.state()?.generation)
    }

    fn require_generation(&self, state: &PlatformState, generation: u64) -> Result<(), BsvHomePlatformAccountError> {
        if state.generation != generation {
            return Err(BsvHomePlatformAccountError::SessionChanged);
        }
        Ok(())
    }

    pub(super) fn device(&self) -> Value {
        json!({ "deviceId": self.device_id, "deviceType": "ios" })
    }

    fn current_key(&self) -> String {
        format!("{}.current", self.namespace)
    }
    fn session_key(&self, user_id: &str) -> String {
        format!("{}.user.{user_id}.session", self.namespace)
    }

    fn stored_session(&self) -> Result<Option<StoredSession>, BsvHomePlatformAccountError> {
        let Some(user_id) = self.store.get(self.current_key())? else { return Ok(None) };
        if user_id.is_empty() || user_id.len() > 20 || !user_id.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(BsvHomePlatformAccountError::SecureStorageUnavailable);
        }
        let envelope = self.store.get(self.session_key(&user_id))?.ok_or(BsvHomePlatformAccountError::SecureStorageUnavailable)?;
        let stored: StoredSession = serde_json::from_str(&envelope)?;
        if stored.environment != self.client.base_url || stored.session.user_id != user_id {
            return Err(BsvHomePlatformAccountError::SecureStorageUnavailable);
        }
        rules::session(&stored.session, &stored.token)?;
        Ok(Some(stored))
    }

    fn save_session(&self, response: SessionResponse, generation: u64) -> Result<BsvHomePlatformSession, BsvHomePlatformAccountError> {
        rules::session(&response.session, &response.token)?;
        rules::unexpired(&response.session.expires_at)?;
        let mut state = self.state()?;
        self.require_generation(&state, generation)?;
        let stored = StoredSession {
            environment: self.client.base_url.clone(),
            token: response.token,
            session: response.session,
        };
        self.store.set(self.session_key(&stored.session.user_id), serde_json::to_string(&stored)?)?;
        self.store.set(self.current_key(), stored.session.user_id.clone())?;
        state.generation = state.generation.wrapping_add(1);
        state.email.clear();
        state.operations.clear();
        Ok(stored.session)
    }

    pub(super) fn clear_for(&self, lease: &PlatformSessionLease) -> Result<(), BsvHomePlatformAccountError> {
        let mut state = self.state()?;
        let Some(current) = self.stored_session()? else { return Ok(()) };
        if current.token == lease.token && current.session.user_id == lease.user_id {
            self.store.remove(self.current_key())?;
            self.store.remove(self.session_key(&lease.user_id))?;
            state.generation = state.generation.wrapping_add(1);
            state.email.clear();
            state.operations.clear();
        }
        Ok(())
    }

    pub(super) async fn email_finish_kind(&self, challenge_id: String, code: String, reopen: bool) -> Result<BsvHomePlatformSession, BsvHomePlatformAccountError> {
        rules::identifier(&challenge_id)?;
        rules::code(&code)?;
        let generation = self.generation()?;
        if self.state()?.email.get(&challenge_id) != Some(&(generation, reopen)) {
            return Err(BsvHomePlatformAccountError::SessionChanged);
        }
        let mut body = self.device();
        body["challengeId"] = challenge_id.into();
        body["code"] = code.into();
        let path = if reopen {
            "portal/account/closure/reopen/email/finish"
        } else {
            "portal/identity/email/finish"
        };
        let response: SessionResponse = serde_json::from_value(self.client.request(path, body, None).await?)?;
        let expected = if reopen {
            super::BsvHomePlatformAuthMethod::AccountReopenPasskeyEmail
        } else {
            super::BsvHomePlatformAuthMethod::EmailOtp
        };
        if response.session.auth_method != expected || (!reopen && response.session.wallet_authority) {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        self.save_session(response, generation)
    }

    pub(super) async fn passkey_response(&self, operation: BsvHomePlatformPasskeyOperation, start: Value) -> Result<Value, BsvHomePlatformAccountError> {
        let id = rules::string(&start, "challengeId")?;
        rules::identifier(id).map_err(|_| BsvHomePlatformAccountError::InvalidResponse)?;
        rules::unexpired(rules::string(&start, "expiresAt")?)?;
        let options = start
            .get("options")
            .and_then(|value| value.get("publicKey"))
            .filter(|value| value.is_object())
            .ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        let _authentication = self.authentication.lock().await;
        let response_json = self.authenticator.authenticate(operation, serde_json::to_string(options)?).await?;
        if response_json.len() > 131_072 {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        let response: Value = serde_json::from_str(&response_json)?;
        if rules::string(&response, "type")? != "public-key" || rules::string(&response, "id")?.is_empty() || rules::string(&response, "rawId")?.is_empty() {
            return Err(BsvHomePlatformAccountError::InvalidResponse);
        }
        let credential = response.get("response").ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        let fields: &[&str] = match operation {
            BsvHomePlatformPasskeyOperation::Register => &["clientDataJSON", "attestationObject"],
            BsvHomePlatformPasskeyOperation::Login | BsvHomePlatformPasskeyOperation::StepUp => &["clientDataJSON", "authenticatorData", "signature"],
        };
        for field in fields {
            if rules::string(credential, field)?.is_empty() {
                return Err(BsvHomePlatformAccountError::InvalidResponse);
            }
        }
        let mut body = self.device();
        body["challengeId"] = id.into();
        body["responseJson"] = response_json.into();
        Ok(body)
    }

    pub(super) fn save_operation(&self, key: String, lease: PlatformSessionLease, value: Value) -> Result<(), BsvHomePlatformAccountError> {
        self.require_lease(&lease)?;
        let mut state = self.state()?;
        self.require_generation(&state, lease.generation)?;
        state.operations.insert(key, PendingOperation { lease, value });
        Ok(())
    }

    pub(super) fn pending_operation(&self, key: &str) -> Result<(PlatformSessionLease, Value), BsvHomePlatformAccountError> {
        let (lease, value) = {
            let state = self.state()?;
            let operation = state.operations.get(key).ok_or(BsvHomePlatformAccountError::InvalidInput)?;
            (operation.lease.clone(), operation.value.clone())
        };
        self.require_lease(&lease)?;
        Ok((lease, value))
    }
}
