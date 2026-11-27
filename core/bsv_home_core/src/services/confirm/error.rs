use crate::BsvHomeCoreError;
use crate::gateway::GatewayError;
use crate::services::balance::BsvHomeBalanceRequirement;
use crate::services::error::BsvHomeServiceError;
use crate::signer::BsvHomeSignerError;
use primitives::{Asset, AssetId, Chain, SwapProvider};

#[derive(Debug, Clone, uniffi::Error)]
pub enum BsvHomeConfirmError {
    ScanMalicious,
    ScanMemoRequired {
        symbol: String,
    },
    FeeRatesMissing,
    Offline,
    Network {
        msg: String,
    },
    Load {
        msg: String,
    },
    Broadcast {
        hashes: Vec<String>,
        msg: String,
    },
    Record {
        msg: String,
    },
    AccountMissing {
        chain: Chain,
    },
    BalanceMissing {
        asset_id: AssetId,
    },
    InsufficientBalance {
        asset: Asset,
        requirement: BsvHomeBalanceRequirement,
    },
    InsufficientNetworkFee {
        asset: Asset,
        requirement: Option<BsvHomeBalanceRequirement>,
    },
    MinimumAccountBalanceTooLow {
        asset: Asset,
        requirement: BsvHomeBalanceRequirement,
    },
    BelowSwapMinimum {
        asset: Asset,
        provider: SwapProvider,
        provider_name: String,
        requirement: BsvHomeBalanceRequirement,
    },
    SenderMismatch {
        from: String,
        signer: String,
    },
    Sign {
        error: BsvHomeSignerError,
        chain: Chain,
        msg: String,
    },
    ApprovalInvalid {
        msg: String,
    },
    Cancelled,
}

impl std::fmt::Display for BsvHomeConfirmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScanMalicious => write!(f, "transaction flagged as malicious"),
            Self::ScanMemoRequired { symbol } => write!(f, "{symbol} transfer requires a memo"),
            Self::FeeRatesMissing => write!(f, "fee rates not found"),
            Self::Offline => write!(f, "network offline"),
            Self::AccountMissing { chain } => write!(f, "wallet has no {chain} account"),
            Self::BalanceMissing { asset_id } => write!(f, "no stored balance for {asset_id}"),
            Self::InsufficientBalance { asset, .. } => write!(f, "not enough {} balance", asset.symbol),
            Self::InsufficientNetworkFee { asset, .. } => write!(f, "not enough {} to pay the network fee", asset.symbol),
            Self::MinimumAccountBalanceTooLow { asset, requirement } => write!(f, "{} balance must stay above {}", asset.symbol, requirement.required),
            Self::BelowSwapMinimum {
                asset,
                provider_name,
                requirement,
                ..
            } => {
                write!(f, "{} amount is below the {} minimum {}", asset.symbol, provider_name, requirement.required)
            }
            Self::SenderMismatch { from, signer } => write!(f, "transaction was built for {from} but would be signed by {signer}"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Network { msg } | Self::Load { msg } | Self::Broadcast { msg, .. } | Self::Record { msg } | Self::Sign { msg, .. } | Self::ApprovalInvalid { msg } => {
                write!(f, "{msg}")
            }
        }
    }
}

impl std::error::Error for BsvHomeConfirmError {}

pub(super) fn sign_error(chain: Chain, error: BsvHomeCoreError) -> BsvHomeConfirmError {
    match error {
        BsvHomeCoreError::Cancelled => BsvHomeConfirmError::Cancelled,
        BsvHomeCoreError::SignerError { error, msg } => BsvHomeConfirmError::Sign { error, chain, msg },
        BsvHomeCoreError::AnyError { msg } => BsvHomeConfirmError::Sign {
            error: BsvHomeSignerError::SigningError(msg.clone()),
            chain,
            msg,
        },
    }
}

impl From<BsvHomeServiceError> for BsvHomeConfirmError {
    fn from(error: BsvHomeServiceError) -> Self {
        match error {
            BsvHomeServiceError::Cancelled => Self::Cancelled,
            error => Self::Load { msg: error.to_string() },
        }
    }
}

pub(super) fn load_error(error: GatewayError) -> BsvHomeConfirmError {
    match error {
        GatewayError::Offline => BsvHomeConfirmError::Offline,
        GatewayError::NetworkError { msg } => BsvHomeConfirmError::Network { msg },
        error => BsvHomeConfirmError::Load { msg: error.to_string() },
    }
}

pub(super) fn broadcast_error(hashes: Vec<String>, error: GatewayError) -> BsvHomeConfirmError {
    match error {
        GatewayError::Offline if hashes.is_empty() => BsvHomeConfirmError::Offline,
        GatewayError::NetworkError { msg } if hashes.is_empty() => BsvHomeConfirmError::Network { msg },
        error => BsvHomeConfirmError::Broadcast { hashes, msg: error.to_string() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a_cancelled_signer_is_a_cancel_not_a_failure() {
        assert!(matches!(sign_error(Chain::Ethereum, BsvHomeCoreError::Cancelled), BsvHomeConfirmError::Cancelled));
        assert!(matches!(
            sign_error(Chain::Ethereum, BsvHomeCoreError::AnyError { msg: "boom".into() }),
            BsvHomeConfirmError::Sign { error: BsvHomeSignerError::SigningError(msg), chain: Chain::Ethereum, .. } if msg == "boom"
        ));
    }

    #[test]
    fn test_a_cancelled_keystore_prompt_is_a_cancel_not_a_load_failure() {
        assert!(matches!(BsvHomeConfirmError::from(BsvHomeServiceError::Cancelled), BsvHomeConfirmError::Cancelled));
        assert!(matches!(
            BsvHomeConfirmError::from(BsvHomeServiceError::Store { msg: "x".to_string() }),
            BsvHomeConfirmError::Load { .. }
        ));
    }

    #[test]
    fn test_gateway_errors_keep_their_kind() {
        match load_error(GatewayError::NetworkError { msg: "timeout".to_string() }) {
            BsvHomeConfirmError::Network { msg } => assert_eq!(msg, "timeout"),
            error => panic!("expected a network error, got {error:?}"),
        }
        assert!(matches!(load_error(GatewayError::Offline), BsvHomeConfirmError::Offline));
        assert!(matches!(broadcast_error(vec![], GatewayError::Offline), BsvHomeConfirmError::Offline));
        assert!(matches!(
            broadcast_error(vec!["h1".to_string()], GatewayError::Offline),
            BsvHomeConfirmError::Broadcast { .. }
        ));
        match load_error(GatewayError::PlatformError { msg: "dust".to_string() }) {
            BsvHomeConfirmError::Load { msg } => assert_eq!(msg, "Platform error: dust"),
            error => panic!("expected a load error, got {error:?}"),
        }
        match broadcast_error(vec![], GatewayError::NetworkError { msg: "offline".to_string() }) {
            BsvHomeConfirmError::Network { msg } => assert_eq!(msg, "offline"),
            error => panic!("expected a network error, got {error:?}"),
        }
        match broadcast_error(vec!["h1".to_string()], GatewayError::NetworkError { msg: "offline".to_string() }) {
            BsvHomeConfirmError::Broadcast { hashes, .. } => assert_eq!(hashes, vec!["h1".to_string()]),
            error => panic!("expected a partial broadcast error, got {error:?}"),
        }
    }
}
