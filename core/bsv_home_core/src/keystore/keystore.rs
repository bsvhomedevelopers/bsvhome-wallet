use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use bsv_home_bsv::cold_wallet::ColdAddress;
use bsv_home_bsv::multisig::{Funding, FundingReview, member_address, review_funding, sign_funding_with_keys};
use bsv_home_bsv::signer::BsvChainSigner;
use bsv_home_derivation::{
    BSV_ACCOUNT_PATH, bsv_address_for_path, derive_account_from_private_key, derive_account_from_private_key_value, derive_accounts_from_mnemonic,
    derive_bsv_account_from_recovery, derive_bsv_private_keys_from_recovery, derive_private_key_from_mnemonic, derive_wallet_id_from_account, import_account_from_private_key,
};
use bsv_home_keystore::{FileKeystore, HdDerivationMode, HdRecovery, Keystore, KeystoreError, KeystoreId, SecretKind, StoredSecretMeta};
use primitives::{Account, BsvAddressBranch, BsvAddressLocator, Chain, SignerError, SignerInput, TransactionLoadMetadata, TransactionType, Wallet, WalletId, WalletType};
use signer::{encode_private_key, secp256k1_public_key};
use zeroize::Zeroizing;

use super::types::{BsvHomeImportType, BsvHomeKeystoreAccount, BsvHomeStoredSecretMigration, BsvHomeStoredWallet, BsvHomeWalletImport};
use crate::BsvHomeCoreError;
use crate::models::transaction::{BsvHomeSignedTransaction, BsvHomeSignerInput};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::wallet::model::BsvHomeWalletSecretKind;
use crate::signer::ChainTransactionSigner;

#[derive(uniffi::Object)]
pub struct BsvHomeKeystore {
    inner: FileKeystore,
    environment: BsvHomeRuntimeEnvironment,
}

#[uniffi::export]
impl BsvHomeKeystore {
    #[uniffi::constructor]
    pub fn new(base_dir: String) -> Result<Arc<Self>, BsvHomeCoreError> {
        Self::new_with_environment(base_dir, BsvHomeRuntimeEnvironment::Production)
    }

    #[uniffi::constructor]
    pub fn new_with_environment(base_dir: String, environment: BsvHomeRuntimeEnvironment) -> Result<Arc<Self>, BsvHomeCoreError> {
        Ok(Arc::new(Self {
            inner: FileKeystore::open(PathBuf::from(base_dir))?,
            environment,
        }))
    }

    pub fn keystore_id(&self, wallet_id: String) -> String {
        keystore_id_for_wallet(wallet_id)
    }

    pub fn preview_import(&self, import: BsvHomeImportType) -> Result<BsvHomeWalletImport, BsvHomeCoreError> {
        self.require_import(&import)?;
        match import {
            BsvHomeImportType::PrivateKey { value, chain } => {
                let value = Zeroizing::new(value);
                let account = derive_account_from_private_key_value(&value, chain)?;
                let wallet_id = derive_wallet_id_from_account(&account, WalletType::PrivateKey)?;
                Ok(BsvHomeWalletImport::new(wallet_id, WalletType::PrivateKey, vec![account]))
            }
            BsvHomeImportType::MulticoinPhrase { words, chains } => {
                let (wallet_id, accounts, _phrase) = derive_mnemonic_wallet(words, chains, WalletType::Multicoin, Chain::Ethereum)?;
                Ok(BsvHomeWalletImport::new(wallet_id, WalletType::Multicoin, accounts))
            }
            BsvHomeImportType::SinglePhrase { words, chain } => {
                let (wallet_id, accounts, _phrase) = derive_mnemonic_wallet(words, vec![chain], WalletType::Single, chain)?;
                Ok(BsvHomeWalletImport::new(wallet_id, WalletType::Single, accounts))
            }
        }
    }

    pub fn create_store(&self, import: BsvHomeImportType, password: Vec<u8>) -> Result<BsvHomeStoredWallet, BsvHomeCoreError> {
        self.require_import(&import)?;
        let password = Zeroizing::new(password);
        match import {
            BsvHomeImportType::PrivateKey { value, chain } => {
                let value = Zeroizing::new(value);
                let imported = import_account_from_private_key(&value, chain)?;
                let wallet_id = derive_wallet_id_from_account(&imported.account, WalletType::PrivateKey)?;
                let meta = self
                    .inner
                    .import_private_key(&imported.private_key, &password, Some(keystore_id_for_wallet(wallet_id.to_string())))?;
                Ok(BsvHomeStoredWallet::new(wallet_id, WalletType::PrivateKey, meta.keystore_id, vec![imported.account]))
            }
            BsvHomeImportType::MulticoinPhrase { words, chains } => {
                let (wallet_id, accounts, phrase) = derive_mnemonic_wallet(words, chains, WalletType::Multicoin, Chain::Ethereum)?;
                let meta = self.inner.import_mnemonic(&phrase, &password, Some(keystore_id_for_wallet(wallet_id.to_string())))?;
                Ok(BsvHomeStoredWallet::new(wallet_id, WalletType::Multicoin, meta.keystore_id, accounts))
            }
            BsvHomeImportType::SinglePhrase { words, chain } => {
                let (wallet_id, accounts, phrase) = derive_mnemonic_wallet(words, vec![chain], WalletType::Single, chain)?;
                let meta = self.inner.import_mnemonic(&phrase, &password, Some(keystore_id_for_wallet(wallet_id.to_string())))?;
                Ok(BsvHomeStoredWallet::new(wallet_id, WalletType::Single, meta.keystore_id, accounts))
            }
        }
    }

    pub fn wallet_secret_kind(&self, wallet: Wallet) -> Result<Option<BsvHomeWalletSecretKind>, BsvHomeCoreError> {
        if wallet.wallet_type == WalletType::View {
            return Ok(None);
        }
        let meta = self
            .inner
            .get_meta(&keystore_id_for_wallet(wallet.id.id()))?
            .ok_or_else(|| BsvHomeCoreError::from("wallet secret is missing"))?;
        Ok(Some(match meta.kind {
            SecretKind::Mnemonic => BsvHomeWalletSecretKind::Phrase,
            SecretKind::PrivateKey => BsvHomeWalletSecretKind::PrivateKey,
            SecretKind::HdRecovery => BsvHomeWalletSecretKind::Recovery,
        }))
    }

