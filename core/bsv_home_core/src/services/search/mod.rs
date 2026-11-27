pub mod model;
pub mod rules;
pub mod store;
#[cfg(test)]
pub(crate) mod testkit;

use std::sync::Arc;

use primitives::perpetual::PerpetualSearchData;
use primitives::{AssetBasic, Wallet};

pub use model::BsvHomeSearchScope;
pub use store::BsvHomeSearchStore;

use crate::services::assets::{BsvHomeAssetStore, BsvHomeAssetsService};
use crate::services::error::BsvHomeServiceError;
use crate::services::perpetual::{BsvHomePerpetualStore, rules as perpetual_rules};
use crate::services::price::BsvHomePriceService;

#[derive(uniffi::Object)]
pub struct BsvHomeSearchService {
    assets: Arc<BsvHomeAssetsService>,
    asset_store: Arc<dyn BsvHomeAssetStore>,
    price: Arc<BsvHomePriceService>,
    perpetual_store: Arc<dyn BsvHomePerpetualStore>,
    store: Arc<dyn BsvHomeSearchStore>,
}

#[uniffi::export]
impl BsvHomeSearchService {
    #[uniffi::constructor]
    pub fn new(
        assets: Arc<BsvHomeAssetsService>,
        asset_store: Arc<dyn BsvHomeAssetStore>,
        price: Arc<BsvHomePriceService>,
        perpetual_store: Arc<dyn BsvHomePerpetualStore>,
        store: Arc<dyn BsvHomeSearchStore>,
    ) -> Self {
        Self {
            assets,
            asset_store,
            price,
            perpetual_store,
            store,
        }
    }

    pub async fn search(&self, wallet: Wallet, query: String, scope: BsvHomeSearchScope) -> Result<bool, BsvHomeServiceError> {
        let query = query.trim().to_string();
        if scope.skips_search(&query) {
            return Ok(false);
        }
        let wallet_chains = rules::wallet_chains(&wallet);
        let (response, tokens) = futures::join!(
            self.assets.search(query.clone(), wallet_chains.clone(), scope.api_tags()),
            self.assets.search_tokens(query.clone(), scope.token_chains(&wallet_chains)),
        );
        let response = response?;
        let assets = rules::merge_assets(response.assets, tokens);
        let key = scope.search_key(&query);
        self.save_assets(&wallet, &assets, &key).await?;
        self.save_perpetuals(&response.perpetuals, &key).await?;
        if scope.stores_lists() {
            self.store.set_lists(key, response.lists).await?;
        }
        Ok(!assets.is_empty() || !response.perpetuals.is_empty())
    }

    pub async fn search_assets(&self, wallet: Wallet, query: String) -> Result<Vec<AssetBasic>, BsvHomeServiceError> {
        let assets = self.assets.search_assets_and_tokens(query.clone(), rules::wallet_chains(&wallet)).await?;
        self.save_assets(&wallet, &assets, &BsvHomeSearchScope::All.search_key(&query)).await?;
        Ok(assets)
    }
}

impl BsvHomeSearchService {
    async fn save_assets(&self, wallet: &Wallet, assets: &[AssetBasic], key: &str) -> Result<(), BsvHomeServiceError> {
        let asset_ids = rules::asset_ids(assets);
        self.asset_store.save_assets(assets.to_vec()).await?;
        self.price.update_prices(rules::prices(assets)).await?;
        self.assets.add_missing_balances(wallet.id.clone(), asset_ids.clone()).await?;
        self.store.set_assets(key.to_string(), asset_ids).await
    }

    async fn save_perpetuals(&self, perpetuals: &[PerpetualSearchData], key: &str) -> Result<(), BsvHomeServiceError> {
        let data = rules::perpetual_data(perpetuals);
        self.asset_store.save_assets(perpetual_rules::perpetual_asset_basics(&data)).await?;
        self.perpetual_store.save_perpetuals(data).await?;
        self.store.set_perpetuals(key.to_string(), rules::perpetual_ids(perpetuals)).await
    }
}
