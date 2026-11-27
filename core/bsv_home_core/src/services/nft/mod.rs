pub mod collectible;
pub mod model;
pub mod rules;
pub mod store;
#[cfg(test)]
pub(crate) mod testkit;

use crate::services::error::BsvHomeServiceError;
use std::future::Future;
use std::sync::Arc;

use primitives::{Account, NFTAssetData, NFTAssetId, NFTData, ReportNft, Wallet, WalletId};

pub use collectible::BsvHomeCollectibleService;
pub use model::{
    BsvHomeCollectibleAttribute, BsvHomeCollectibleAttributeValue, BsvHomeCollectibleDetails, BsvHomeCollectibleIdentifier, BsvHomeCollectibleRow, BsvHomeCollectibleSection,
    BsvHomeNftItem, BsvHomeNftList,
};
pub use store::BsvHomeNftStore;

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomeNftService {
    api: Arc<BsvHomeDeviceApiClient>,
    store: Arc<dyn BsvHomeNftStore>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomeNftService {
    #[uniffi::constructor]
    pub fn new(api: Arc<BsvHomeDeviceApiClient>, store: Arc<dyn BsvHomeNftStore>, session: Arc<BsvHomeWalletSessionService>) -> Self {
        Self { api, store, session }
    }

    pub async fn sync(&self) -> Result<u32, BsvHomeServiceError> {
        self.sync_wallet(self.session.current_wallet_id()?).await
    }

    pub async fn ensure_asset(&self, asset_id: NFTAssetId) -> Result<NFTAssetData, BsvHomeServiceError> {
        self.require_nft(asset_id.chain)?;
        cached_or_loaded(self.store.as_ref(), asset_id.clone(), async move {
            Ok(self.api.client.get_nft_asset(asset_id).await.map_err(BsvHomeApiError::from)?)
        })
        .await
    }

    pub async fn refresh_asset(&self, wallet_id: WalletId, asset_id: NFTAssetId) -> Result<(), BsvHomeServiceError> {
        self.require_nft(asset_id.chain)?;
        self.api.client.refresh_nft_asset(wallet_id.id(), asset_id).await.map_err(BsvHomeApiError::from)?;
        Ok(())
    }

    pub async fn report(&self, report: ReportNft) -> Result<(), BsvHomeServiceError> {
        if self.session.environment == BsvHomeRuntimeEnvironment::WowPay {
            return Err(BsvHomeServiceError::Unsupported {
                msg: "NFTs are unavailable in this environment".into(),
            });
        }
        self.api.client.report_nft(report).await.map_err(BsvHomeApiError::from)?;
        Ok(())
    }

    pub fn receive_accounts(&self, wallet: Wallet, query: String) -> Vec<Account> {
        let wallet = self.session.environment.visible_wallet(wallet);
        rules::receive_accounts(&wallet, &query)
            .into_iter()
            .filter(|account| self.allows_nft(account.chain))
            .collect()
    }

    pub fn list_items(&self, data: Vec<NFTData>, list: BsvHomeNftList) -> Vec<BsvHomeNftItem> {
        rules::list_items(data.into_iter().filter(|data| self.allows_nft(data.collection.chain)).collect(), list)
    }

    pub fn unverified_collections(&self, data: Vec<NFTData>) -> Vec<NFTData> {
        rules::unverified_collections(data.into_iter().filter(|data| self.allows_nft(data.collection.chain)).collect())
    }
}

async fn cached_or_loaded<F>(store: &dyn BsvHomeNftStore, asset_id: NFTAssetId, load: F) -> Result<NFTAssetData, BsvHomeServiceError>
where
    F: Future<Output = Result<NFTAssetData, BsvHomeServiceError>>,
{
    if let Some(data) = store.get_asset_data(asset_id).await? {
        return Ok(data);
    }
    let data = load.await?;
    store.save_asset(data.clone()).await?;
    Ok(data)
}

impl BsvHomeNftService {
    fn allows_nft(&self, chain: primitives::Chain) -> bool {
        self.session.environment != BsvHomeRuntimeEnvironment::WowPay && self.session.environment.allows_chain(chain)
    }

    fn require_nft(&self, chain: primitives::Chain) -> Result<(), BsvHomeServiceError> {
        self.session.environment.require_chain(chain)?;
        if !self.allows_nft(chain) {
            return Err(BsvHomeServiceError::Unsupported {
                msg: "NFTs are unavailable in this environment".into(),
            });
        }
        Ok(())
    }

    pub async fn sync_wallet(&self, wallet_id: WalletId) -> Result<u32, BsvHomeServiceError> {
        let wallet = self.session.get_wallet(wallet_id.clone()).await?.ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: "wallet not found".to_string(),
        })?;
        if !wallet
            .accounts
            .iter()
            .any(|account| self.allows_nft(account.chain) && account.chain.supports_bsv_home_backend())
        {
            return Ok(0);
        }
        let data = self
            .api
            .client
            .get_nft_assets(wallet_id.id())
            .await
            .map_err(BsvHomeApiError::from)?
            .into_iter()
            .filter(|data| self.allows_nft(data.collection.chain))
            .collect::<Vec<_>>();
        let count = data.len() as u32;
        self.store.save_nfts(wallet_id, data).await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use futures::executor::block_on;
    use primitives::{Chain, NFTAsset, NFTCollection, NFTCollectionId, NFTImages, NFTResource, NFTType, VerificationStatus};

    use super::testkit::MemoryNftStore;
    use super::*;
    use crate::api::BsvHomeDeviceApiClient;
    use crate::gateway::EmptyPreferences;
    use crate::services::device::BsvHomeDeviceKeyService;
    use crate::services::wallet::testkit::MemoryWalletStore;
    use crate::services::wallet_session::testkit::MemoryWalletSessionStore;
    use crate::testkit::TestAlienProvider;

    #[test]
    fn test_unsupported_backend_nft_wallets_skip_requests_and_mixed_wallets_propagate_failure() {
        block_on(async {
            for (accounts, expected_paths) in [
                (vec![Account::mock(Chain::BitcoinSv, "bsv")], vec![]),
                (vec![Account::mock(Chain::TronShasta, "testnet")], vec![]),
                (
                    vec![Account::mock(Chain::BitcoinSv, "bsv"), Account::mock(Chain::Tron, "mainnet")],
                    vec!["/v2/devices/nft_assets".to_string()],
                ),
            ] {
                let wallet = Wallet::mock_with_accounts(accounts);
                let provider = Arc::new(TestAlienProvider::with_status(503));
                let store = Arc::new(MemoryNftStore::default());
                let session = Arc::new(BsvHomeWalletSessionService::new(
                    Arc::new(MemoryWalletSessionStore::default()),
                    Arc::new(MemoryWalletStore {
                        wallets: Mutex::new(vec![wallet.clone()]),
                        ..Default::default()
                    }),
                ));
                let service = BsvHomeNftService::new(
                    Arc::new(BsvHomeDeviceApiClient::new(
                        provider.clone(),
                        Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
                    )),
                    store,
                    session,
                );
                let result = service.sync_wallet(wallet.id).await;
                if expected_paths.is_empty() {
                    assert_eq!(result.unwrap(), 0);
                } else {
                    assert!(result.is_err());
                }
                assert_eq!(provider.requested_paths(), expected_paths);
            }
        });
    }

    fn asset_data(name: &str) -> NFTAssetData {
        let collection_id = NFTCollectionId::new(Chain::Ethereum, "0xcollection");
        let asset_id = NFTAssetId::new(Chain::Ethereum, "0xcollection", "1");
        let images = NFTImages {
            preview: NFTResource {
                url: "".into(),
                mime_type: "".into(),
            },
        };
        NFTAssetData {
            collection: NFTCollection {
                id: collection_id.clone(),
                name: name.into(),
                symbol: None,
                description: None,
                chain: Chain::Ethereum,
                contract_address: "0xcollection".into(),
                images: images.clone(),
                is_verified: true,
                status: VerificationStatus::Verified,
                links: vec![],
            },
            asset: NFTAsset {
                id: asset_id,
                collection_id,
                contract_address: Some("0xcollection".into()),
                token_id: "1".into(),
                token_type: NFTType::ERC721,
                name: name.into(),
                description: None,
                chain: Chain::Ethereum,
                resource: NFTResource {
                    url: "".into(),
                    mime_type: "".into(),
                },
                images,
                attributes: vec![],
            },
        }
    }

    #[test]
    fn test_cached_asset_skips_loading() {
        let store = MemoryNftStore {
            cached: Some(asset_data("cached")),
            ..Default::default()
        };
        let loaded = Mutex::new(false);

        let data = futures::executor::block_on(cached_or_loaded(&store, asset_data("cached").asset.id, async {
            *loaded.lock().unwrap() = true;
            Ok(asset_data("remote"))
        }))
        .unwrap();

        assert_eq!(data.collection.name, "cached");
        assert!(!*loaded.lock().unwrap());
        assert!(store.added.lock().unwrap().is_empty());
    }

    #[test]
    fn test_missing_asset_is_loaded_and_added() {
        let store = MemoryNftStore::default();

        let data = futures::executor::block_on(cached_or_loaded(&store, asset_data("remote").asset.id, async { Ok(asset_data("remote")) })).unwrap();

        assert_eq!(data.collection.name, "remote");
        assert_eq!(store.added.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_wowpay_nft_scope_hides_legacy_data_without_loading_or_deleting() {
        block_on(async {
            let wallet = Wallet::mock_with_accounts(vec![
                Account::mock(Chain::BitcoinSv, "mainnet"),
                Account::mock(Chain::Tron, "legacy-tron"),
                Account::mock(Chain::Ethereum, "legacy"),
            ]);
            let provider = Arc::new(TestAlienProvider::with_status(503));
            let cached = asset_data("retained");
            let store = Arc::new(MemoryNftStore {
                cached: Some(cached.clone()),
                ..Default::default()
            });
            let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
                Arc::new(MemoryWalletSessionStore::default()),
                Arc::new(MemoryWalletStore {
                    wallets: Mutex::new(vec![wallet.clone()]),
                    ..Default::default()
                }),
                BsvHomeRuntimeEnvironment::WowPay,
            ));
            let service = BsvHomeNftService::new(
                Arc::new(BsvHomeDeviceApiClient::new(
                    provider.clone(),
                    Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
                )),
                store.clone(),
                session,
            );
            assert_eq!(service.sync_wallet(wallet.id.clone()).await.unwrap(), 0);
            assert!(service.ensure_asset(cached.asset.id.clone()).await.is_err());
            assert!(service.refresh_asset(wallet.id.clone(), NFTAssetId::new(Chain::Tron, "nft", "1")).await.is_err());
            assert!(
                service
                    .report(ReportNft {
                        collection_id: cached.collection.id.to_string(),
                        asset_id: None,
                        reason: None
                    })
                    .await
                    .is_err()
            );
            assert!(service.receive_accounts(wallet, String::new()).is_empty());
            assert!(
                service
                    .list_items(
                        vec![NFTData {
                            collection: cached.collection.clone(),
                            assets: vec![cached.asset]
                        }],
                        BsvHomeNftList::Collections
                    )
                    .is_empty()
            );
            assert_eq!(store.cached.as_ref().unwrap().collection.name, "retained");
            assert!(store.added.lock().unwrap().is_empty());
            assert!(provider.requested_paths().is_empty());
        });
    }
}
