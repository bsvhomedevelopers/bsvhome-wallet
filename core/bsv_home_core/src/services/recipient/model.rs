use primitives::{Asset, NFTAsset};

use crate::payment::BsvHomePaymentRecipient;
use crate::services::transfer::BsvHomeTransferData;

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeRecipientValidation {
    pub is_valid: bool,
    pub address: String,
    pub shows_error: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Error)]
pub enum BsvHomeRecipientError {
    InvalidAddress,
    NameRecordMismatch,
}

impl std::fmt::Display for BsvHomeRecipientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAddress => write!(f, "invalid recipient address"),
            Self::NameRecordMismatch => write!(f, "name record does not match the input"),
        }
    }
}

impl std::error::Error for BsvHomeRecipientError {}

#[derive(Debug, Clone, uniffi::Enum)]
#[allow(clippy::large_enum_variant)]
pub enum BsvHomeRecipientType {
    Asset { asset: Asset },
    Nft { nft_asset: NFTAsset },
}

#[uniffi::export]
impl BsvHomeRecipientType {
    pub fn identifier(&self) -> String {
        match self {
            Self::Asset { asset } => asset.id.to_string(),
            Self::Nft { nft_asset } => nft_asset.id.to_string(),
        }
    }
}

impl BsvHomeRecipientType {
    pub fn asset(&self) -> Asset {
        match self {
            Self::Asset { asset } => asset.clone(),
            Self::Nft { nft_asset } => Asset::from_chain(nft_asset.chain),
        }
    }
}

#[derive(Debug, Clone, uniffi::Enum)]
#[allow(clippy::large_enum_variant)]
pub enum BsvHomeRecipientScan {
    Confirm { transfer: BsvHomeTransferData },
    Recipient { payment: BsvHomePaymentRecipient },
}

#[derive(Debug, Clone, uniffi::Enum)]
#[allow(clippy::large_enum_variant)]
pub enum BsvHomeRecipientNext {
    Amount { payment: BsvHomePaymentRecipient },
    Confirm { transfer: BsvHomeTransferData },
}
