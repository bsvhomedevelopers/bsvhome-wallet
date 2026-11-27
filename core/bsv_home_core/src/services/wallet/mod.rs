pub mod addresses;
pub mod error;
pub mod model;
mod multisig;
pub mod password;
pub mod rules;
pub mod store;
impl BsvHomeWalletService {
    fn migrate_wallet_password(&self, wallet: &Wallet, password: &str, shared: &str) -> Result<bool, BsvHomeServiceError> {
        let keystore_id = keystore_id_for_wallet(wallet.id.id());
        if !self.keystore.exists(keystore_id.clone()) {
            return Ok(false);
        }
        let shared_bytes = decode_password(shared);
        let rekeyed = password != shared && !self.keystore.opens_with(keystore_id.clone(), shared_bytes.clone());
        if rekeyed {
            self.keystore.change_password(keystore_id.clone(), decode_password(password), shared_bytes.clone())?;
            if !self.keystore.opens_with(keystore_id, shared_bytes) {
                return Err(BsvHomeServiceError::Core {
                    msg: "did not accept the shared password".to_string(),
                });
            }
        }
        self.password.delete_wallet_password(wallet.id.clone())?;
        Ok(rekeyed)
    }
}

#[cfg(test)]
mod retained_payer_tests;
#[cfg(test)]
pub(crate) mod testkit;

use std::sync::Arc;
use zeroize::Zeroizing;

use bsv_home_bsv::cold_wallet::{Pairing, decode_json};
use bsv_home_derivation::{derive_bsv_account_from_recovery, derive_wallet_id_from_account};
use bsv_home_keystore::Mnemonic;
use primitives::{Chain, NameRecord, Wallet, WalletId, WalletSource, WalletType};

use crate::keystore::decode_password;
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore, BsvHomeWalletImport, keystore_id_for_wallet};
use crate::services::chain::rules as chain_rules;
use crate::services::error::BsvHomeServiceError;
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::file::BsvHomeFileStore;
use crate::services::localization::{BsvHomeLocalizedText, BsvHomeLocalizer};
use crate::services::name::BsvHomeAddressStore;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::wallet_preferences::BsvHomeWalletPreferencesService;
use crate::services::wallet_session::BsvHomeWalletSessionService;
use addresses::BsvHomeBsvAddressPool;
use primitives::BlockExplorerLink;

pub use error::BsvHomeWalletImportError;
pub use model::{
    BsvHomeBsvHdImportMode, BsvHomeWalletDefaultName, BsvHomeWalletDeletion, BsvHomeWalletImportKind, BsvHomeWalletImportResult, BsvHomeWalletImportType, BsvHomeWalletSecret,
    BsvHomeWalletSecretKind,
};
pub use password::{BsvHomeKeystoreAuthentication, BsvHomeKeystorePassword};
pub use store::BsvHomeWalletStore;

const SETUP_CHAINS_WALLETS_LIMIT: usize = 25;

#[derive(Default)]
pub struct SetupChainsOutcome {
    pub wallets: Vec<Wallet>,
    pub failures: Vec<(WalletId, BsvHomeServiceError)>,
}

#[derive(uniffi::Object)]
pub struct BsvHomeWalletService {
    keystore: Arc<BsvHomeKeystore>,
    password: Arc<dyn BsvHomeKeystorePassword>,
    store: Arc<dyn BsvHomeWalletStore>,
    session: Arc<BsvHomeWalletSessionService>,
    app_preferences: Arc<BsvHomePreferencesService>,
    files: Arc<dyn BsvHomeFileStore>,
    preferences: Arc<BsvHomeWalletPreferencesService>,
    explorer: Arc<BsvHomeExplorerService>,
    addresses: Arc<dyn BsvHomeAddressStore>,
    localizer: Arc<dyn BsvHomeLocalizer>,
    bsv_addresses: Arc<BsvHomeBsvAddressPool>,
}

#[uniffi::export]
impl BsvHomeWalletService {
    #[uniffi::constructor]
    pub fn new(
        keystore: Arc<BsvHomeKeystore>,
        password: Arc<dyn BsvHomeKeystorePassword>,
        store: Arc<dyn BsvHomeWalletStore>,
        session: Arc<BsvHomeWalletSessionService>,
        app_preferences: Arc<BsvHomePreferencesService>,
        files: Arc<dyn BsvHomeFileStore>,
        preferences: Arc<BsvHomeWalletPreferencesService>,
        explorer: Arc<BsvHomeExplorerService>,
        addresses: Arc<dyn BsvHomeAddressStore>,
        localizer: Arc<dyn BsvHomeLocalizer>,
        bsv_addresses: Arc<BsvHomeBsvAddressPool>,
    ) -> Self {
        Self {
            keystore,
            password,
            store,
            session,
            app_preferences,
            files,
            preferences,
            explorer,
            addresses,
            localizer,
            bsv_addresses,
        }
    }

    pub fn current_wallet_id(&self) -> Result<Option<WalletId>, BsvHomeServiceError> {
        self.session.get_current_wallet_id()
    }

    pub fn set_current_wallet_id(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        self.session.set_current_wallet_id(Some(wallet_id))
    }

    pub fn address_url(&self, chain: Chain, address: String) -> BlockExplorerLink {
        self.explorer.get_address_url(chain, address)
    }

    pub fn create_wallet(&self) -> Result<Vec<String>, BsvHomeServiceError> {
        Mnemonic::generate(12).map_err(|error| BsvHomeServiceError::Core { msg: error.to_string() })
    }

    pub async fn default_wallet_name(&self, chain: Option<Chain>) -> Result<BsvHomeWalletDefaultName, BsvHomeServiceError> {
        let index = rules::next_wallet_index(&self.store.get_wallets().await?);
        let text = match chain {
            Some(chain) => BsvHomeLocalizedText::WalletDefaultNameChain { chain, index },
            None => BsvHomeLocalizedText::WalletDefaultName { index },
        };
        Ok(BsvHomeWalletDefaultName {
            name: self.localizer.text(text),
            has_existing_wallets: index > 1,
        })
    }

    pub fn import_kinds(&self, chain: Option<Chain>) -> Vec<BsvHomeWalletImportKind> {
        if let Some(chain) = chain {
            if !self.session.environment.allows_chain(chain) {
                return Vec::new();
            }
            if chain.is_testnet() {
                return vec![BsvHomeWalletImportKind::Phrase];
            }
        }
        rules::import_kinds(chain)
    }

    pub fn import_chains(&self, kind: BsvHomeWalletImportKind, query: String) -> Vec<Chain> {
        let chains = chain_rules::chains_by_rank()
            .into_iter()
            .chain(self.available_chains().into_iter().filter(|chain| chain.is_testnet()))
            .filter(|chain| self.import_kinds(Some(*chain)).contains(&kind))
            .collect();
        chain_rules::matching_chains(chains, &query)
    }

    pub fn bsv_hd_import_mode(&self, kind: BsvHomeWalletImportKind, chain: Option<Chain>, input: String, has_overrides: bool) -> Option<BsvHomeBsvHdImportMode> {
        let input = Zeroizing::new(input);
        rules::bsv_hd_import_mode(kind, chain, &input, has_overrides)
    }

    pub fn import_request(
        &self,
        kind: BsvHomeWalletImportKind,
        chain: Option<Chain>,
        input: String,
        name_record: Option<NameRecord>,
    ) -> Result<BsvHomeWalletImportType, BsvHomeServiceError> {
        if let Some(chain) = chain {
            self.session.environment.require_chain(chain)?;
            if !self.import_kinds(Some(chain)).contains(&kind) {
                return Err(BsvHomeServiceError::Unsupported {
                    msg: format!("wallet import method unavailable for {chain}"),
                });
            }
        }
        Ok(match rules::import_request(kind, chain, &input, name_record.as_ref())? {
            BsvHomeWalletImportType::MulticoinPhrase { words, .. } => BsvHomeWalletImportType::MulticoinPhrase {
                words,
                chains: self.available_chains(),
            },
            import => import,
        })
    }

    pub fn import_name(&self, name_record: Option<NameRecord>, default_name: String) -> String {
        rules::import_name(name_record.as_ref(), default_name)
    }

