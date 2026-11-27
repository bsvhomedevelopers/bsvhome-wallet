use std::sync::Arc;

use async_trait::async_trait;
use futures::executor::block_on;
use primitives::{Asset, AssetId, Chain, Wallet, WalletId, known_assets::TRON_USDT};
use swapper::{AssetList, SwapperError};

use super::{BsvHomeSwapPair, BsvHomeSwapService, BsvHomeSwapStore, scoped_supported_assets};
use crate::bsv_home_swapper::BsvHomeSwapper;
use crate::keystore::BsvHomeKeystore;
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::testkit::MemoryPreferencesStore;
use crate::services::wallet::testkit::MemoryKeystorePassword;
use crate::testkit::TestAlienProvider;

struct Store;
#[async_trait]
impl BsvHomeSwapStore for Store {
    async fn get_swap_pairs(&self, _: WalletId) -> Result<Vec<BsvHomeSwapPair>, BsvHomeServiceError> {
        Ok(vec![BsvHomeSwapPair {
            from_asset_id: Chain::Tron.as_asset_id(),
            to_asset_id: AssetId::from_token(Chain::Tron, "legacy-token"),
        }])
    }
    async fn get_recent_asset_ids(&self, _: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        Ok(vec![AssetId::from_token(Chain::Tron, "legacy-token"), Chain::Ethereum.as_asset_id(), TRON_USDT.id.clone()])
    }
    async fn get_pay_asset_ids(&self, _: WalletId) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        Ok(vec![
            Chain::Ethereum.as_asset_id(),
            AssetId::from_token(Chain::Tron, "legacy-token"),
            Chain::Tron.as_asset_id(),
        ])
    }
    async fn get_receive_asset_ids(&self, _: WalletId, _: Vec<Chain>, _: Vec<AssetId>) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        panic!("legal recent must avoid candidate loading")
    }
}

#[test]
fn test_wowpay_swap_skips_hidden_defaults_recents_and_quotes_without_network_or_password() {
    block_on(async {
        let directory = tempfile::tempdir().unwrap();
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let password = Arc::new(MemoryKeystorePassword::default());
        let service = BsvHomeSwapService::new_with_environment(
            Arc::new(BsvHomeSwapper::new(provider.clone(), Arc::new(MemoryPreferencesStore::default()))),
            BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap(),
            password.clone(),
            Arc::new(Store),
            BsvHomeRuntimeEnvironment::WowPay,
        );
        let wallet = Wallet::mock();
        for hidden in [Chain::Ethereum.as_asset_id(), Chain::Tron.as_asset_id(), TRON_USDT.id.clone()] {
            assert!(service.pair_for_asset(hidden.clone(), true).is_err());
            assert_eq!(
                service.supported_assets(hidden),
                AssetList {
                    chains: vec![],
                    asset_ids: vec![]
                }
            );
        }
        let initial = service.pair_for_asset(Chain::BitcoinSv.as_asset_id(), true).unwrap();
        assert_eq!(initial.pay_asset_id, Chain::BitcoinSv.as_asset_id());
        assert_eq!(initial.receive_asset_id, None);
        assert_eq!(service.suggest_pair(wallet.id.clone(), None).await.unwrap(), None);
        assert!(service.suggest_pair(wallet.id.clone(), Some(Chain::Ethereum.as_asset_id())).await.is_err());
        for asset in [
            Asset::from_chain(Chain::Ethereum),
            Asset::from_chain(Chain::Tron),
            TRON_USDT.clone(),
            Asset {
                id: AssetId::from_token(Chain::Tron, "counterfeit-usdt"),
                ..TRON_USDT.clone()
            },
        ] {
            assert!(matches!(
                service
                    .get_quotes(wallet.clone(), Asset::from_chain(Chain::BitcoinSv), asset, 1u32.into(), false, None)
                    .await,
                Err(SwapperError::NotSupportedAsset)
            ));
        }
        assert!(provider.requested_paths().is_empty());
        assert!(password.create_requests.lock().unwrap().is_empty());
    });
}

#[test]
fn test_wowpay_swap_supported_chain_expands_only_exact_allowed_assets() {
    let supported = AssetList {
        chains: vec![Chain::Tron, Chain::Ethereum],
        asset_ids: vec![AssetId::from_token(Chain::Tron, "legacy-token")],
    };
    assert_eq!(scoped_supported_assets(BsvHomeRuntimeEnvironment::Production, supported.clone()), supported);
    assert_eq!(
        scoped_supported_assets(BsvHomeRuntimeEnvironment::WowPay, supported),
        AssetList {
            chains: vec![],
            asset_ids: vec![]
        }
    );
    assert_eq!(
        scoped_supported_assets(
            BsvHomeRuntimeEnvironment::WowPay,
            AssetList {
                chains: vec![Chain::BitcoinSv, Chain::Tron],
                asset_ids: vec![AssetId::from_token(Chain::BitcoinSv, "mnee")]
            }
        ),
        AssetList {
            chains: vec![],
            asset_ids: vec![Chain::BitcoinSv.as_asset_id()]
        }
    );
}
