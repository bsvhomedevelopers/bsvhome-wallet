use std::fmt;

use bsv_home_keystore::SecretKind;
use primitives::{Account, Chain, WalletId, WalletType};

pub type BsvHomeSecretKind = SecretKind;

#[uniffi::remote(Enum)]
pub enum BsvHomeSecretKind {
    Mnemonic,
    PrivateKey,
    HdRecovery,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeKeystoreAccount {
    pub chain: Chain,
    pub address: String,
    pub derivation_path: String,
    pub public_key: Option<String>,
}

impl From<Account> for BsvHomeKeystoreAccount {
    fn from(account: Account) -> Self {
        Self {
            chain: account.chain,
            address: account.address,
            derivation_path: account.derivation_path,
            public_key: Some(account.extended_public_key.unwrap_or_default()),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeWalletImport {
    pub wallet_id: String,
    pub wallet_type: WalletType,
    pub accounts: Vec<BsvHomeKeystoreAccount>,
}

impl BsvHomeWalletImport {
    pub(super) fn new(wallet_id: WalletId, wallet_type: WalletType, accounts: Vec<Account>) -> Self {
        Self {
            wallet_id: wallet_id.to_string(),
            wallet_type,
            accounts: accounts.into_iter().map(BsvHomeKeystoreAccount::from).collect(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeStoredWallet {
    pub wallet_id: String,
    pub wallet_type: WalletType,
    pub keystore_id: String,
    pub accounts: Vec<BsvHomeKeystoreAccount>,
}

impl BsvHomeStoredWallet {
    pub(super) fn new(wallet_id: WalletId, wallet_type: WalletType, keystore_id: String, accounts: Vec<Account>) -> Self {
        Self {
            wallet_id: wallet_id.to_string(),
            wallet_type,
            keystore_id,
            accounts: accounts.into_iter().map(BsvHomeKeystoreAccount::from).collect(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeStoredSecretMigration {
    pub keystore_id: String,
    pub kind: BsvHomeSecretKind,
}

#[derive(Clone, uniffi::Enum)]
pub enum BsvHomeImportType {
    MulticoinPhrase { words: Vec<String>, chains: Vec<Chain> },
    SinglePhrase { words: Vec<String>, chain: Chain },
    PrivateKey { value: String, chain: Chain },
}

impl fmt::Debug for BsvHomeImportType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BsvHomeImportType::MulticoinPhrase { words, chains } => f.debug_struct("MulticoinPhrase").field("word_count", &words.len()).field("chains", chains).finish(),
            BsvHomeImportType::SinglePhrase { words, chain } => f.debug_struct("SinglePhrase").field("word_count", &words.len()).field("chain", chain).finish(),
            BsvHomeImportType::PrivateKey { chain, .. } => f.debug_struct("PrivateKey").field("chain", chain).field("value", &"<redacted>").finish(),
        }
    }
}