    pub async fn import_wallet(&self, name: String, import: BsvHomeWalletImportType, source: WalletSource) -> Result<BsvHomeWalletImportResult, BsvHomeServiceError> {
        let import = import.validated()?;
        let bsv_words = match &import {
            BsvHomeWalletImportType::MulticoinPhrase { words, .. } | BsvHomeWalletImportType::SinglePhrase { words, .. } => Some(Zeroizing::new(words.join(" "))),
            BsvHomeWalletImportType::PrivateKey { .. } | BsvHomeWalletImportType::Address { .. } => None,
        };
        let preview = self.preview_import(import.clone())?;
        let wallet_id = WalletId::from_id(&preview.wallet_id).ok_or_else(|| BsvHomeServiceError::Core {
            msg: format!("invalid wallet id {}", preview.wallet_id),
        })?;
        let wallets = self.store.get_wallets().await?;
        if let Some(mut wallet) = rules::existing_wallet(&wallets, &wallet_id, preview.wallet_type) {
            let missing = match preview.wallet_type {
                WalletType::Multicoin | WalletType::Single => preview
                    .accounts
                    .iter()
                    .filter(|account| wallet.account(account.chain).is_none())
                    .map(|account| account.chain)
                    .collect::<Vec<_>>(),
                WalletType::PrivateKey | WalletType::View => Vec::new(),
            };
            if !missing.is_empty() {
                let password = decode_password(&self.password.get_password(false)?);
                self.add_chains(&mut wallet, missing.clone(), password).await?;
                self.invalidate_subscriptions(&missing).await?;
            }
            if let Some(words) = &bsv_words {
                self.bsv_addresses.initialize_import(&wallet, words).await?;
            }
            return Ok(BsvHomeWalletImportResult::Existing {
                wallet: self.session.environment.visible_wallet(wallet),
            });
        }
        let index = rules::next_wallet_index(&wallets);
        let (wallet, stored_secret) = match import {
            BsvHomeWalletImportType::Address { address, chain } => (
                Wallet {
                    index,
                    ..rules::view_wallet(name, chain, address)
                },
                None,
            ),
            import => {
                let keystore_id = keystore_id_for_wallet(wallet_id.id());
                let is_new_secret = !self.keystore.exists(keystore_id.clone());
                let password = decode_password(&self.password.get_password(!self.keystore.has_stored_wallets()?)?);
                let stored = self.keystore.create_store(keystore_import(import), password)?;
                let wallet = Wallet {
                    id: wallet_id,
                    external_id: None,
                    name,
                    index,
                    wallet_type: stored.wallet_type,
                    accounts: preview.accounts.into_iter().map(rules::account).collect(),
                    is_pinned: false,
                    image_url: None,
                    source,
                };
                (wallet, is_new_secret.then_some(keystore_id))
            }
        };
        let saved = async {
            let bsv_addresses = self
                .bsv_addresses
                .import_record(&wallet, || {
                    bsv_words
                        .as_ref()
                        .map(|words| BsvHomeBsvAddressPool::initial_record(&wallet, words))
                        .transpose()
                        .map(Option::flatten)
                })
                .await?;
            self.store.add_wallet_with_bsv_addresses(wallet.clone(), bsv_addresses).await
        }
        .await;
        if let Err(error) = saved {
            if let Some(keystore_id) = stored_secret
                && let Err(cleanup_error) = self.keystore.delete(keystore_id)
            {
                return Err(BsvHomeServiceError::Core {
                    msg: format!("wallet initialization failed: {error}; secret cleanup failed: {cleanup_error}"),
                });
            }
            return Err(error);
        }
        self.addresses.save_address_names(rules::wallet_address_names(&wallet)).await?;
        if wallet.source == WalletSource::Create {
            self.preferences.complete_initial_synchronization(wallet.id.clone())?;
        }
        self.invalidate_subscriptions(&wallet.accounts.iter().map(|account| account.chain).collect::<Vec<_>>())
            .await?;
        Ok(BsvHomeWalletImportResult::New { wallet })
    }

    pub async fn bsv_hd_import(
        &self,
        name: String,
        recovery_string: String,
        account_path: Option<String>,
        passphrase: Option<String>,
        expected_address: Option<String>,
        legacy_non_compliant: bool,
    ) -> Result<BsvHomeWalletImportResult, BsvHomeServiceError> {
        self.session.environment.require_chain(Chain::BitcoinSv)?;
        let input = Zeroizing::new(recovery_string);
        let passphrase = passphrase.map(Zeroizing::new);
        let preview = self.keystore.bsv_hd_preview(
            input.to_string(),
            account_path.clone(),
            passphrase.as_ref().map(|value| value.to_string()),
            expected_address.clone(),
            legacy_non_compliant,
        )?;
        let pairing: Pairing = decode_json(&preview).map_err(|error| BsvHomeServiceError::InvalidInput { msg: error.to_string() })?;
        let account = pairing.account().map_err(|error| BsvHomeServiceError::InvalidInput { msg: error.to_string() })?;
        let wallet_id = derive_wallet_id_from_account(&account, WalletType::Single).map_err(|error| BsvHomeServiceError::InvalidInput { msg: error.to_string() })?;
        let wallets = self.store.get_wallets().await?;
        if let Some(existing) = rules::existing_wallet(&wallets, &wallet_id, WalletType::Single) {
            let password = Zeroizing::new(decode_password(&self.password.get_password(false)?));
            let existing_account = existing.account(Chain::BitcoinSv).ok_or_else(|| BsvHomeServiceError::InvalidInput {
                msg: "existing wallet has no BSV account".into(),
            })?;
            if existing_account.address != account.address || existing_account.derivation_path != account.derivation_path {
                return Err(BsvHomeServiceError::InvalidInput {
                    msg: "existing wallet does not match recovery profile".into(),
                });
            }
            let recovery = self.keystore.bsv_recovery(&keystore_id_for_wallet(existing.id.id()), &password)?;
            let actual = derive_bsv_account_from_recovery(&recovery).map_err(|error| BsvHomeServiceError::InvalidInput { msg: error.to_string() })?;
            if Pairing::from_account(&actual).map_err(|error| BsvHomeServiceError::InvalidInput { msg: error.to_string() })? != pairing {
                return Err(BsvHomeServiceError::InvalidInput {
                    msg: "existing wallet secret does not match recovery profile".into(),
                });
            }
            self.bsv_addresses.initialize_public_import(&existing, &account).await?;
            return Ok(BsvHomeWalletImportResult::Existing {
                wallet: self.session.environment.visible_wallet(existing),
            });
        }
        let keystore_id = keystore_id_for_wallet(wallet_id.id());
        let is_new_secret = !self.keystore.contains_stored_secret(&keystore_id)?;
        let password = Zeroizing::new(decode_password(&self.password.get_password(!self.keystore.has_stored_wallets()?)?));
        let stored = self.keystore.import_bsv_hd(
            &input,
            account_path.as_deref(),
            passphrase.as_ref().map(|value| value.as_str()),
            expected_address.as_deref(),
            legacy_non_compliant,
            &password,
        )?;
        let wallet = Wallet {
            id: wallet_id,
            external_id: None,
            name,
            index: rules::next_wallet_index(&wallets),
            wallet_type: WalletType::Single,
            accounts: stored.accounts.into_iter().map(rules::account).collect(),
            is_pinned: false,
            image_url: None,
            source: WalletSource::Import,
        };
        let saved = async {
            let addresses = self
                .bsv_addresses
                .import_record(&wallet, || BsvHomeBsvAddressPool::initial_public_record(&wallet, &account))
                .await?;
            self.store.add_wallet_with_bsv_addresses(wallet.clone(), addresses).await
        }
        .await;
        if let Err(error) = saved {
            if is_new_secret && let Err(cleanup_error) = self.keystore.delete(stored.keystore_id) {
                return Err(BsvHomeServiceError::Core {
                    msg: format!("wallet initialization failed: {error}; secret cleanup failed: {cleanup_error}"),
                });
            }
            return Err(error);
        }
        self.addresses.save_address_names(rules::wallet_address_names(&wallet)).await?;
        self.invalidate_subscriptions(&[Chain::BitcoinSv]).await?;
        Ok(BsvHomeWalletImportResult::New {
            wallet: self.session.environment.visible_wallet(wallet),
        })
    }

    pub async fn bsv_cold_wallet_import_pairing(&self, name: String, pairing_json: String) -> Result<BsvHomeWalletImportResult, BsvHomeServiceError> {
        self.session.environment.require_chain(Chain::BitcoinSv)?;
        let pairing: Pairing = decode_json(&pairing_json).map_err(|error| BsvHomeServiceError::InvalidInput { msg: error.to_string() })?;
        let account = pairing.account().map_err(|error| BsvHomeServiceError::InvalidInput { msg: error.to_string() })?;
        let wallets = self.store.get_wallets().await?;
        let mut wallet = rules::view_wallet(name, Chain::BitcoinSv, account.address.clone());
        wallet.accounts = vec![account.clone()];
        wallet.index = rules::next_wallet_index(&wallets);
        if let Some(existing) = rules::existing_wallet(&wallets, &wallet.id, WalletType::View) {
            if existing.accounts.len() == 1 && Pairing::from_account(&existing.accounts[0]).is_ok_and(|existing| existing == pairing) {
                self.bsv_addresses.initialize_public_import(&existing, &account).await?;
                return Ok(BsvHomeWalletImportResult::Existing {
                    wallet: self.session.environment.visible_wallet(existing),
                });
            }
            if existing
                .accounts
                .iter()
                .any(|account| account.extended_public_key.as_ref().is_some_and(|key| !key.is_empty()))
            {
                return Err(BsvHomeServiceError::InvalidInput {
                    msg: "existing observer has a different account configuration".into(),
                });
            }
            let upgraded = Wallet {
                accounts: wallet.accounts,
                ..existing
            };
            let addresses = self
                .bsv_addresses
                .import_record(&upgraded, || BsvHomeBsvAddressPool::initial_public_record(&upgraded, &account))
                .await?;
            self.store.add_wallet_with_bsv_addresses(upgraded.clone(), addresses).await?;
            self.addresses.save_address_names(rules::wallet_address_names(&upgraded)).await?;
            self.invalidate_subscriptions(&[Chain::BitcoinSv]).await?;
            return Ok(BsvHomeWalletImportResult::Existing {
                wallet: self.session.environment.visible_wallet(upgraded),
            });
        }
        let addresses = self
            .bsv_addresses
            .import_record(&wallet, || BsvHomeBsvAddressPool::initial_public_record(&wallet, &account))
            .await?;
        self.store.add_wallet_with_bsv_addresses(wallet.clone(), addresses).await?;
        self.addresses.save_address_names(rules::wallet_address_names(&wallet)).await?;
        self.invalidate_subscriptions(&[Chain::BitcoinSv]).await?;
        Ok(BsvHomeWalletImportResult::New {
            wallet: self.session.environment.visible_wallet(wallet),
        })
    }

