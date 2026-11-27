use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use crate::{CONTENT_TYPE, ContentType, MULTIPART_FORM_DATA};

#[derive(Debug, Clone)]
pub struct Response {
    pub status: Option<u16>,
    pub data: Vec<u8>,
}

#[derive(Clone)]
pub enum ClientError<E = Vec<u8>> {
    Network(String),
    Timeout,
    Http { status: u16, body: E },
    Serialization(String),
}

impl ClientError {
    pub fn decode_body<E: DeserializeOwned>(self) -> ClientError<Option<E>> {
        match self {
            Self::Http { status, body } => ClientError::Http {
                status,
                body: serde_json::from_slice(&body).ok(),
            },
            Self::Network(message) => ClientError::Network(message),
            Self::Timeout => ClientError::Timeout,
            Self::Serialization(message) => ClientError::Serialization(message),
        }
    }
}

impl fmt::Debug for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(msg) => f.debug_tuple("Network").field(msg).finish(),
            Self::Timeout => write!(f, "Timeout"),
            Self::Http { status, body } => {
                let body_str = String::from_utf8_lossy(&body[..body.len().min(256)]);
                f.debug_struct("Http").field("status", status).field("body", &body_str).finish()
            }
            Self::Serialization(msg) => f.debug_tuple("Serialization").field(msg).finish(),
        }
    }
}

pub fn decode_json_byte_array(values: Vec<Value>) -> Result<Vec<u8>, ClientError> {
    let mut bytes = Vec::with_capacity(values.len());
    for value in values {
        let byte = value
            .as_u64()
            .ok_or_else(|| ClientError::Serialization("Expected byte array for binary content-type".to_string()))?;
        if byte > u8::MAX as u64 {
            return Err(ClientError::Serialization("Binary body byte out of range".to_string()));
        }
        bytes.push(byte as u8);
    }
    Ok(bytes)
}

impl<E> fmt::Display for ClientError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(msg) => write!(f, "Network error: {}", msg),
            Self::Timeout => write!(f, "Timeout error"),
            Self::Http { status, .. } => write!(f, "HTTP error: status {}", status),
            Self::Serialization(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<serde_json::Error> for ClientError {
    fn from(err: serde_json::Error) -> Self {
        ClientError::Serialization(format!("JSON error: {err}"))
    }
}

pub fn encode_request_body<T: Serialize>(headers: &HashMap<String, String>, body: &T) -> Result<Vec<u8>, ClientError> {
    let content_type = headers.get(CONTENT_TYPE).map(String::as_str);
    let is_multipart = content_type.is_some_and(|value| value.starts_with(MULTIPART_FORM_DATA));
    match content_type.and_then(|value| ContentType::from_str(value).ok()) {
        Some(ContentType::TextPlain) | Some(ContentType::ApplicationFormUrlEncoded) => match serde_json::to_value(body)? {
            Value::String(text) => Ok(text.into_bytes()),
            _ => Err(ClientError::Serialization("Expected string body for text content-type".to_string())),
        },
        Some(ContentType::ApplicationXBinary) | Some(ContentType::ApplicationAptosBcs) => decode_binary_body(serde_json::to_value(body)?),
        _ if is_multipart => decode_binary_body(serde_json::to_value(body)?),
        _ => Ok(serde_json::to_vec(body)?),
    }
}

fn decode_binary_body(value: Value) -> Result<Vec<u8>, ClientError> {
    match value {
        Value::String(text) => hex::decode(&text).map_err(|error| ClientError::Serialization(format!("Failed to decode hex string: {error}"))),
        Value::Array(values) => decode_json_byte_array(values),
        _ => Err(ClientError::Serialization("Expected hex string or byte array body for binary content-type".to_string())),
    }
}

pub fn deserialize_response<R>(response: &Response) -> Result<R, ClientError>
where
    R: DeserializeOwned,
{
    validate_http_status(response)?;
    let data: &[u8] = if response.data.is_empty() { b"null" } else { &response.data };
    serde_json::from_slice(data).map_err(|error| ClientError::Serialization(error.to_string()))
}

fn validate_http_status(response: &Response) -> Result<(), ClientError> {
    if let Some(status) = response.status {
        if !(200..400).contains(&status) {
            return Err(ClientError::Http {
                status,
                body: response.data.clone(),
            });
        }
    }
    Ok(())
}

pub fn deserialize_text_response(response: &Response) -> Result<String, ClientError> {
    validate_http_status(response)?;
    let text = String::from_utf8(response.data.clone()).map_err(|error| ClientError::Serialization(error.to_string()))?;
    Ok(serde_json::from_str::<String>(&text).unwrap_or(text))
}

#[cfg(test)]
mod json_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_codec_rejects_http_failures_before_decoding_and_retains_error_bodies() {
        for status in [400, 404, 429, 500, 503] {
            for value in [json!({}), json!({"result": "valid-looking"}), json!({"error": {"code": -32000, "message": "failed"}})] {
                let data = serde_json::to_vec(&value).unwrap();
                let error = deserialize_response::<Value>(&Response { status: Some(status), data }).unwrap_err();
                match error.decode_body::<Value>() {
                    ClientError::Http { status: actual, body } => {
                        assert_eq!(actual, status);
                        assert_eq!(body, Some(value));
                    }
                    _ => panic!("expected retained HTTP error body"),
                }
            }
        }
    }

    #[test]
    fn json_codec_preserves_success_rpc_error_envelopes_and_parse_failures() {
        for status in [Some(200), Some(201), None] {
            for value in [json!({"result": 1}), json!({"id": 1, "error": {"code": -32601, "message": "Method not found"}})] {
                let data = serde_json::to_vec(&value).unwrap();
                assert_eq!(deserialize_response::<Value>(&Response { status, data }).unwrap(), value);
            }
            assert!(matches!(
                deserialize_response::<Value>(&Response {
                    status,
                    data: b"invalid-json".to_vec()
                }),
                Err(ClientError::Serialization(_))
            ));
        }
        assert_eq!(
            deserialize_response::<Option<Value>>(&Response {
                status: Some(204),
                data: Vec::new()
            })
            .unwrap(),
            None
        );
    }
}

#[cfg(test)]
mod text_tests {
    use super::*;
    #[test]
    fn text_codec_handles_supplier_txids_and_preserves_http_failures() {
        for data in [b"abc123".to_vec(), br#""abc123""#.to_vec()] {
            assert_eq!(deserialize_text_response(&Response { status: Some(200), data }).unwrap(), "abc123");
        }
        assert!(matches!(
            deserialize_text_response(&Response {
                status: Some(429),
                data: b"\"limited\"".to_vec()
            }),
            Err(ClientError::Http { status: 429, .. })
        ));
        assert!(matches!(
            deserialize_text_response(&Response {
                status: Some(200),
                data: vec![255]
            }),
            Err(ClientError::Serialization(_))
        ));
    }
}
