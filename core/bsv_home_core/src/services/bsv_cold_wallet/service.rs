use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicU64, Ordering},
    },
};

use bsv_home_bsv::cold_wallet::{Pairing, Request, Review, Signed, decode_json, qr_assemble, qr_frames};
use bsv_home_derivation::derive_wallet_id_from_account;
use futures::lock::{Mutex, MutexGuard};
use primitives::{Asset, Chain, SignerInput, TransactionFee, TransactionInputType, TransactionLoadInput, TransactionLoadMetadata, Wallet, WalletId, WalletSource, WalletType};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use super::state::{Pending, Reservation, State, invalid};
use super::{
    BsvHomeBsvColdWalletActionIntent, BsvHomeBsvColdWalletActionScope, BsvHomeBsvColdWalletPending, BsvHomeBsvColdWalletSigner, BsvHomeBsvColdWalletSnapshot,
    BsvHomeBsvColdWalletStore, BsvHomeBsvColdWalletStoredState, BsvHomeOfflineSigningPasswordDomain,
};
use super::{BsvHomeBsvColdWalletCandidate, BsvHomeBsvColdWalletRole, bsv_cold_wallet_role};
use crate::gateway::BsvHomeGateway;
use crate::keystore::BsvHomeKeystore;
use crate::services::balance::BsvHomeAssetBalance;
use crate::services::confirm::BsvHomeTransactionCheckpointData;
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::addresses::{BsvHomeBsvAddressPool, BsvHomeBsvColdAddressState};
use crate::services::wallet::model::BsvHomeWalletSecretKind;
use crate::services::wallet::{BsvHomeWalletImportResult, BsvHomeWalletImportType, BsvHomeWalletSecret, BsvHomeWalletService};

const MAX_STORED_BYTES: usize = 3_200_000;

struct WalletSelection {
    generation: u64,
    wallet_id: WalletId,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Online,
    Offline,
    Unified,
}

#[derive(uniffi::Object)]
pub struct BsvHomeBsvColdWalletService {
    wallets: Arc<BsvHomeWalletService>,
    addresses: Arc<BsvHomeBsvAddressPool>,
    gateway: Option<Arc<BsvHomeGateway>>,
    mode: Mode,
    keystore: Arc<BsvHomeKeystore>,
    store: Arc<dyn BsvHomeBsvColdWalletStore>,
    signer: Arc<dyn BsvHomeBsvColdWalletSigner>,
    owner: Uuid,
    epoch: Arc<AtomicU64>,
    password_domain: BsvHomeOfflineSigningPasswordDomain,
    selection: StdMutex<Option<WalletSelection>>,
    mutation: Mutex<()>,
    broadcasts: StdMutex<HashMap<WalletId, Pending>>,
}

#[uniffi::export]
impl BsvHomeBsvColdWalletService {
    #[uniffi::constructor]
    pub fn online(
        wallet: Arc<BsvHomeWalletService>,
        addresses: Arc<BsvHomeBsvAddressPool>,
        gateway: Arc<BsvHomeGateway>,
        keystore: Arc<BsvHomeKeystore>,
        store: Arc<dyn BsvHomeBsvColdWalletStore>,
        signer: Arc<dyn BsvHomeBsvColdWalletSigner>,
    ) -> Self {
        Self {
            wallets: wallet,
            addresses,
            gateway: Some(gateway),
            mode: Mode::Online,
            keystore,
            store,
            signer,
            owner: Uuid::new_v4(),
            epoch: Arc::new(AtomicU64::new(1)),
            password_domain: BsvHomeOfflineSigningPasswordDomain::LegacyCold,
            selection: StdMutex::new(None),
            mutation: Mutex::new(()),
            broadcasts: StdMutex::new(HashMap::new()),
        }
    }

    #[uniffi::constructor]
    pub fn offline(
        wallet: Arc<BsvHomeWalletService>,
        addresses: Arc<BsvHomeBsvAddressPool>,
        keystore: Arc<BsvHomeKeystore>,
        store: Arc<dyn BsvHomeBsvColdWalletStore>,
        signer: Arc<dyn BsvHomeBsvColdWalletSigner>,
    ) -> Self {
        Self {
            wallets: wallet,
            addresses,
            gateway: None,
            mode: Mode::Offline,
            keystore,
            store,
            signer,
            owner: Uuid::new_v4(),
            epoch: Arc::new(AtomicU64::new(1)),
            password_domain: BsvHomeOfflineSigningPasswordDomain::LegacyCold,
            selection: StdMutex::new(None),
            mutation: Mutex::new(()),
            broadcasts: StdMutex::new(HashMap::new()),
        }
    }

    #[uniffi::constructor]
    pub fn unified(
        wallet: Arc<BsvHomeWalletService>,
        addresses: Arc<BsvHomeBsvAddressPool>,
        gateway: Arc<BsvHomeGateway>,
        keystore: Arc<BsvHomeKeystore>,
        store: Arc<dyn BsvHomeBsvColdWalletStore>,
        signer: Arc<dyn BsvHomeBsvColdWalletSigner>,
    ) -> Self {
        Self {
            wallets: wallet,
            addresses,
            gateway: Some(gateway),
            mode: Mode::Unified,
            keystore,
            store,
            signer,
            owner: Uuid::new_v4(),
            epoch: Arc::new(AtomicU64::new(1)),
            password_domain: BsvHomeOfflineSigningPasswordDomain::UnifiedCold,
            selection: StdMutex::new(None),
            mutation: Mutex::new(()),
            broadcasts: StdMutex::new(HashMap::new()),
        }
    }

    pub fn begin_action(&self, wallet_id: WalletId, intent: BsvHomeBsvColdWalletActionIntent) -> Result<Arc<BsvHomeBsvColdWalletActionScope>, BsvHomeServiceError> {
        let scope = BsvHomeBsvColdWalletActionScope::new(
            self.owner,
            self.epoch.clone(),
            wallet_id,
            self.wallets.clone(),
            self.store.clone(),
            self.password_domain,
            intent,
        );
        scope.check_liveness()?;
        Ok(scope)
    }