    pub async fn delete_wallet(&self, wallet_id: WalletId) -> Result<BsvHomeWalletDeletion, BsvHomeServiceError> {
        let wallet = self.store.get_wallet(wallet_id.clone()).await?.ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: format!("wallet {} not found", wallet_id.id()),
        })?;
        if wallet.wallet_type != WalletType::View {
            self.keystore.delete_wallet_secrets(wallet.id.id(), rules::legacy_keystore_id(&wallet))?;
        }
        self.store.delete_wallet(wallet.id.clone()).await?;
        self.addresses.delete_address_names(rules::wallet_address_names(&wallet)).await?;
        if let Some(image_url) = wallet.image_url.clone() {
            self.files.remove(image_url)?;
        }
        self.preferences.delete_preferences(wallet.id.clone())?;
        let remaining = self.store.get_wallets().await?;
        if remaining.is_empty() || self.session.get_current_wallet_id()? == Some(wallet.id) {
            self.session.set_current_wallet_id(rules::next_current_wallet(&remaining))?;
        }
        self.invalidate_subscriptions(&wallet.accounts.iter().map(|account| account.chain).collect::<Vec<_>>())
            .await?;
        Ok(match remaining.is_empty() {
            true => BsvHomeWalletDeletion::LastWalletDeleted,
            false => BsvHomeWalletDeletion::WalletsRemaining,
        })
    }

    pub async fn bsv_hd_pairing(&self, wallet_id: WalletId) -> Result<String, BsvHomeServiceError> {
        let account = self.bsv_addresses.cold_account(wallet_id).await?;
        let pairing = Pairing::from_account(&account).map_err(|error| BsvHomeServiceError::InvalidInput { msg: error.to_string() })?;
        serde_json::to_string(&pairing).map_err(|error| BsvHomeServiceError::Core { msg: error.to_string() })
    }

    pub async fn secret_kind(&self, wallet_id: WalletId) -> Result<Option<BsvHomeWalletSecretKind>, BsvHomeServiceError> {
        let wallet = self.store.get_wallet(wallet_id.clone()).await?.ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: format!("wallet {} not found", wallet_id.id()),
        })?;
        Ok(self.keystore.wallet_secret_kind(wallet)?)
    }

    pub async fn export_secret(&self, wallet_id: WalletId) -> Result<BsvHomeWalletSecret, BsvHomeServiceError> {
        let wallet = self.store.get_wallet(wallet_id.clone()).await?.ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: format!("wallet {} not found", wallet_id.id()),
        })?;
        let keystore_id = keystore_id_for_wallet(wallet.id.id());
        let kind = self.keystore.wallet_secret_kind(wallet.clone())?.ok_or_else(|| BsvHomeServiceError::Core {
            msg: format!("wallet {} keeps no secret", wallet.id.id()),
        })?;
        let password = decode_password(&self.password.get_password(false)?);
        match kind {
            BsvHomeWalletSecretKind::Phrase => Ok(BsvHomeWalletSecret::Words {
                words: self.keystore.export_recovery_phrase(keystore_id, password)?,
            }),
            BsvHomeWalletSecretKind::Recovery => Ok(BsvHomeWalletSecret::Recovery {
                recovery: self.keystore.export_hd_recovery(keystore_id, password)?,
            }),
            BsvHomeWalletSecretKind::PrivateKey => {
                let chain = wallet.accounts.first().map(|account| account.chain).ok_or_else(|| BsvHomeServiceError::InvalidInput {
                    msg: "wallet has no signing account".into(),
                })?;
                Ok(BsvHomeWalletSecret::PrivateKey {
                    key: self.keystore.export_private_key(keystore_id, chain, password)?,
                })
            }
        }
    }

    pub async fn setup_chains(&self, chains: Vec<Chain>) -> Result<Vec<Wallet>, BsvHomeServiceError> {
        let SetupChainsOutcome { wallets, failures } = self.setup_chains_outcome(chains).await?;
        match failures.into_iter().next() {
            Some((_, error)) if wallets.is_empty() => Err(error),
            _ => Ok(wallets),
        }
    }

    pub async fn migrate_to_shared_password(&self) -> Result<u32, BsvHomeServiceError> {
        let legacy: Vec<(Wallet, String)> = self
            .store
            .get_wallets()
            .await?
            .into_iter()
            .filter(|wallet| wallet.wallet_type != WalletType::View)
            .filter_map(|wallet| match self.password.get_wallet_password(wallet.id.clone()) {
                Ok(Some(password)) => Some(Ok((wallet, password))),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<_, _>>()?;
        if legacy.is_empty() {
            return Ok(0);
        }
        let shared = self.password.get_password(true)?;
        let mut migrated = 0;
        let mut failures = Vec::new();
        for (wallet, password) in legacy {
            match self.migrate_wallet_password(&wallet, &password, &shared) {
                Ok(true) => migrated += 1,
                Ok(false) => {}
                Err(error) => failures.push(format!("keystore {}: {error}", wallet.id.id())),
            }
        }
        if failures.is_empty() {
            return Ok(migrated);
        }
        Err(BsvHomeServiceError::Core { msg: failures.join("; ") })
    }

    pub async fn set_pinned(&self, wallet_id: WalletId, pinned: bool) -> Result<(), BsvHomeServiceError> {
        self.store.set_pinned(wallet_id, pinned).await
    }

    pub async fn rename(&self, wallet_id: WalletId, name: String) -> Result<(), BsvHomeServiceError> {
        let wallet = self.store.get_wallet(wallet_id.clone()).await?.ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: format!("wallet {} not found", wallet_id.id()),
        })?;
        self.store.set_name(wallet_id, name.clone()).await?;
        self.addresses.save_address_names(rules::wallet_address_names(&Wallet { name, ..wallet })).await
    }

    pub fn sorted_wallets(&self, wallets: Vec<Wallet>) -> Vec<Wallet> {
        rules::sorted_wallets(
            wallets
                .into_iter()
                .filter(|wallet| self.session.environment.allows_wallet(wallet))
                .filter(|wallet| !self.is_multisig_identity(wallet.id.clone()))
                .map(|wallet| self.session.environment.visible_wallet(wallet))
                .collect(),
        )
    }

    pub async fn wallets(&self) -> Result<Vec<Wallet>, BsvHomeServiceError> {
        self.session.get_wallets().await
    }
}

impl BsvHomeWalletService {
    pub fn preview_import(&self, import: BsvHomeWalletImportType) -> Result<BsvHomeWalletImport, BsvHomeServiceError> {
        let import = import.validated()?;
        let chains = match &import {
            BsvHomeWalletImportType::MulticoinPhrase { chains, .. } => chains.clone(),
            BsvHomeWalletImportType::SinglePhrase { chain, .. } | BsvHomeWalletImportType::PrivateKey { chain, .. } | BsvHomeWalletImportType::Address { chain, .. } => {
                vec![*chain]
            }
        };
        for chain in chains {
            self.session.environment.require_chain(chain)?;
        }
        match import {
            BsvHomeWalletImportType::Address { address, chain } => {
                self.session.environment.require_chain(chain)?;
                if chain.is_testnet() {
                    return Err(BsvHomeServiceError::Unsupported {
                        msg: "test networks require a mnemonic wallet".into(),
                    });
                }
                let wallet = rules::view_wallet(String::new(), chain, address);
                Ok(BsvHomeWalletImport {
                    wallet_id: wallet.id.id(),
                    wallet_type: WalletType::View,
                    accounts: wallet.accounts.into_iter().map(Into::into).collect(),
                })
            }
            import => {
                let mut preview = self.keystore.preview_import(keystore_import(import.clone()))?;
                let words = match import {
                    BsvHomeWalletImportType::MulticoinPhrase { words, .. } | BsvHomeWalletImportType::SinglePhrase { words, .. } => Some(words),
                    BsvHomeWalletImportType::PrivateKey { .. } | BsvHomeWalletImportType::Address { .. } => None,
                };
                if let Some(words) = words {
                    let chains = [Chain::BitcoinSv, Chain::Tron]
                        .into_iter()
                        .chain(self.session.environment.available_chains().into_iter().filter(|chain| chain.is_testnet()))
                        .filter(|chain| self.session.environment.allows_chain(*chain))
                        .filter(|chain| !preview.accounts.iter().any(|account| account.chain == *chain))
                        .collect::<Vec<_>>();
                    if !chains.is_empty() {
                        preview
                            .accounts
                            .extend(self.keystore.preview_import(BsvHomeImportType::MulticoinPhrase { words, chains })?.accounts);
                    }
                }
                Ok(preview)
            }
        }
    }

    pub async fn setup_chains_outcome(&self, chains: Vec<Chain>) -> Result<SetupChainsOutcome, BsvHomeServiceError> {
        for chain in &chains {
            self.session.environment.require_chain(*chain)?;
        }
        let candidates: Vec<(Wallet, Vec<Chain>)> = rules::wallets_missing_chains(self.store.get_wallets().await?, &chains)
            .into_iter()
            .filter(|(wallet, _)| self.keystore.exists(keystore_id_for_wallet(wallet.id.id())))
            .take(SETUP_CHAINS_WALLETS_LIMIT)
            .collect();
        if candidates.is_empty() {
            return Ok(SetupChainsOutcome::default());
        }
        let password = decode_password(&self.password.get_password(false)?);
        let mut outcome = SetupChainsOutcome::default();
        let mut subscribed_chains = Vec::new();
        for (mut wallet, missing) in candidates {
            match self.add_chains(&mut wallet, missing.clone(), password.clone()).await {
                Ok(()) => {
                    subscribed_chains.extend(missing);
                    outcome.wallets.push(wallet);
                }
                Err(error) => outcome.failures.push((wallet.id, error)),
            }
        }
        if !outcome.wallets.is_empty() {
            self.invalidate_subscriptions(&subscribed_chains).await?;
        }
        Ok(outcome)
    }

    pub(crate) fn available_chains(&self) -> Vec<Chain> {
        self.session.environment.available_chains()
    }

    async fn add_chains(&self, wallet: &mut Wallet, missing: Vec<Chain>, password: Vec<u8>) -> Result<(), BsvHomeServiceError> {
        let accounts = self.keystore.add_accounts(keystore_id_for_wallet(wallet.id.id()), password, missing.clone())?;
        self.preferences.invalidate_initial_load(wallet.id.clone(), &missing)?;
        wallet.accounts.extend(accounts.into_iter().map(rules::account));
        self.store_wallet(wallet).await
    }

