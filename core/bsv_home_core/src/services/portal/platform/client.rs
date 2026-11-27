use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::alien::{AlienHttpMethod, AlienProvider, AlienTarget};

use super::target::PlatformTarget;
use super::{BsvHomePlatformAccountError, rules};

const MAX_RESPONSE_BYTES: usize = 4_194_304;

pub(super) struct PlatformClient {
    provider: Arc<dyn AlienProvider>,
    pub base_url: String,
    language: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    success: Option<bool>,
    code: Option<String>,
    force_logout: Option<bool>,
    data: Option<Value>,
}

impl PlatformClient {
    pub fn new(provider: Arc<dyn AlienProvider>, base_url: String, language: String) -> Result<Self, BsvHomePlatformAccountError> {
        if language != "zh-CN" && language != "en" && language != "th" {
            return Err(BsvHomePlatformAccountError::InvalidConfiguration);
        }
        Ok(Self {
            provider,
            base_url: rules::base_url(&base_url)?,
            language,
        })
    }

    pub async fn request(&self, path: &str, body: Value, token: Option<&str>) -> Result<Value, BsvHomePlatformAccountError> {
        self.send(path, AlienHttpMethod::Post, Some(body), token).await
    }

    async fn send(&self, path: &str, method: AlienHttpMethod, body: Option<Value>, token: Option<&str>) -> Result<Value, BsvHomePlatformAccountError> {
        let target = PlatformTarget::try_from(path)?;
        if target.method() != method {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        let mut headers = HashMap::from([
            ("Content-Type".into(), "application/json; charset=utf-8".into()),
            ("Cache-Control".into(), "no-store".into()),
            ("language".into(), self.language.clone()),
        ]);
        if let Some(token) = token {
            headers.insert("Authorization".into(), token.into());
        }
        let response = self
            .provider
            .request(AlienTarget {
                url: format!("{}{}", self.base_url, target.path()),
                method,
                headers: Some(headers),
                body: body.map(|body| serde_json::to_vec(&body)).transpose()?,
            })
            .await
            .map_err(|_| BsvHomePlatformAccountError::NetworkUnavailable)?
            .to_rpc_response();
        let status = response.status.ok_or(BsvHomePlatformAccountError::InvalidResponse)?;
        if status == 401 {
            return Err(BsvHomePlatformAccountError::LoginRequired);
        }
        parse_response(status, &response.data)
    }

    pub async fn authenticated_get(&self, path: &str, token: &str) -> Result<Value, BsvHomePlatformAccountError> {
        if !matches!(path, "portal/auth/paymail/config" | "portal/auth/paymail/me") {
            return Err(BsvHomePlatformAccountError::InvalidInput);
        }
        self.send(path, AlienHttpMethod::Get, None, Some(token)).await
    }
}

pub(super) fn parse_response(status: u16, bytes: &[u8]) -> Result<Value, BsvHomePlatformAccountError> {
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(BsvHomePlatformAccountError::InvalidResponse);
    }
    let envelope: Envelope = serde_json::from_slice(bytes)?;
    if envelope.force_logout == Some(true) || envelope.code.as_deref() == Some("LOGIN_REQUIRED") {
        return Err(BsvHomePlatformAccountError::LoginRequired);
    }
    if !(200..300).contains(&status) || envelope.success != Some(true) || envelope.code.as_deref() != Some("OK") {
        return Err(BsvHomePlatformAccountError::Api {
            code: rules::public_code(envelope.code.as_deref().unwrap_or("SERVICE_ERROR")),
        });
    }
    envelope.data.filter(|data| !data.is_null()).ok_or(BsvHomePlatformAccountError::InvalidResponse)
}
