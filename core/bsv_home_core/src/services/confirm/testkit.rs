use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use async_trait::async_trait;
use futures::channel::oneshot;
use primitives::{Asset, AssetBasic, AssetFull, AssetId, Chain, DelegationBase, DelegationValidator, StakeProviderType, Transaction, Wallet, WalletId};

use super::SendInput;
use super::{
    BsvHomeConfirmAuthorization, BsvHomeConfirmData, BsvHomeConfirmInput, BsvHomeConfirmService, BsvHomeConfirmTransferService, BsvHomeTransactionCheckpoint,
    BsvHomeTransactionCheckpointData, BsvHomeTransactionSigner,
};
use crate::BsvHomeCoreError;
use crate::alien::AlienProvider;
use crate::alien::{AlienError, AlienResponse, AlienTarget};
use crate::api::{BsvHomeApiClient, BsvHomeDeviceApiClient, BsvHomeStaticApiClient};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::keystore::BsvHomeKeystore;
use crate::keystore::{BsvHomeImportType, decode_password};
use crate::models::transaction::{BsvHomeSignedTransaction, BsvHomeSignerInput, BsvHomeTransactionLoadFee, BsvHomeTransactionLoadMetadata};
use crate::services::assets::{BsvHomeAssetStore, BsvHomeAssetsService, config::BsvHomeAssetConfigService};
use crate::services::balance::{BsvHomeAssetBalance, BsvHomeBalanceRecord, BsvHomeBalanceService, BsvHomeBalanceStore};
use crate::services::bsv_cold_wallet::testkit::MemoryColdWalletStore;
use crate::services::device::BsvHomeDeviceKeyService;
use crate::services::error::BsvHomeServiceError;
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::name::BsvHomeNameService;
use crate::services::nft::{BsvHomeNftService, testkit::MemoryNftStore};
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
use crate::services::price::{BsvHomePriceService, testkit::MemoryPriceStore};
use crate::services::stake::{BsvHomeStakeService, BsvHomeStakeStore};
use crate::services::stream::testkit::SubscriptionTestkit;
use crate::services::transaction_state::{BsvHomeTransactionStateService, BsvHomeTransactionStatusService, testkit::MemoryTransactionStateStore};
use crate::services::transfer::{BsvHomeRecentActivityService, testkit::MemoryRecentActivityStore};
use crate::services::transfer::{BsvHomeRecipient, BsvHomeTransferData};
use crate::services::wallet::addresses::BsvHomeBsvAddressPool;
use crate::services::wallet::addresses::testkit::{MemoryBsvAddressStore, pool, pool_with_keystore};
use crate::services::wallet::rules as wallet_rules;
use crate::services::wallet::testkit::TEST_PASSWORD;
use crate::services::wallet::testkit::{MemoryAddressStore, MemoryKeystorePassword, MemoryWalletStore};
use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
use crate::services::{BsvHomeScanService, BsvHomeSimulationService};
use crate::testkit::TestAlienProvider;
use bsv_home_bsv::BsvAddress;
use bsv_home_bsv::signer::BsvChainSigner;
use bsv_home_bsv::signer::{finalize_spend_metadata, plan_transfer};
use bsv_home_derivation::{bsv_account_xpub, bsv_address};
use bsv_home_tron::models::TronTransactionBroadcast;
use bsv_home_tron::signer::TronChainSigner;
use num_bigint::BigInt;
use primitives::testkit::ABANDON_PHRASE;
use primitives::testkit::signer_mock::TEST_PRIVATE_KEY;
use primitives::{Account, FeePriority, GasPriceType, TransactionInputType};
use primitives::{Address, BsvAddressBranch, BsvAddressLocator, BsvSpendChange, BsvSpendInput, BsvSpendMetadata, TransactionLoadMetadata, UTXO};
use primitives::{ChainSigner, TransactionType};
use tempfile::TempDir;

pub struct ConfirmTestkit {
    pub service: Arc<BsvHomeConfirmTransferService>,
    pub confirm: Arc<BsvHomeConfirmService>,
    pub balances: Arc<MemoryBalanceStore>,
    pub transaction_status: Arc<MemoryTransactionStatus>,
    pub bsv_addresses: Arc<BsvHomeBsvAddressPool>,
    pub wallet_session: Arc<BsvHomeWalletSessionService>,
    pub wallets: Arc<MemoryWalletStore>,
    pub cold_store: Arc<MemoryColdWalletStore>,
}

