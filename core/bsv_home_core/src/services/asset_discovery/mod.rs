pub mod rules;

use crate::services::error::BsvHomeServiceError;
use std::future::Future;
use std::sync::Arc;

use chrono::Utc;
use primitives::WalletId;

pub use crate::services::wallet_preferences::BsvHomeDiscoveryStep;

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::services::balance::BsvHomeBalanceService;
use crate::services::nft::BsvHomeNftService;
use crate::services::transactions::BsvHomeTransactionsService;
use crate::services::wallet::BsvHomeWalletStore;
use crate::services::wallet_preferences::BsvHomeWalletPreferencesService;

#[derive(uniffi::Object)]
pub struct BsvHomeAssetDiscoveryService {
    api: Arc<BsvHomeDeviceApiClient>,
    balance: Arc<BsvHomeBalanceService>,
    transactions: Arc<BsvHomeTransactionsService>,
    nft: Arc<BsvHomeNftService>,
    wallet_store: Arc<dyn BsvHomeWalletStore>,
    preferences: Arc<BsvHomeWalletPreferencesService>,
}

#[uniffi::export]
impl BsvHomeAssetDiscoveryService {
    #[uniffi::constructor]
    pub fn new(
        api: Arc<BsvHomeDeviceApiClient>,
        balance: Arc<BsvHomeBalanceService>,
        transactions: Arc<BsvHomeTransactionsService>,
        nft: Arc<BsvHomeNftService>,
        wallet_store: Arc<dyn BsvHomeWalletStore>,
        preferences: Arc<BsvHomeWalletPreferencesService>,
    ) -> Self {
        Self {
            api,
            balance,
            transactions,
            nft,
            wallet_store,
            preferences,
        }
    }
}

impl BsvHomeAssetDiscoveryService {
    pub async fn discover(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        let (assets, transactions, nfts) = futures::join!(self.sync_assets(wallet_id.clone()), self.sync_transactions(wallet_id.clone()), self.sync_nfts(wallet_id),);
        assets?;
        transactions?;
        nfts
    }

    async fn sync_assets(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        let Some(mut wallet) = self.wallet_store.get_wallet(wallet_id.clone()).await? else {
            return Ok(());
        };
        wallet.accounts.retain(|account| self.balance.allows_asset(&account.chain.as_asset_id()));
        if !rules::has_bsv_home_backend_accounts(&wallet.accounts) {
            return self.preferences.set_initial_load_completed(wallet_id, BsvHomeDiscoveryStep::Assets);
        }
        let from_timestamp = self.preferences.get_assets_timestamp(wallet_id.clone());
        let timestamp = Utc::now().timestamp() as u64;
        let asset_ids = self.api.client.get_assets_list(wallet_id.id(), from_timestamp).await.map_err(BsvHomeApiError::from)?;
        let asset_ids = rules::discoverable_asset_ids(asset_ids, &wallet.accounts)
            .into_iter()
            .filter(|asset_id| self.balance.allows_asset(asset_id))
            .collect::<Vec<_>>();
        if !asset_ids.is_empty() {
            self.balance.set_assets_enabled(wallet_id.clone(), asset_ids.clone(), true).await?;
        }
        self.preferences.set_assets_timestamp(wallet_id.clone(), timestamp)?;
        self.preferences.set_initial_load_completed(wallet_id, BsvHomeDiscoveryStep::Assets)?;
        Ok(())
    }

    async fn sync_transactions(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        self.run_initial_load(wallet_id.clone(), BsvHomeDiscoveryStep::Transactions, self.transactions.sync_wallet(wallet_id, None))
            .await
    }

    async fn sync_nfts(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        let Some(mut wallet) = self.wallet_store.get_wallet(wallet_id.clone()).await? else {
            return Ok(());
        };
        wallet.accounts.retain(|account| self.balance.allows_asset(&account.chain.as_asset_id()));
        if !rules::has_bsv_home_backend_accounts(&wallet.accounts) {
            return self.preferences.set_initial_load_completed(wallet_id, BsvHomeDiscoveryStep::Nfts);
        }
        self.run_initial_load(wallet_id.clone(), BsvHomeDiscoveryStep::Nfts, self.nft.sync_wallet(wallet_id)).await
    }