    pub fn has_stored_wallets(&self) -> Result<bool, BsvHomeCoreError> {
        Ok(!self.inner.list()?.is_empty())
    }

    pub fn export_recovery_phrase(&self, keystore_id: String, password: Vec<u8>) -> Result<Vec<String>, BsvHomeCoreError> {
        let password = Zeroizing::new(password);
        Ok(self
            .inner
            .decrypt_mnemonic(&keystore_id, &password)?
            .split_whitespace()
            .map(|word| word.to_string())
            .collect())
    }

    pub fn export_private_key(&self, keystore_id: String, chain: Chain, password: Vec<u8>) -> Result<String, BsvHomeCoreError> {
        let password = Zeroizing::new(password);
        let private_key = self.load_private_key(&keystore_id, chain, &password)?;
        Ok(encode_private_key(&chain, &private_key)?)
    }

    pub fn migrate_v3(&self, v3_path: String, v3_password: Vec<u8>, new_password: Vec<u8>, wallet_id: String) -> Result<BsvHomeStoredSecretMigration, BsvHomeCoreError> {
        let v3_password = Zeroizing::new(v3_password);
        let new_password = Zeroizing::new(new_password);
        let expected_wallet_id = WalletId::from_id(&wallet_id).ok_or_else(|| BsvHomeCoreError::from("invalid wallet id"))?;
        let keystore_id = KeystoreId::from_wallet_id(&wallet_id).into_string();
        let meta = self.inner.import_v3(&PathBuf::from(&v3_path), &v3_password, &new_password, Some(keystore_id.clone()))?;
        if let Err(error) = self.verify_migrated_secret(&keystore_id, &new_password, &expected_wallet_id, meta.kind) {
            let _ = self.inner.delete(&keystore_id);
            return Err(error);
        }
        // The v3 file is the migration-pending marker; remove it only after the binding is verified.
        match std::fs::remove_file(&v3_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(BsvHomeCoreError::from(format!("v3 cleanup failed: {error}"))),
        }
        Ok(BsvHomeStoredSecretMigration {
            keystore_id: meta.keystore_id,
            kind: meta.kind,
        })
    }

    pub fn delete(&self, keystore_id: String) -> Result<bool, BsvHomeCoreError> {
        Ok(self.inner.delete(&keystore_id)?)
    }

    pub fn exists(&self, keystore_id: String) -> bool {
        matches!(self.inner.get_meta(&keystore_id), Ok(Some(_)))
    }

    pub fn decode_password(&self, password: String) -> Vec<u8> {
        decode_password(&password)
    }

    pub fn sign(&self, keystore_id: String, chain: Chain, input: BsvHomeSignerInput, password: Vec<u8>) -> Result<Vec<BsvHomeSignedTransaction>, BsvHomeCoreError> {
        self.require_chain(chain)?;
        if input.input.input_type.get_asset().chain() != chain || input.fee.fee_asset.chain != chain {
            return Err(BsvHomeCoreError::from("signer, transaction and fee networks must match"));
        }
        if let TransactionLoadMetadata::BitcoinSv { spend } = &input.input.metadata {
            if chain != Chain::BitcoinSv {
                return Err(BsvHomeCoreError::from("BSV spend metadata requires BSV network"));
            }
            let password = Zeroizing::new(password);
            let recovery = self.bsv_recovery(&keystore_id, &password)?;
            let account = derive_bsv_account_from_recovery(&recovery)?;
            if account.address != input.input.sender_address {
                return Err(BsvHomeCoreError::from("BSV spend does not match keystore identity"));
            }
            if let Some(change) = &spend.change
                && (change.locator.branch != BsvAddressBranch::Change
                    || bsv_address_for_path(
                        account.extended_public_key.as_deref().ok_or_else(|| BsvHomeCoreError::from("BSV recovery xpub missing"))?,
                        &recovery.account_path,
                        change.locator,
                    )? != change.address)
            {
                return Err(BsvHomeCoreError::from("BSV change does not belong to signing wallet"));
            }
            let addresses = spend
                .inputs
                .iter()
                .map(|source| ColdAddress {
                    address: source.address.clone(),
                    branch: source.locator.branch.index(),
                    index: source.locator.index,
                })
                .collect::<Vec<_>>();
            let keys = derive_bsv_private_keys_from_recovery(&recovery, &addresses)?;
            let keys = spend
                .inputs
                .iter()
                .map(|source| source.locator)
                .zip(keys.into_iter().map(|(_, key)| key))
                .collect::<HashMap<_, _>>();
            let input: SignerInput = input.into();
            let data = BsvChainSigner.sign_transfer_with_keys(&input, |locator| {
                keys.get(&locator)
                    .map(|key| Zeroizing::new(key.to_vec()))
                    .ok_or_else(|| SignerError::invalid_input("missing BSV input key"))
            })?;
            return Ok(vec![BsvHomeSignedTransaction {
                data,
                transaction_type: TransactionType::Transfer,
            }]);
        }
        ChainTransactionSigner::new(chain).sign_input(input, self.signing_key(&keystore_id, chain, password)?)
    }
}