impl ConfirmTestkit {
    pub fn new(wallet: Wallet, selected_wallet: Wallet) -> Self {
        let provider = Arc::new(TestAlienProvider::with_status(503));
        Self::with_provider(wallet, selected_wallet, provider)
    }

    pub fn with_provider(wallet: Wallet, selected_wallet: Wallet, provider: Arc<dyn AlienProvider>) -> Self {
        Self::with_environment(wallet, selected_wallet, provider, crate::services::chain::BsvHomeRuntimeEnvironment::Production)
    }

    pub fn with_environment(wallet: Wallet, selected_wallet: Wallet, provider: Arc<dyn AlienProvider>, environment: crate::services::chain::BsvHomeRuntimeEnvironment) -> Self {
        Self::with_address_dependencies(wallet, selected_wallet, provider, None, environment, Arc::new(UnusedSigner))
    }

    pub fn with_bsv_provider(wallet: Wallet, provider: Arc<dyn AlienProvider>, keystore: Arc<BsvHomeKeystore>, store: Arc<MemoryBsvAddressStore>) -> Self {
        Self::with_address_dependencies(
            wallet.clone(),
            wallet,
            provider,
            Some((keystore, store)),
            crate::services::chain::BsvHomeRuntimeEnvironment::Production,
            Arc::new(UnusedSigner),
        )
    }

    pub fn with_signer(wallet: Wallet, provider: Arc<dyn AlienProvider>, signer: Arc<dyn BsvHomeTransactionSigner>) -> Self {
        Self::with_address_dependencies(
            wallet.clone(),
            wallet,
            provider,
            None,
            crate::services::chain::BsvHomeRuntimeEnvironment::Production,
            signer,
        )
    }

    pub fn with_bsv_signer(
        wallet: Wallet,
        provider: Arc<dyn AlienProvider>,
        keystore: Arc<BsvHomeKeystore>,
        store: Arc<MemoryBsvAddressStore>,
        signer: Arc<dyn BsvHomeTransactionSigner>,
    ) -> Self {
        Self::with_address_dependencies(
            wallet.clone(),
            wallet,
            provider,
            Some((keystore, store)),
            crate::services::chain::BsvHomeRuntimeEnvironment::Production,
            signer,
        )
    }

    fn with_address_dependencies(
        wallet: Wallet,
        selected_wallet: Wallet,
        provider: Arc<dyn AlienProvider>,
        bsv: Option<(Arc<BsvHomeKeystore>, Arc<MemoryBsvAddressStore>)>,
        environment: crate::services::chain::BsvHomeRuntimeEnvironment,
        signer: Arc<dyn BsvHomeTransactionSigner>,
    ) -> Self {
        let preferences_store = Arc::new(MemoryPreferencesStore::default());
        let preferences = Arc::new(BsvHomePreferencesService::new(preferences_store.clone()));
        let selected = Arc::new(MemoryWalletSessionStore {
            current: Mutex::new(Some(selected_wallet.id.clone())),
        });
        let wallets = Arc::new(MemoryWalletStore {
            wallets: Mutex::new(vec![wallet.clone(), selected_wallet]),
            bsv_addresses: bsv.as_ref().map(|(_, store)| store.clone()).unwrap_or_default(),
            ..Default::default()
        });
        let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(selected.clone(), wallets.clone(), environment));
        let gateway = Arc::new(BsvHomeGateway::new_with_environment(
            provider.clone(),
            preferences_store,
            Arc::new(EmptyPreferences),
            environment,
        ));
        let api = Arc::new(BsvHomeApiClient::new(provider.clone()));
        let device_api = Arc::new(BsvHomeDeviceApiClient::new(
            provider.clone(),
            Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
        ));
        let price = Arc::new(BsvHomePriceService::new(Arc::new(MemoryPriceStore::default()), preferences.clone()));
        let asset_store = Arc::new(MemoryAssetStore);
        let assets = Arc::new(BsvHomeAssetsService::new(
            api,
            gateway.clone(),
            asset_store.clone(),
            price.clone(),
            preferences.clone(),
            session.clone(),
        ));
        let balances = Arc::new(MemoryBalanceStore {
            wallet_id: wallet.id,
            requests: Mutex::new(Vec::new()),
        });
        let address_pool = match bsv {
            Some((keystore, store)) => pool_with_keystore(gateway.clone(), wallets.clone(), keystore, Arc::new(MemoryKeystorePassword::default()), store),
            None => pool(gateway.clone(), wallets.clone()),
        };
        let balance = Arc::new(BsvHomeBalanceService::new(
            gateway.clone(),
            wallets.clone(),
            asset_store,
            balances.clone(),
            assets.clone(),
            Arc::new(SubscriptionTestkit::new(&[], &[]).service),
            address_pool.clone(),
        ));
        let explorer = Arc::new(BsvHomeExplorerService::new(preferences.clone()));
        let addresses = Arc::new(MemoryAddressStore::default());
        let stake = Arc::new(BsvHomeStakeService::new(
            gateway.clone(),
            Arc::new(BsvHomeStaticApiClient::new(provider.clone())),
            Arc::new(UnusedStakeStore),
            addresses.clone(),
            explorer.clone(),
            preferences.clone(),
            session.clone(),
        ));
        let nft = Arc::new(BsvHomeNftService::new(device_api.clone(), Arc::new(MemoryNftStore::default()), session.clone()));
        let transactions = Arc::new(BsvHomeTransactionStateService::new(
            gateway.clone(),
            Arc::new(MemoryTransactionStateStore::default()),
            assets.clone(),
            balance.clone(),
            stake,
            nft,
        ));
        let transaction_status = Arc::new(MemoryTransactionStatus::default());
        let cold_store = Arc::new(MemoryColdWalletStore::default());
        let confirm = Arc::new(BsvHomeConfirmService::new(
            gateway,
            Arc::new(BsvHomeSimulationService::new(provider, Arc::new(EmptyPreferences))),
            Arc::new(BsvHomeScanService::new(device_api.clone())),
            transactions,
            balance,
            price,
            assets,
            transaction_status.clone(),
            address_pool.clone(),
            cold_store.clone(),
        ));
        let service = Arc::new(BsvHomeConfirmTransferService::new(
            confirm.clone(),
            explorer,
            Arc::new(BsvHomeNameService::new(device_api, addresses)),
            Arc::new(BsvHomeAssetConfigService::new()),
            signer,
            Arc::new(MemoryKeystorePassword::default()),
            Arc::new(BsvHomeRecentActivityService::new(Arc::new(MemoryRecentActivityStore::default()), session.clone())),
            preferences,
        ));
        Self {
            service,
            confirm,
            balances,
            transaction_status,
            bsv_addresses: address_pool,
            wallet_session: session,
            wallets,
            cold_store,
        }
    }
}

