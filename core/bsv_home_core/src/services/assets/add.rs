use std::sync::Arc;

use primitives::{Asset, AssetId, Chain, Wallet};

use super::rules;
use crate::address::checksum_address;
use crate::services::assets::BsvHomeAssetsService;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::chain::rules::matching_chains;
use crate::services::error::BsvHomeServiceError;
use crate::services::explorer::BsvHomeExplorerService;
use primitives::BlockExplorerLink;

#[derive(uniffi::Object)]
pub struct BsvHomeAddAssetService {
    assets: Arc<BsvHomeAssetsService>,
    balances: Arc<BsvHomeBalanceService>,
    explorer: Arc<BsvHomeExplorerService>,
}

#[uniffi::export]
impl BsvHomeAddAssetService {
    #[uniffi::constructor]
    pub fn new(assets: Arc<BsvHomeAssetsService>, balances: Arc<BsvHomeBalanceService>, explorer: Arc<BsvHomeExplorerService>) -> Self {
        Self { assets, balances, explorer }
    }

    pub fn chains(&self, wallet: Wallet) -> Vec<Chain> {
        rules::token_chains(&self.assets.environment().visible_wallet(wallet))
    }

    pub fn default_chain(&self, chains: Vec<Chain>) -> Option<Chain> {
        rules::default_token_chain(&chains.into_iter().filter(|chain| self.assets.environment().allows_chain(*chain)).collect::<Vec<_>>())
    }

    pub fn matching_chains(&self, chains: Vec<Chain>, query: String) -> Vec<Chain> {
        matching_chains(chains.into_iter().filter(|chain| self.assets.environment().allows_chain(*chain)).collect(), &query)
    }

    pub fn token_url(&self, chain: Chain, token_id: String) -> Option<BlockExplorerLink> {
        if !self.assets.environment().allows_asset(&AssetId::from_token(chain, &token_id)) {
            return None;
        }
        self.explorer.get_token_url(chain, token_id)
    }

    pub async fn token(&self, chain: Chain, address: String) -> Result<Asset, BsvHomeServiceError> {
        self.assets.ensure_token_asset(AssetId::from(chain, Some(checksum_address(&address, chain)))).await
    }

    pub async fn add(&self, wallet: Wallet, asset_id: AssetId) -> Result<(), BsvHomeServiceError> {
        if wallet.account(asset_id.chain).is_none() {
            return Err(BsvHomeServiceError::NotFound {
                msg: format!("wallet has no account for {}", asset_id.chain),
            });
        }
        let asset = self.assets.ensure_token_asset(asset_id).await?;
        self.balances.set_assets_enabled(wallet.id, vec![asset.id], true).await
    }
}