    async fn run_initial_load<F, T>(&self, wallet_id: WalletId, step: BsvHomeDiscoveryStep, sync: F) -> Result<(), BsvHomeServiceError>
    where
        F: Future<Output = Result<T, BsvHomeServiceError>>,
    {
        if self.preferences.is_initial_load_completed(wallet_id.clone(), step)? {
            return Ok(());
        }
        sync.await?;
        self.preferences.set_initial_load_completed(wallet_id, step)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::executor::block_on;
    use primitives::{Account, Asset, AssetId, Chain, Wallet, known_assets::TRON_USDT};

    use super::{BsvHomeAssetDiscoveryService, BsvHomeDiscoveryStep};
    use crate::alien::AlienResponse;
    use crate::api::BsvHomeDeviceApiClient;
    use crate::config::public::API_URL;
    use crate::gateway::EmptyPreferences;
    use crate::services::assets::{BsvHomeAssetStore, rules as asset_rules};
    use crate::services::balance::{BsvHomeAssetBalance, BsvHomeBalanceService, BsvHomeBalanceStore};
    use crate::services::chain::BsvHomeRuntimeEnvironment;
    use crate::services::device::BsvHomeDeviceKeyService;
    use crate::services::nft::{BsvHomeNftService, testkit::MemoryNftStore};
    use crate::services::stream::testkit::{MemoryBalanceStore, SubscriptionTestkit};
    use crate::services::transactions::testkit::{TEST_BSV_ADDRESS, TransactionsTestkit};
    use crate::services::wallet::addresses::testkit::pool;
    use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};
    use crate::testkit::TestAlienProvider;