pub struct MemoryBalanceStore {
    wallet_id: WalletId,
    pub requests: Mutex<Vec<WalletId>>,
}

#[async_trait]
impl BsvHomeBalanceStore for MemoryBalanceStore {
    async fn get_available_balances(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>) -> Result<Vec<BsvHomeAssetBalance>, BsvHomeServiceError> {
        self.requests.lock().unwrap().push(wallet_id.clone());
        if wallet_id != self.wallet_id {
            return Ok(Vec::new());
        }
        Ok(asset_ids
            .into_iter()
            .map(|asset_id| BsvHomeAssetBalance {
                asset_id,
                available: 100u32.into(),
                ..BsvHomeAssetBalance::mock()
            })
            .collect())
    }
    async fn update_balances(&self, _: WalletId, _: Vec<BsvHomeBalanceRecord>) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected balance write")
    }
    async fn get_enabled_asset_ids(&self, _: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        panic!("unexpected enabled assets read")
    }
    async fn set_assets_enabled(&self, _: WalletId, _: Vec<AssetId>, _: bool) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected asset enable")
    }
    async fn set_asset_pinned(&self, _: WalletId, _: AssetId, _: bool) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected asset pin")
    }
}

struct MemoryAssetStore;

#[async_trait]
impl BsvHomeAssetStore for MemoryAssetStore {
    async fn get_asset_ids(&self, asset_ids: Vec<AssetId>) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        Ok(asset_ids)
    }
    async fn get_assets(&self, asset_ids: Vec<AssetId>) -> Result<Vec<Asset>, BsvHomeServiceError> {
        Ok(asset_ids.into_iter().map(|id| Asset::from_chain(id.chain)).collect())
    }
    async fn save_assets(&self, _: Vec<AssetBasic>) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected asset write")
    }
    async fn save_asset(&self, _: AssetFull) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected asset write")
    }
    async fn add_missing_balances(&self, _: WalletId, _: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected balance write")
    }
    async fn add_balances(&self, _: WalletId, _: Vec<AssetId>, _: bool) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected balance write")
    }
    async fn set_buyable_assets(&self, _: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected asset write")
    }
    async fn set_sellable_assets(&self, _: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected asset write")
    }
    async fn set_swappable_assets(&self, _: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected asset write")
    }
    async fn set_stakeable_assets(&self, _: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected asset write")
    }
}

