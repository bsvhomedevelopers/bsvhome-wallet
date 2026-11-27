pub mod model;
pub mod rules;
pub mod store;
#[cfg(test)]
pub(crate) mod testkit;

use crate::services::error::BsvHomeServiceError;
use std::sync::Arc;

use futures::future::join_all;
use primitives::{Asset, AssetBalance, AssetId, Chain, Wallet, WalletId};

pub use model::{BsvHomeAssetBalance, BsvHomeBalanceRecord, BsvHomeBalanceRequirement, BsvHomeBalanceRow, BsvHomeBalanceUpdate, BsvHomeBalanceUpdateType, BsvHomeBalanceValue};
pub use store::BsvHomeBalanceStore;

use crate::gateway::BsvHomeGateway;
use crate::services::assets::{BsvHomeAssetStore, BsvHomeAssetsService};
use crate::services::stream::BsvHomeStreamSubscriptionService;
use crate::services::wallet::BsvHomeWalletStore;
use crate::services::wallet::addresses::BsvHomeBsvAddressPool;
use rules::{BalanceKind, BalanceRequest};

#[derive(Clone, Copy)]
enum BalanceRefresh {
    Independent,
    Display,
}

#[derive(uniffi::Object)]
pub struct BsvHomeBalanceService {
    gateway: Arc<BsvHomeGateway>,
    wallet_store: Arc<dyn BsvHomeWalletStore>,
    asset_store: Arc<dyn BsvHomeAssetStore>,
    store: Arc<dyn BsvHomeBalanceStore>,
    assets: Arc<BsvHomeAssetsService>,
    stream: Arc<BsvHomeStreamSubscriptionService>,
    addresses: Arc<BsvHomeBsvAddressPool>,
}

#[uniffi::export]
impl BsvHomeBalanceService {
    pub async fn balances(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>) -> Result<Vec<BsvHomeAssetBalance>, BsvHomeServiceError> {
        for asset_id in &asset_ids {
            self.gateway.require_asset(asset_id)?;
        }
        self.store.get_available_balances(wallet_id, asset_ids).await
    }

    #[uniffi::constructor]
    pub fn new(
        gateway: Arc<BsvHomeGateway>,
        wallet_store: Arc<dyn BsvHomeWalletStore>,
        asset_store: Arc<dyn BsvHomeAssetStore>,
        store: Arc<dyn BsvHomeBalanceStore>,
        assets: Arc<BsvHomeAssetsService>,
        stream: Arc<BsvHomeStreamSubscriptionService>,
        addresses: Arc<BsvHomeBsvAddressPool>,
    ) -> Self {
        Self {
            gateway,
            wallet_store,
            asset_store,
            store,
            assets,
            stream,
            addresses,
        }
    }

    pub async fn set_assets_enabled(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>, enabled: bool) -> Result<(), BsvHomeServiceError> {
        for asset_id in &asset_ids {
            self.gateway.require_asset(asset_id)?;
        }
        let asset_ids = rules::unique_asset_ids(asset_ids);
        if asset_ids.is_empty() {
            return Ok(());
        }
        if !enabled
            && asset_ids.iter().any(rules::is_required_asset)
            && let Some(wallet) = self.wallet_store.get_wallet(wallet_id.clone()).await?
            && asset_ids
                .iter()
                .any(|asset_id| rules::is_required_asset(asset_id) && wallet.account(asset_id.chain).is_some())
        {
            return Err(BsvHomeServiceError::Unsupported {
                msg: "ASSET_REQUIRED:TRX".to_string(),
            });
        }
        if enabled {
            self.assets.sync_missing_assets(asset_ids.clone()).await?;
        }
        let enabled_ids = self.store.get_enabled_asset_ids(wallet_id.clone()).await?;
        self.add_missing_balances(wallet_id.clone(), asset_ids.clone()).await?;
        self.store.set_assets_enabled(wallet_id.clone(), asset_ids.clone(), enabled).await?;
        if enabled {
            self.refresh_enabled_assets(wallet_id, rules::missing_asset_ids(&asset_ids, &enabled_ids)).await;
        } else {
            let _ = self.stream.resubscribe().await;
        }
        Ok(())
    }

    pub async fn set_asset_pinned(&self, wallet_id: WalletId, asset_id: AssetId, pinned: bool) -> Result<(), BsvHomeServiceError> {
        self.gateway.require_asset(&asset_id)?;
        if pinned {
            self.set_assets_enabled(wallet_id.clone(), vec![asset_id.clone()], true).await?;
        }
        self.store.set_asset_pinned(wallet_id, asset_id, pinned).await
    }

    pub async fn update(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        self.refresh_asset_balances(wallet_id, asset_ids, BalanceRefresh::Independent).await
    }
}