impl BsvHomeKeystore {
    pub(crate) fn sign_bsv_multisig_funding_payload(&self, keystore_id: &str, funding: &Funding, review: &FundingReview, password: Vec<u8>) -> Result<String, BsvHomeCoreError> {
        self.require_chain(Chain::BitcoinSv)?;
        if &review_funding(funding)? != review {
            return Err(BsvHomeCoreError::from("BSV funding differs from confirmed review"));
        }
        let password = Zeroizing::new(password);
        let meta = self.inner.get_meta(keystore_id)?.ok_or_else(|| BsvHomeCoreError::from("wallet secret is missing"))?;
        match meta.kind {
            SecretKind::Mnemonic | SecretKind::HdRecovery => {
                let recovery = self.bsv_recovery(keystore_id, &password)?;
                let account = derive_bsv_account_from_recovery(&recovery)?;
                let xpub = account.extended_public_key.as_deref().ok_or_else(|| BsvHomeCoreError::from("BSV recovery xpub missing"))?;
                for address in &funding.addresses {
                    if bsv_address_for_path(xpub, &recovery.account_path, address.locator)? != address.address {
                        return Err(BsvHomeCoreError::from("BSV funding input does not belong to the selected payment wallet"));
                    }
                }
                let mut addresses = funding
                    .utxos
                    .iter()
                    .map(|source| {
                        let address = funding
                            .addresses
                            .iter()
                            .find(|address| address.address == source.address)
                            .ok_or_else(|| BsvHomeCoreError::from("BSV funding source has no derivation locator"))?;
                        Ok(ColdAddress {
                            address: source.address.clone(),
                            branch: address.locator.branch.index(),
                            index: address.locator.index,
                        })
                    })
                    .collect::<Result<Vec<_>, BsvHomeCoreError>>()?;
                if let Some(change) = &funding.change {
                    if bsv_address_for_path(xpub, &recovery.account_path, change.locator)? != change.address {
                        return Err(BsvHomeCoreError::from("BSV funding change does not belong to the selected payment wallet"));
                    }
                    addresses.push(ColdAddress {
                        address: change.address.clone(),
                        branch: change.locator.branch.index(),
                        index: change.locator.index,
                    });
                }
                let keys = derive_bsv_private_keys_from_recovery(&recovery, &addresses)?;
                let locators = addresses.iter().map(|address| BsvAddressLocator {
                    branch: if address.branch == 0 { BsvAddressBranch::Receive } else { BsvAddressBranch::Change },
                    index: address.index,
                });
                let keys = locators.zip(keys.into_iter().map(|(_, key)| key)).collect::<HashMap<_, _>>();
                Ok(sign_funding_with_keys(funding, review, |locator| {
                    keys.get(&locator)
                        .map(|key| Zeroizing::new(key.to_vec()))
                        .ok_or_else(|| SignerError::invalid_input("BSV funding input key missing"))
                })?)
            }
            SecretKind::PrivateKey => {
                if funding.change.is_some() {
                    return Err(BsvHomeCoreError::from("BSV funding sender does not match the selected private-key wallet"));
                }
                let key = self.inner.decrypt_private_key(keystore_id, &password)?;
                let address = member_address(&hex::encode(secp256k1_public_key(key.as_slice())?))?;
                if funding.sender_address != address || funding.addresses.len() != 1 {
                    return Err(BsvHomeCoreError::from("BSV funding sender does not match the selected private-key wallet"));
                }
                Ok(sign_funding_with_keys(funding, review, |_| Ok(Zeroizing::new(key.to_vec())))?)
            }
        }
    }

    pub(crate) fn bsv_account_xpub(&self, keystore_id: String, main_address: &str, password: Vec<u8>) -> Result<String, BsvHomeCoreError> {
        let password = Zeroizing::new(password);
        let recovery = self.bsv_recovery(&keystore_id, &password)?;
        let account = derive_bsv_account_from_recovery(&recovery)?;
        if account.address != main_address {
            return Err(BsvHomeCoreError::from("BSV derived account does not match existing wallet"));
        }
        account.extended_public_key.ok_or_else(|| BsvHomeCoreError::from("BSV recovery xpub missing"))
    }

    pub(crate) fn contains_stored_secret(&self, keystore_id: &str) -> Result<bool, BsvHomeCoreError> {
        Ok(self.inner.get_meta(keystore_id)?.is_some())
    }

    pub(crate) fn bsv_recovery(&self, keystore_id: &str, password: &[u8]) -> Result<HdRecovery, BsvHomeCoreError> {
        let meta = self.inner.get_meta(keystore_id)?.ok_or_else(|| BsvHomeCoreError::from("wallet secret is missing"))?;
        match meta.kind {
            SecretKind::Mnemonic => {
                let phrase = self.inner.decrypt_mnemonic(keystore_id, password)?;
                Ok(HdRecovery::new(&phrase, "", BSV_ACCOUNT_PATH, HdDerivationMode::Standard)?)
            }
            SecretKind::HdRecovery => self.decrypt_hd_recovery(keystore_id, password),
            SecretKind::PrivateKey => Err(BsvHomeCoreError::from("cold wallet requires an HD recovery secret")),
        }
    }

    pub(crate) fn decrypt_hd_recovery(&self, keystore_id: &str, password: &[u8]) -> Result<HdRecovery, BsvHomeCoreError> {
        Ok(self.inner.decrypt_hd_recovery(keystore_id, password)?)
    }

    pub(super) fn inner_import_hd_recovery(&self, recovery: &HdRecovery, password: &[u8], keystore_id: &str) -> Result<StoredSecretMeta, BsvHomeCoreError> {
        self.require_chain(Chain::BitcoinSv)?;
        Ok(self.inner.import_hd_recovery(recovery, password, Some(keystore_id.to_string()))?)
    }

    pub(crate) fn verify_wallet_password(&self, keystore_id: String, password: Vec<u8>) -> Result<(), BsvHomeCoreError> {
        let password = Zeroizing::new(password);
        self.inner.verify(&keystore_id, &password)?;
        Ok(())
    }

    pub fn add_accounts(&self, keystore_id: String, password: Vec<u8>, chains: Vec<Chain>) -> Result<Vec<BsvHomeKeystoreAccount>, BsvHomeCoreError> {
        for chain in &chains {
            self.require_chain(*chain)?;
        }
        let password = Zeroizing::new(password);
        let phrase = match self.inner.decrypt_mnemonic(&keystore_id, &password) {
            Ok(phrase) => phrase,
            Err(KeystoreError::CorruptFile(message)) if message == "stored secret is not a mnemonic" => {
                return Err(BsvHomeCoreError::from("add_accounts does not support private-key wallets"));
            }
            Err(error) => return Err(error.into()),
        };
        Ok(derive_accounts_from_mnemonic(&phrase, chains)?.into_iter().map(BsvHomeKeystoreAccount::from).collect())
    }
    pub fn change_password(&self, keystore_id: String, old_password: Vec<u8>, new_password: Vec<u8>) -> Result<(), BsvHomeCoreError> {
        let old_password = Zeroizing::new(old_password);
        let new_password = Zeroizing::new(new_password);
        self.inner.change_password(&keystore_id, &old_password, &new_password)?;
        Ok(())
    }
    pub fn opens_with(&self, keystore_id: String, password: Vec<u8>) -> bool {
        let password = Zeroizing::new(password);
        self.inner.verify(&keystore_id, &password).is_ok()
    }
    pub fn sign_auth(&self, keystore_id: String, chain: Chain, hash: [u8; 32], password: Vec<u8>) -> Result<String, BsvHomeCoreError> {
        crate::auth::sign_auth_message_hash(hash, self.signing_key(&keystore_id, chain, password)?)
    }
}

