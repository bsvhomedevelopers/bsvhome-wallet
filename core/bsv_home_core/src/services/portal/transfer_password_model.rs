use serde::{Deserialize, Serialize};

use crate::alien::AlienHttpMethod;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeTransferPasswordSnapshot {
    pub wallet_id: String,
    pub address: String,
    pub configured: bool,
    pub generation: u64,
}

#[derive(Deserialize)]
pub(super) struct TransferPasswordStatusResponse {
    pub configured: bool,
}

#[derive(Clone, Copy)]
pub(super) enum TransferPasswordAction {
    Status,
    Set,
    Verify,
}

impl TransferPasswordAction {
    pub fn path(self) -> &'static str {
        match self {
            Self::Status => "/wallet-auth/transfer-password/status",
            Self::Set => "/wallet-auth/transfer-password/set",
            Self::Verify => "/wallet-auth/transfer-password/verify",
        }
    }

    pub fn permits(base_url: &str, path: &str, method: AlienHttpMethod) -> bool {
        !base_url.ends_with("/v2/api/portal") && method == AlienHttpMethod::Post && [Self::Status, Self::Set, Self::Verify].into_iter().any(|action| action.path() == path)
    }
}

#[derive(Serialize)]
pub(super) struct TransferPasswordStatusRequest<'a> {
    pub address: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TransferPasswordSetRequest<'a> {
    pub address: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_password: Option<&'a str>,
    pub new_password: &'a str,
}

#[derive(Serialize)]
pub(super) struct TransferPasswordVerifyRequest<'a> {
    pub address: &'a str,
    pub password: &'a str,
}
