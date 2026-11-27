use primitives::{Chain, Wallet};

use super::rules;

#[derive(Debug, Clone, uniffi::Enum)]
pub enum BsvHomeWalletImportType {
    MulticoinPhrase { words: Vec<String>, chains: Vec<Chain> },
    SinglePhrase { words: Vec<String>, chain: Chain },
    PrivateKey { value: String, chain: Chain },
    Address { address: String, chain: Chain },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvHdImportMode {
    Multicoin,
    Recovery,
    Observer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeWalletImportKind {
    Phrase,
    PrivateKey,
    Address,
}

#[uniffi::export]
impl BsvHomeWalletImportKind {
    pub fn protects_input(&self) -> bool {
        !matches!(self, Self::Address)
    }

    pub fn supports_phrase_suggestions(&self) -> bool {
        matches!(self, Self::Phrase)
    }

    pub fn shows_view_only_warning(&self) -> bool {
        matches!(self, Self::Address)
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeWalletDefaultName {
    pub name: String,
    pub has_existing_wallets: bool,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum BsvHomeWalletImportResult {
    New { wallet: Wallet },
    Existing { wallet: Wallet },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeWalletDeletion {
    WalletsRemaining,
    LastWalletDeleted,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeWalletSecret {
    Words { words: Vec<String> },
    PrivateKey { key: String },
    Recovery { recovery: String },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeWalletSubtitle {
    Multicoin,
    Account { chain: Chain, address: String },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeWalletPlaceholder {
    Multicoin,
    Chain { chain: Chain },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeWalletRow {
    pub subtitle: BsvHomeWalletSubtitle,
    pub placeholder: BsvHomeWalletPlaceholder,
    pub shows_watch_badge: bool,
}

#[uniffi::export]
pub fn wallet_row(wallet: Wallet) -> BsvHomeWalletRow {
    rules::row(&wallet)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeWalletSecretKind {
    Phrase,
    PrivateKey,
    Recovery,
}

#[uniffi::export]
pub fn wallet_secret_kind(wallet: Wallet) -> Option<BsvHomeWalletSecretKind> {
    rules::secret_kind(&wallet)
}
