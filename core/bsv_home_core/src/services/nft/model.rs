use chrono::{DateTime, Utc};
use primitives::{AssetLink, BlockExplorerLink, Chain, NFTAssetData, NFTData, VerificationStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeNftList {
    Collections,
    Unverified,
    Collection,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, uniffi::Enum)]
pub enum BsvHomeNftItem {
    Collection { data: NFTData },
    Asset { data: NFTAssetData },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeCollectibleDetails {
    pub can_send: bool,
    pub sections: Vec<BsvHomeCollectibleSection>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeCollectibleSection {
    Status { status: VerificationStatus },
    Info { rows: Vec<BsvHomeCollectibleRow> },
    Attributes { attributes: Vec<BsvHomeCollectibleAttribute> },
    Links { links: Vec<AssetLink> },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeCollectibleRow {
    Collection { name: String },
    Network { chain: Chain },
    Contract { identifier: BsvHomeCollectibleIdentifier },
    TokenId { identifier: BsvHomeCollectibleIdentifier },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeCollectibleIdentifier {
    pub value: String,
    pub text: String,
    pub explorer: Option<BlockExplorerLink>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeCollectibleAttribute {
    pub name: String,
    pub value: BsvHomeCollectibleAttributeValue,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeCollectibleAttributeValue {
    Text { value: String },
    Date { date: DateTime<Utc> },
}
