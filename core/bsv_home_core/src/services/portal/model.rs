use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, de::Error};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IdentityRequest<'a> {
    pub chain: &'a str,
    pub address: &'a str,
    pub device_id: &'a str,
    pub device_type: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LoginRequest<'a> {
    #[serde(flatten)]
    pub identity: IdentityRequest<'a>,
    pub challenge_id: &'a str,
    pub signature: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Challenge {
    pub challenge_id: String,
    pub address: String,
    pub chain: String,
    pub message: String,
    pub expires_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LoginResponse {
    pub token: String,
    pub address: Option<String>,
    pub chain: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub expires_at: Option<DateTime<Utc>>,
}

pub(super) fn deserialize_optional_string<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.filter(|value| !value.is_empty()))
}

pub(super) fn deserialize_optional_datetime<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error> {
    deserialize_optional_string(deserializer)?
        .map(|value| DateTime::parse_from_rfc3339(&value).map(|date| date.with_timezone(&Utc)))
        .transpose()
        .map_err(serde::de::Error::custom)
}

pub(super) fn deserialize_u64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Number {
        Integer(u64),
        Text(String),
    }
    match Number::deserialize(deserializer)? {
        Number::Integer(value) => Ok(value),
        Number::Text(value) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => value.parse().map_err(D::Error::custom),
        Number::Text(_) => Err(D::Error::custom("invalid unsigned integer")),
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct Session {
    pub base_url: String,
    pub wallet_id: String,
    pub address: String,
    pub token: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone, PartialEq)]
pub(super) struct Identity {
    pub wallet_id: String,
    pub address: String,
}
