pub mod model;
pub mod rules;

use std::sync::Arc;

use primitives::{Asset, AssetId, Chain, Wallet, WalletId};

use crate::services::assets::BsvHomeAssetsService;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::addresses::{BsvHomeBsvAddressPool, BsvHomeBsvAddressSnapshot};
pub use model::BsvHomeMemoWarning;

#[derive(uniffi::Object)]
pub struct BsvHomeReceiveService {
    balances: Arc<BsvHomeBalanceService>,
    assets: Arc<BsvHomeAssetsService>,
    addresses: Arc<BsvHomeBsvAddressPool>,
}

#[uniffi::export]
impl BsvHomeReceiveService {
    #[uniffi::constructor]
    pub fn new(balances: Arc<BsvHomeBalanceService>, assets: Arc<BsvHomeAssetsService>, addresses: Arc<BsvHomeBsvAddressPool>) -> Self {
        Self { balances, assets, addresses }
    }

    pub async fn prepare_bsv_addresses(&self, wallet_id: WalletId) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.addresses.prepare(wallet_id).await
    }
    pub async fn bsv_addresses(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvAddressSnapshot>, BsvHomeServiceError> {
        self.addresses.snapshot(wallet_id).await
    }
    pub async fn add_bsv_address(&self, wallet_id: WalletId) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.addresses.add_address(wallet_id).await
    }
    pub async fn select_bsv_address(&self, wallet_id: WalletId, index: u32) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.addresses.select_address(wallet_id, index).await
    }
    pub fn bsv_scan_generation(&self, wallet_id: WalletId) -> Result<u64, BsvHomeServiceError> {
        self.addresses.scan_generation(wallet_id)
    }
    pub async fn sync_bsv_addresses(&self, wallet_id: WalletId, generation: u64) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.addresses.sync_at_generation(wallet_id, generation, false).await
    }
    pub async fn continue_bsv_scan(&self, wallet_id: WalletId, generation: u64) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.addresses.sync_at_generation(wallet_id, generation, true).await
    }

    pub fn cancel_bsv_scan(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        self.addresses.cancel_scan(wallet_id)
    }

    pub fn memo_warning(&self, chain: Chain) -> BsvHomeMemoWarning {
        rules::memo_warning(chain)
    }

    pub fn network_asset_ids(&self, asset_id: AssetId, associations: Vec<AssetId>, wallet: Wallet) -> Vec<AssetId> {
        rules::network_asset_ids(asset_id, associations, &wallet, self.assets.environment())
    }

    pub async fn enable_asset(&self, wallet_id: WalletId, asset_id: AssetId) -> Result<(), BsvHomeServiceError> {
        self.balances.set_assets_enabled(wallet_id, vec![asset_id], true).await
    }

    pub async fn sync_missing_assets(&self, asset_ids: Vec<AssetId>) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        self.assets.sync_missing_assets(asset_ids).await
    }

    pub async fn asset(&self, asset_id: AssetId) -> Result<Asset, BsvHomeServiceError> {
        self.assets.ensure_asset(asset_id).await
    }
}