    pub fn capture_generation(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }

    pub fn invalidate_actions(&self) -> Result<(), BsvHomeServiceError> {
        self.epoch
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| value.checked_add(1))
            .map_err(|_| invalid("Cold wallet action epoch exhausted"))?;
        *self.selection.lock().map_err(|_| invalid("Cold wallet selection state unavailable"))? = None;
        Ok(())
    }

    pub fn consume_wallet_selection(&self, wallet_id: WalletId) -> Result<bool, BsvHomeServiceError> {
        let current = self.wallets.current_wallet_id()?;
        let mut selection = self.selection.lock().map_err(|_| invalid("Cold wallet selection state unavailable"))?;
        if selection
            .as_ref()
            .is_some_and(|selection| selection.generation == self.capture_generation() && selection.wallet_id == wallet_id && current == Some(wallet_id.clone()))
        {
            *selection = None;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn current_wallet_id(&self) -> Result<Option<WalletId>, BsvHomeServiceError> {
        self.wallets.current_wallet_id()
    }

    pub async fn wallet_candidates(&self) -> Result<Vec<BsvHomeBsvColdWalletCandidate>, BsvHomeServiceError> {
        self.wallets()
            .await?
            .into_iter()
            .map(|wallet| {
                let role = bsv_cold_wallet_role(wallet.clone(), self.store.clone())?;
                Ok(BsvHomeBsvColdWalletCandidate { wallet, role })
            })
            .collect()
    }

    pub async fn wallets(&self) -> Result<Vec<Wallet>, BsvHomeServiceError> {
        let mut output = Vec::new();
        for wallet in self.wallets.wallets().await? {
            if self.mode == Mode::Unified {
                if bsv_cold_wallet_role(wallet.clone(), self.store.clone())? == BsvHomeBsvColdWalletRole::None {
                    continue;
                }
            } else {
                if wallet.account(Chain::BitcoinSv).is_none() || wallet.wallet_type == WalletType::PrivateKey {
                    continue;
                }
                if wallet.wallet_type == WalletType::View
                    && !wallet
                        .account(Chain::BitcoinSv)
                        .and_then(|account| account.extended_public_key.as_ref())
                        .is_some_and(|key| key.starts_with("xpub"))
                {
                    continue;
                }
            }
            let account = self.addresses.cold_account(wallet.id.clone()).await?;
            Pairing::from_account(&account).map_err(invalid)?;
            output.push(wallet);
        }
        Ok(output)
    }

    pub async fn select_wallet(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        let _mutation = self.begin()?;
        self.invalidate_actions()?;
        let generation = self.capture_generation();
        let selection = self.wallets.current_wallet_id()?;
        self.wallet(&wallet_id).await?;
        self.check_selection(&selection)?;
        self.select_wallet_at_generation(wallet_id, generation)
    }

    pub fn create_words(&self, expected_generation: u64) -> Result<Vec<String>, BsvHomeServiceError> {
        self.check_generation(expected_generation)?;
        self.require_offline()?;
        let words = Zeroizing::new(self.wallets.create_wallet()?);
        self.check_generation(expected_generation)?;
        Ok(words.to_vec())
    }

    pub fn preview_recovery(
        &self,
        input: String,
        account_path: Option<String>,
        passphrase: Option<String>,
        expected_address: Option<String>,
        legacy_non_compliant: Option<bool>,
        expected_generation: u64,
    ) -> Result<String, BsvHomeServiceError> {
        let input = Zeroizing::new(input);
        let passphrase = passphrase.map(Zeroizing::new);
        self.check_generation(expected_generation)?;
        self.require_offline()?;
        let pairing = self.keystore.bsv_hd_preview(
            input.to_string(),
            account_path,
            passphrase.as_ref().map(|value| value.to_string()),
            expected_address,
            legacy_non_compliant.unwrap_or(false),
        )?;
        self.check_generation(expected_generation)?;
        Ok(pairing)
    }

    pub fn preview_observer(&self, input: String, account_path: Option<String>, expected_address: Option<String>) -> Result<String, BsvHomeServiceError> {
        Ok(self.keystore.bsv_hd_public_preview(input, account_path, expected_address)?)
    }

    pub async fn create(&self, name: String, words: Vec<String>, expected_generation: u64) -> Result<BsvHomeBsvColdWalletSnapshot, BsvHomeServiceError> {
        let words = Zeroizing::new(words);
        self.check_generation(expected_generation)?;
        let _mutation = self.begin()?;
        self.require_offline()?;
        let pairing = self.preview_recovery(words.join(" "), None, None, None, None, expected_generation)?;
        let selection = self.wallets.current_wallet_id()?;
        self.signer.authenticate_import().await?;
        self.check_generation(expected_generation)?;
        self.check_selection(&selection)?;
        self.reserve_import_pairing(pairing.clone()).await?;
        self.check_generation(expected_generation)?;
        self.check_selection(&selection)?;
        let result = self
            .wallets
            .import_wallet(
                name,
                BsvHomeWalletImportType::SinglePhrase {
                    words: words.to_vec(),
                    chain: Chain::BitcoinSv,
                },
                WalletSource::Create,
            )
            .await?;
        self.check_generation(expected_generation)?;
        self.check_selection(&selection)?;
        self.finish_import(result, pairing, expected_generation, &selection).await
    }

    pub async fn import_recovery(
        &self,
        name: String,
        input: String,
        account_path: Option<String>,
        passphrase: Option<String>,
        pairing_json: String,
        legacy_non_compliant: Option<bool>,
        expected_generation: u64,
    ) -> Result<BsvHomeBsvColdWalletSnapshot, BsvHomeServiceError> {
        let input = Zeroizing::new(input);
        let passphrase = passphrase.map(Zeroizing::new);
        self.check_generation(expected_generation)?;
        self.require_offline()?;
        let _mutation = self.begin()?;
        let preview = self.preview_recovery(
            input.to_string(),
            account_path.clone(),
            passphrase.as_ref().map(|value| value.to_string()),
            None,
            legacy_non_compliant,
            expected_generation,
        )?;
        let expected: Pairing = decode_json(&pairing_json).map_err(invalid)?;
        if decode_json::<Pairing>(&preview).map_err(invalid)? != expected {
            return Err(invalid("Cold wallet import changed after preview"));
        }
        let selection = self.wallets.current_wallet_id()?;
        self.signer.authenticate_import().await?;
        self.check_generation(expected_generation)?;
        self.check_selection(&selection)?;
        self.reserve_import_pairing(pairing_json.clone()).await?;
        self.check_generation(expected_generation)?;
        self.check_selection(&selection)?;
        let result = self
            .wallets
            .bsv_hd_import(
                name,
                input.to_string(),
                account_path,
                passphrase.as_ref().map(|value| value.to_string()),
                Some(expected.first_address),
                legacy_non_compliant.unwrap_or(false),
            )
            .await?;
        self.check_generation(expected_generation)?;
        self.check_selection(&selection)?;
        self.finish_import(result, pairing_json, expected_generation, &selection).await
    }

    pub async fn import_observer(&self, name: String, pairing_json: String, expected_generation: u64) -> Result<BsvHomeBsvColdWalletSnapshot, BsvHomeServiceError> {
        self.check_generation(expected_generation)?;
        let _mutation = self.begin()?;
        self.online_gateway()?;
        decode_json::<Pairing>(&pairing_json).map_err(invalid)?.validate().map_err(invalid)?;
        let selection = self.wallets.current_wallet_id()?;
        self.signer.authenticate_import().await?;
        self.check_generation(expected_generation)?;
        self.check_selection(&selection)?;
        let result = self.wallets.bsv_cold_wallet_import_pairing(name, pairing_json.clone()).await?;
        self.check_generation(expected_generation)?;
        self.check_selection(&selection)?;
        self.finish_import(result, pairing_json, expected_generation, &selection).await
    }

    pub async fn secret_kind(&self, wallet_id: WalletId) -> Result<Option<BsvHomeWalletSecretKind>, BsvHomeServiceError> {
        self.wallets.secret_kind(wallet_id).await
    }

    pub async fn export_secret(&self, wallet_id: WalletId, expected_generation: u64) -> Result<BsvHomeWalletSecret, BsvHomeServiceError> {
        self.check_generation(expected_generation)?;
        let _mutation = self.begin()?;
        let wallet = self.selected_wallet(&wallet_id).await?;
        self.require_unified_role(&wallet, BsvHomeBsvColdWalletRole::Signer)?;
        self.check_generation(expected_generation)?;
        self.signer.authenticate_import().await?;
        self.check_generation(expected_generation)?;
        self.check_wallet(&wallet).await?;
        self.check_generation(expected_generation)?;
        let mut secret = self.wallets.export_secret(wallet_id).await?;
        let current = async {
            self.check_generation(expected_generation)?;
            self.check_wallet(&wallet).await?;
            self.check_generation(expected_generation)
        }
        .await;
        if let Err(error) = current {
            match &mut secret {
                BsvHomeWalletSecret::Words { words } => words.zeroize(),
                BsvHomeWalletSecret::PrivateKey { key } => key.zeroize(),
                BsvHomeWalletSecret::Recovery { recovery } => recovery.zeroize(),
            }
            return Err(error);
        }
        Ok(secret)
    }

    pub fn qr_frames(&self, payload_json: String) -> Result<Vec<String>, BsvHomeServiceError> {
        qr_frames(&payload_json).map_err(invalid)
    }
    pub fn qr_assemble(&self, frames: Vec<String>) -> Result<Option<String>, BsvHomeServiceError> {
        qr_assemble(&frames).map_err(invalid)
    }

    pub async fn pairing(&self, wallet_id: WalletId) -> Result<String, BsvHomeServiceError> {
        let account = self.addresses.cold_account(wallet_id).await?;
        serde_json::to_string(&Pairing::from_account(&account).map_err(invalid)?).map_err(invalid)
    }

    pub async fn export_pairing(&self, wallet_id: WalletId, expected_generation: u64) -> Result<BsvHomeBsvColdWalletSnapshot, BsvHomeServiceError> {
        self.check_generation(expected_generation)?;
        let _mutation = self.begin()?;
        self.require_offline()?;
        let selection = self.wallets.current_wallet_id()?;
        let wallet = self.wallet(&wallet_id).await?;
        self.check_pairing_source(&wallet, &selection, expected_generation).await?;
        let pairing_json = self.pairing(wallet_id.clone()).await?;
        self.check_pairing_source(&wallet, &selection, expected_generation).await?;
        let pairing: Pairing = decode_json(&pairing_json).map_err(invalid)?;
        pairing.validate().map_err(invalid)?;
        let record = self.store.get(wallet_id.clone()).await?;
        self.check_pairing_source(&wallet, &selection, expected_generation).await?;
        match record {
            Some(record) => {
                Self::decode_state(&record.value, &pairing)?;
            }
            None => self.write(&wallet_id, None, &State::new(pairing_json)?).await?,
        }
        self.check_pairing_source(&wallet, &selection, expected_generation).await?;
        self.select_wallet_at_generation(wallet_id.clone(), expected_generation)?;
        let snapshot = self.snapshot(wallet_id).await?;
        self.check_pairing_source(&wallet, &Some(wallet.id.clone()), expected_generation).await?;
        if snapshot.role != BsvHomeBsvColdWalletRole::Signer {
            return Err(invalid("Cold wallet pairing metadata changed"));
        }
        Ok(snapshot)
    }

    pub async fn snapshot(&self, wallet_id: WalletId) -> Result<BsvHomeBsvColdWalletSnapshot, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        let pairing_json = self.pairing(wallet_id.clone()).await?;
        let pairing = decode_json::<Pairing>(&pairing_json).map_err(invalid)?;
        let pending = self
            .read(&wallet_id, &pairing)
            .await?
            .1
            .and_then(|state| state.next.or(state.pending))
            .map(|pending| pending.validate(&pairing))
            .transpose()?;
        let addresses = self.receive(wallet_id).await?;
        let role = bsv_cold_wallet_role(wallet.clone(), self.store.clone())?;
        Ok(BsvHomeBsvColdWalletSnapshot {
            wallet,
            pairing_json,
            addresses,
            balance: None,
            transactions: Vec::new(),
            pending,
            role,
        })
    }

    pub async fn refresh(&self, wallet_id: WalletId) -> Result<BsvHomeBsvColdWalletSnapshot, BsvHomeServiceError> {
        self.online_gateway()?;
        let wallet = self.selected_wallet(&wallet_id).await?;
        self.require_unified_role(&wallet, BsvHomeBsvColdWalletRole::Observer)?;
        self.addresses.sync(wallet_id.clone()).await?;
        let balance = self.addresses.balance(wallet_id.clone(), false).await?;
        let transactions = self.addresses.transactions(wallet_id.clone(), 100).await?;
        self.check_wallet(&wallet).await?;
        let mut snapshot = self.snapshot(wallet_id).await?;
        let value = balance.balance;
        snapshot.balance = Some(BsvHomeAssetBalance {
            asset_id: balance.asset_id,
            available: value.available,
            frozen: value.frozen,
            locked: value.locked,
            staked: value.staked,
            pending: value.pending,
            pending_unconfirmed: value.pending_unconfirmed,
            rewards: value.rewards,
            reserved: value.reserved,
            withdrawable: value.withdrawable,
            earn: value.earn,
            metadata: value.metadata,
            is_active: balance.is_active,
        });
        snapshot.transactions = transactions;
        Ok(snapshot)
    }

    pub async fn receive(&self, wallet_id: WalletId) -> Result<BsvHomeBsvColdAddressState, BsvHomeServiceError> {
        match self.mode {
            Mode::Unified => self.addresses.prepare(wallet_id).await.map(Into::into),
            Mode::Online | Mode::Offline => self.addresses.cold_addresses(wallet_id).await,
        }
    }
    pub async fn select_address(&self, wallet_id: WalletId, index: u32) -> Result<BsvHomeBsvColdAddressState, BsvHomeServiceError> {
        let _mutation = self.begin()?;
        self.selected_wallet(&wallet_id).await?;
        match self.mode {
            Mode::Unified => self.addresses.select_address(wallet_id, index).await.map(Into::into),
            Mode::Online | Mode::Offline => self.addresses.cold_select_address(wallet_id, index).await,
        }
    }
    pub async fn add_address(&self, wallet_id: WalletId) -> Result<BsvHomeBsvColdAddressState, BsvHomeServiceError> {
        let _mutation = self.begin()?;
        self.selected_wallet(&wallet_id).await?;
        match self.mode {
            Mode::Unified => self.addresses.add_address(wallet_id).await.map(Into::into),
            Mode::Online | Mode::Offline => self.addresses.cold_add_address(wallet_id).await,
        }
    }

    pub async fn prepare(
        &self,
        wallet_id: WalletId,
        recipient: String,
        amount: u64,
        scope: Arc<BsvHomeBsvColdWalletActionScope>,
    ) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
        scope.check_liveness()?;
        let _mutation = self.begin()?;
        let _action = scope.claim(
            self.owner,
            &wallet_id,
            &BsvHomeBsvColdWalletActionIntent::Prepare {
                recipient: recipient.clone(),
                amount,
            },
        )?;
        let gateway = self.online_gateway()?;
        let wallet = self.selected_wallet(&wallet_id).await?;
        self.require_unified_role(&wallet, BsvHomeBsvColdWalletRole::Observer)?;
        scope.check_liveness()?;
        let pairing_json = self.pairing(wallet_id.clone()).await?;
        let pairing = decode_json::<Pairing>(&pairing_json).map_err(invalid)?;
        let (revision, previous) = self.read(&wallet_id, &pairing).await?;
        self.bind_action(&scope, &wallet, revision, previous.as_ref(), None).await?;
        let mut state = previous.unwrap_or(State::new(pairing_json)?);
        if state.pending.as_ref().is_some_and(|pending| !pending.broadcasted) {
            return Err(invalid("Cold wallet already has an unfinished request"));
        }
        let account = self.addresses.cold_account(wallet_id.clone()).await?;
        scope.validate().await?;
        let spend = self
            .addresses
            .preload_spend(wallet_id.clone(), &account.address, false)
            .await?
            .ok_or_else(|| invalid("Cold wallet address metadata unavailable"))?;
        scope.validate().await?;
        let prepared = gateway.prepare_cold_spend(account, recipient, amount, spend).await?;
        scope.validate().await?;
        self.check_wallet(&wallet).await?;
        let review = prepared.request.review(&pairing).map_err(invalid)?;
        let input = SignerInput::new(
            TransactionLoadInput {
                input_type: TransactionInputType::Transfer {
                    asset: Asset::from_chain(Chain::BitcoinSv),
                },
                sender_address: pairing.first_address.clone(),
                destination_address: review.recipient.clone(),
                value: review.amount.into(),
                gas_price: prepared.gas_price.clone(),
                memo: None,
                is_max_value: false,
                metadata: TransactionLoadMetadata::BitcoinSv { spend: prepared.spend.clone() },
            },
            TransactionFee {
                fee: review.fee.into(),
                gas_price_type: prepared.gas_price.clone(),
                gas_limit: 1.into(),
                options: Default::default(),
                fee_asset: Chain::BitcoinSv.as_asset_id(),
            },
        );
        scope.validate().await?;
        let lease = self.addresses.reserve(&wallet, &input, review.amount.to_string(), false).await?;
        state.pending = Some(Pending {
            request: serde_json::to_string(&prepared.request).map_err(invalid)?,
            review: serde_json::to_string(&review).map_err(invalid)?,
            signed: None,
            broadcasted: false,
            persistence_error: None,
            reservation: Some(Reservation {
                id: lease.id.clone(),
                spend: prepared.spend,
                gas_price: prepared.gas_price,
            }),
        });
        let saved = async {
            scope.validate().await?;
            scope.begin_write()?;
            self.write(&wallet_id, revision, &state).await?;
            scope.write_committed()
        }
        .await;
        if let Err(error) = saved {
            if let Err(release) = self.addresses.release_unsigned(&wallet, &lease.id).await {
                return Err(BsvHomeServiceError::Store {
                    msg: format!("Cold wallet request persistence failed: {error}; reservation release failed: {release}"),
                });
            }
            return Err(error);
        }
        state.pending.ok_or_else(|| invalid("Cold wallet request missing"))?.validate(&pairing)
    }

    pub async fn import_request(&self, wallet_id: WalletId, request_json: String) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
        self.review_request(wallet_id, request_json, None).await
    }

    pub async fn review_request(&self, wallet_id: WalletId, request_json: String, replace_commitment: Option<String>) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
        let _mutation = self.begin()?;
        self.require_offline()?;
        let wallet = self.selected_wallet(&wallet_id).await?;
        self.require_unified_role(&wallet, BsvHomeBsvColdWalletRole::Signer)?;
        if wallet.wallet_type == WalletType::View {
            return Err(invalid("Cold wallet signing requires a held secret"));
        }
        let pairing_json = self.pairing(wallet_id.clone()).await?;
        let pairing: Pairing = decode_json(&pairing_json).map_err(invalid)?;
        let request: Request = decode_json(&request_json).map_err(invalid)?;
        let review = request.review(&pairing).map_err(invalid)?;
        let (revision, state) = self.read(&wallet_id, &pairing).await?;
        let mut state = state.unwrap_or(State::new(pairing_json)?);
        if let Some(next) = &state.next {
            if next.request == request_json {
                return next.validate(&pairing);
            }
            return Err(invalid("Cold wallet already has an unfinished next request"));
        }
        let candidate = Pending {
            request: request_json,
            review: serde_json::to_string(&review).map_err(invalid)?,
            signed: None,
            broadcasted: false,
            persistence_error: None,
            reservation: None,
        };
        match &state.pending {
            Some(previous) if previous.request == candidate.request => return previous.validate(&pairing),
            Some(previous) if !previous.broadcasted => {
                let saved = previous.validate(&pairing)?;
                if previous.signed.is_none() || replace_commitment.as_deref() != Some(saved.review.commitment.as_str()) {
                    return Err(invalid("Cold wallet previous signature requires explicit acknowledgement before starting another request"));
                }
                state.next = Some(candidate);
            }
            Some(_) | None => {
                if replace_commitment.is_some() {
                    return Err(invalid("Cold wallet replacement context changed"));
                }
                state.pending = Some(candidate);
            }
        }
        self.check_wallet(&wallet).await?;
        self.write(&wallet_id, revision, &state).await?;
        pending(&state, &pairing, &review.commitment)
    }

    pub async fn sign(&self, wallet_id: WalletId, commitment: String, scope: Arc<BsvHomeBsvColdWalletActionScope>) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
        scope.check_liveness()?;
        let _mutation = self.begin()?;
        let _action = scope.claim(self.owner, &wallet_id, &BsvHomeBsvColdWalletActionIntent::Sign { commitment: commitment.clone() })?;
        self.require_offline()?;
        let wallet = self.selected_wallet(&wallet_id).await?;
        self.require_unified_role(&wallet, BsvHomeBsvColdWalletRole::Signer)?;
        if wallet.wallet_type == WalletType::View {
            return Err(invalid("Cold wallet signing requires a held secret"));
        }
        let (revision, mut state, pairing) = self.required(&wallet_id).await?;
        let previous = pending(&state, &pairing, &commitment)?;
        self.bind_action(&scope, &wallet, Some(revision), Some(&state), Some((&previous.request_json, &previous.review_json)))
            .await?;
        if previous.signed_json.is_some() {
            return Ok(previous);
        }
        scope.validate().await?;
        self.signer.authorize(wallet.clone(), scope.clone()).await?;
        scope.validate().await?;
        scope.consume_password()?;
        self.signer.authenticate(wallet.clone(), scope.clone()).await?;
        scope.validate().await?;
        self.check_wallet(&wallet).await?;
        self.check_revision(&wallet_id, revision).await?;
        scope.check_signing(wallet.clone(), previous.request_json.clone(), previous.review_json.clone())?;
        let signed_json = self.signer.sign_request(wallet.clone(), previous.request_json, previous.review_json, scope.clone()).await?;
        scope.validate().await?;
        self.check_wallet(&wallet).await?;
        let record = match state.next.as_mut() {
            Some(next) => next,
            None => state.pending.as_mut().ok_or_else(|| invalid("Cold wallet request missing"))?,
        };
        record.signed = Some(signed_json);
        let result = record.validate(&pairing)?;
        if let Some(next) = state.next.take() {
            state.pending = Some(next);
        }
        scope.validate().await?;
        scope.begin_write()?;
        self.write(&wallet_id, Some(revision), &state).await?;
        scope.write_committed()?;
        if let Err(error) = self.save_checkpoint(&wallet, &state).await {
            scope.write_pending()?;
            return Err(error);
        }
        Ok(result)
    }

    pub async fn import_signed(&self, wallet_id: WalletId, signed_json: String) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
        let _mutation = self.begin()?;
        self.online_gateway()?;
        let wallet = self.selected_wallet(&wallet_id).await?;
        self.require_unified_role(&wallet, BsvHomeBsvColdWalletRole::Observer)?;
        let (revision, mut state, pairing) = self.required(&wallet_id).await?;
        let record = state.pending.as_mut().ok_or_else(|| invalid("Cold wallet request missing"))?;
        if record.signed.as_ref().is_some_and(|previous| previous != &signed_json) {
            return Err(invalid("Cold wallet signed transaction cannot be replaced"));
        }
        record.signed = Some(signed_json);
        let result = record.validate(&pairing)?;
        self.write(&wallet_id, Some(revision), &state).await?;
        self.save_checkpoint(&wallet, &state).await?;
        Ok(result)
    }

    pub async fn broadcast(
        &self,
        wallet_id: WalletId,
        commitment: String,
        scope: Arc<BsvHomeBsvColdWalletActionScope>,
    ) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
        scope.check_liveness()?;
        let _mutation = self.begin()?;
        let _action = scope.claim(self.owner, &wallet_id, &BsvHomeBsvColdWalletActionIntent::Broadcast { commitment: commitment.clone() })?;
        let gateway = self.online_gateway()?;
        let wallet = self.selected_wallet(&wallet_id).await?;
        self.require_unified_role(&wallet, BsvHomeBsvColdWalletRole::Observer)?;
        let (revision, mut state, pairing) = self.required(&wallet_id).await?;
        let previous = pending(&state, &pairing, &commitment)?;
        if previous.broadcasted {
            scope.submitted(previous.transaction_id.clone().ok_or_else(|| invalid("Cold wallet accepted transaction ID missing"))?)?;
            if previous.persistence_error.is_some() {
                scope.write_pending()?;
            }
            return Ok(previous);
        }
        self.bind_action(&scope, &wallet, Some(revision), Some(&state), None).await?;
        let signed = previous.signed_json.ok_or_else(|| invalid("Cold wallet verified signature missing"))?;
        scope.validate().await?;
        self.signer.authenticate(wallet.clone(), scope.clone()).await?;
        scope.validate().await?;
        self.check_wallet(&wallet).await?;
        self.check_revision(&wallet_id, revision).await?;
        self.save_checkpoint(&wallet, &state).await?;
        scope.validate().await?;
        let check = || scope.check();
        let before_send = || scope.validate();
        let before_submission = || scope.begin_submission();
        let transaction_id = gateway
            .bsv_cold_broadcast_checked(previous.request_json, previous.review_json, signed, &check, before_send, &before_submission)
            .await?;
        scope.submitted(transaction_id.clone())?;
        let record = state.pending.as_mut().ok_or_else(|| invalid("Cold wallet request missing"))?;
        let result = record.validate(&pairing)?;
        if result.transaction_id != Some(transaction_id) {
            return Err(invalid("Cold wallet broadcast identity mismatch"));
        }
        record.broadcasted = true;
        scope.write_pending()?;
        record.persistence_error = Some("Confirmed broadcast state could not be persisted".into());
        self.broadcasts
            .lock()
            .map_err(|_| invalid("Cold wallet broadcast state lock failed"))?
            .insert(wallet_id.clone(), record.clone());
        let mut durable = state.clone();
        durable.pending.as_mut().ok_or_else(|| invalid("Cold wallet request missing"))?.persistence_error = None;
        if self.write(&wallet_id, Some(revision), &durable).await.is_ok() {
            scope.write_committed()?;
            return durable.pending.ok_or_else(|| invalid("Cold wallet request missing"))?.validate(&pairing);
        }
        state.pending.ok_or_else(|| invalid("Cold wallet request missing"))?.validate(&pairing)
    }

    pub async fn cancel_request(&self, wallet_id: WalletId, commitment: String) -> Result<(), BsvHomeServiceError> {
        let _mutation = self.begin()?;
        let wallet = self.selected_wallet(&wallet_id).await?;
        let (revision, mut state, pairing) = self.required(&wallet_id).await?;
        let previous = pending(&state, &pairing, &commitment)?;
        if previous.signed_json.is_some() {
            return Err(invalid("Cold wallet signed request cannot be cancelled"));
        }
        if state.next.is_some() {
            state.next = None;
        } else {
            let reservation = state.pending.as_ref().and_then(|pending| pending.reservation.clone());
            if let Some(reservation) = reservation {
                self.addresses.release_unsigned(&wallet, &reservation.id).await?;
            }
            state.pending = None;
        }
        self.write(&wallet_id, Some(revision), &state).await
    }
}