impl BsvHomeBalanceService {
    async fn refresh_asset_balances(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>, refresh: BalanceRefresh) -> Result<(), BsvHomeServiceError> {
        for asset_id in &asset_ids {
            self.gateway.require_asset(asset_id)?;
        }
        let Some(wallet) = self
            .wallet_store
            .get_wallet(wallet_id.clone())
            .await
            .map_err(|error| BsvHomeServiceError::Store { msg: error.to_string() })?
        else {
            return Ok(());
        };
        for asset_id in &asset_ids {
            self.gateway.require_wallet_chain(&wallet, asset_id.chain)?;
        }
        let wallet = self.gateway.visible_wallet(wallet);
        let requests = rules::balance_requests(&wallet.accounts, &asset_ids);
        let (balances, failures): (Vec<_>, Vec<_>) = join_all(requests.iter().map(|request| self.chain_balances(wallet_id.clone(), request, refresh)))
            .await
            .into_iter()
            .partition(Result::is_ok);
        let balances: Vec<(BalanceKind, AssetBalance)> = balances
            .into_iter()
            .flatten()
            .flatten()
            .filter(|(_, balance)| self.gateway.allows_asset(&balance.asset_id))
            .collect();
        if !balances.is_empty() {
            let assets = self
                .asset_store
                .get_assets(balances.iter().map(|(_, balance)| balance.asset_id.clone()).collect())
                .await
                .map_err(|error| BsvHomeServiceError::Store { msg: error.to_string() })?;
            self.write_balances(wallet_id, rules::balance_updates(balances), &assets).await?;
        }
        match failures.into_iter().find_map(Result::err) {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(crate) async fn publish_bsv_receipt_balance(&self, wallet_id: WalletId, balance: AssetBalance) -> Result<(), BsvHomeServiceError> {
        self.assets.ensure_asset(Chain::BitcoinSv.as_asset_id()).await?;
        self.update_balances(wallet_id, rules::balance_updates(vec![(BalanceKind::Coin, balance)])).await
    }

    pub(crate) fn allows_asset(&self, asset_id: &AssetId) -> bool {
        self.gateway.allows_asset(asset_id)
    }

    pub(crate) async fn enabled_asset_ids(&self, wallet_id: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        Ok(self
            .store
            .get_enabled_asset_ids(wallet_id)
            .await?
            .into_iter()
            .filter(|asset_id| self.gateway.allows_asset(asset_id))
            .collect())
    }

    pub(crate) async fn update_enabled_display_balances(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        let asset_ids = self.enabled_asset_ids(wallet_id.clone()).await?;
        self.refresh_asset_balances(wallet_id, asset_ids, BalanceRefresh::Display).await
    }

    pub async fn update_enabled_balances(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        let asset_ids = self.enabled_asset_ids(wallet_id.clone()).await?;
        self.update(wallet_id, asset_ids).await
    }

    pub async fn setup_wallet(&self, wallet: Wallet) -> Result<(), BsvHomeServiceError> {
        let enabled = self.initialize_wallet_balances(wallet.clone()).await?;
        self.refresh_enabled_assets(wallet.id, enabled).await;
        Ok(())
    }

    pub(crate) async fn initialize_wallet_balances(&self, wallet: Wallet) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        let wallet = self.gateway.visible_wallet(wallet);
        let (mut enabled, mut disabled) = crate::services::assets::rules::default_balances(&wallet);
        let development_ids = self.assets.development_asset_ids(&wallet);
        disabled.retain(|asset_id| !development_ids.contains(asset_id));
        enabled.extend(development_ids);
        enabled.retain(|asset_id| self.gateway.allows_asset(asset_id));
        disabled.retain(|asset_id| self.gateway.allows_asset(asset_id));
        let enabled = rules::unique_asset_ids(enabled);
        let stored = self.store.get_available_balances(wallet.id.clone(), [enabled.clone(), disabled.clone()].concat()).await?;
        let stored_ids: Vec<AssetId> = stored.into_iter().map(|balance| balance.asset_id).collect();
        let required = enabled
            .iter()
            .filter(|asset_id| rules::is_required_asset(asset_id) && stored_ids.contains(asset_id))
            .cloned()
            .collect::<Vec<_>>();
        let reenabled = if required.is_empty() {
            Vec::new()
        } else {
            let enabled_ids = self.store.get_enabled_asset_ids(wallet.id.clone()).await?;
            rules::missing_asset_ids(&required, &enabled_ids)
        };
        let enabled = rules::missing_asset_ids(&enabled, &stored_ids);
        let disabled = rules::missing_asset_ids(&disabled, &stored_ids);
        self.assets.add_balances(wallet.id.clone(), enabled.clone(), true).await?;
        self.assets.add_balances(wallet.id.clone(), disabled, false).await?;
        if !reenabled.is_empty() {
            self.store.set_assets_enabled(wallet.id, reenabled.clone(), true).await?;
        }
        Ok([enabled, reenabled].concat())
    }

    async fn add_missing_balances(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        let stored = self.store.get_available_balances(wallet_id.clone(), asset_ids.clone()).await?;
        let stored_ids: Vec<AssetId> = stored.into_iter().map(|balance| balance.asset_id).collect();
        self.assets.add_missing_balances(wallet_id, rules::missing_asset_ids(&asset_ids, &stored_ids)).await
    }

    async fn refresh_enabled_assets(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>) {
        if asset_ids.is_empty() {
            return;
        }
        let _ = self.stream.resubscribe().await;
        let _ = self.update(wallet_id, asset_ids).await;
    }

    pub async fn update_balances(&self, wallet_id: WalletId, updates: Vec<BsvHomeBalanceUpdate>) -> Result<(), BsvHomeServiceError> {
        let updates = updates.into_iter().filter(|update| self.gateway.allows_asset(&update.asset_id)).collect::<Vec<_>>();
        let assets = self
            .asset_store
            .get_assets(updates.iter().map(|update| update.asset_id.clone()).collect())
            .await
            .map_err(|error| BsvHomeServiceError::Store { msg: error.to_string() })?;
        self.write_balances(wallet_id, updates, &assets).await
    }

    async fn write_balances(&self, wallet_id: WalletId, updates: Vec<BsvHomeBalanceUpdate>, assets: &[Asset]) -> Result<(), BsvHomeServiceError> {
        let asset_ids: Vec<AssetId> = rules::unique_asset_ids(updates.iter().map(|update| update.asset_id.clone()).collect());
        let stored = self.store.get_available_balances(wallet_id.clone(), asset_ids.clone()).await?;
        let stored_ids: Vec<AssetId> = stored.iter().map(|balance| balance.asset_id.clone()).collect();
        self.assets
            .add_missing_balances(wallet_id.clone(), rules::missing_asset_ids(&asset_ids, &stored_ids))
            .await?;
        let records = rules::balance_records(rules::changed_balances(stored, updates), assets);
        if records.is_empty() {
            return Ok(());
        }
        self.store.update_balances(wallet_id, records).await
    }

    async fn chain_balances(&self, wallet_id: WalletId, request: &BalanceRequest, refresh: BalanceRefresh) -> Result<Vec<(BalanceKind, AssetBalance)>, BsvHomeServiceError> {
        let token_ids = rules::request_token_ids(&request.token_ids);
        let (coin, stake, tokens, earn) = futures::join!(
            async {
                if request.coin {
                    if request.chain == Chain::BitcoinSv {
                        let balance = match refresh {
                            BalanceRefresh::Independent => {
                                if self.addresses.snapshot(wallet_id.clone()).await?.is_some() {
                                    self.addresses.sync(wallet_id.clone()).await?;
                                }
                                self.addresses.balance(wallet_id, false).await
                            }
                            BalanceRefresh::Display => self.addresses.refresh_display_balance(wallet_id).await,
                        }?;
                        Ok(vec![balance])
                    } else {
                        self.gateway
                            .get_balance_coin(request.chain, request.address.clone())
                            .await
                            .map(|balance| vec![balance])
                            .map_err(BsvHomeServiceError::from)
                    }
                } else {
                    Ok(Vec::new())
                }
            },
            async {
                if request.coin {
                    self.gateway
                        .get_balance_staking(request.chain, request.address.clone())
                        .await
                        .map(|balance| balance.into_iter().collect())
                } else {
                    Ok(Vec::new())
                }
            },
            async {
                if token_ids.is_empty() {
                    Ok(Vec::new())
                } else {
                    self.gateway.get_balance_tokens(request.chain, request.address.clone(), token_ids.clone()).await
                }
            },
            async {
                if token_ids.is_empty() {
                    Ok(Vec::new())
                } else {
                    self.gateway.get_balance_earn(request.chain, request.address.clone(), token_ids.clone()).await
                }
            },
        );
        Ok(rules::chain_balances(coin?, stake?, tokens?, earn?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alien::{AlienProvider, AlienResponse};
    use crate::api::BsvHomeApiClient;
    use crate::gateway::{BsvHomeGateway, EmptyPreferences};
    use crate::services::assets::testkit::MemoryAssetStore;
    use crate::services::chain::BsvHomeRuntimeEnvironment;
    use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
    use crate::services::price::{BsvHomePriceService, testkit::MemoryPriceStore};
    use crate::services::stream::testkit::SubscriptionTestkit;
    use crate::services::wallet::addresses::BsvHomeBsvAddressStatus;
    use crate::services::wallet::addresses::testkit::{BsvAddressProvider, complete_record, pool};
    use crate::services::wallet::testkit::MemoryWalletStore;
    use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
    use crate::testkit::TestAlienProvider;
    use async_trait::async_trait;
    use bsv_home_derivation::{bsv_account_xpub, bsv_address};
    use futures::executor::block_on;
    use primitives::known_assets::{TRON_SHASTA_USDT, TRON_USDT};
    use primitives::testkit::ABANDON_PHRASE;
    use primitives::{Account, BsvAddressBranch, BsvAddressLocator, Chain, WalletType};
    use serde_json::Value;
    use std::sync::Mutex;
    use url::Url;

    struct MemoryBalanceStore {
        rows: Mutex<Vec<AssetId>>,
        enabled: Mutex<Vec<AssetId>>,
        enabled_updates: Mutex<Vec<(WalletId, Vec<AssetId>, bool)>>,
        updates: Mutex<usize>,
    }

    #[async_trait]
    impl BsvHomeBalanceStore for MemoryBalanceStore {
        async fn get_available_balances(&self, _wallet_id: WalletId, asset_ids: Vec<AssetId>) -> Result<Vec<BsvHomeAssetBalance>, BsvHomeServiceError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .iter()
                .filter(|id| asset_ids.contains(id))
                .cloned()
                .map(BsvHomeAssetBalance::zero)
                .collect())
        }
        async fn update_balances(&self, _wallet_id: WalletId, _balances: Vec<BsvHomeBalanceRecord>) -> Result<(), BsvHomeServiceError> {
            *self.updates.lock().unwrap() += 1;
            Ok(())
        }
        async fn get_enabled_asset_ids(&self, _wallet_id: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError> {
            Ok(self.enabled.lock().unwrap().clone())
        }
        async fn set_assets_enabled(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>, enabled: bool) -> Result<(), BsvHomeServiceError> {
            let mut enabled_ids = self.enabled.lock().unwrap();
            enabled_ids.retain(|asset_id| !asset_ids.contains(asset_id));
            if enabled {
                enabled_ids.extend(asset_ids.clone());
            }
            self.enabled_updates.lock().unwrap().push((wallet_id, asset_ids, enabled));
            Ok(())
        }
        async fn set_asset_pinned(&self, _wallet_id: WalletId, _asset_id: AssetId, _pinned: bool) -> Result<(), BsvHomeServiceError> {
            Ok(())
        }
    }

    fn service(rows: Vec<AssetId>) -> (BsvHomeBalanceService, Arc<MemoryAssetStore>, Arc<MemoryBalanceStore>) {
        let (service, assets, balances, _) = service_with_environment(rows, BsvHomeRuntimeEnvironment::Production);
        (service, assets, balances)
    }

    fn service_with_environment(
        rows: Vec<AssetId>,
        environment: BsvHomeRuntimeEnvironment,
    ) -> (BsvHomeBalanceService, Arc<MemoryAssetStore>, Arc<MemoryBalanceStore>, Arc<TestAlienProvider>) {
        let provider = Arc::new(TestAlienProvider::with_status(503));
        service_with_provider(rows, environment, provider)
    }

    fn service_with_provider<P: AlienProvider + 'static>(
        rows: Vec<AssetId>,
        environment: BsvHomeRuntimeEnvironment,
        provider: Arc<P>,
    ) -> (BsvHomeBalanceService, Arc<MemoryAssetStore>, Arc<MemoryBalanceStore>, Arc<P>) {
        let preferences_store = Arc::new(MemoryPreferencesStore::default());
        let preferences = Arc::new(BsvHomePreferencesService::new(preferences_store.clone()));
        let gateway = Arc::new(BsvHomeGateway::new_with_environment(
            provider.clone(),
            preferences_store,
            Arc::new(EmptyPreferences),
            environment,
        ));
        let wallets = Arc::new(MemoryWalletStore::default());
        let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
            Arc::new(MemoryWalletSessionStore { current: Mutex::new(None) }),
            wallets.clone(),
            environment,
        ));
        let asset_store = Arc::new(MemoryAssetStore::default());
        let assets = Arc::new(BsvHomeAssetsService::new(
            Arc::new(BsvHomeApiClient::new(provider.clone())),
            gateway.clone(),
            asset_store.clone(),
            Arc::new(BsvHomePriceService::new(Arc::new(MemoryPriceStore::default()), preferences.clone())),
            preferences,
            session,
        ));
        let store = Arc::new(MemoryBalanceStore {
            enabled: Mutex::new(rows.clone()),
            enabled_updates: Mutex::new(Vec::new()),
            rows: Mutex::new(rows),
            updates: Mutex::new(0),
        });
        let address_pool = pool(gateway.clone(), wallets.clone());
        let service = BsvHomeBalanceService::new(
            gateway,
            wallets,
            asset_store.clone(),
            store.clone(),
            assets,
            Arc::new(SubscriptionTestkit::new(&[], &[]).service),
            address_pool,
        );
        (service, asset_store, store, provider)
    }