pub(crate) struct UnusedStakeStore;

#[async_trait]
impl BsvHomeStakeStore for UnusedStakeStore {
    async fn get_apr(&self, _: AssetId, _: StakeProviderType) -> Result<Option<f64>, BsvHomeServiceError> {
        panic!("unexpected stake read")
    }
    async fn get_validators(&self, _: AssetId, _: StakeProviderType) -> Result<Vec<DelegationValidator>, BsvHomeServiceError> {
        panic!("unexpected stake read")
    }
    async fn save_validators(&self, _: Vec<DelegationValidator>) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected stake write")
    }
    async fn deactivate_validators(&self, _: AssetId, _: Vec<String>) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected stake write")
    }
    async fn get_delegation_ids(&self, _: WalletId, _: AssetId, _: StakeProviderType) -> Result<Vec<String>, BsvHomeServiceError> {
        panic!("unexpected stake read")
    }
    async fn update_delegations(&self, _: WalletId, _: Vec<DelegationBase>, _: Vec<String>) -> Result<(), BsvHomeServiceError> {
        panic!("unexpected stake write")
    }
}

struct UnusedSigner;

#[async_trait]
impl BsvHomeTransactionSigner for UnusedSigner {
    async fn sign(&self, _: Wallet, _: BsvHomeSignerInput) -> Result<Vec<BsvHomeSignedTransaction>, BsvHomeCoreError> {
        panic!("unexpected signing")
    }
}

#[derive(Default)]
pub struct MemoryTransactionCheckpoint {
    pub saved: Mutex<Option<BsvHomeTransactionCheckpointData>>,
    pub fail_writes: bool,
    pub cancel_after_save: bool,
    pub cancelled: AtomicBool,
    pub authorizations: AtomicUsize,
    pub authorization_gate: Mutex<Option<oneshot::Receiver<()>>>,
}

#[async_trait]
impl BsvHomeTransactionCheckpoint for MemoryTransactionCheckpoint {
    async fn load(&self) -> Result<Option<BsvHomeTransactionCheckpointData>, BsvHomeCoreError> {
        Ok(self.saved.lock().unwrap().clone())
    }