    fn discovery(
        wallet: Wallet,
        provider: Arc<TestAlienProvider>,
        environment: BsvHomeRuntimeEnvironment,
    ) -> (BsvHomeAssetDiscoveryService, TransactionsTestkit, Arc<MemoryBalanceStore>) {
        let kit = TransactionsTestkit::new_with_environment(wallet, provider.clone(), environment);
        let subscriptions = SubscriptionTestkit::new(&[], &[]);
        let balances = subscriptions.balances.clone();
        let api = Arc::new(BsvHomeDeviceApiClient::new(provider, Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences)))));
        let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
            Arc::new(MemoryWalletSessionStore::default()),
            kit.wallet_store.clone(),
            environment,
        ));
        let balance = Arc::new(BsvHomeBalanceService::new(
            kit.gateway.clone(),
            kit.wallet_store.clone(),
            kit.asset_store.clone(),
            balances.clone(),
            kit.assets.clone(),
            Arc::new(subscriptions.service),
            pool(kit.gateway.clone(), kit.wallet_store.clone()),
        ));
        let discovery = BsvHomeAssetDiscoveryService::new(
            api.clone(),
            balance,
            kit.service.clone(),
            Arc::new(BsvHomeNftService::new(api, Arc::new(MemoryNftStore::default()), session)),
            kit.wallet_store.clone(),
            kit.wallet_preferences.clone(),
        );
        (discovery, kit, balances)
    }

    #[test]
    fn test_wowpay_discovery_ignores_remote_assets_and_completes_without_changing_hidden_rows() {
        block_on(async {
            let wallet = Wallet::mock_with_accounts(Account::mock_chains(&[Chain::Tron, Chain::Ethereum, Chain::TronShasta], "public-fixture"));
            let hidden = vec![
                Asset::from_chain(Chain::Ethereum),
                Asset::from_chain(Chain::Tron),
                TRON_USDT.clone(),
                Asset {
                    id: AssetId::from_token(Chain::Tron, "legacy-token"),
                    ..TRON_USDT.clone()
                },
            ];
            let hidden_ids = hidden.iter().map(|asset| asset.id.clone()).collect::<Vec<_>>();
            let remote_ids = [
                hidden_ids.clone(),
                vec![TRON_USDT.id.clone(), Chain::Tron.as_asset_id(), TRON_USDT.id.clone(), Chain::TronShasta.as_asset_id()],
            ]
            .concat()
            .into_iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>();
            let provider = Arc::new(TestAlienProvider::with_responses(vec![(
                format!("{API_URL}/v2/devices/assets?from_timestamp=0"),
                AlienResponse::new(Some(200), serde_json::to_vec(&remote_ids).unwrap()),
            )]));
            let (discovery, kit, balances) = discovery(wallet, provider.clone(), BsvHomeRuntimeEnvironment::WowPay);
            kit.asset_store
                .save_assets(hidden.clone().into_iter().map(asset_rules::default_asset_basic).collect())
                .await
                .unwrap();
            balances.set_assets_enabled(kit.wallet_id.clone(), hidden_ids.clone(), true).await.unwrap();
            *balances.rows.lock().unwrap() = hidden_ids
                .iter()
                .map(|id| BsvHomeAssetBalance {
                    asset_id: id.clone(),
                    ..BsvHomeAssetBalance::mock()
                })
                .collect();
            let saved_wallets = serde_json::to_value(&*kit.wallet_store.wallets.lock().unwrap()).unwrap();
            let saved_balances = balances.rows.lock().unwrap().clone();

            discovery.sync_assets(kit.wallet_id.clone()).await.unwrap();
            assert!(
                kit.wallet_preferences
                    .is_initial_load_completed(kit.wallet_id.clone(), BsvHomeDiscoveryStep::Assets)
                    .unwrap()
            );
            assert_eq!(kit.wallet_preferences.get_assets_timestamp(kit.wallet_id.clone()), 0);
            assert_eq!(discovery.balance.enabled_asset_ids(kit.wallet_id.clone()).await.unwrap(), Vec::<AssetId>::new());
            assert_eq!(balances.get_enabled_asset_ids(kit.wallet_id.clone()).await.unwrap(), hidden_ids.clone());
            assert_eq!(kit.asset_store.get_assets(hidden_ids).await.unwrap(), hidden);
            assert_eq!(serde_json::to_value(&*kit.wallet_store.wallets.lock().unwrap()).unwrap(), saved_wallets);
            assert_eq!(*balances.rows.lock().unwrap(), saved_balances);
            assert_eq!(provider.requested_paths(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_wowpay_discovery_skips_hidden_backend_accounts_before_request_and_keeps_raw_wallet() {
        block_on(async {
            let wallet = Wallet::mock_with_accounts(Account::mock_chains(&[Chain::BitcoinSv, Chain::Ethereum, Chain::TronShasta], "public-fixture"));
            let provider = Arc::new(TestAlienProvider::with_status(503));
            let (discovery, kit, _) = discovery(wallet, provider.clone(), BsvHomeRuntimeEnvironment::WowPay);
            let saved_wallets = serde_json::to_value(&*kit.wallet_store.wallets.lock().unwrap()).unwrap();
            discovery.sync_assets(kit.wallet_id.clone()).await.unwrap();
            discovery.sync_nfts(kit.wallet_id.clone()).await.unwrap();
            for step in [BsvHomeDiscoveryStep::Assets, BsvHomeDiscoveryStep::Nfts] {
                assert!(kit.wallet_preferences.is_initial_load_completed(kit.wallet_id.clone(), step).unwrap());
            }
            assert_eq!(serde_json::to_value(&*kit.wallet_store.wallets.lock().unwrap()).unwrap(), saved_wallets);
            assert!(provider.requested_paths().is_empty());
        });
    }

    #[test]
    fn test_discovery_skips_unsupported_backend_steps_and_retries_failed_transaction_sources() {
        block_on(async {
            let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, TEST_BSV_ADDRESS)]);
            let provider = Arc::new(TestAlienProvider::with_status(404));
            let kit = TransactionsTestkit::new(wallet, provider.clone());
            let subscriptions = SubscriptionTestkit::new(&[], &[]);
            let api = Arc::new(BsvHomeDeviceApiClient::new(
                provider.clone(),
                Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
            ));
            let session = Arc::new(BsvHomeWalletSessionService::new(Arc::new(MemoryWalletSessionStore::default()), kit.wallet_store.clone()));
            let preferences = kit.wallet_preferences.clone();
            let address_pool = pool(kit.gateway.clone(), kit.wallet_store.clone());
            let balance = Arc::new(BsvHomeBalanceService::new(
                kit.gateway,
                kit.wallet_store.clone(),
                kit.asset_store,
                subscriptions.balances,
                kit.assets,
                Arc::new(subscriptions.service),
                address_pool,
            ));
            let discovery = BsvHomeAssetDiscoveryService::new(
                api.clone(),
                balance,
                kit.service,
                Arc::new(BsvHomeNftService::new(api, Arc::new(MemoryNftStore::default()), session)),
                kit.wallet_store,
                preferences.clone(),
            );
            for _ in 0..2 {
                assert!(discovery.discover(kit.wallet_id.clone()).await.is_err());
                assert!(preferences.is_initial_load_completed(kit.wallet_id.clone(), BsvHomeDiscoveryStep::Assets).unwrap());
                assert!(preferences.is_initial_load_completed(kit.wallet_id.clone(), BsvHomeDiscoveryStep::Nfts).unwrap());
                assert!(!preferences.is_initial_load_completed(kit.wallet_id.clone(), BsvHomeDiscoveryStep::Transactions).unwrap());
                assert_eq!(preferences.get_transactions_timestamp(kit.wallet_id.clone(), None), 0);
            }
            assert_eq!(provider.requested_paths(), vec!["/v1/bsv/main/addresses/history/all".to_string(); 2]);
        });
    }
}