    fn wallet() -> Wallet {
        Wallet {
            wallet_type: WalletType::Multicoin,
            ..Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Cosmos, Chain::Ethereum, Chain::BitcoinSv, Chain::Tron], "address"))
        }
    }

    #[test]
    fn test_wowpay_balances_reject_hidden_selections_and_initialize_only_product_assets() {
        block_on(async {
            let hidden = vec![
                AssetId::from_token(Chain::Tron, "other-usdt"),
                Chain::Ethereum.as_asset_id(),
                TRON_SHASTA_USDT.id.clone(),
                Chain::Tron.as_asset_id(),
                TRON_USDT.id.clone(),
            ];
            let (service, asset_store, store, provider) = service_with_environment(hidden.clone(), BsvHomeRuntimeEnvironment::WowPay);
            let wallet = wallet();
            service.wallet_store.add_wallet(wallet.clone()).await.unwrap();
            assert_eq!(service.enabled_asset_ids(wallet.id.clone()).await.unwrap(), Vec::<AssetId>::new());
            let allowed = vec![Chain::BitcoinSv.as_asset_id()];
            assert_eq!(service.initialize_wallet_balances(wallet.clone()).await.unwrap(), allowed);
            assert_eq!(*asset_store.added_balances.lock().unwrap(), vec![(wallet.id.clone(), allowed, true)]);
            for asset_id in &hidden {
                assert!(service.set_assets_enabled(wallet.id.clone(), vec![asset_id.clone()], true).await.is_err());
                assert!(service.set_asset_pinned(wallet.id.clone(), asset_id.clone(), false).await.is_err());
                assert!(service.update(wallet.id.clone(), vec![asset_id.clone()]).await.is_err());
            }
            assert_eq!(*store.rows.lock().unwrap(), hidden);
            assert_eq!(*store.enabled.lock().unwrap(), hidden);
            assert_eq!(store.enabled_updates.lock().unwrap().len(), 0);
            assert_eq!(provider.requested_paths(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_bsv_native_balance_update_succeeds_without_staking_requests() {
        block_on(async {
            let address = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
            let native_asset_id = Chain::BitcoinSv.as_asset_id();
            let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, address)]);
            let provider = Arc::new(TestAlienProvider::with_responses(vec![(
                format!("https://api.whatsonchain.com/v1/bsv/main/address/{address}/unspent/all"),
                AlienResponse::new(Some(200), br#"{"address":"1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD","result":[],"error":""}"#.to_vec()),
            )]));
            let (service, _, store, provider) = service_with_provider(vec![native_asset_id.clone()], BsvHomeRuntimeEnvironment::Production, provider);
            service.wallet_store.add_wallet(wallet.clone()).await.unwrap();

            service.update(wallet.id, vec![native_asset_id]).await.unwrap();

            assert_eq!(provider.requested_paths(), vec![format!("/v1/bsv/main/address/{address}/unspent/all")]);
            assert_eq!(*store.updates.lock().unwrap(), 0);
        });
    }

    #[test]
    fn test_bsv_background_balance_refresh_preserves_explicit_selection_and_legacy_rotation() {
        block_on(async {
            for fixed_selection in [false, true] {
                let account_xpub = bsv_account_xpub(ABANDON_PHRASE).unwrap();
                let main = bsv_address(
                    &account_xpub,
                    BsvAddressLocator {
                        branch: BsvAddressBranch::Receive,
                        index: 0,
                    },
                )
                .unwrap();
                let next = bsv_address(
                    &account_xpub,
                    BsvAddressLocator {
                        branch: BsvAddressBranch::Receive,
                        index: 1,
                    },
                )
                .unwrap();
                let wallet = Wallet {
                    wallet_type: WalletType::Multicoin,
                    ..Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, &main)])
                };
                let provider = Arc::new(BsvAddressProvider::default());
                provider.add_output(&main, &"1".repeat(64), 50_000, false);
                let native_asset_id = Chain::BitcoinSv.as_asset_id();
                let (service, _, store, provider) = service_with_provider(vec![native_asset_id.clone()], BsvHomeRuntimeEnvironment::Production, provider);
                service.assets.ensure_default_assets().await.unwrap();
                let mut record = complete_record(&wallet, ABANDON_PHRASE);
                if !fixed_selection {
                    let mut value: Value = serde_json::from_str(&record.value).unwrap();
                    for key in ["receive_count", "selected_index"] {
                        value.as_object_mut().unwrap().remove(key);
                    }
                    record.value = serde_json::to_string(&value).unwrap();
                }
                service.wallet_store.add_wallet_with_bsv_addresses(wallet.clone(), Some(record)).await.unwrap();

                service.update(wallet.id.clone(), vec![native_asset_id]).await.unwrap();

                let snapshot = service.addresses.snapshot(wallet.id.clone()).await.unwrap().unwrap();
                assert_eq!(snapshot.current_address, Some(if fixed_selection { main.clone() } else { next }));
                assert_eq!(snapshot.selected_index, Some(if fixed_selection { 0 } else { 1 }));
                assert_eq!(
                    snapshot
                        .records
                        .iter()
                        .filter(|record| record.status == BsvHomeBsvAddressStatus::Used)
                        .map(|record| record.index)
                        .collect::<Vec<_>>(),
                    vec![0]
                );
                assert_eq!(snapshot.records.len(), 11);
                assert_eq!(service.wallet_store.get_wallet(wallet.id).await.unwrap().unwrap().accounts[0].address, main);
                assert_eq!(*store.updates.lock().unwrap(), 1);
                assert_eq!(
                    provider.requested.lock().unwrap().iter().filter(|path| path.as_str() == "/addresses/history/all").count(),
                    1
                );
            }
        });
    }

    #[test]
    fn test_bsv_native_balance_error_does_not_write_balances() {
        block_on(async {
            let address = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
            let native_asset_id = Chain::BitcoinSv.as_asset_id();
            let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, address)]);
            let provider = Arc::new(TestAlienProvider::with_status(404));
            let (service, _, store, provider) = service_with_provider(vec![native_asset_id.clone()], BsvHomeRuntimeEnvironment::Production, provider);
            service.wallet_store.add_wallet(wallet.clone()).await.unwrap();

            assert_eq!(
                service.update(wallet.id, vec![native_asset_id.clone()]).await,
                Err(BsvHomeServiceError::Gateway {
                    msg: "Network error: HTTP error: status 404".to_string(),
                })
            );
            assert_eq!(provider.requested_paths(), vec![format!("/v1/bsv/main/address/{address}/unspent/all")]);
            assert_eq!(*store.rows.lock().unwrap(), vec![native_asset_id]);
            assert_eq!(*store.updates.lock().unwrap(), 0);
        });
    }

    #[test]
    fn test_trx_initialization_restores_only_the_required_existing_balance_and_is_idempotent() {
        block_on(async {
            let wallet = wallet();
            let trx = Chain::Tron.as_asset_id();
            let (enabled, disabled) = crate::services::assets::rules::default_balances(&wallet);
            let (service, asset_store, store, provider) = service_with_environment([enabled, disabled].concat(), BsvHomeRuntimeEnvironment::Production);
            *store.enabled.lock().unwrap() = Vec::new();
            service.wallet_store.add_wallet(wallet.clone()).await.unwrap();

            assert_eq!(service.initialize_wallet_balances(wallet.clone()).await.unwrap(), vec![trx.clone()]);
            assert_eq!(service.enabled_asset_ids(wallet.id.clone()).await.unwrap(), vec![trx.clone()]);
            assert_eq!(*store.enabled_updates.lock().unwrap(), vec![(wallet.id.clone(), vec![trx.clone()], true)]);
            assert_eq!(*asset_store.added_balances.lock().unwrap(), Vec::new());
            assert_eq!(service.initialize_wallet_balances(wallet.clone()).await.unwrap(), Vec::<AssetId>::new());
            assert_eq!(*store.enabled_updates.lock().unwrap(), vec![(wallet.id.clone(), vec![trx], true)]);
            assert!(service.update_enabled_balances(wallet.id).await.is_err());
            assert_eq!(
                provider
                    .requested_paths()
                    .into_iter()
                    .map(|url| Url::parse(&url).unwrap().path().to_string())
                    .collect::<Vec<_>>(),
                vec!["/tron/wallet/getaccount", "/tron/wallet/getaccount"]
            );
        });
    }

    #[test]
    fn test_trx_initialization_does_not_enable_an_asset_without_a_mainnet_account() {
        block_on(async {
            let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::Ethereum, "address")]);
            let (enabled, disabled) = crate::services::assets::rules::default_balances(&wallet);
            let (service, asset_store, store) = service([enabled, disabled, vec![Chain::Tron.as_asset_id()]].concat());
            *store.enabled.lock().unwrap() = Vec::new();

            assert_eq!(service.initialize_wallet_balances(wallet).await.unwrap(), Vec::<AssetId>::new());
            assert_eq!(*store.enabled_updates.lock().unwrap(), Vec::new());
            assert_eq!(*asset_store.added_balances.lock().unwrap(), Vec::new());
        });
    }

    #[test]
    fn test_trx_setup_refreshes_restored_balances_for_every_wallet_type_without_enabling_test_assets() {
        block_on(async {
            for wallet_type in [WalletType::Multicoin, WalletType::Single, WalletType::PrivateKey, WalletType::View] {
                let wallet = Wallet {
                    wallet_type,
                    ..Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Tron, Chain::TronShasta], "address"))
                };
                let trx = Chain::Tron.as_asset_id();
                let test_assets = vec![Chain::TronShasta.as_asset_id(), TRON_SHASTA_USDT.id.clone()];
                let (enabled, disabled) = crate::services::assets::rules::default_balances(&wallet);
                let (service, asset_store, store, provider) = service_with_environment([enabled, disabled, test_assets].concat(), BsvHomeRuntimeEnvironment::LocalDevelopment);
                *store.enabled.lock().unwrap() = Vec::new();
                service.wallet_store.add_wallet(wallet.clone()).await.unwrap();

                service.setup_wallet(wallet.clone()).await.unwrap();

                assert_eq!(*store.enabled_updates.lock().unwrap(), vec![(wallet.id.clone(), vec![trx.clone()], true)]);
                assert_eq!(service.enabled_asset_ids(wallet.id).await.unwrap(), vec![trx]);
                assert_eq!(*asset_store.added_balances.lock().unwrap(), Vec::new());
                assert_eq!(
                    provider
                        .requested_paths()
                        .into_iter()
                        .map(|url| Url::parse(&url).unwrap().path().to_string())
                        .collect::<Vec<_>>(),
                    vec!["/tron/wallet/getaccount", "/tron/wallet/getaccount"]
                );
            }
        });
    }

    #[test]
    fn test_trx_cannot_be_disabled_when_the_wallet_has_a_mainnet_account() {
        block_on(async {
            let trx = Chain::Tron.as_asset_id();
            let ethereum = Chain::Ethereum.as_asset_id();
            let (service, _, store) = service(vec![trx.clone(), ethereum.clone()]);
            service.wallet_store.add_wallet(wallet()).await.unwrap();

            assert_eq!(
                service.set_assets_enabled(wallet().id, vec![trx.clone()], false).await,
                Err(BsvHomeServiceError::Unsupported {
                    msg: "ASSET_REQUIRED:TRX".to_string(),
                })
            );
            assert_eq!(*store.enabled.lock().unwrap(), vec![trx.clone(), ethereum.clone()]);
            assert_eq!(*store.enabled_updates.lock().unwrap(), Vec::new());
            service.set_assets_enabled(wallet().id, vec![ethereum], false).await.unwrap();
            assert_eq!(*store.enabled.lock().unwrap(), vec![trx]);
        });
    }

    #[test]
    fn test_trx_disable_preserves_other_network_and_missing_account_behavior() {
        block_on(async {
            let trx = Chain::Tron.as_asset_id();
            let shasta = Chain::TronShasta.as_asset_id();
            let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::Ethereum, "address")]);
            let (service, _, store, _) = service_with_environment(vec![trx.clone(), shasta.clone()], BsvHomeRuntimeEnvironment::LocalDevelopment);
            service.wallet_store.add_wallet(wallet.clone()).await.unwrap();

            service.set_assets_enabled(wallet.id.clone(), vec![trx.clone()], false).await.unwrap();
            if BsvHomeRuntimeEnvironment::LocalDevelopment.allows_chain(Chain::TronShasta) {
                service.set_assets_enabled(wallet.id.clone(), vec![shasta.clone()], false).await.unwrap();
                assert_eq!(
                    *store.enabled_updates.lock().unwrap(),
                    vec![(wallet.id.clone(), vec![trx], false), (wallet.id, vec![shasta], false)]
                );
            } else {
                assert_eq!(*store.enabled_updates.lock().unwrap(), vec![(wallet.id, vec![trx], false)]);
            }
        });
    }

    #[test]
    fn test_production_balance_refresh_preserves_hidden_test_rows_without_egress() {
        block_on(async {
            let retained = vec![Chain::TronShasta.as_asset_id(), primitives::known_assets::TRON_SHASTA_USDT.id.clone()];
            let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::TronShasta, "retained-development")]);
            let (service, asset_store, store, provider) = service_with_environment(retained.clone(), BsvHomeRuntimeEnvironment::Production);
            service.wallet_store.add_wallet(wallet.clone()).await.unwrap();
            assert_eq!(service.enabled_asset_ids(wallet.id.clone()).await.unwrap(), Vec::<AssetId>::new());
            service.update_enabled_balances(wallet.id.clone()).await.unwrap();
            assert!(service.balances(wallet.id.clone(), retained.clone()).await.is_err());
            assert!(service.update(wallet.id.clone(), retained.clone()).await.is_err());
            assert!(service.set_assets_enabled(wallet.id, retained.clone(), true).await.is_err());
            assert_eq!(*store.rows.lock().unwrap(), retained);
            assert_eq!(*store.updates.lock().unwrap(), 0);
            assert_eq!(asset_store.added_balances.lock().unwrap().len(), 0);
            assert_eq!(provider.requested_paths(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_local_balance_initialization_excludes_retained_test_accounts_for_private_and_view_wallets() {
        block_on(async {
            for wallet_type in [WalletType::PrivateKey, WalletType::View] {
                let mainnet = Wallet {
                    id: if wallet_type == WalletType::PrivateKey {
                        WalletId::PrivateKey(Chain::Tron, "shared".into())
                    } else {
                        WalletId::View(Chain::Tron, "shared".into())
                    },
                    wallet_type,
                    ..Wallet::mock_with_accounts(vec![Account::mock(Chain::Tron, "shared")])
                };
                let mut retained = mainnet.clone();
                retained.accounts.push(Account::mock(Chain::TronShasta, "shared"));
                let expected = crate::services::assets::rules::default_balances(&mainnet);
                let (service, asset_store, _, provider) = service_with_environment(Vec::new(), BsvHomeRuntimeEnvironment::LocalDevelopment);
                assert_eq!(service.initialize_wallet_balances(retained).await.unwrap(), expected.0);
                assert_eq!(
                    asset_store.added_balances.lock().unwrap().iter().flat_map(|(_, ids, _)| ids.clone()).collect::<Vec<_>>(),
                    [expected.0, expected.1].concat()
                );
                assert_eq!(provider.requested_paths(), Vec::<String>::new());
            }
        });
    }

    #[test]
    fn test_initialization_of_a_new_wallet_adds_every_default_balance() {
        block_on(async {
            let (enabled, disabled) = crate::services::assets::rules::default_balances(&wallet());
            let (service, asset_store, _) = service(Vec::new());

            assert_eq!(service.initialize_wallet_balances(wallet()).await.unwrap(), enabled);

            let added = asset_store.added_balances.lock().unwrap();
            assert_eq!(added.len(), 2);
            assert!(enabled.contains(&AssetId::from_chain(Chain::BitcoinSv)));
            assert!(enabled.contains(&primitives::known_assets::TRON_USDT.id));
            assert_eq!(added[0], (wallet().id, enabled, true));
            assert_eq!(added[1], (wallet().id, disabled, false));
        });
    }

    #[test]
    fn test_setup_of_a_complete_wallet_writes_nothing() {
        block_on(async {
            let (enabled, disabled) = crate::services::assets::rules::default_balances(&wallet());
            let (service, asset_store, store) = service([enabled, disabled].concat());

            service.setup_wallet(wallet()).await.unwrap();

            assert!(asset_store.added_balances.lock().unwrap().is_empty());
            assert_eq!(*store.updates.lock().unwrap(), 0);
        });
    }

    #[test]
    fn test_a_balance_update_creates_only_the_rows_it_lacks() {
        block_on(async {
            let ethereum = AssetId::from_chain(Chain::Ethereum);
            let cosmos = AssetId::from_chain(Chain::Cosmos);
            let (service, asset_store, store) = service(vec![ethereum.clone()]);
            asset_store
                .save_assets(vec![
                    crate::services::assets::rules::default_asset_basic(Asset::from_chain(Chain::Ethereum)),
                    crate::services::assets::rules::default_asset_basic(Asset::from_chain(Chain::Cosmos)),
                ])
                .await
                .unwrap();
            let update = |asset_id: AssetId| BsvHomeBalanceUpdate {
                asset_id,
                update_type: BsvHomeBalanceUpdateType::Token {
                    available: num_bigint::BigUint::ZERO,
                },
                is_active: true,
            };

            service.update_balances(wallet().id, vec![update(ethereum), update(cosmos.clone())]).await.unwrap();

            assert_eq!(*asset_store.added_balances.lock().unwrap(), vec![(wallet().id, vec![cosmos], false)]);
            assert_eq!(*store.updates.lock().unwrap(), 1);
        });
    }

    #[test]
    fn test_setup_adds_only_the_balances_the_wallet_lacks() {
        block_on(async {
            let (enabled, disabled) = crate::services::assets::rules::default_balances(&wallet());
            let missing_enabled = enabled[0].clone();
            let missing_disabled = disabled[0].clone();
            let rows = [enabled, disabled]
                .concat()
                .into_iter()
                .filter(|id| id != &missing_enabled && id != &missing_disabled)
                .collect();
            let (service, asset_store, _) = service(rows);

            service.setup_wallet(wallet()).await.unwrap();

            let added = asset_store.added_balances.lock().unwrap();
            assert_eq!(*added, vec![(wallet().id, vec![missing_enabled], true), (wallet().id, vec![missing_disabled], false)]);
        });
    }
}