    async fn save(&self, data: BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeCoreError> {
        if self.fail_writes {
            return Err(BsvHomeCoreError::AnyError {
                msg: "checkpoint storage failed".to_string(),
            });
        }
        let mut saved = self.saved.lock().unwrap();
        if saved.as_ref().is_some_and(|saved| *saved != data) {
            return Err(BsvHomeCoreError::AnyError {
                msg: "payment is already signed".to_string(),
            });
        }
        *saved = Some(data);
        if self.cancel_after_save {
            self.cancelled.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    async fn authorize_broadcast(&self, data: BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeCoreError> {
        self.authorizations.fetch_add(1, Ordering::SeqCst);
        let gate = self.authorization_gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.await.unwrap();
        }
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(BsvHomeCoreError::Cancelled);
        }
        if self.saved.lock().unwrap().as_ref() != Some(&data) {
            return Err(BsvHomeCoreError::AnyError {
                msg: "payment is not durably saved".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryConfirmAuthorization {
    pub calls: Mutex<Vec<(Wallet, BsvHomeTransferData)>>,
    pub gate: Mutex<Option<oneshot::Receiver<()>>>,
    pub denied: AtomicBool,
}

#[async_trait]
impl BsvHomeConfirmAuthorization for MemoryConfirmAuthorization {
    async fn authorize(&self, wallet: Wallet, transfer: BsvHomeTransferData) -> Result<(), BsvHomeCoreError> {
        self.calls.lock().unwrap().push((wallet, transfer));
        let gate = self.gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.await.unwrap();
        }
        if self.denied.load(Ordering::SeqCst) {
            return Err(BsvHomeCoreError::Cancelled);
        }
        Ok(())
    }
}

pub struct GatedTransactionSigner {
    pub signer: Arc<dyn BsvHomeTransactionSigner>,
    pub gate: Mutex<Option<oneshot::Receiver<()>>>,
}

#[async_trait]
impl BsvHomeTransactionSigner for GatedTransactionSigner {
    async fn sign(&self, wallet: Wallet, input: BsvHomeSignerInput) -> Result<Vec<BsvHomeSignedTransaction>, BsvHomeCoreError> {
        let gate = self.gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.await.unwrap();
        }
        self.signer.sign(wallet, input).await
    }
}

#[derive(Default)]
pub struct BsvTransactionSigner {
    pub calls: AtomicUsize,
}

#[derive(Default)]
pub struct DerivedBsvTransactionSigner {
    pub calls: AtomicUsize,
    pub fail_signed_save: Option<Arc<MemoryBsvAddressStore>>,
}

#[async_trait]
impl BsvHomeTransactionSigner for DerivedBsvTransactionSigner {
    async fn sign(&self, _: Wallet, input: BsvHomeSignerInput) -> Result<Vec<BsvHomeSignedTransaction>, BsvHomeCoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let data = BsvChainSigner
            .sign_transfer_with_keys(&input.into(), |locator| {
                bsv_home_derivation::bsv_private_key(primitives::testkit::ABANDON_PHRASE, locator).map_err(primitives::SignerError::from_display)
            })
            .map_err(|error| BsvHomeCoreError::AnyError { msg: error.to_string() })?;
        if let Some(store) = &self.fail_signed_save {
            *store.save_error.lock().unwrap() = Some(BsvHomeServiceError::Store {
                msg: "HD signed checkpoint storage failed".to_string(),
            });
        }
        Ok(vec![BsvHomeSignedTransaction {
            data,
            transaction_type: TransactionType::Transfer,
        }])
    }
}

#[async_trait]
impl BsvHomeTransactionSigner for BsvTransactionSigner {
    async fn sign(&self, _: Wallet, input: BsvHomeSignerInput) -> Result<Vec<BsvHomeSignedTransaction>, BsvHomeCoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let data = BsvChainSigner
            .sign_transfer(&input.into(), &TEST_PRIVATE_KEY)
            .map_err(|error| BsvHomeCoreError::AnyError { msg: error.to_string() })?;
        Ok(vec![BsvHomeSignedTransaction {
            data,
            transaction_type: TransactionType::Transfer,
        }])
    }
}

#[derive(Default)]
pub struct TronTransactionSigner {
    pub calls: AtomicUsize,
}

#[async_trait]
impl BsvHomeTransactionSigner for TronTransactionSigner {
    async fn sign(&self, _: Wallet, input: BsvHomeSignerInput) -> Result<Vec<BsvHomeSignedTransaction>, BsvHomeCoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let data = TronChainSigner
            .sign_transfer(&input.into(), &TEST_PRIVATE_KEY)
            .map_err(|error| BsvHomeCoreError::AnyError { msg: error.to_string() })?;
        Ok(vec![BsvHomeSignedTransaction {
            data,
            transaction_type: TransactionType::Transfer,
        }])
    }
}

#[derive(Debug, Default)]
pub struct RecordingBroadcastProvider {
    pub requests: Mutex<Vec<AlienTarget>>,
    pub tron_transaction_id: Option<String>,
}

#[async_trait]
impl AlienProvider for RecordingBroadcastProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        let broadcast = target.url.ends_with("/tx/raw");
        let tron_broadcast = target.url.ends_with("/wallet/broadcasttransaction");
        self.requests.lock().unwrap().push(target);
        if tron_broadcast {
            if let Some(hash) = &self.tron_transaction_id {
                let response = TronTransactionBroadcast {
                    txid: Some(hash.clone()),
                    code: None,
                    message: None,
                };
                return Ok(Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&response).unwrap())));
            }
            return Err(AlienError::RequestError {
                msg: "broadcast timed out".into(),
            });
        }
        if broadcast {
            return Err(AlienError::RequestError {
                msg: "broadcast timed out".to_string(),
            });
        }
        Ok(Arc::new(AlienResponse::new(Some(400), Vec::new())))
    }
}

#[derive(Default)]
pub struct MemoryTransactionStatus {
    pub tracked: Mutex<Vec<(WalletId, Vec<Transaction>)>>,
}

impl BsvHomeTransactionStatusService for MemoryTransactionStatus {
    fn track(&self, wallet_id: WalletId, transactions: Vec<Transaction>) {
        self.tracked.lock().unwrap().push((wallet_id, transactions));
    }
}