    async fn store_wallet(&self, wallet: &Wallet) -> Result<(), BsvHomeServiceError> {
        self.store.add_wallet(wallet.clone()).await?;
        self.addresses.save_address_names(rules::wallet_address_names(wallet)).await
    }

    async fn invalidate_subscriptions(&self, chains: &[Chain]) -> Result<(), BsvHomeServiceError> {
        if chains.iter().any(|chain| chain.supports_bsv_home_backend()) {
            let version = self.app_preferences.get_subscriptions_version().checked_add(1).ok_or_else(|| BsvHomeServiceError::Core {
                msg: "local BsvHome subscription version is exhausted".to_string(),
            })?;
            self.app_preferences.set_subscriptions_version(version)?;
        }
        Ok(())
    }
}

fn keystore_import(import: BsvHomeWalletImportType) -> BsvHomeImportType {
    match import {
        BsvHomeWalletImportType::MulticoinPhrase { words, chains } => BsvHomeImportType::MulticoinPhrase { words, chains },
        BsvHomeWalletImportType::SinglePhrase { words, chain } => BsvHomeImportType::SinglePhrase { words, chain },
        BsvHomeWalletImportType::PrivateKey { value, chain } => BsvHomeImportType::PrivateKey { value, chain },
        BsvHomeWalletImportType::Address { address, chain } => BsvHomeImportType::PrivateKey { value: address, chain },
    }
}

#[cfg(test)]
mod tests {
    mod multisig_identity;
    use std::fs;
    use std::path::PathBuf;

    use futures::executor::block_on;
    use primitives::{AddressType, Currency};
    use tempfile::TempDir;

    use super::addresses::testkit::pool_with_keystore;
    use super::testkit::{MemoryAddressStore, MemoryKeystorePassword, MemoryWalletStore, TEST_PASSWORD};
    use super::*;
    use crate::gateway::{BsvHomeGateway, EmptyPreferences};
    use crate::services::chain::BsvHomeRuntimeEnvironment;
    use crate::services::file::testkit::NoopFileStore;
    use crate::services::localization::testkit::EnglishLocalizer;
    use crate::services::preferences::testkit::MemoryPreferencesStore;
    use crate::services::wallet_preferences::{BsvHomeDiscoveryStep, testkit::MemoryWalletPreferencesStore};
    use crate::services::wallet_session::testkit::MemoryWalletSessionStore;
    use crate::testkit::TestAlienProvider;

    const PHRASE: [&str; 12] = [
        "shoot", "island", "position", "soft", "burden", "budget", "tooth", "cruel", "issue", "economy", "destroy", "above",
    ];
    const OTHER_PHRASE: [&str; 12] = [
        "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "abandon", "about",
    ];

    pub(super) struct TestContext {
        pub(super) service: BsvHomeWalletService,
        pub(super) wallets: Arc<MemoryWalletStore>,
        passwords: Arc<MemoryKeystorePassword>,
        addresses: Arc<MemoryAddressStore>,
        directory: TempDir,
    }

    impl TestContext {
        pub(super) fn new() -> Self {
            Self::new_with_environment(BsvHomeRuntimeEnvironment::Production)
        }

