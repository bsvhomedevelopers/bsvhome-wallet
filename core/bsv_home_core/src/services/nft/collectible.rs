use std::sync::Arc;

use primitives::{NFTAssetData, NFTAssetId, ReportNft, WalletType};

use super::model::BsvHomeCollectibleDetails;
use super::{BsvHomeNftService, rules};
use crate::services::avatar::BsvHomeAvatarService;
use crate::services::error::BsvHomeServiceError;
use crate::services::explorer::BsvHomeExplorerService;

#[derive(uniffi::Object)]
pub struct BsvHomeCollectibleService {
    nfts: Arc<BsvHomeNftService>,
    avatars: Arc<BsvHomeAvatarService>,
    explorer: Arc<BsvHomeExplorerService>,
}

#[uniffi::export]
impl BsvHomeCollectibleService {
    #[uniffi::constructor]
    pub fn new(nfts: Arc<BsvHomeNftService>, avatars: Arc<BsvHomeAvatarService>, explorer: Arc<BsvHomeExplorerService>) -> Self {
        Self { nfts, avatars, explorer }
    }

    pub fn details(&self, wallet_type: WalletType, asset_data: NFTAssetData, is_owned: bool) -> BsvHomeCollectibleDetails {
        let chain = asset_data.asset.chain;
        let contract = asset_data.collection.contract_address.clone();
        let (contract_explorer, token_explorer) = if contract.is_empty() {
            (None, None)
        } else {
            (
                self.explorer.get_token_url(chain, contract.clone()),
                self.explorer.get_nft_url(chain, contract, asset_data.asset.token_id.clone()),
            )
        };
        rules::collectible_details(&wallet_type, &asset_data, is_owned, contract_explorer, token_explorer)
    }

    pub async fn refresh_asset(&self, asset_id: NFTAssetId) -> Result<(), BsvHomeServiceError> {
        self.nfts.refresh_asset(self.nfts.session.current_wallet_id()?, asset_id).await
    }

    pub async fn report(&self, report: ReportNft) -> Result<(), BsvHomeServiceError> {
        self.nfts.report(report).await
    }

    pub async fn set_wallet_avatar(&self, url: String) -> Result<(), BsvHomeServiceError> {
        self.avatars.set_image_url(self.nfts.session.current_wallet_id()?, url).await
    }
}
