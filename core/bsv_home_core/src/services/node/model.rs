use primitives::Latency;

use super::rules;
use crate::gateway::GatewayError;
use crate::service_status::BsvHomeLatencyStatus;

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeNodeStatusState {
    Loading,
    Error,
    Result { latest_block_number: u64, latency: Latency },
}

#[uniffi::export]
impl BsvHomeNodeStatusState {
    pub fn latency_status(&self) -> BsvHomeLatencyStatus {
        rules::latency_status(self)
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeNodeCheck {
    pub url: String,
    pub chain_id: Option<String>,
    pub latest_block_number: u64,
    pub is_in_sync: bool,
    pub latency: Latency,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeNodeSelection {
    pub url: String,
    pub host: String,
    pub is_selected: bool,
}

#[derive(Debug, Clone, uniffi::Error)]
pub enum BsvHomeAddNodeError {
    InvalidUrl,
    InvalidNetworkId,
    Gateway(GatewayError),
}

impl std::fmt::Display for BsvHomeAddNodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl => write!(f, "invalid node url"),
            Self::InvalidNetworkId => write!(f, "node answered for a different network"),
            Self::Gateway(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for BsvHomeAddNodeError {}

impl From<GatewayError> for BsvHomeAddNodeError {
    fn from(error: GatewayError) -> Self {
        match error {
            GatewayError::NetworkIdMismatch { .. } => Self::InvalidNetworkId,
            error => Self::Gateway(error),
        }
    }
}
