use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

use crate::alien::{AlienHttpMethod, AlienProvider, AlienTarget};
use crate::services::error::BsvHomeServiceError;

use super::error::BsvHomePortalAuthError;
use super::model::{Challenge, IdentityRequest, LoginRequest, LoginResponse};
use super::rules;
use super::transfer_password_model::TransferPasswordAction;

const MULTISIG_BACKOFF: Duration = Duration::from_secs(60);

pub(super) struct PortalClient {
    provider: Arc<dyn AlienProvider>,
    pub base_url: String,
    language: String,
    multisig_retry_after: Mutex<Option<Instant>>,
}

enum Request<'a> {
    Challenge(&'a IdentityRequest<'a>),
    Login(&'a LoginRequest<'a>),
    Authenticated {
        token: &'a str,
        path: &'a str,
        method: AlienHttpMethod,
        body: Option<Vec<u8>>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    success: Option<bool>,
    force_logout: Option<bool>,
    code: Option<ResponseCode>,
    message: Option<String>,
    data: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ResponseCode {
    Text(String),
    SignedInteger(i64),
    UnsignedInteger(u64),
}

impl From<ResponseCode> for String {
    fn from(code: ResponseCode) -> Self {
        match code {
            ResponseCode::Text(value) => value,
            ResponseCode::SignedInteger(value) => value.to_string(),
            ResponseCode::UnsignedInteger(value) => value.to_string(),
        }
    }
}

pub(super) struct RequestError {
    pub error: BsvHomePortalAuthError,
    pub requires_authentication: bool,
}

impl RequestError {
    fn invalid() -> Self {
        Self {
            error: BsvHomePortalAuthError::InvalidSession,
            requires_authentication: false,
        }
    }

    fn invalid_response() -> Self {
        Self {
            error: BsvHomePortalAuthError::InvalidResponse,
            requires_authentication: false,
        }
    }
}

impl From<RequestError> for BsvHomePortalAuthError {
    fn from(error: RequestError) -> Self {
        error.error
    }
}

impl PortalClient {
    pub fn new(provider: Arc<dyn AlienProvider>, base_url: String, language: String) -> Result<Self, BsvHomePortalAuthError> {
        if !matches!(language.as_str(), "zh-CN" | "en" | "th") {
            return Err(BsvHomePortalAuthError::InvalidSession);
        }
        Ok(Self {
            provider,
            base_url: rules::base_url(&base_url)?,
            language,
            multisig_retry_after: Mutex::new(None),
        })
    }

    pub fn multisig_request_ready(&self) -> Result<(), RequestError> {
        self.multisig_request_ready_at(Instant::now())
    }

    fn multisig_request_ready_at(&self, now: Instant) -> Result<(), RequestError> {
        let mut retry_after = self.multisig_retry_after.lock().map_err(|_| RequestError::invalid())?;
        if retry_after.is_some_and(|deadline| now < deadline) {
            return Err(RequestError {
                error: BsvHomePortalAuthError::Api {
                    code: "MULTISIG_RATE_LIMITED".into(),
                    msg: "multisig requests are temporarily rate limited".into(),
                },
                requires_authentication: false,
            });
        }
        *retry_after = None;
        Ok(())
    }

    fn record_rate_limit(&self, status: u16, code: &str) -> Result<(), RequestError> {
        if status == 429 || matches!(code, "HTTP_429" | "RATE_LIMITED" | "MULTISIG_RATE_LIMITED") {
            *self.multisig_retry_after.lock().map_err(|_| RequestError::invalid())? = Some(Instant::now() + MULTISIG_BACKOFF);
        }
        Ok(())
    }

    pub async fn challenge(&self, identity: &IdentityRequest<'_>) -> Result<Challenge, RequestError> {
        self.request(Request::Challenge(identity)).await
    }

    pub async fn login(&self, login: &LoginRequest<'_>) -> Result<LoginResponse, RequestError> {
        self.request(Request::Login(login)).await
    }

    pub async fn authenticated<R: DeserializeOwned>(&self, token: &str, path: &str, method: AlienHttpMethod, body: Option<Vec<u8>>) -> Result<R, RequestError> {
        self.request(Request::Authenticated { token, path, method, body }).await
    }

    async fn request<R: DeserializeOwned>(&self, request: Request<'_>) -> Result<R, RequestError> {
        let mut headers = HashMap::from([
            ("Content-Type".into(), "application/json".into()),
            ("Cache-Control".into(), "no-store".into()),
            ("language".into(), self.language.clone()),
        ]);
        let (path, method, body) = match request {
            Request::Challenge(identity) => (
                "/wallet-auth/challenge",
                AlienHttpMethod::Post,
                Some(serde_json::to_vec(identity).map_err(|_| RequestError::invalid())?),
            ),
            Request::Login(login) => (
                "/wallet-auth/login",
                AlienHttpMethod::Post,
                Some(serde_json::to_vec(login).map_err(|_| RequestError::invalid())?),
            ),
            Request::Authenticated { token, path, method, body } => {
                if !(path.starts_with("/auth/swaps/")
                    || path.starts_with("/auth/paymail/")
                    || path.starts_with("/auth/multisig/")
                    || TransferPasswordAction::permits(&self.base_url, path, method))
                    || path.contains("..")
                    || path.contains('#')
                {
                    return Err(RequestError::invalid());
                }
                if path.starts_with("/auth/multisig/") {
                    self.multisig_request_ready()?;
                }
                headers.insert("Authorization".into(), token.to_string());
                (path, method, body)
            }
        };
        let target = AlienTarget {
            url: format!("{}{path}", self.base_url),
            method,
            headers: Some(headers),
            body,
        };
        let response = self
            .provider
            .request(target)
            .await
            .map_err(|error| RequestError {
                error: BsvHomeServiceError::Api { msg: error.to_string() }.into(),
                requires_authentication: false,
            })?
            .to_rpc_response();
        let status = response.status.unwrap_or(0);
        let decoded = serde_json::from_slice::<Response>(&response.data);
        if !(200..300).contains(&status) {
            let (code, message, force_logout) = match decoded {
                Ok(response) => (
                    response.code.map(String::from).unwrap_or_else(|| format!("HTTP_{status}")),
                    response.message.unwrap_or_else(|| format!("HTTP {status}")),
                    response.force_logout == Some(true),
                ),
                Err(_) => (format!("HTTP_{status}"), format!("HTTP {status}"), false),
            };
            self.record_rate_limit(status, &code)?;
            return Err(RequestError {
                requires_authentication: status == 401 || force_logout || rules::requires_authentication(&code),
                error: BsvHomePortalAuthError::Api { code, msg: message },
            });
        }
        let response = decoded.map_err(|_| RequestError::invalid_response())?;
        let requires_ok = self.base_url.ends_with("/v2/api/portal");
        let successful_code = match response.code.as_ref() {
            Some(ResponseCode::Text(code)) => !requires_ok || code == "OK",
            Some(ResponseCode::SignedInteger(_) | ResponseCode::UnsignedInteger(_)) => TransferPasswordAction::permits(&self.base_url, path, method),
            None => !requires_ok,
        };
        if response.success != Some(true) || response.force_logout == Some(true) || !successful_code {
            if response.success.is_none() && response.code.is_none() && response.message.is_none() && response.force_logout != Some(true) {
                return Err(RequestError::invalid_response());
            }
            let code = response.code.map(String::from).unwrap_or_else(|| format!("HTTP_{status}"));
            self.record_rate_limit(status, &code)?;
            return Err(RequestError {
                requires_authentication: response.force_logout == Some(true) || rules::requires_authentication(&code),
                error: BsvHomePortalAuthError::Api {
                    code,
                    msg: response.message.unwrap_or_else(|| "Portal request failed".into()),
                },
            });
        }
        serde_json::from_value(match response.data {
            Some(data) => data,
            None => Value::Null,
        })
        .map_err(|_| RequestError::invalid_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alien::{AlienError, AlienResponse};
    use crate::testkit::TestAlienProvider;
    use futures::executor::block_on;
    use serde_json::Map;

    #[test]
    fn test_online_transfer_password_accepts_only_the_three_exact_post_targets() {
        block_on(async {
            let provider = Arc::new(TestAlienProvider::with_json(200, r#"{"success":true,"data":{}}"#));
            let client = PortalClient::new(provider.clone(), "https://portal.test/api/portal".into(), "en".into()).unwrap();
            for path in [
                "/wallet-auth/transfer-password/status",
                "/wallet-auth/transfer-password/set",
                "/wallet-auth/transfer-password/verify",
            ] {
                let response = client
                    .authenticated::<Map<String, Value>>("synthetic-session", path, AlienHttpMethod::Post, Some(b"{}".to_vec()))
                    .await
                    .map_err(|error| error.error);
                assert_eq!(response, Ok(Map::new()));
            }
            assert_eq!(provider.requested_paths().len(), 3);
        });
    }

    #[derive(Debug)]
    struct NoRequests;

    #[async_trait::async_trait]
    impl AlienProvider for NoRequests {
        async fn request(&self, _: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
            panic!("cooldown clock checks must not issue network requests")
        }
    }

    #[test]
    fn test_multisig_cooldown_expires_at_monotonic_deadline() {
        let client = PortalClient::new(Arc::new(NoRequests), "https://portal.test/api/portal".into(), "en".into()).unwrap();
        let before = Instant::now();
        client.record_rate_limit(429, "CUSTOM_QUOTA").unwrap_or_else(|_| panic!("valid cooldown"));
        let after = Instant::now();
        let deadline = client.multisig_retry_after.lock().unwrap().unwrap();
        assert!(deadline >= before + MULTISIG_BACKOFF);
        assert!(deadline <= after + MULTISIG_BACKOFF);
        assert!(matches!(
            client.multisig_request_ready_at(deadline - Duration::from_nanos(1)),
            Err(RequestError { error: BsvHomePortalAuthError::Api { code, .. }, requires_authentication: false }) if code == "MULTISIG_RATE_LIMITED"
        ));
        assert!(client.multisig_request_ready_at(deadline).is_ok());
        assert!(client.multisig_retry_after.lock().unwrap().is_none());
        assert!(client.multisig_request_ready_at(deadline + Duration::from_secs(1)).is_ok());
    }
}