impl BsvHomeBsvColdWalletService {
    async fn bind_action(
        &self,
        scope: &BsvHomeBsvColdWalletActionScope,
        wallet: &Wallet,
        revision: Option<u64>,
        state: Option<&State>,
        signing: Option<(&str, &str)>,
    ) -> Result<(), BsvHomeServiceError> {
        scope.check_liveness()?;
        self.check_wallet(wallet).await?;
        scope.check_liveness()?;
        let record = self.store.get(wallet.id.clone()).await?;
        scope.check_liveness()?;
        if record.as_ref().map(|record| record.revision) != revision {
            return Err(BsvHomeServiceError::Cancelled);
        }
        match (record.as_ref(), state) {
            (None, None) => {}
            (Some(record), Some(expected)) => {
                let actual: State = serde_json::from_str(&record.value).map_err(invalid)?;
                actual.validate()?;
                if serde_json::to_string(&actual).map_err(invalid)? != serde_json::to_string(expected).map_err(invalid)? {
                    return Err(BsvHomeServiceError::Cancelled);
                }
            }
            (None, Some(_)) | (Some(_), None) => return Err(BsvHomeServiceError::Cancelled),
        }
        scope.bind(wallet.clone(), record.as_ref(), signing)
    }

    fn select_wallet_at_generation(&self, wallet_id: WalletId, generation: u64) -> Result<(), BsvHomeServiceError> {
        self.check_generation(generation)?;
        *self.selection.lock().map_err(|_| invalid("Cold wallet selection state unavailable"))? = Some(WalletSelection {
            generation,
            wallet_id: wallet_id.clone(),
        });
        self.check_generation(generation)?;
        if let Err(error) = self.wallets.set_current_wallet_id(wallet_id.clone()) {
            let mut selection = self.selection.lock().map_err(|_| invalid("Cold wallet selection state unavailable"))?;
            if selection
                .as_ref()
                .is_some_and(|selection| selection.generation == generation && selection.wallet_id == wallet_id)
            {
                *selection = None;
            }
            return Err(error);
        }
        Ok(())
    }