pub(super) fn confirm_data(chain: Chain, input_type: TransactionInputType, from: &str) -> BsvHomeConfirmData {
    BsvHomeConfirmData {
        input: BsvHomeConfirmInput {
            from: Account::mock(chain, from),
            transfer: BsvHomeTransferData {
                input_type,
                recipient: BsvHomeRecipient {
                    address: "recipient".to_string(),
                    name: None,
                    memo: Some("memo".to_string()),
                    references: vec![],
                },
                value: BigInt::from(10),
                use_max_amount: true,
            },
        },
        fee: BsvHomeTransactionLoadFee {
            fee: BigInt::ZERO,
            gas_price_type: GasPriceType::Regular { gas_price: BigInt::from(5) },
            gas_limit: BigInt::from(21_000),
            options: Default::default(),
            fee_asset: AssetId::from_chain(Chain::Solana),
        },
        selected_priority: FeePriority::Normal,
        fee_rates: vec![],
        metadata: BsvHomeTransactionLoadMetadata::None,
        simulation: None,
    }
}

pub struct DerivedSendContext {
    _directory: TempDir,
    pub keystore: Arc<BsvHomeKeystore>,
    pub input: SendInput,
}

impl DerivedSendContext {
    pub fn new() -> Self {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let stored = keystore
            .create_store(
                BsvHomeImportType::SinglePhrase {
                    words: ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                    chain: Chain::BitcoinSv,
                },
                decode_password(TEST_PASSWORD),
            )
            .unwrap();
        let wallet = Wallet {
            id: WalletId::from_id(&stored.wallet_id).unwrap(),
            wallet_type: stored.wallet_type,
            accounts: stored.accounts.into_iter().map(wallet_rules::account).collect(),
            ..Wallet::mock()
        };
        let xpub = bsv_account_xpub(ABANDON_PHRASE).unwrap();
        let inputs = (0..2)
            .map(|index| {
                let locator = BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index,
                };
                let address = bsv_address(&xpub, locator).unwrap();
                BsvSpendInput {
                    transaction_id: format!("{:064}", index + 1),
                    vout: 0,
                    value: [50000, 30000][index as usize],
                    script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
                    address,
                    locator,
                }
            })
            .collect();
        let locator = BsvAddressLocator {
            branch: BsvAddressBranch::Change,
            index: 0,
        };
        let address = bsv_address(&xpub, locator).unwrap();
        let spend = BsvSpendMetadata {
            inputs,
            change: Some(BsvSpendChange {
                script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
                address,
                locator,
            }),
        };
        let mut input = bsv_send_input();
        input.confirm.input.from = wallet.account(Chain::BitcoinSv).unwrap().clone();
        input.wallet = wallet;
        input.confirm.input.transfer.recipient = BsvHomeRecipient::address("1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".to_string());
        input.confirm.input.transfer.value = 60000.into();
        input.confirm.metadata = TransactionLoadMetadata::BitcoinSv { spend };
        input.confirm.fee.fee = 38.into();
        input.value = 60000.into();
        input.network_fee = 38.into();
        let signer_input: primitives::SignerInput = input.signer_input().unwrap().into();
        let plan = plan_transfer(&signer_input.input).unwrap();
        input.confirm.metadata = finalize_spend_metadata(&signer_input.input, &plan).unwrap();
        Self {
            _directory: directory,
            keystore,
            input,
        }
    }

    pub fn testkit(&self, provider: Arc<RecordingBroadcastProvider>, store: Arc<MemoryBsvAddressStore>) -> ConfirmTestkit {
        ConfirmTestkit::with_bsv_provider(self.input.wallet.clone(), provider, self.keystore.clone(), store)
    }
}

pub fn bsv_send_input() -> SendInput {
    let chain = Chain::BitcoinSv;
    let address = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
    let wallet = Wallet::mock_with_accounts(vec![Account::mock(chain, address)]);
    let input_type = TransactionInputType::Transfer { asset: Asset::from_chain(chain) };
    let mut confirm = confirm_data(chain, input_type, address);
    confirm.input.transfer.recipient = BsvHomeRecipient::address(address.to_string());
    confirm.input.transfer.value = 10000.into();
    confirm.input.transfer.use_max_amount = false;
    confirm.fee.fee = 23.into();
    confirm.fee.gas_price_type = GasPriceType::regular(100);
    confirm.fee.gas_limit = 1.into();
    confirm.fee.fee_asset = chain.as_asset_id();
    confirm.metadata = TransactionLoadMetadata::Bitcoin {
        utxos: vec![UTXO {
            transaction_id: format!("{:064}", 1),
            vout: 0,
            value: 50000u32.into(),
            address: address.to_string(),
        }],
    };
    SendInput {
        wallet,
        confirm,
        value: 10000.into(),
        network_fee: 23.into(),
        simulation: None,
    }
}