        pub(super) fn new_with_environment(environment: BsvHomeRuntimeEnvironment) -> Self {
            let directory = TempDir::new().unwrap();
            let wallets = Arc::new(MemoryWalletStore::default());
            let passwords = Arc::new(MemoryKeystorePassword::default());
            let addresses = Arc::new(MemoryAddressStore::default());
            let preferences = Arc::new(MemoryPreferencesStore::default());
            let keystore = BsvHomeKeystore::new_with_environment(directory.path().to_string_lossy().to_string(), environment).unwrap();
            let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
                Arc::new(MemoryWalletSessionStore::default()),
                wallets.clone(),
                environment,
            ));
            let app_preferences = Arc::new(BsvHomePreferencesService::new(preferences.clone()));
            let gateway = Arc::new(BsvHomeGateway::new_with_environment(
                Arc::new(TestAlienProvider::with_status(503)),
                preferences.clone(),
                Arc::new(EmptyPreferences),
                environment,
            ));
            let bsv_addresses = pool_with_keystore(gateway, wallets.clone(), keystore.clone(), passwords.clone(), wallets.bsv_addresses.clone());
            let service = BsvHomeWalletService::new(
                keystore,
                passwords.clone(),
                wallets.clone(),
                session,
                app_preferences.clone(),
                Arc::new(NoopFileStore),
                Arc::new(BsvHomeWalletPreferencesService::new(Arc::new(MemoryWalletPreferencesStore::default()))),
                Arc::new(BsvHomeExplorerService::new(app_preferences)),
                addresses.clone(),
                Arc::new(EnglishLocalizer),
                bsv_addresses,
            );
            Self {
                service,
                wallets,
                passwords,
                addresses,
                directory,
            }
        }

        async fn import(&self, name: &str, words: [&str; 12]) -> Wallet {
            let import = BsvHomeWalletImportType::MulticoinPhrase {
                words: words.iter().map(|word| word.to_string()).collect(),
                chains: vec![Chain::Ethereum],
            };
            match self.service.import_wallet(name.to_string(), import, WalletSource::Import).await.unwrap() {
                BsvHomeWalletImportResult::New { wallet } => wallet,
                BsvHomeWalletImportResult::Existing { wallet } => wallet,
            }
        }

        fn keystore_path(&self, wallet: &Wallet) -> PathBuf {
            self.directory.path().join(format!("{}.json", keystore_id_for_wallet(wallet.id.id())))
        }

        fn lock_out(&self, wallet: &Wallet) {
            let password = decode_password(&self.service.password.get_password(false).unwrap());
            self.service
                .keystore
                .change_password(keystore_id_for_wallet(wallet.id.id()), password, b"other".to_vec())
                .unwrap();
        }
    }

    #[test]
    fn test_wowpay_import_candidates_follow_supported_key_formats_and_mnemonic_defaults() {
        block_on(async {
            let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
            assert_eq!(context.service.import_chains(BsvHomeWalletImportKind::Phrase, String::new()), vec![Chain::BitcoinSv]);
            assert_eq!(context.service.import_chains(BsvHomeWalletImportKind::Address, String::new()), vec![Chain::BitcoinSv]);
            assert_eq!(context.service.import_chains(BsvHomeWalletImportKind::PrivateKey, String::new()), vec![Chain::BitcoinSv]);
            assert_eq!(context.service.import_chains(BsvHomeWalletImportKind::Phrase, "bsv".into()), vec![Chain::BitcoinSv]);
            assert_eq!(context.service.import_kinds(Some(Chain::Ethereum)), Vec::<BsvHomeWalletImportKind>::new());
            assert_eq!(context.service.import_kinds(Some(Chain::TronShasta)), Vec::<BsvHomeWalletImportKind>::new());
            let import = context.service.import_request(BsvHomeWalletImportKind::Phrase, None, PHRASE.join(" "), None).unwrap();
            if let BsvHomeWalletImportType::MulticoinPhrase { chains, .. } = &import {
                assert_eq!(*chains, vec![Chain::BitcoinSv]);
            } else {
                panic!("multicoin import required");
            }
            let expected_id = BsvHomeKeystore::new(context.directory.path().to_string_lossy().into_owned())
                .unwrap()
                .preview_import(BsvHomeImportType::MulticoinPhrase {
                    words: PHRASE.iter().map(|word| word.to_string()).collect(),
                    chains: vec![Chain::Ethereum],
                })
                .unwrap()
                .wallet_id;
            let preview = context.service.preview_import(import.clone()).unwrap();
            assert_eq!(preview.wallet_id, expected_id);
            assert_eq!(preview.accounts.iter().map(|account| account.chain).collect::<Vec<_>>(), vec![Chain::BitcoinSv]);
            let wallet = match context.service.import_wallet("Scoped mnemonic".into(), import, WalletSource::Create).await.unwrap() {
                BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
            };
            assert_eq!(wallet.id.id(), expected_id);
            assert_eq!(wallet.chains(), vec![Chain::BitcoinSv]);
            assert_eq!(context.service.wallets().await.unwrap().len(), 1);
            let import = context
                .service
                .import_request(BsvHomeWalletImportKind::PrivateKey, Some(Chain::BitcoinSv), hex::encode([1; 32]), None)
                .unwrap();
            let preview = context.service.preview_import(import.clone()).unwrap();
            assert_eq!(preview.wallet_type, WalletType::PrivateKey);
            assert_eq!(preview.accounts[0].address, "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD");
            let private = match context.service.import_wallet("BSV private key".into(), import, WalletSource::Import).await.unwrap() {
                BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
            };
            assert_eq!(private.chains(), vec![Chain::BitcoinSv]);
            assert_eq!(private.wallet_type, WalletType::PrivateKey);
            assert_eq!(context.service.wallets().await.unwrap().len(), 2);
        });
    }

    #[test]
    fn test_wowpay_private_and_watch_imports_accept_only_supported_mainnet_accounts() {
        block_on(async {
            let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
            let source = context
                .service
                .keystore
                .create_store(
                    BsvHomeImportType::SinglePhrase {
                        words: PHRASE.iter().map(|word| word.to_string()).collect(),
                        chain: Chain::BitcoinSv,
                    },
                    decode_password(TEST_PASSWORD),
                )
                .unwrap();
            let key = context
                .service
                .keystore
                .export_private_key(source.keystore_id, Chain::BitcoinSv, decode_password(TEST_PASSWORD))
                .unwrap();
            let import = context
                .service
                .import_request(BsvHomeWalletImportKind::PrivateKey, Some(Chain::BitcoinSv), key, None)
                .unwrap();
            let private = match context.service.import_wallet("Scoped private".into(), import, WalletSource::Import).await.unwrap() {
                BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
            };
            assert_eq!(private.wallet_type, WalletType::PrivateKey);
            assert_eq!(private.chains(), vec![Chain::BitcoinSv]);
            let preview = context
                .service
                .preview_import(BsvHomeWalletImportType::MulticoinPhrase {
                    words: PHRASE.iter().map(|word| word.to_string()).collect(),
                    chains: vec![Chain::BitcoinSv],
                })
                .unwrap();
            for account in preview.accounts {
                let import = context
                    .service
                    .import_request(BsvHomeWalletImportKind::Address, Some(account.chain), account.address, None)
                    .unwrap();
                let watch = match context.service.import_wallet("Scoped watch".into(), import, WalletSource::Import).await.unwrap() {
                    BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
                };
                assert_eq!(watch.wallet_type, WalletType::View);
                assert_eq!(watch.chains(), vec![account.chain]);
            }
            let auth_before = context.passwords.create_requests.lock().unwrap().clone();
            for chain in [Chain::Ethereum, Chain::Tron, Chain::TronShasta] {
                assert!(matches!(
                    context.service.preview_import(BsvHomeWalletImportType::MulticoinPhrase {
                        words: PHRASE.iter().map(|word| word.to_string()).collect(),
                        chains: vec![chain],
                    }),
                    Err(BsvHomeServiceError::Unsupported { .. })
                ));
                for kind in [BsvHomeWalletImportKind::PrivateKey, BsvHomeWalletImportKind::Address] {
                    assert!(context.service.import_request(kind, Some(chain), "invalid-input".into(), None).is_err());
                }
            }
            assert_eq!(*context.passwords.create_requests.lock().unwrap(), auth_before);
            assert_eq!(context.wallets.get_wallets().await.unwrap().len(), 2);
        });
    }

    #[test]
    fn test_wowpay_reimport_preserves_legacy_identity_accounts_and_encrypted_secret() {
        block_on(async {
            let context = TestContext::new_with_environment(BsvHomeRuntimeEnvironment::WowPay);
            let keystore = BsvHomeKeystore::new(context.directory.path().to_string_lossy().into_owned()).unwrap();
            let stored = keystore
                .create_store(
                    BsvHomeImportType::MulticoinPhrase {
                        words: PHRASE.iter().map(|word| word.to_string()).collect(),
                        chains: vec![Chain::Ethereum, Chain::Tron],
                    },
                    decode_password(TEST_PASSWORD),
                )
                .unwrap();
            let original = Wallet {
                id: WalletId::from_id(&stored.wallet_id).unwrap(),
                wallet_type: stored.wallet_type,
                accounts: stored.accounts.into_iter().map(rules::account).collect(),
                ..Wallet::mock()
            };
            context.wallets.add_wallet(original.clone()).await.unwrap();
            let before = fs::read(context.keystore_path(&original)).unwrap();
            let import = context.service.import_request(BsvHomeWalletImportKind::Phrase, None, PHRASE.join(" "), None).unwrap();
            let result = context.service.import_wallet("Duplicate".into(), import, WalletSource::Import).await.unwrap();
            let projected = match result {
                BsvHomeWalletImportResult::Existing { wallet } => wallet,
                BsvHomeWalletImportResult::New { .. } => panic!("existing wallet required"),
            };
            assert_eq!(projected.id, original.id);
            assert_eq!(projected.chains(), vec![Chain::BitcoinSv]);
            let raw = context.wallets.get_wallet(original.id).await.unwrap().unwrap();
            assert!(serde_json::to_value(&raw.accounts[0]).unwrap() == serde_json::to_value(&original.accounts[0]).unwrap());
            assert_eq!(raw.chains(), vec![Chain::Ethereum, Chain::Tron, Chain::BitcoinSv]);
            assert_eq!(context.wallets.get_wallets().await.unwrap().len(), 1);
            assert!(fs::read(context.keystore_path(&raw)).unwrap() == before);
        });
    }

    #[test]
    fn test_single_mnemonic_testnet_backfill_is_local_only_idempotent_and_preserves_the_wallet_and_secret() {
        block_on(async {
            for environment in [BsvHomeRuntimeEnvironment::Production, BsvHomeRuntimeEnvironment::LocalDevelopment] {
                let context = TestContext::new_with_environment(environment);
                let stored = context
                    .service
                    .keystore
                    .create_store(
                        BsvHomeImportType::SinglePhrase {
                            words: PHRASE.iter().map(|word| word.to_string()).collect(),
                            chain: Chain::Ethereum,
                        },
                        decode_password(TEST_PASSWORD),
                    )
                    .unwrap();
                let original = Wallet {
                    id: WalletId::from_id(&stored.wallet_id).unwrap(),
                    wallet_type: WalletType::Single,
                    accounts: stored.accounts.into_iter().map(rules::account).collect(),
                    ..Wallet::mock()
                };
                context.service.store_wallet(&original).await.unwrap();
                let secret_before = fs::read(context.keystore_path(&original)).unwrap();
                let reads_before = context.passwords.create_requests.lock().unwrap().len();
                let outcome = context.service.setup_chains_outcome(vec![Chain::TronShasta]).await;
                if environment.allows_chain(Chain::TronShasta) {
                    let outcome = outcome.unwrap();
                    assert_eq!(outcome.failures.len(), 0);
                    assert_eq!(outcome.wallets.len(), 1);
                    let wallet = &outcome.wallets[0];
                    assert_eq!(wallet.id, original.id);
                    assert_eq!(wallet.wallet_type, original.wallet_type);
                    assert_eq!(wallet.accounts[0].address, original.accounts[0].address);
                    assert_eq!(wallet.chains(), vec![Chain::Ethereum, Chain::TronShasta]);
                    let repeated = context.service.setup_chains_outcome(vec![Chain::TronShasta]).await.unwrap();
                    assert_eq!((repeated.wallets.len(), repeated.failures.len()), (0, 0));
                    assert_eq!(context.passwords.create_requests.lock().unwrap().len(), reads_before + 1);
                } else {
                    assert!(outcome.is_err());
                    assert_eq!(context.passwords.create_requests.lock().unwrap().len(), reads_before);
                    assert_eq!(context.wallets.get_wallet(original.id.clone()).await.unwrap().unwrap().chains(), original.chains());
                }
                assert_eq!(fs::read(context.keystore_path(&original)).unwrap(), secret_before);
            }
        });
    }

    #[test]
    fn test_phrase_import_includes_bsv_and_tron_without_changing_identity() {
        block_on(async {
            for import in [
                BsvHomeWalletImportType::SinglePhrase {
                    words: PHRASE.iter().map(|word| word.to_string()).collect(),
                    chain: Chain::Ethereum,
                },
                BsvHomeWalletImportType::MulticoinPhrase {
                    words: PHRASE.iter().map(|word| word.to_string()).collect(),
                    chains: vec![Chain::Ethereum],
                },
            ] {
                let context = TestContext::new();
                let original = context.service.keystore.preview_import(keystore_import(import.clone())).unwrap();
                for source in [WalletSource::Create, WalletSource::Import] {
                    let result = context.service.import_wallet("Wallet".into(), import.clone(), source).await.unwrap();
                    let wallet = match result {
                        BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
                    };
                    assert_eq!(wallet.id.id(), original.wallet_id);
                    assert_eq!(wallet.wallet_type, original.wallet_type);
                    assert_eq!(
                        wallet.accounts.iter().map(|account| account.chain).collect::<Vec<_>>(),
                        vec![Chain::Ethereum, Chain::BitcoinSv, Chain::Tron]
                    );
                    assert_eq!(wallet.accounts[0].address, original.accounts[0].address);
                    assert_eq!(wallet.accounts[0].derivation_path, original.accounts[0].derivation_path);
                    assert!(wallet.account(Chain::Tron).unwrap().address.starts_with('T'));
                    assert_eq!(wallet.account(Chain::Tron).unwrap().derivation_path, "m/44'/195'/0'/0/0");
                    for chain in [Chain::BitcoinSv, Chain::Tron] {
                        let key = context
                            .service
                            .keystore
                            .private_key(keystore_id_for_wallet(wallet.id.id()), chain, decode_password(TEST_PASSWORD))
                            .unwrap();
                        let account = bsv_home_derivation::derive_account_from_private_key(&key, chain).unwrap();
                        assert_eq!(wallet.account(chain).unwrap().address, account.address);
                    }
                }
                assert_eq!(context.service.wallets().await.unwrap().len(), 1);
            }
        });
    }

    #[test]
    fn test_reimport_only_adds_missing_accounts_and_preserves_the_secret() {
        block_on(async {
            for chain in [Chain::BitcoinSv, Chain::Tron] {
                let context = TestContext::new();
                let import = BsvHomeWalletImportType::SinglePhrase {
                    words: PHRASE.iter().map(|word| word.to_string()).collect(),
                    chain,
                };
                let stored = context
                    .service
                    .keystore
                    .create_store(keystore_import(import.clone()), decode_password(TEST_PASSWORD))
                    .unwrap();
                let original = Wallet {
                    id: WalletId::from_id(&stored.wallet_id).unwrap(),
                    wallet_type: stored.wallet_type,
                    name: "Existing".into(),
                    accounts: stored.accounts.into_iter().map(rules::account).collect(),
                    ..Wallet::mock()
                };
                context.service.store_wallet(&original).await.unwrap();
                context.service.preferences.complete_initial_synchronization(original.id.clone()).unwrap();
                context.service.preferences.set_transactions_timestamp(original.id.clone(), None, 42).unwrap();
                let secret_before = fs::read(context.keystore_path(&original)).unwrap();
                let subscriptions_before = context.service.app_preferences.get_subscriptions_version();
                let BsvHomeWalletImportResult::Existing { wallet } = context.service.import_wallet("Replacement".into(), import.clone(), WalletSource::Import).await.unwrap()
                else {
                    panic!("expected an existing wallet");
                };
                assert_eq!(wallet.accounts.len(), 2);
                assert_eq!(wallet.accounts[0].address, original.accounts[0].address);
                assert_eq!(wallet.accounts[0].derivation_path, original.accounts[0].derivation_path);
                assert_eq!(wallet.accounts[0].extended_public_key, original.accounts[0].extended_public_key);
                assert_eq!(wallet.name, original.name);
                assert_eq!(wallet.id, original.id);
                assert_eq!(wallet.index, original.index);
                assert!(wallet.account(Chain::Tron).is_some());
                assert!(wallet.account(Chain::BitcoinSv).is_some());
                assert_eq!(fs::read(context.keystore_path(&wallet)).unwrap(), secret_before);
                assert_eq!(
                    context.service.app_preferences.get_subscriptions_version(),
                    subscriptions_before + i32::from(chain == Chain::BitcoinSv)
                );
                assert!(
                    !context
                        .service
                        .preferences
                        .is_initial_load_completed(wallet.id.clone(), BsvHomeDiscoveryStep::Transactions)
                        .unwrap()
                );
                assert_eq!(
                    context.service.preferences.get_transactions_timestamp(wallet.id.clone(), None),
                    if chain == Chain::BitcoinSv { 0 } else { 42 }
                );
                assert_eq!(
                    context.service.preferences.is_wallet_configuration_completed(wallet.id.clone()).unwrap(),
                    chain == Chain::Tron
                );
                let password_reads = context.passwords.create_requests.lock().unwrap().len();
                let BsvHomeWalletImportResult::Existing { wallet: repeated } = context.service.import_wallet("Again".into(), import, WalletSource::Import).await.unwrap() else {
                    panic!("expected an existing wallet");
                };
                assert_eq!(repeated.id, wallet.id);
                assert_eq!(repeated.accounts.len(), wallet.accounts.len());
                assert_eq!(context.passwords.create_requests.lock().unwrap().len(), password_reads);
                assert_eq!(
                    context.service.app_preferences.get_subscriptions_version(),
                    subscriptions_before + i32::from(chain == Chain::BitcoinSv)
                );
            }
        });
    }

    #[test]
    fn test_wallet_import_preserves_validation_errors() {
        block_on(async {
            let context = TestContext::new();
            let invalid_imports = [
                (
                    BsvHomeWalletImportType::MulticoinPhrase {
                        words: vec![],
                        chains: vec![Chain::Ethereum],
                    },
                    BsvHomeWalletImportError::InvalidSecretPhrase,
                ),
                (
                    BsvHomeWalletImportType::SinglePhrase {
                        words: vec!["not-a-mnemonic-word".into()],
                        chain: Chain::Ethereum,
                    },
                    BsvHomeWalletImportError::InvalidSecretPhraseWords {
                        words: vec!["not-a-mnemonic-word".into()],
                    },
                ),
                (
                    BsvHomeWalletImportType::PrivateKey {
                        value: "invalid-key".into(),
                        chain: Chain::Ethereum,
                    },
                    BsvHomeWalletImportError::InvalidPrivateKey,
                ),
                (
                    BsvHomeWalletImportType::Address {
                        address: "invalid-address".into(),
                        chain: Chain::Ethereum,
                    },
                    BsvHomeWalletImportError::InvalidAddress,
                ),
            ];
            for (import, error) in invalid_imports {
                let expected = BsvHomeServiceError::WalletImport { error };
                assert_eq!(context.service.preview_import(import.clone()).unwrap_err(), expected);
                assert_eq!(context.service.import_wallet("Invalid".into(), import, WalletSource::Import).await.unwrap_err(), expected);
            }
            assert_eq!(
                context
                    .service
                    .import_request(BsvHomeWalletImportKind::PrivateKey, None, "invalid-key".into(), None)
                    .unwrap_err(),
                BsvHomeServiceError::WalletImport {
                    error: BsvHomeWalletImportError::MissingChain
                },
            );
            assert_eq!(context.wallets.get_wallets().await.unwrap().len(), 0);
            assert_eq!(context.passwords.create_requests.lock().unwrap().len(), 0);
        });
    }

    #[test]
    fn test_delete_wallet_removes_secret_copies_preserves_preferences_and_reports_outcome() {
        block_on(async {
            let context = TestContext::new();
            let kept = context.import("Kept", OTHER_PHRASE).await;
            let deleted = context.import("Deleted", PHRASE).await;
            let legacy_path = context.directory.path().join(rules::legacy_keystore_id(&deleted));
            fs::write(&legacy_path, "{}").unwrap();
            context.service.session.set_current_wallet_id(Some(deleted.id.clone())).unwrap();

            let outcome = context.service.delete_wallet(deleted.id.clone()).await.unwrap();

            assert_eq!(outcome, BsvHomeWalletDeletion::WalletsRemaining);
            assert!(!context.keystore_path(&deleted).exists());
            assert!(!legacy_path.exists());
            assert!(context.keystore_path(&kept).exists());
            assert_eq!(context.service.session.get_current_wallet_id().unwrap(), Some(kept.id.clone()));

            context.service.app_preferences.set_currency(Currency::EUR).unwrap();

            let outcome = context.service.delete_wallet(kept.id.clone()).await.unwrap();

            assert_eq!(outcome, BsvHomeWalletDeletion::LastWalletDeleted);
            assert!(!context.keystore_path(&kept).exists());
            assert_eq!(context.service.session.get_current_wallet_id().unwrap(), None);
            assert_eq!(context.service.app_preferences.get_currency(), Currency::EUR);
        });
    }

    #[test]
    fn test_wallet_accounts_are_named_after_the_wallet_until_it_is_deleted() {
        block_on(async {
            let context = TestContext::new();
            let wallet = context.import("Savings", PHRASE).await;
            let account = wallet.accounts[0].clone();
            let name = async |context: &TestContext| context.addresses.get_address_name(account.chain, account.address.clone()).await.unwrap();

            let stored = name(&context).await.unwrap();
            assert_eq!((stored.name.as_str(), stored.address_type), ("Savings", AddressType::InternalWallet));

            context.service.rename(wallet.id.clone(), "Spending".to_string()).await.unwrap();
            assert_eq!(name(&context).await.unwrap().name, "Spending");

            context.service.delete_wallet(wallet.id.clone()).await.unwrap();
            assert!(name(&context).await.is_none());
        });
    }

    #[test]
    fn test_reimport_backfill_failure_preserves_existing_wallet_and_can_retry() {
        block_on(async {
            let context = TestContext::new();
            let mut original = context.import("Existing", PHRASE).await;
            original.accounts.retain(|account| account.chain == Chain::Ethereum);
            context.service.store_wallet(&original).await.unwrap();
            let secret_before = fs::read(context.keystore_path(&original)).unwrap();
            let import = BsvHomeWalletImportType::MulticoinPhrase {
                words: PHRASE.iter().map(|word| word.to_string()).collect(),
                chains: vec![Chain::Ethereum],
            };
            *context.wallets.add_wallet_error.lock().unwrap() = Some(BsvHomeServiceError::Store { msg: "disk full".into() });
            assert!(context.service.import_wallet("Replacement".into(), import.clone(), WalletSource::Import).await.is_err());
            assert_eq!(context.service.wallets().await.unwrap()[0].accounts.len(), 1);
            assert_eq!(fs::read(context.keystore_path(&original)).unwrap(), secret_before);
            *context.wallets.add_wallet_error.lock().unwrap() = None;
            let BsvHomeWalletImportResult::Existing { wallet } = context.service.import_wallet("Replacement".into(), import, WalletSource::Import).await.unwrap() else {
                panic!("expected an existing wallet");
            };
            assert_eq!(wallet.accounts.len(), 3);
        });
    }

    #[test]
    fn test_reimport_with_missing_accounts_surfaces_keystore_authentication_failure() {
        block_on(async {
            let context = TestContext::new();
            let mut wallet = context.import("Existing", PHRASE).await;
            wallet.accounts.retain(|account| account.chain == Chain::Ethereum);
            context.service.store_wallet(&wallet).await.unwrap();
            context.lock_out(&wallet);
            let secret_before = fs::read(context.keystore_path(&wallet)).unwrap();
            let import = BsvHomeWalletImportType::MulticoinPhrase {
                words: PHRASE.iter().map(|word| word.to_string()).collect(),
                chains: vec![Chain::Ethereum],
            };
            assert!(context.service.import_wallet("Replacement".into(), import, WalletSource::Import).await.is_err());
            assert_eq!(context.service.wallets().await.unwrap()[0].accounts.len(), 1);
            assert_eq!(fs::read(context.keystore_path(&wallet)).unwrap(), secret_before);
        });
    }

    #[test]
    fn test_import_creates_the_keystore_password_only_while_the_keystore_is_empty() {
        block_on(async {
            let context = TestContext::new();
            context.import("First", PHRASE).await;
            context.import("Second", OTHER_PHRASE).await;

            assert_eq!(*context.passwords.create_requests.lock().unwrap(), vec![true, false]);
        });
    }

    #[test]
    fn test_a_failed_wallet_write_removes_the_secret_it_stored() {
        block_on(async {
            let context = TestContext::new();
            let import = BsvHomeWalletImportType::MulticoinPhrase {
                words: PHRASE.iter().map(|word| word.to_string()).collect(),
                chains: vec![Chain::Ethereum],
            };
            *context.wallets.add_wallet_error.lock().unwrap() = Some(BsvHomeServiceError::Store { msg: "disk full".to_string() });

            let error = context.service.import_wallet("First".to_string(), import.clone(), WalletSource::Create).await.unwrap_err();

            assert!(matches!(error, BsvHomeServiceError::Store { .. }));
            assert!(!context.service.keystore.has_stored_wallets().unwrap());
            assert!(context.wallets.get_wallets().await.unwrap().is_empty());

            *context.wallets.add_wallet_error.lock().unwrap() = None;
            let result = context.service.import_wallet("First".to_string(), import, WalletSource::Create).await.unwrap();
            assert!(matches!(result, BsvHomeWalletImportResult::New { .. }));
        });
    }

    #[test]
    fn test_export_secret_follows_the_wallet_type() {
        block_on(async {
            let context = TestContext::new();
            let phrase = context.import("Phrase", PHRASE).await;
            let key = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318".to_string();
            let import = BsvHomeWalletImportType::PrivateKey {
                value: key.clone(),
                chain: Chain::Ethereum,
            };
            let BsvHomeWalletImportResult::New { wallet: private } = context.service.import_wallet("Key".to_string(), import, WalletSource::Import).await.unwrap() else {
                panic!("expected a new wallet");
            };
            assert_eq!(private.accounts.len(), 1);
            let view = rules::view_wallet("View".to_string(), Chain::Ethereum, private.accounts[0].address.clone());
            context.service.store.add_wallet(view.clone()).await.unwrap();

            assert_eq!(
                context.service.export_secret(phrase.id.clone()).await.unwrap(),
                BsvHomeWalletSecret::Words {
                    words: PHRASE.iter().map(|word| word.to_string()).collect()
                }
            );
            assert_eq!(context.service.export_secret(private.id.clone()).await.unwrap(), BsvHomeWalletSecret::PrivateKey { key });
            assert!(context.service.export_secret(view.id.clone()).await.is_err());
        });
    }

    #[test]
    fn test_setup_chains_keeps_going_when_one_keystore_cannot_be_read() {
        block_on(async {
            let context = TestContext::new();
            let broken = context.import("Broken", PHRASE).await;
            let healthy = context.import("Healthy", OTHER_PHRASE).await;
            context.lock_out(&broken);

            let outcome = context.service.setup_chains_outcome(vec![Chain::Ethereum, Chain::Solana]).await.unwrap();

            assert_eq!(outcome.failures.len(), 1);
            assert_eq!(outcome.failures[0].0, broken.id);
            assert_eq!(outcome.wallets.len(), 1, "the healthy wallet must still be set up");
            assert_eq!(outcome.wallets[0].id, healthy.id);
            let stored = context.service.wallets().await.unwrap();
            let stored_healthy = stored.iter().find(|wallet| wallet.id == healthy.id).unwrap();
            assert!(stored_healthy.accounts.iter().any(|account| account.chain == Chain::Solana));
        });
    }

    #[test]
    fn test_setup_chains_reports_the_error_when_no_wallet_could_be_set_up() {
        block_on(async {
            let context = TestContext::new();
            let only = context.import("Only", PHRASE).await;
            context.lock_out(&only);

            let error = context.service.setup_chains(vec![Chain::Ethereum, Chain::Solana]).await;

            assert!(error.is_err(), "a setup that added nothing must surface the failure");
        });
    }

    #[test]
    fn test_every_wallet_change_bumps_the_subscriptions_version() {
        block_on(async {
            let context = TestContext::new();
            context.service.app_preferences.set_subscriptions_version(4).unwrap();

            let wallet = context.import("Imported", PHRASE).await;
            assert_eq!(context.service.app_preferences.get_subscriptions_version(), 5, "import must bump");

            context.service.setup_chains(vec![Chain::Ethereum, Chain::Solana]).await.unwrap();
            assert_eq!(context.service.app_preferences.get_subscriptions_version(), 6, "adding a chain must bump");

            let before = context.service.app_preferences.get_subscriptions_version();
            context.service.setup_chains(vec![Chain::Ethereum, Chain::Solana]).await.unwrap();
            assert_eq!(
                context.service.app_preferences.get_subscriptions_version(),
                before,
                "a setup that adds no chain must not bump"
            );

            let second = context.import("Second", OTHER_PHRASE).await;
            let before = context.service.app_preferences.get_subscriptions_version();
            context.service.delete_wallet(second.id.clone()).await.unwrap();
            assert_eq!(
                context.service.app_preferences.get_subscriptions_version(),
                before + 1,
                "deleting one of several wallets must bump"
            );

            let before = context.service.app_preferences.get_subscriptions_version();
            context.service.delete_wallet(wallet.id.clone()).await.unwrap();
            assert_eq!(
                context.service.app_preferences.get_subscriptions_version(),
                before + 1,
                "deleting the last wallet must bump without resetting app preferences"
            );
        });
    }

    #[test]
    fn test_local_chain_wallet_import_and_deletion_do_not_invalidate_bsv_home_subscriptions() {
        block_on(async {
            let context = TestContext::new();
            context.service.app_preferences.set_subscriptions_version(4).unwrap();
            let imported = context
                .service
                .import_wallet(
                    "BSV".into(),
                    BsvHomeWalletImportType::Address {
                        address: "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".into(),
                        chain: Chain::BitcoinSv,
                    },
                    WalletSource::Import,
                )
                .await
                .unwrap();
            let wallet = match imported {
                BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
            };
            assert_eq!(context.service.app_preferences.get_subscriptions_version(), 4);
            context.service.delete_wallet(wallet.id).await.unwrap();
            assert_eq!(context.service.app_preferences.get_subscriptions_version(), 4);
        });
    }

    #[test]
    fn test_migration_rekeys_a_legacy_wallet_and_can_be_run_again() {
        block_on(async {
            let context = TestContext::new();
            let wallet = context.import("Legacy", PHRASE).await;
            let keystore_id = keystore_id_for_wallet(wallet.id.id());
            let legacy = "0f0e0d0c0b0a09080706050403020100f0e0d0c0b0a090807060504030201000";
            context
                .service
                .keystore
                .change_password(keystore_id.clone(), decode_password(TEST_PASSWORD), decode_password(legacy))
                .unwrap();
            context.passwords.wallet_passwords.lock().unwrap().insert(wallet.id.id(), legacy.to_string());

            assert_eq!(context.service.migrate_to_shared_password().await.unwrap(), 1);
            assert!(context.service.keystore.opens_with(keystore_id.clone(), decode_password(TEST_PASSWORD)));
            assert!(context.passwords.wallet_passwords.lock().unwrap().is_empty());

            assert_eq!(context.service.migrate_to_shared_password().await.unwrap(), 0);
            assert!(context.service.keystore.opens_with(keystore_id, decode_password(TEST_PASSWORD)));
        });
    }

    #[test]
    fn test_migration_drops_an_alias_that_already_holds_the_shared_password() {
        block_on(async {
            let context = TestContext::new();
            let wallet = context.import("Aliased", PHRASE).await;
            context.passwords.wallet_passwords.lock().unwrap().insert(wallet.id.id(), TEST_PASSWORD.to_string());

            assert_eq!(context.service.migrate_to_shared_password().await.unwrap(), 0);
            assert!(context.passwords.wallet_passwords.lock().unwrap().is_empty());
            assert!(context.service.keystore.opens_with(keystore_id_for_wallet(wallet.id.id()), decode_password(TEST_PASSWORD)));
        });
    }

    #[test]
    fn test_migration_keeps_the_legacy_password_when_rekeying_fails() {
        block_on(async {
            let context = TestContext::new();
            let wallet = context.import("Unmigratable", PHRASE).await;
            let keystore_id = keystore_id_for_wallet(wallet.id.id());
            let actual = "0f0e0d0c0b0a09080706050403020100f0e0d0c0b0a090807060504030201000";
            let wrong = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
            context
                .service
                .keystore
                .change_password(keystore_id.clone(), decode_password(TEST_PASSWORD), decode_password(actual))
                .unwrap();
            context.passwords.wallet_passwords.lock().unwrap().insert(wallet.id.id(), wrong.to_string());
            let other = context.import("Migratable", OTHER_PHRASE).await;
            let other_keystore_id = keystore_id_for_wallet(other.id.id());
            context
                .service
                .keystore
                .change_password(other_keystore_id.clone(), decode_password(TEST_PASSWORD), decode_password(actual))
                .unwrap();
            context.passwords.wallet_passwords.lock().unwrap().insert(other.id.id(), actual.to_string());

            assert!(context.service.migrate_to_shared_password().await.is_err());
            assert_eq!(context.passwords.wallet_passwords.lock().unwrap().get(&wallet.id.id()).map(String::as_str), Some(wrong));
            assert!(context.service.keystore.opens_with(keystore_id, decode_password(actual)));
            assert!(
                context.service.keystore.opens_with(other_keystore_id, decode_password(TEST_PASSWORD)),
                "one wallet that cannot be re-keyed does not stop the others from migrating"
            );
            assert_eq!(context.passwords.wallet_passwords.lock().unwrap().get(&other.id.id()), None);
        });
    }

    #[test]
    fn test_migration_keeps_the_password_of_a_wallet_with_no_v4_keystore() {
        block_on(async {
            let context = TestContext::new();
            let wallet = context.import("PendingV3", PHRASE).await;
            let keystore_id = keystore_id_for_wallet(wallet.id.id());
            let legacy = "0f0e0d0c0b0a09080706050403020100f0e0d0c0b0a090807060504030201000";
            context.passwords.wallet_passwords.lock().unwrap().insert(wallet.id.id(), legacy.to_string());
            context.service.keystore.delete(keystore_id.clone()).unwrap();

            assert_eq!(context.service.migrate_to_shared_password().await.unwrap(), 0);
            assert_eq!(context.passwords.wallet_passwords.lock().unwrap().get(&wallet.id.id()).map(String::as_str), Some(legacy));
        });
    }

    #[test]
    fn test_migration_clears_a_legacy_entry_left_by_an_interrupted_run() {
        block_on(async {
            let context = TestContext::new();
            let wallet = context.import("Interrupted", PHRASE).await;
            let stale = "0f0e0d0c0b0a09080706050403020100f0e0d0c0b0a090807060504030201000";
            context.passwords.wallet_passwords.lock().unwrap().insert(wallet.id.id(), stale.to_string());

            assert_eq!(context.service.migrate_to_shared_password().await.unwrap(), 0);
            assert!(context.passwords.wallet_passwords.lock().unwrap().is_empty());
            assert!(context.service.keystore.opens_with(keystore_id_for_wallet(wallet.id.id()), decode_password(TEST_PASSWORD)));
        });
    }

    #[test]
    fn test_delete_wallet_keeps_the_record_when_a_secret_copy_survives() {
        block_on(async {
            let context = TestContext::new();
            let wallet = context.import("Blocked", PHRASE).await;
            fs::remove_file(context.keystore_path(&wallet)).unwrap();
            fs::create_dir(context.keystore_path(&wallet)).unwrap();

            let error = context.service.delete_wallet(wallet.id.clone()).await.unwrap_err();

            assert!(matches!(error, BsvHomeServiceError::Core { .. }), "{error:?}");
            assert_eq!(context.service.wallets().await.unwrap().len(), 1);
        });
    }
    #[test]
    fn test_hd_import_preserves_existing_default_account_and_mnemonic_secret() {
        block_on(async {
            let context = TestContext::new();
            let result = context
                .service
                .import_wallet(
                    "Original".into(),
                    BsvHomeWalletImportType::SinglePhrase {
                        words: PHRASE.iter().map(|word| word.to_string()).collect(),
                        chain: Chain::BitcoinSv,
                    },
                    WalletSource::Import,
                )
                .await
                .unwrap();
            let original = match result {
                BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
            };
            let secret = fs::read(context.keystore_path(&original)).unwrap();
            let result = context
                .service
                .bsv_hd_import("Replacement".into(), PHRASE.join(" "), None, None, Some(original.accounts[0].address.clone()), false)
                .await
                .unwrap();
            let restored = match result {
                BsvHomeWalletImportResult::Existing { wallet } => wallet,
                BsvHomeWalletImportResult::New { .. } => panic!("existing wallet required"),
            };
            assert_eq!(restored.id, original.id);
            assert_eq!(restored.name, original.name);
            assert_eq!(restored.accounts, original.accounts);
            let preview = context.service.keystore.bsv_hd_preview(PHRASE.join(" "), None, None, None, false).unwrap();
            assert_eq!(context.service.bsv_hd_pairing(restored.id.clone()).await.unwrap(), preview);
            assert_eq!(fs::read(context.keystore_path(&restored)).unwrap(), secret);
            assert_eq!(context.service.secret_kind(restored.id.clone()).await.unwrap(), Some(BsvHomeWalletSecretKind::Phrase));
            assert_eq!(context.service.bsv_addresses.snapshot(restored.id).await.unwrap().unwrap().records.len(), 10);
        });
    }

    #[test]
    fn test_hd_import_backup_repeat_and_secret_metadata_errors() {
        block_on(async {
            let context = TestContext::new();
            let result = context
                .service
                .bsv_hd_import("Custom".into(), PHRASE.join(" "), Some("m/44h/145H/2'/7".into()), Some(" public:Pass ".into()), None, true)
                .await
                .unwrap();
            let wallet = match result {
                BsvHomeWalletImportResult::New { wallet } => wallet,
                BsvHomeWalletImportResult::Existing { .. } => panic!("new wallet required"),
            };
            assert_eq!(wallet.accounts[0].derivation_path, "m/44'/145'/2'/7/0/0");
            assert_eq!(context.service.secret_kind(wallet.id.clone()).await.unwrap(), Some(BsvHomeWalletSecretKind::Recovery));
            let backup = match context.service.export_secret(wallet.id.clone()).await.unwrap() {
                BsvHomeWalletSecret::Recovery { recovery } => recovery,
                _ => panic!("full recovery required"),
            };
            let preview = context
                .service
                .keystore
                .bsv_hd_preview(backup.clone(), None, None, Some(wallet.accounts[0].address.clone()), false)
                .unwrap();
            assert_eq!(decode_json::<Pairing>(&preview).unwrap().account().unwrap(), wallet.accounts[0]);
            let bytes = fs::read(context.keystore_path(&wallet)).unwrap();
            let result = context.service.bsv_hd_import("Again".into(), backup.clone(), None, None, None, false).await.unwrap();
            let repeated = match result {
                BsvHomeWalletImportResult::Existing { wallet } => wallet,
                BsvHomeWalletImportResult::New { .. } => panic!("existing wallet required"),
            };
            assert_eq!(repeated.id, wallet.id);
            assert_eq!(fs::read(context.keystore_path(&wallet)).unwrap(), bytes);
            assert!(
                context
                    .service
                    .bsv_hd_import("Conflict".into(), backup, Some("m/44'/236'/0'".into()), None, None, false)
                    .await
                    .is_err()
            );
            fs::write(context.keystore_path(&wallet), b"corrupt").unwrap();
            assert!(context.service.secret_kind(wallet.id.clone()).await.is_err());
            assert!(context.service.export_secret(wallet.id.clone()).await.is_err());
            assert_eq!(context.service.wallets().await.unwrap().len(), 1);
        });
    }

    #[test]
    fn test_hd_import_creates_password_only_for_empty_keystore() {
        block_on(async {
            let context = TestContext::new();
            for path in ["m/44'/236'/0'", "m/44'/145'/0'", "m/44'/236'/0'"] {
                context
                    .service
                    .bsv_hd_import("HD".into(), PHRASE.join(" "), Some(path.into()), None, None, false)
                    .await
                    .unwrap();
            }
            assert_eq!(*context.passwords.create_requests.lock().unwrap(), vec![true, false, false]);
        });
    }

    #[test]
    fn test_pairing_import_upgrades_observer_without_a_secret_and_rejects_conflicts() {
        block_on(async {
            let context = TestContext::new();
            let pairing_json = context
                .service
                .keystore
                .bsv_hd_preview(PHRASE.join(" "), Some("m/44'/145'/0'".into()), None, None, false)
                .unwrap();
            let pairing: Pairing = decode_json(&pairing_json).unwrap();
            let original = rules::view_wallet("Observed".into(), Chain::BitcoinSv, pairing.first_address.clone());
            context.wallets.add_wallet(original.clone()).await.unwrap();
            let result = context.service.bsv_cold_wallet_import_pairing("Replacement".into(), pairing_json.clone()).await.unwrap();
            let upgraded = match result {
                BsvHomeWalletImportResult::Existing { wallet } => wallet,
                BsvHomeWalletImportResult::New { .. } => panic!("existing wallet required"),
            };
            assert_eq!(upgraded.id, original.id);
            assert_eq!(upgraded.name, original.name);
            assert_eq!(upgraded.accounts, vec![pairing.account().unwrap()]);
            assert_eq!(context.service.secret_kind(upgraded.id.clone()).await.unwrap(), None);
            assert!(context.service.export_secret(upgraded.id.clone()).await.is_err());
            assert_eq!(context.service.bsv_addresses.snapshot(upgraded.id.clone()).await.unwrap().unwrap().records.len(), 10);
            assert_eq!(fs::read_dir(context.directory.path()).unwrap().count(), 0);
            let mut conflicting = upgraded.clone();
            conflicting.accounts[0].extended_public_key = Some("different".into());
            context.wallets.add_wallet(conflicting).await.unwrap();
            assert!(context.service.bsv_cold_wallet_import_pairing("Again".into(), pairing_json).await.is_err());
            assert_eq!(
                context.wallets.get_wallet(upgraded.id).await.unwrap().unwrap().accounts[0].extended_public_key.as_deref(),
                Some("different")
            );
        });
    }

    #[test]
    fn test_hd_import_store_failure_cleans_new_secret_and_preserves_existing_secret() {
        block_on(async {
            let context = TestContext::new();
            *context.wallets.add_wallet_error.lock().unwrap() = Some(BsvHomeServiceError::Store { msg: "unavailable".into() });
            assert!(
                context
                    .service
                    .bsv_hd_import("Failed".into(), PHRASE.join(" "), Some("m/44'/145'/0'".into()), None, None, false)
                    .await
                    .is_err()
            );
            assert_eq!(fs::read_dir(context.directory.path()).unwrap().count(), 0);
            assert_eq!(context.wallets.get_wallets().await.unwrap().len(), 0);
            assert_eq!(context.wallets.bsv_addresses.states.lock().unwrap().len(), 0);
            let password = decode_password(&context.passwords.get_password(false).unwrap());
            let stored = context
                .service
                .keystore
                .import_bsv_hd(&PHRASE.join(" "), Some("m/44'/145'/0'"), None, None, false, &password)
                .unwrap();
            let path = context.directory.path().join(format!("{}.json", stored.keystore_id));
            let before = fs::read(&path).unwrap();
            assert!(
                context
                    .service
                    .bsv_hd_import("Failed again".into(), PHRASE.join(" "), Some("m/44'/145'/0'".into()), None, None, false)
                    .await
                    .is_err()
            );
            assert_eq!(fs::read(path).unwrap(), before);
        });
    }
    #[test]
    fn test_hd_import_expected_preview_address_is_checked_before_writing() {
        block_on(async {
            let context = TestContext::new();
            let preview_json = context
                .service
                .keystore
                .bsv_hd_preview(PHRASE.join(" "), Some("m/44'/145'/0'".into()), None, None, false)
                .unwrap();
            let preview: Pairing = decode_json(&preview_json).unwrap();
            assert!(
                context
                    .service
                    .bsv_hd_import("Changed".into(), PHRASE.join(" "), Some("m/44'/236'/0'".into()), None, Some(preview.first_address), false)
                    .await
                    .is_err()
            );
            assert_eq!(context.wallets.get_wallets().await.unwrap().len(), 0);
            assert_eq!(fs::read_dir(context.directory.path()).unwrap().count(), 0);
        });
    }
}