    fn check_generation(&self, expected: u64) -> Result<(), BsvHomeServiceError> {
        if expected == u64::MAX || self.capture_generation() != expected {
            return Err(BsvHomeServiceError::Cancelled);
        }
        Ok(())
    }

    fn check_selection(&self, expected: &Option<WalletId>) -> Result<(), BsvHomeServiceError> {
        if self.wallets.current_wallet_id()? != *expected {
            return Err(invalid("Cold wallet selection changed during import"));
        }
        Ok(())
    }
    fn require_offline(&self) -> Result<(), BsvHomeServiceError> {
        match self.mode {
            Mode::Online => Err(BsvHomeServiceError::Unsupported {
                msg: "Cold signing is available only in the offline application".into(),
            }),
            Mode::Offline | Mode::Unified => Ok(()),
        }
    }

    fn require_unified_role(&self, wallet: &Wallet, expected: BsvHomeBsvColdWalletRole) -> Result<(), BsvHomeServiceError> {
        if self.mode == Mode::Unified && bsv_cold_wallet_role(wallet.clone(), self.store.clone())? != expected {
            return Err(invalid("Cold wallet role does not permit this operation"));
        }
        Ok(())
    }

    fn begin(&self) -> Result<MutexGuard<'_, ()>, BsvHomeServiceError> {
        self.mutation.try_lock().ok_or_else(|| invalid("Cold wallet is busy"))
    }
    fn online_gateway(&self) -> Result<&Arc<BsvHomeGateway>, BsvHomeServiceError> {
        self.gateway.as_ref().ok_or_else(|| BsvHomeServiceError::Unsupported {
            msg: "Cold wallet offline mode forbids network operations".into(),
        })
    }
    async fn wallet(&self, wallet_id: &WalletId) -> Result<Wallet, BsvHomeServiceError> {
        self.wallets
            .wallets()
            .await?
            .into_iter()
            .find(|wallet| wallet.id == *wallet_id)
            .ok_or_else(|| invalid("Cold wallet not found"))
    }
    async fn check_pairing_source(&self, wallet: &Wallet, selection: &Option<WalletId>, generation: u64) -> Result<(), BsvHomeServiceError> {
        self.check_generation(generation)?;
        self.check_selection(selection)?;
        if wallet.account(Chain::BitcoinSv).is_none() {
            return Err(invalid("Cold wallet pairing requires a BSV recovery wallet"));
        }
        match wallet.wallet_type {
            WalletType::Single | WalletType::Multicoin => {}
            WalletType::PrivateKey | WalletType::View => return Err(invalid("Cold wallet pairing requires a BSV recovery wallet")),
        }
        let kind = self.wallets.secret_kind(wallet.id.clone()).await?;
        self.check_generation(generation)?;
        self.check_selection(selection)?;
        match kind {
            Some(BsvHomeWalletSecretKind::Phrase | BsvHomeWalletSecretKind::Recovery) => {}
            Some(BsvHomeWalletSecretKind::PrivateKey) | None => return Err(invalid("Cold wallet pairing requires a recovery secret")),
        }
        let actual = self.wallet(&wallet.id).await?;
        self.check_generation(generation)?;
        self.check_selection(selection)?;
        if actual.id != wallet.id || actual.wallet_type != wallet.wallet_type || actual.accounts != wallet.accounts {
            return Err(invalid("Cold wallet pairing source changed"));
        }
        Ok(())
    }

    async fn selected_wallet(&self, wallet_id: &WalletId) -> Result<Wallet, BsvHomeServiceError> {
        if self.wallets.current_wallet_id()? != Some(wallet_id.clone()) {
            return Err(invalid("Cold wallet selection changed"));
        }
        self.wallet(wallet_id).await
    }
    async fn check_wallet(&self, wallet: &Wallet) -> Result<(), BsvHomeServiceError> {
        let current = self.selected_wallet(&wallet.id).await?;
        if current.id != wallet.id || current.wallet_type != wallet.wallet_type || current.accounts != wallet.accounts {
            return Err(invalid("Cold wallet context changed"));
        }
        Ok(())
    }
    async fn finish_import(
        &self,
        result: BsvHomeWalletImportResult,
        pairing: String,
        expected_generation: u64,
        expected_selection: &Option<WalletId>,
    ) -> Result<BsvHomeBsvColdWalletSnapshot, BsvHomeServiceError> {
        let wallet = match result {
            BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
        };
        self.check_generation(expected_generation)?;
        self.check_selection(expected_selection)?;
        self.save_pairing_state(&wallet.id, pairing).await?;
        self.check_generation(expected_generation)?;
        self.check_selection(expected_selection)?;
        self.select_wallet_at_generation(wallet.id.clone(), expected_generation)?;
        let snapshot = self.snapshot(wallet.id.clone()).await?;
        self.check_generation(expected_generation)?;
        if self.wallets.current_wallet_id()? != Some(wallet.id) {
            return Err(BsvHomeServiceError::Cancelled);
        }
        Ok(snapshot)
    }
    async fn save_pairing_state(&self, wallet_id: &WalletId, pairing_json: String) -> Result<(), BsvHomeServiceError> {
        let actual: Pairing = decode_json(&self.pairing(wallet_id.clone()).await?).map_err(invalid)?;
        let expected: Pairing = decode_json(&pairing_json).map_err(invalid)?;
        if actual != expected {
            return Err(invalid("Cold wallet pairing differs from wallet"));
        }
        self.ensure_pairing_state(wallet_id, pairing_json).await
    }
    async fn reserve_import_pairing(&self, pairing_json: String) -> Result<(), BsvHomeServiceError> {
        let pairing: Pairing = decode_json(&pairing_json).map_err(invalid)?;
        pairing.validate().map_err(invalid)?;
        let account = pairing.account().map_err(invalid)?;
        let wallet_id = derive_wallet_id_from_account(&account, WalletType::Single).map_err(invalid)?;
        self.ensure_pairing_state(&wallet_id, pairing_json).await
    }
    async fn ensure_pairing_state(&self, wallet_id: &WalletId, pairing_json: String) -> Result<(), BsvHomeServiceError> {
        let state = State::new(pairing_json)?;
        let pairing = state.validate()?;
        if let Some(record) = self.store.get(wallet_id.clone()).await? {
            Self::decode_state(&record.value, &pairing)?;
            return Ok(());
        }
        self.write(wallet_id, None, &state).await
    }
    fn decode_state(value: &str, pairing: &Pairing) -> Result<State, BsvHomeServiceError> {
        if value.len() > MAX_STORED_BYTES {
            return Err(invalid("Cold wallet saved state exceeds limit"));
        }
        let state: State = serde_json::from_str(value).map_err(invalid)?;
        if state.validate()? != *pairing {
            return Err(invalid("Cold wallet saved pairing differs from wallet"));
        }
        Ok(state)
    }
    async fn read(&self, wallet_id: &WalletId, pairing: &Pairing) -> Result<(Option<u64>, Option<State>), BsvHomeServiceError> {
        match self.store.get(wallet_id.clone()).await? {
            None => Ok((None, None)),
            Some(record) => {
                let state = Self::decode_state(&record.value, pairing)?;
                self.restore_broadcast(wallet_id, record.revision, state, pairing).await
            }
        }
    }
    async fn restore_broadcast(&self, wallet_id: &WalletId, revision: u64, mut state: State, pairing: &Pairing) -> Result<(Option<u64>, Option<State>), BsvHomeServiceError> {
        let cached = self
            .broadcasts
            .lock()
            .map_err(|_| invalid("Cold wallet broadcast state lock failed"))?
            .get(wallet_id)
            .cloned();
        let Some(cached) = cached else {
            return Ok((Some(revision), Some(state)));
        };
        cached.validate(pairing)?;
        let matches = state
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request == cached.request && pending.review == cached.review && pending.signed == cached.signed);
        if !cached.broadcasted || cached.signed.is_none() || !matches {
            return Ok((Some(revision), Some(state)));
        }
        state.pending = Some(cached);
        let mut durable = state.clone();
        durable.pending.as_mut().ok_or_else(|| invalid("Cold wallet request missing"))?.persistence_error = None;
        if self.write(wallet_id, Some(revision), &durable).await.is_ok() {
            let next = revision.checked_add(1).ok_or_else(|| invalid("Cold wallet revision exhausted"))?;
            return Ok((Some(next), Some(durable)));
        }
        Ok((Some(revision), Some(state)))
    }

    async fn write(&self, wallet_id: &WalletId, revision: Option<u64>, state: &State) -> Result<(), BsvHomeServiceError> {
        state.validate()?;
        let value = serde_json::to_string(state).map_err(invalid)?;
        if value.len() > MAX_STORED_BYTES {
            return Err(invalid("Cold wallet saved state exceeds limit"));
        }
        let next = revision
            .map(|value| value.checked_add(1).ok_or_else(|| invalid("Cold wallet revision exhausted")))
            .transpose()?
            .unwrap_or(0);
        if !self
            .store
            .compare_and_swap(wallet_id.clone(), revision, BsvHomeBsvColdWalletStoredState { revision: next, value })
            .await?
        {
            return Err(invalid("Cold wallet state changed concurrently"));
        }
        self.broadcasts.lock().map_err(|_| invalid("Cold wallet broadcast state lock failed"))?.remove(wallet_id);
        Ok(())
    }
    async fn required(&self, wallet_id: &WalletId) -> Result<(u64, State, Pairing), BsvHomeServiceError> {
        let pairing = decode_json::<Pairing>(&self.pairing(wallet_id.clone()).await?).map_err(invalid)?;
        let (revision, state) = self.read(wallet_id, &pairing).await?;
        Ok((
            revision.ok_or_else(|| invalid("Cold wallet saved state missing"))?,
            state.ok_or_else(|| invalid("Cold wallet saved state missing"))?,
            pairing,
        ))
    }
    async fn check_revision(&self, wallet_id: &WalletId, revision: u64) -> Result<(), BsvHomeServiceError> {
        if self.store.get(wallet_id.clone()).await?.map(|record| record.revision) != Some(revision) {
            return Err(invalid("Cold wallet state changed during authentication"));
        }
        Ok(())
    }
    async fn save_checkpoint(&self, wallet: &Wallet, state: &State) -> Result<(), BsvHomeServiceError> {
        let Some(pending) = &state.pending else {
            return Ok(());
        };
        let Some(reservation) = &pending.reservation else {
            return Ok(());
        };
        let Some(signed_json) = &pending.signed else {
            return Ok(());
        };
        let signed: Signed = decode_json(signed_json).map_err(invalid)?;
        let review: Review = decode_json(&pending.review).map_err(invalid)?;
        let sender = wallet.account(Chain::BitcoinSv).ok_or_else(|| invalid("Cold wallet BSV account missing"))?;
        self.addresses
            .save_signed(
                wallet,
                &reservation.id,
                BsvHomeTransactionCheckpointData {
                    version: 2,
                    bsv_spend: Some(reservation.spend.clone()),
                    original_value: Some(review.amount.into()),
                    use_max_amount: false,
                    gas_price: Some(reservation.gas_price.clone()),
                    wallet_id: wallet.id.clone(),
                    asset_id: Chain::BitcoinSv.as_asset_id(),
                    sender_address: sender.address.clone(),
                    recipient_address: review.recipient,
                    value: review.amount.into(),
                    network_fee: review.fee.into(),
                    fee_asset_id: Chain::BitcoinSv.as_asset_id(),
                    transaction_id: signed.transaction_id,
                    signed_data: signed.raw_transaction,
                },
            )
            .await
    }
}

fn pending(state: &State, pairing: &Pairing, commitment: &str) -> Result<BsvHomeBsvColdWalletPending, BsvHomeServiceError> {
    let value = state
        .next
        .as_ref()
        .or(state.pending.as_ref())
        .ok_or_else(|| invalid("Cold wallet request missing"))?
        .validate(pairing)?;
    if value.review.commitment != commitment {
        return Err(invalid("Cold wallet request changed after review"));
    }
    Ok(value)
}