impl BsvHomeKeystore {
    fn require_chain(&self, chain: Chain) -> Result<(), BsvHomeCoreError> {
        self.environment.require_chain(chain).map_err(|error| BsvHomeCoreError::from(error.to_string()))
    }

    fn require_import(&self, import: &BsvHomeImportType) -> Result<(), BsvHomeCoreError> {
        match import {
            BsvHomeImportType::PrivateKey { chain, .. } => {
                self.require_chain(*chain)?;
                if chain.is_testnet() {
                    return Err(BsvHomeCoreError::from("test networks require a mnemonic wallet"));
                }
            }
            BsvHomeImportType::MulticoinPhrase { chains, .. } => {
                for chain in chains {
                    self.require_chain(*chain)?;
                }
            }
            BsvHomeImportType::SinglePhrase { chain, .. } => self.require_chain(*chain)?,
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn private_key(&self, keystore_id: String, chain: Chain, password: Vec<u8>) -> Result<Vec<u8>, BsvHomeCoreError> {
        let password = Zeroizing::new(password);
        Ok(self.load_private_key(&keystore_id, chain, &password)?.to_vec())
    }
}

pub(crate) fn keystore_id_for_wallet(wallet_id: String) -> String {
    KeystoreId::from_wallet_id(&wallet_id).into_string()
}

impl BsvHomeKeystore {
    pub(crate) fn delete_wallet_secrets(&self, wallet_id: String, legacy_id: String) -> Result<(), BsvHomeCoreError> {
        self.inner.delete(&keystore_id_for_wallet(wallet_id))?;
        self.inner.delete_v3(&legacy_id)?;
        Ok(())
    }

    pub(crate) fn signing_key(&self, keystore_id: &str, chain: Chain, password: Vec<u8>) -> Result<Zeroizing<Vec<u8>>, BsvHomeCoreError> {
        let password = Zeroizing::new(password);
        self.load_private_key(keystore_id, chain, &password)
    }

    fn load_private_key(&self, keystore_id: &str, chain: Chain, password: &[u8]) -> Result<Zeroizing<Vec<u8>>, BsvHomeCoreError> {
        self.require_chain(chain)?;
        let meta = self.inner.get_meta(keystore_id)?.ok_or_else(|| BsvHomeCoreError::from("wallet secret is missing"))?;
        match meta.kind {
            SecretKind::Mnemonic => {
                let phrase = self.inner.decrypt_mnemonic(keystore_id, password)?;
                Ok(derive_private_key_from_mnemonic(&phrase, chain)?)
            }
            SecretKind::PrivateKey => {
                if chain.is_testnet() {
                    return Err(BsvHomeCoreError::from("test networks require a mnemonic wallet"));
                }
                Ok(self.inner.decrypt_private_key(keystore_id, password)?)
            }
            SecretKind::HdRecovery => {
                if chain != Chain::BitcoinSv {
                    return Err(BsvHomeCoreError::from("this HD recovery profile supports BSV only"));
                }
                let recovery = self.decrypt_hd_recovery(keystore_id, password)?;
                let account = derive_bsv_account_from_recovery(&recovery)?;
                let address = ColdAddress {
                    address: account.address,
                    branch: 0,
                    index: 0,
                };
                derive_bsv_private_keys_from_recovery(&recovery, &[address])?
                    .into_iter()
                    .next()
                    .map(|(_, key)| key)
                    .ok_or_else(|| BsvHomeCoreError::from("BSV recovery first key missing"))
            }
        }
    }

    fn verify_migrated_secret(&self, keystore_id: &str, password: &[u8], expected: &WalletId, kind: SecretKind) -> Result<(), BsvHomeCoreError> {
        let (wallet_type, chain) = match expected {
            WalletId::Multicoin(_) => (WalletType::Multicoin, Chain::Ethereum),
            WalletId::Single(chain, _) => (WalletType::Single, *chain),
            WalletId::PrivateKey(chain, _) => (WalletType::PrivateKey, *chain),
            WalletId::View(_, _) => return Err(BsvHomeCoreError::from("view wallets have no keystore secret")),
        };
        let kind_matches_type = match kind {
            SecretKind::Mnemonic | SecretKind::HdRecovery => wallet_type != WalletType::PrivateKey,
            SecretKind::PrivateKey => wallet_type == WalletType::PrivateKey,
        };
        if !kind_matches_type {
            return Err(BsvHomeCoreError::from("migrated secret kind does not match wallet type"));
        }
        let account = match kind {
            SecretKind::Mnemonic => {
                let phrase = self.inner.decrypt_mnemonic(keystore_id, password)?;
                derive_accounts_from_mnemonic(&phrase, vec![chain])?
                    .into_iter()
                    .next()
                    .ok_or_else(|| BsvHomeCoreError::from("wallet id account derivation returned no account"))?
            }
            SecretKind::HdRecovery => {
                let recovery = self.decrypt_hd_recovery(keystore_id, password)?;
                derive_bsv_account_from_recovery(&recovery)?
            }
            SecretKind::PrivateKey => {
                let private_key = self.inner.decrypt_private_key(keystore_id, password)?;
                derive_account_from_private_key(&private_key, chain)?
            }
        };
        let is_legacy_single_solana_mnemonic = wallet_type == WalletType::Single && chain == Chain::Solana && kind == SecretKind::Mnemonic;
        let derived = derive_wallet_id_from_account(&account, wallet_type)?;
        if &derived != expected {
            if is_legacy_single_solana_mnemonic {
                return Ok(());
            }
            return Err(BsvHomeCoreError::from(format!(
                "migrated secret does not derive the wallet id: expected_wallet_id={expected}, derived_wallet_id={derived}, derived_chain={}, derived_address={}",
                account.chain, account.address
            )));
        }
        Ok(())
    }
}

fn derive_mnemonic_wallet(
    words: Vec<String>,
    requested_chains: Vec<Chain>,
    wallet_type: WalletType,
    wallet_id_chain: Chain,
) -> Result<(WalletId, Vec<Account>, Zeroizing<String>), BsvHomeCoreError> {
    if requested_chains.is_empty() {
        return Err(bsv_home_derivation::AccountDerivationError::invalid_input("mnemonic derivation requires at least one chain").into());
    }

    let words = Zeroizing::new(words);
    let phrase = Zeroizing::new(words.join(" "));
    let mut chains = requested_chains.clone();
    if !chains.contains(&wallet_id_chain) {
        chains.push(wallet_id_chain);
    }

    let derived_accounts = derive_accounts_from_mnemonic(&phrase, chains)?;
    let wallet_id_account = derived_accounts
        .iter()
        .find(|account| account.chain == wallet_id_chain)
        .ok_or_else(|| bsv_home_derivation::AccountDerivationError::unsupported("wallet id account derivation returned no account"))?;
    let wallet_id = derive_wallet_id_from_account(wallet_id_account, wallet_type)?;
    let accounts = derived_accounts.into_iter().filter(|account| requested_chains.contains(&account.chain)).collect();

    Ok((wallet_id, accounts, phrase))
}

#[cfg(test)]
mod migration_tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use primitives::{Chain, hex};

    use super::{BsvHomeKeystore, keystore_id_for_wallet};

    const V3_MNEMONIC: &str = include_str!("../../../crates/bsv_home_keystore/testdata/v3_ios_mnemonic.json");
    const V3_PRIVATE_KEY: &str = include_str!("../../../crates/bsv_home_keystore/testdata/v3_ios_private_key.json");
    const V3_EMPTY_SALT_MNEMONIC: &str = include_str!("../../../crates/bsv_home_keystore/testdata/v3_empty_salt_mnemonic.json");
    const V3_PASSWORD: &[u8] = b"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const NEW_PASSWORD: &[u8] = b"raw-v4-password-bytes";
    const EXPECTED_PHRASE: &str =
        "dignity possible oppose wolf early kingdom essay arctic ten fence prepare mango source federal chief south dynamic rebuild wear envelope bulb picnic own scorpion";
    const EXPECTED_PRIVATE_KEY: &str = "ae8794f84919b14ff9d1f0f7cf490a4c04e608de16864f53fe8b40af127b9da3";
    const EXPECTED_ETHEREUM_ADDRESS: &str = "0x5a8f70b44aFa00Cb70615D9c9CCb9A24933ED2D3";
    const EXPECTED_SOLANA_ADDRESS: &str = "5T1JAioMm5vd9S5RE2JHBu3GVfG5FpUoV1BT5CNhZR2W";
    const MNEMONIC_WALLET_ID: &str = "multicoin_0x5a8f70b44aFa00Cb70615D9c9CCb9A24933ED2D3";
    const PRIVATE_KEY_WALLET_ID: &str = "privateKey_ethereum_0x5a8f70b44aFa00Cb70615D9c9CCb9A24933ED2D3";

    fn v4_path(base: &Path, keystore_id: &str) -> PathBuf {
        base.join(format!("{keystore_id}.json"))
    }

    fn prepare(name: &str, fixture: &str) -> (PathBuf, String) {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let unique = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!("bsv_home_core_migration_{name}_{}_{unique}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let v3_path = base.join("legacy.json");
        std::fs::write(&v3_path, fixture).unwrap();
        (base, v3_path.to_string_lossy().into_owned())
    }

    #[test]
    fn migrate_v3_mnemonic_round_trip_and_idempotent() {
        let (base, v3_path) = prepare("mnemonic", V3_MNEMONIC);
        let keystore = BsvHomeKeystore::new(base.to_string_lossy().into_owned()).unwrap();
        let wallet_id = MNEMONIC_WALLET_ID.to_string();
        let keystore_id = keystore_id_for_wallet(wallet_id.clone());

        let migration = keystore
            .migrate_v3(v3_path.clone(), V3_PASSWORD.to_vec(), NEW_PASSWORD.to_vec(), wallet_id.clone())
            .unwrap();
        assert_eq!(migration.keystore_id, keystore_id);
        assert!(!Path::new(&v3_path).exists(), "v3 file must be removed after a verified migration");

        assert_eq!(
            keystore.export_recovery_phrase(keystore_id.clone(), NEW_PASSWORD.to_vec()).unwrap().join(" "),
            EXPECTED_PHRASE
        );
        let accounts = keystore.add_accounts(keystore_id.clone(), NEW_PASSWORD.to_vec(), vec![Chain::Ethereum]).unwrap();
        assert_eq!(accounts[0].address, EXPECTED_ETHEREUM_ADDRESS);

        // idempotent re-run with the v3 file already gone
        let again = keystore.migrate_v3(v3_path, V3_PASSWORD.to_vec(), NEW_PASSWORD.to_vec(), wallet_id).unwrap();
        assert_eq!(again.keystore_id, keystore_id);
        assert_eq!(keystore.export_recovery_phrase(keystore_id, NEW_PASSWORD.to_vec()).unwrap().join(" "), EXPECTED_PHRASE);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn migrate_v3_empty_salt_mnemonic_round_trip() {
        let (base, v3_path) = prepare("empty_salt", V3_EMPTY_SALT_MNEMONIC);
        let keystore = BsvHomeKeystore::new(base.to_string_lossy().into_owned()).unwrap();
        let wallet_id = MNEMONIC_WALLET_ID.to_string();
        let keystore_id = keystore_id_for_wallet(wallet_id.clone());

        let migration = keystore
            .migrate_v3(v3_path.clone(), V3_PASSWORD.to_vec(), NEW_PASSWORD.to_vec(), wallet_id.clone())
            .unwrap();
        assert_eq!(migration.keystore_id, keystore_id);
        assert!(!Path::new(&v3_path).exists(), "v3 file must be removed after a verified migration");
        assert_eq!(
            keystore.export_recovery_phrase(keystore_id.clone(), NEW_PASSWORD.to_vec()).unwrap().join(" "),
            EXPECTED_PHRASE
        );
        let accounts = keystore.add_accounts(keystore_id, NEW_PASSWORD.to_vec(), vec![Chain::Ethereum]).unwrap();
        assert_eq!(accounts[0].address, EXPECTED_ETHEREUM_ADDRESS);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn migrate_v3_private_key_round_trip_and_idempotent() {
        let (base, v3_path) = prepare("private_key", V3_PRIVATE_KEY);
        let keystore = BsvHomeKeystore::new(base.to_string_lossy().into_owned()).unwrap();
        let wallet_id = PRIVATE_KEY_WALLET_ID.to_string();
        let keystore_id = keystore_id_for_wallet(wallet_id.clone());

        let migration = keystore
            .migrate_v3(v3_path.clone(), V3_PASSWORD.to_vec(), NEW_PASSWORD.to_vec(), wallet_id.clone())
            .unwrap();
        assert_eq!(migration.keystore_id, keystore_id);
        assert!(!Path::new(&v3_path).exists(), "v3 file must be removed after a verified migration");
        assert_eq!(
            hex::encode(keystore.private_key(keystore_id.clone(), Chain::Ethereum, NEW_PASSWORD.to_vec()).unwrap()),
            EXPECTED_PRIVATE_KEY
        );
        let account = keystore
            .preview_import(super::BsvHomeImportType::PrivateKey {
                value: EXPECTED_PRIVATE_KEY.to_string(),
                chain: Chain::Ethereum,
            })
            .unwrap();
        assert_eq!(account.accounts[0].address, EXPECTED_ETHEREUM_ADDRESS);

        let again = keystore.migrate_v3(v3_path, V3_PASSWORD.to_vec(), NEW_PASSWORD.to_vec(), wallet_id).unwrap();
        assert_eq!(again.keystore_id, keystore_id);
        assert_eq!(
            hex::encode(keystore.private_key(keystore_id, Chain::Ethereum, NEW_PASSWORD.to_vec()).unwrap()),
            EXPECTED_PRIVATE_KEY
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn migrate_v3_rejects_secret_that_does_not_derive_the_wallet_id() {
        let (base, v3_path) = prepare("mismatch", V3_MNEMONIC);
        let keystore = BsvHomeKeystore::new(base.to_string_lossy().into_owned()).unwrap();
        let wrong_wallet_id = "multicoin_0x0000000000000000000000000000000000000000".to_string();
        let keystore_id = keystore_id_for_wallet(wrong_wallet_id.clone());

        let error = keystore
            .migrate_v3(v3_path.clone(), V3_PASSWORD.to_vec(), NEW_PASSWORD.to_vec(), wrong_wallet_id.clone())
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "migrated secret does not derive the wallet id: expected_wallet_id={wrong_wallet_id}, derived_wallet_id={MNEMONIC_WALLET_ID}, derived_chain=ethereum, derived_address={EXPECTED_ETHEREUM_ADDRESS}"
            )
        );
        assert!(!v4_path(&base, &keystore_id).exists(), "mismatched v4 file must not be left behind");
        assert!(Path::new(&v3_path).exists(), "v3 file must be preserved when the migration is rejected");

        // wrong wallet type for the secret kind is also rejected
        let error = keystore
            .migrate_v3(v3_path.clone(), V3_PASSWORD.to_vec(), NEW_PASSWORD.to_vec(), PRIVATE_KEY_WALLET_ID.to_string())
            .unwrap_err();
        assert!(error.to_string().contains("does not match wallet type"), "{error}");
        assert!(Path::new(&v3_path).exists());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn migrate_v3_allows_legacy_single_solana_derivation_mismatch() {
        let (base, v3_path) = prepare("single_solana_mismatch", V3_MNEMONIC);
        let keystore = BsvHomeKeystore::new(base.to_string_lossy().into_owned()).unwrap();
        let wallet_id = "single_solana_2e6GNStEYy7M31duwabbAx3R4Tzz62LxryGrqcMhoaaV".to_string();
        let keystore_id = keystore_id_for_wallet(wallet_id.clone());

        let migration = keystore.migrate_v3(v3_path.clone(), V3_PASSWORD.to_vec(), NEW_PASSWORD.to_vec(), wallet_id).unwrap();

        assert_eq!(migration.keystore_id, keystore_id);
        assert!(!Path::new(&v3_path).exists(), "legacy Solana v3 file must be removed after import");
        assert_eq!(
            keystore.export_recovery_phrase(keystore_id.clone(), NEW_PASSWORD.to_vec()).unwrap().join(" "),
            EXPECTED_PHRASE
        );
        let accounts = keystore.add_accounts(keystore_id, NEW_PASSWORD.to_vec(), vec![Chain::Solana]).unwrap();
        assert_eq!(accounts[0].address, EXPECTED_SOLANA_ADDRESS);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn migrate_v3_replaces_corrupt_staged_v4_file() {
        let (base, v3_path) = prepare("corrupt_v4", V3_MNEMONIC);
        let keystore = BsvHomeKeystore::new(base.to_string_lossy().into_owned()).unwrap();
        let wallet_id = MNEMONIC_WALLET_ID.to_string();
        let keystore_id = keystore_id_for_wallet(wallet_id.clone());
        std::fs::write(v4_path(&base, &keystore_id), b"not a keystore").unwrap();

        let migration = keystore.migrate_v3(v3_path.clone(), V3_PASSWORD.to_vec(), NEW_PASSWORD.to_vec(), wallet_id).unwrap();
        assert_eq!(migration.keystore_id, keystore_id);
        assert!(!Path::new(&v3_path).exists());
        assert_eq!(keystore.export_recovery_phrase(keystore_id, NEW_PASSWORD.to_vec()).unwrap().join(" "), EXPECTED_PHRASE);

        let _ = std::fs::remove_dir_all(&base);
    }
}

pub fn decode_password(password: &str) -> Vec<u8> {
    hex::decode(password.strip_prefix("0x").unwrap_or(password)).unwrap_or_else(|_| password.as_bytes().to_vec())
}

#[cfg(test)]
mod password_tests {
    use super::decode_password;

    #[test]
    fn test_password_decodes_as_hex_and_falls_back_to_utf8_for_legacy_wallets() {
        assert_eq!(decode_password("000102"), vec![0x00, 0x01, 0x02]);
        assert_eq!(decode_password("0x000102"), vec![0x00, 0x01, 0x02]);

        // Wallets created before the hex generator stored a UUID, which WalletCore consumed as raw utf8.
        let legacy = "9B2D3F14-7C58-4A21-93E0-5D6F8A0C1E77";
        assert_eq!(decode_password(legacy), legacy.as_bytes());
        assert_eq!(decode_password("abc"), b"abc", "odd-length hex is not hex");
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use primitives::testkit::signer_mock::TEST_PRIVATE_KEY;
    use primitives::{Asset, SignerInput, TransactionFee, TransactionInputType, TransactionLoadMetadata};
    use tempfile::tempdir;

    fn test_input() -> BsvHomeSignerInput {
        SignerInput::mock_tron(
            TransactionInputType::Transfer {
                asset: Asset::from_chain(Chain::TronShasta),
            },
            "TJRyWwFs9wTFGZg3JbrVriFbNfCug5tDeC",
            "THTR75o8xXAgCTQqpiot2AFRAjvW1tSbVV",
            "1000000",
            TransactionFee::new_from_fee(0.into(), Chain::TronShasta.as_asset_id()),
            None,
            TransactionLoadMetadata::None,
        )
        .into()
    }

    #[test]
    fn test_shasta_keystore_production_rejects_import_and_signing_before_secret_access() {
        let directory = tempdir().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let import = BsvHomeImportType::SinglePhrase {
            words: Vec::new(),
            chain: Chain::TronShasta,
        };
        let expected = BsvHomeCoreError::from("network unavailable in this environment: tronshasta");

        assert_eq!(keystore.preview_import(import.clone()).unwrap_err(), expected);
        assert_eq!(keystore.create_store(import, Vec::new()).unwrap_err(), expected);
        assert_eq!(keystore.add_accounts("missing".to_string(), Vec::new(), vec![Chain::TronShasta]).unwrap_err(), expected);
        assert_eq!(keystore.sign("missing".to_string(), Chain::TronShasta, test_input(), Vec::new()).unwrap_err(), expected);
        assert_eq!(keystore.export_private_key("missing".to_string(), Chain::TronShasta, Vec::new()).unwrap_err(), expected);
        assert!(!keystore.has_stored_wallets().unwrap());
    }

    #[test]
    fn test_shasta_keystore_binds_transaction_and_fee_networks_before_secret_access() {
        let directory = tempdir().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();

        for (asset_chain, fee_chain) in [(Chain::TronShasta, Chain::Tron), (Chain::Tron, Chain::TronShasta)] {
            let mut input = test_input();
            input.input.input_type = TransactionInputType::Transfer {
                asset: Asset::from_chain(asset_chain),
            };
            input.fee.fee_asset = fee_chain.as_asset_id();

            assert_eq!(
                keystore.sign("missing".to_string(), Chain::Tron, input, Vec::new()).unwrap_err(),
                BsvHomeCoreError::from("signer, transaction and fee networks must match")
            );
        }
    }

    #[test]
    fn test_shasta_keystore_local_environment_rejects_private_wallets() {
        let directory = tempdir().unwrap();
        let keystore = BsvHomeKeystore::new_with_environment(directory.path().to_string_lossy().into_owned(), BsvHomeRuntimeEnvironment::LocalDevelopment).unwrap();
        let import = BsvHomeImportType::PrivateKey {
            value: String::new(),
            chain: Chain::TronShasta,
        };
        let expected = if cfg!(debug_assertions) {
            BsvHomeCoreError::from("test networks require a mnemonic wallet")
        } else {
            BsvHomeCoreError::from("network unavailable in this environment: tronshasta")
        };

        assert_eq!(keystore.preview_import(import.clone()).unwrap_err(), expected);
        assert_eq!(keystore.create_store(import, Vec::new()).unwrap_err(), expected);
        let stored = keystore
            .inner
            .import_private_key(&TEST_PRIVATE_KEY, b"runtime-test", Some(keystore_id_for_wallet("runtime-private".to_string())))
            .unwrap();
        assert_eq!(
            keystore.sign(stored.keystore_id, Chain::TronShasta, test_input(), b"runtime-test".to_vec()).unwrap_err(),
            expected
        );
    }
}

#[cfg(test)]
mod bsv_signing_tests {
    use std::sync::Arc;

    use bsv_home_bsv::BsvAddress;
    use bsv_home_bsv::signer::{BsvChainSigner, finalize_spend_metadata, plan_transfer};
    use bsv_home_derivation::{BSV_ACCOUNT_PATH, bsv_account_xpub, bsv_address, bsv_address_for_path, bsv_private_key};
    use primitives::testkit::ABANDON_PHRASE;
    use primitives::{
        Address, Asset, BsvAddressBranch, BsvAddressLocator, BsvSpendChange, BsvSpendInput, BsvSpendMetadata, Chain, GasPriceType, SignerError, SignerInput, TransactionFee,
        TransactionInputType, TransactionLoadInput, TransactionLoadMetadata, TransactionType,
    };
    use tempfile::{TempDir, tempdir};

    use super::{BsvHomeImportType, BsvHomeKeystore};

    const TEST_PASSWORD: &[u8] = b"bsv-derived-signing-fixture";
    const FOREIGN_ADDRESS: &str = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";

    struct Context {
        _directory: TempDir,
        keystore: Arc<BsvHomeKeystore>,
        id: String,
        xpub: String,
        account_path: String,
    }

    impl Context {
        fn new() -> Self {
            let directory = tempdir().unwrap();
            let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
            let wallet = keystore
                .create_store(
                    BsvHomeImportType::SinglePhrase {
                        words: ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                        chain: Chain::BitcoinSv,
                    },
                    TEST_PASSWORD.to_vec(),
                )
                .unwrap();
            Self {
                _directory: directory,
                keystore,
                id: wallet.keystore_id,
                xpub: bsv_account_xpub(ABANDON_PHRASE).unwrap(),
                account_path: BSV_ACCOUNT_PATH.into(),
            }
        }

        fn from_recovery(account_path: &str, legacy: bool) -> Self {
            let directory = tempdir().unwrap();
            let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
            let wallet = keystore
                .import_bsv_hd(ABANDON_PHRASE, Some(account_path), Some(" public:Pass "), None, legacy, TEST_PASSWORD)
                .unwrap();
            Self {
                _directory: directory,
                keystore,
                id: wallet.keystore_id,
                xpub: wallet.accounts[0].public_key.clone().unwrap(),
                account_path: account_path.into(),
            }
        }

        fn input(&self) -> SignerInput {
            let locators = [
                BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index: 0,
                },
                BsvAddressLocator {
                    branch: BsvAddressBranch::Change,
                    index: 0,
                },
            ];
            let inputs = locators
                .iter()
                .enumerate()
                .map(|(index, locator)| {
                    let address = bsv_address_for_path(&self.xpub, &self.account_path, *locator).unwrap();
                    BsvSpendInput {
                        transaction_id: format!("{:064}", index + 1),
                        vout: 0,
                        value: [50000, 30000][index],
                        script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
                        address,
                        locator: *locator,
                    }
                })
                .collect();
            let locator = BsvAddressLocator {
                branch: BsvAddressBranch::Change,
                index: 1,
            };
            let change_address = bsv_address_for_path(&self.xpub, &self.account_path, locator).unwrap();
            let mut input = TransactionLoadInput {
                input_type: TransactionInputType::Transfer {
                    asset: Asset::from_chain(Chain::BitcoinSv),
                },
                sender_address: bsv_address_for_path(&self.xpub, &self.account_path, locators[0]).unwrap(),
                destination_address: FOREIGN_ADDRESS.to_string(),
                value: 60000u64.into(),
                gas_price: GasPriceType::regular(100),
                memo: None,
                is_max_value: false,
                metadata: TransactionLoadMetadata::BitcoinSv {
                    spend: BsvSpendMetadata {
                        inputs,
                        change: Some(BsvSpendChange {
                            script_pubkey: hex::encode(BsvAddress::from_str(&change_address).unwrap().script_pubkey().as_bytes()),
                            address: change_address,
                            locator,
                        }),
                    },
                },
            };
            let plan = plan_transfer(&input).unwrap();
            input.metadata = finalize_spend_metadata(&input, &plan).unwrap();
            SignerInput::new(
                input,
                TransactionFee {
                    fee: plan.fee.into(),
                    gas_price_type: GasPriceType::regular(100),
                    gas_limit: 1.into(),
                    options: Default::default(),
                    fee_asset: Chain::BitcoinSv.as_asset_id(),
                },
            )
        }
    }

    #[test]
    fn test_keystore_signs_receive_and_change_inputs_from_one_mnemonic_root() {
        let context = Context::new();
        let input = context.input();
        let transactions = context
            .keystore
            .sign(context.id.clone(), Chain::BitcoinSv, input.clone().into(), TEST_PASSWORD.to_vec())
            .unwrap();
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].transaction_type, TransactionType::Transfer);
        let expected = BsvChainSigner
            .sign_transfer_with_keys(&input, |locator| bsv_private_key(ABANDON_PHRASE, locator).map_err(SignerError::from_display))
            .unwrap();
        assert_eq!(transactions[0].data, expected);
        assert!(BsvChainSigner::validate_signed_spend(&transactions[0].data, &input).is_ok());
        assert_ne!(
            bsv_address(
                &context.xpub,
                BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index: 0
                }
            )
            .unwrap(),
            bsv_address(
                &context.xpub,
                BsvAddressLocator {
                    branch: BsvAddressBranch::Change,
                    index: 0
                }
            )
            .unwrap()
        );
    }

    #[test]
    fn test_keystore_rejects_foreign_root_locator_sender_and_change() {
        let context = Context::new();
        for alteration in 0..5 {
            let mut input = context.input();
            if let TransactionLoadMetadata::BitcoinSv { spend } = &mut input.input.metadata {
                match alteration {
                    0 => spend.inputs[1].locator.index += 1,
                    1 => {
                        spend.inputs[0].address = FOREIGN_ADDRESS.to_string();
                        spend.inputs[0].script_pubkey = hex::encode(BsvAddress::from_str(FOREIGN_ADDRESS).unwrap().script_pubkey().as_bytes());
                    }
                    2 => {
                        let change = spend.change.as_mut().unwrap();
                        change.address = FOREIGN_ADDRESS.to_string();
                        change.script_pubkey = hex::encode(BsvAddress::from_str(FOREIGN_ADDRESS).unwrap().script_pubkey().as_bytes());
                    }
                    3 => spend.change.as_mut().unwrap().locator.branch = BsvAddressBranch::Receive,
                    4 => input.input.sender_address = FOREIGN_ADDRESS.to_string(),
                    _ => panic!(),
                }
            }
            assert!(
                context.keystore.sign(context.id.clone(), Chain::BitcoinSv, input.into(), TEST_PASSWORD.to_vec()).is_err(),
                "alteration {alteration}"
            );
        }
    }

    #[test]
    fn test_keystore_rejects_changed_script_fee_and_unfrozen_selection() {
        let context = Context::new();
        for alteration in 0..4 {
            let mut input = context.input();
            if let TransactionLoadMetadata::BitcoinSv { spend } = &mut input.input.metadata {
                match alteration {
                    0 => spend.inputs[0].script_pubkey = spend.inputs[1].script_pubkey.clone(),
                    1 => spend.change.as_mut().unwrap().script_pubkey = spend.inputs[0].script_pubkey.clone(),
                    2 => input.fee.fee += 1,
                    3 => {
                        let mut extra = spend.inputs[0].clone();
                        extra.transaction_id = format!("{:064}", 3);
                        extra.value = 70000;
                        spend.inputs.push(extra);
                    }
                    _ => panic!(),
                }
            }
            assert!(
                context.keystore.sign(context.id.clone(), Chain::BitcoinSv, input.into(), TEST_PASSWORD.to_vec()).is_err(),
                "alteration {alteration}"
            );
        }
    }
    #[test]
    fn test_keystore_signs_custom_recovery_standard_and_legacy_inputs() {
        for account_path in ["m", "m/44'/145'/2'", "m/44'/236'/1'/7"] {
            for legacy in [false, true] {
                let context = Context::from_recovery(account_path, legacy);
                let input = context.input();
                let signed = context
                    .keystore
                    .sign(context.id.clone(), Chain::BitcoinSv, input.clone().into(), TEST_PASSWORD.to_vec())
                    .unwrap();
                assert_eq!(signed.len(), 1);
                assert_eq!(signed[0].transaction_type, TransactionType::Transfer);
                assert!(BsvChainSigner::validate_signed_spend(&signed[0].data, &input).is_ok());
                assert!(
                    context
                        .keystore
                        .sign(context.id.clone(), Chain::BitcoinSv, input.clone().into(), b"wrong".to_vec())
                        .is_err()
                );
                let public_key = context.keystore.bsv_multisig_public_key(context.id.clone(), TEST_PASSWORD.to_vec()).unwrap();
                assert_eq!(context.keystore.bsv_multisig_member_address(public_key).unwrap(), input.input.sender_address);
            }
        }
    }
}
