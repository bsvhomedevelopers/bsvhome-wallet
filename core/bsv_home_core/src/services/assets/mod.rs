pub mod add;
pub mod config;
pub mod details;
pub mod icon;
pub mod model;
pub mod rules;
pub mod selection;
#[cfg(test)]
mod selection_tests;
pub mod store;
#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests;

use crate::services::error::BsvHomeServiceError;
use std::{collections::HashMap, sync::Arc};

use primitives::{Asset, AssetBasic, AssetFull, AssetId, AssetPrice, Chain, ConfigVersions, Currency, FiatAssets, FiatQuoteType, Price, SearchResponse, Wallet, WalletId};

pub use add::BsvHomeAddAssetService;
pub use details::BsvHomeAssetDetailsService;
pub use model::{
    AssetList, BsvHomeAssetAction, BsvHomeAssetDetailsState, BsvHomeAssetEmptyAction, BsvHomeAssetFilter, BsvHomeAssetNetworkDestination, BsvHomeHeaderButton,
    BsvHomeHeaderButtonKind, BsvHomeSelectAssetFlow, BsvHomeSelectAssetType, BsvHomeSelectRowAction, BsvHomeWalletSearchLimits,
};
pub use selection::BsvHomeAssetSelectionService;
pub use store::BsvHomeAssetStore;

use crate::api::{BsvHomeApiClient, BsvHomeApiError};
use crate::gateway::BsvHomeGateway;
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::collections::unique_by;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::price::BsvHomePriceService;
use crate::services::search::rules::matching_assets;
use crate::services::wallet_session::BsvHomeWalletSessionService;
use primitives::known_assets::TRON_SHASTA_USDT;
use rules::local_asset;

#[derive(uniffi::Object)]
pub struct BsvHomeAssetsService {
    api: Arc<BsvHomeApiClient>,
    gateway: Arc<BsvHomeGateway>,
    store: Arc<dyn BsvHomeAssetStore>,
    price: Arc<BsvHomePriceService>,
    preferences: Arc<BsvHomePreferencesService>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomeAssetsService {
    #[uniffi::constructor]
    pub fn new(
        api: Arc<BsvHomeApiClient>,
        gateway: Arc<BsvHomeGateway>,
        store: Arc<dyn BsvHomeAssetStore>,
        price: Arc<BsvHomePriceService>,
        preferences: Arc<BsvHomePreferencesService>,
        session: Arc<BsvHomeWalletSessionService>,
    ) -> Self {
        Self {
            api,
            gateway,
            store,
            price,
            preferences,
            session,
        }
    }

    pub async fn ensure_asset(&self, asset_id: AssetId) -> Result<Asset, BsvHomeServiceError> {
        self.session.environment.require_asset(&asset_id)?;
        if let Some(asset) = self.stored_asset(&asset_id).await? {
            return Ok(asset);
        }
        self.sync_missing_assets(vec![asset_id.clone()]).await?;
        self.stored_asset(&asset_id).await?.ok_or_else(|| BsvHomeServiceError::NotFound {
            msg: format!("asset not found: {asset_id}"),
        })
    }

    pub async fn ensure_token_asset(&self, asset_id: AssetId) -> Result<Asset, BsvHomeServiceError> {
        self.session.environment.require_asset(&asset_id)?;
        if asset_id.chain.is_testnet() {
            return self.ensure_asset(asset_id).await;
        }
        if asset_id.chain == Chain::BitcoinSv && !asset_id.is_native() {
            // TODO(BSV-API): implement BSV token metadata before enabling token requests.
            return Err(BsvHomeServiceError::Unsupported {
                msg: "BSV_API_UNSUPPORTED: token metadata".to_string(),
            });
        }
        if let Some(asset) = self.stored_asset(&asset_id).await? {
            return Ok(asset);
        }
        let Some(token_id) = asset_id.token_id.clone() else {
            return self.ensure_asset(asset_id).await;
        };
        let asset = match self.gateway.get_token_data(asset_id.chain, token_id.clone()).await {
            Ok(asset) => asset,
            Err(error) => match self.search_token_asset(&asset_id, token_id).await {
                Ok(Some(asset)) => asset,
                Ok(None) | Err(_) => return Err(error.into()),
            },
        };
        if asset.id != asset_id {
            return Err(BsvHomeServiceError::InvalidInput {
                msg: "token response does not match the requested asset".into(),
            });
        }
        self.store.save_assets(vec![rules::default_asset_basic(asset.clone())]).await?;
        Ok(asset)
    }

    pub async fn sync_assets(&self, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        let asset_ids = crate::services::collections::unique(asset_ids.into_iter().filter(|asset_id| self.session.environment.allows_asset(asset_id)));
        if asset_ids.is_empty() {
            return Ok(());
        }
        let (assets, prices) = futures::join!(self.get_assets(asset_ids.clone(), Some(Currency::USD.to_string())), self.sync_defillama_prices(&asset_ids));
        let mut assets = assets?;
        if let Ok(prices) = &prices {
            for asset in &mut assets {
                if let Some(price) = prices.get(&asset.asset.id) {
                    asset.price = Some(*price);
                }
            }
        }
        self.store.save_assets(assets.clone()).await?;
        self.price.update_prices(rules::asset_prices(&assets)).await?;
        prices?;
        Ok(())
    }

    pub async fn sync_missing_assets(&self, asset_ids: Vec<AssetId>) -> Result<Vec<AssetId>, BsvHomeServiceError> {
        let asset_ids = asset_ids.into_iter().filter(|asset_id| self.session.environment.allows_asset(asset_id)).collect::<Vec<_>>();
        let existing = self.store.get_asset_ids(asset_ids.clone()).await?;
        let missing = rules::missing_asset_ids(asset_ids, existing);
        if missing.is_empty() {
            return Ok(vec![]);
        }
        let assets = self.get_assets(missing, None).await?;
        self.store.save_assets(assets.clone()).await?;
        Ok(assets.into_iter().map(|asset| asset.asset.id).collect())
    }

    pub async fn open_asset(&self, asset_id: AssetId) -> Result<Option<Asset>, BsvHomeServiceError> {
        let wallet = self.session.current_wallet().await?;
        self.open_wallet_asset(wallet, asset_id).await
    }

    pub async fn open_wallet_asset(&self, wallet: Wallet, asset_id: AssetId) -> Result<Option<Asset>, BsvHomeServiceError> {
        self.session.environment.require_asset(&asset_id)?;
        let wallet = self.session.environment.visible_wallet(wallet);
        if !rules::can_open(&wallet, &asset_id) {
            return Ok(None);
        }
        let asset = match self.ensure_asset(asset_id.clone()).await {
            Ok(asset) => asset,
            Err(BsvHomeServiceError::NotFound { .. }) => return Ok(None),
            Err(error) => return Err(error),
        };
        self.add_missing_balances(wallet.id, vec![asset_id]).await?;
        Ok(Some(asset))
    }
}

impl BsvHomeAssetsService {
    pub(crate) fn environment(&self) -> BsvHomeRuntimeEnvironment {
        self.session.environment
    }

    pub(crate) fn development_asset_ids(&self, wallet: &Wallet) -> Vec<AssetId> {
        self.session.environment.development_asset_ids(wallet)
    }

    pub(crate) async fn sync_defillama_prices(&self, asset_ids: &[AssetId]) -> Result<HashMap<AssetId, Price>, BsvHomeServiceError> {
        let asset_ids = asset_ids
            .iter()
            .filter(|asset_id| !asset_id.chain.is_testnet() && self.session.environment.allows_asset(asset_id))
            .cloned()
            .collect::<Vec<_>>();
        if asset_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let prices = self.api.defillama.get_prices(&asset_ids).await.map_err(BsvHomeApiError::from)?;
        let updates = prices
            .iter()
            .map(|(asset_id, price)| AssetPrice::new(asset_id.clone(), price.price, price.price_change_percentage_24h, price.updated_at))
            .collect();
        self.price.update_defillama_prices(updates).await?;
        Ok(prices)
    }

    pub async fn sync_asset(&self, asset_id: AssetId) -> Result<AssetFull, BsvHomeServiceError> {
        self.session.environment.require_asset(&asset_id)?;
        let (asset, prices) = futures::join!(self.get_asset(asset_id.clone()), self.sync_defillama_prices(std::slice::from_ref(&asset_id)));
        let mut asset = asset?;
        if let Ok(prices) = &prices
            && let Some(price) = prices.get(&asset_id)
        {
            asset.price = Some(*price);
        }
        self.store.save_asset(asset.clone()).await?;
        let price = asset
            .price
            .as_ref()
            .map(|price| AssetPrice::new(asset_id.clone(), price.price, price.price_change_percentage_24h, price.updated_at));
        self.price.update_asset_price(asset_id.clone(), price).await?;
        if let Some(market) = asset.market.clone() {
            self.price.update_market(asset_id, market).await?;
        }
        prices?;
        Ok(asset)
    }

    pub(crate) async fn ensure_simulation_assets(&self, asset_ids: Vec<AssetId>) -> Result<Vec<Asset>, BsvHomeServiceError> {
        let existing = self.store.get_asset_ids(asset_ids.clone()).await?;
        let missing = rules::missing_asset_ids(asset_ids.clone(), existing);
        if missing.is_empty() {
            return self.assets(asset_ids).await;
        }
        // Simulation assets may not exist in the backend; fall back to the node.
        if let Ok(assets) = self.get_assets(missing.clone(), None).await {
            self.store.save_assets(assets).await?;
        }
        for asset_id in missing {
            self.ensure_token_asset(asset_id).await?;
        }
        self.assets(asset_ids).await
    }

    pub async fn add_missing_balances(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>) -> Result<(), BsvHomeServiceError> {
        let asset_ids = asset_ids.into_iter().filter(|asset_id| self.session.environment.allows_asset(asset_id)).collect::<Vec<_>>();
        if asset_ids.is_empty() {
            return Ok(());
        }
        let stored = self.store.get_asset_ids(asset_ids).await?;
        if stored.is_empty() {
            return Ok(());
        }
        self.store.add_missing_balances(wallet_id, stored).await
    }

    pub async fn add_balances(&self, wallet_id: WalletId, asset_ids: Vec<AssetId>, enabled: bool) -> Result<(), BsvHomeServiceError> {
        let asset_ids = asset_ids.into_iter().filter(|asset_id| self.session.environment.allows_asset(asset_id)).collect::<Vec<_>>();
        if asset_ids.is_empty() {
            return Ok(());
        }
        self.store.add_balances(wallet_id, asset_ids, enabled).await
    }

    pub async fn assets(&self, asset_ids: Vec<AssetId>) -> Result<Vec<Asset>, BsvHomeServiceError> {
        for asset_id in &asset_ids {
            self.session.environment.require_asset(asset_id)?;
            if asset_id.chain.is_testnet() && local_asset(asset_id).is_none() {
                return Err(BsvHomeServiceError::Unsupported {
                    msg: format!("unsupported test asset: {asset_id}"),
                });
            }
        }
        self.store.get_assets(asset_ids).await
    }

    pub async fn get_asset(&self, asset_id: AssetId) -> Result<AssetFull, BsvHomeApiError> {
        self.require_asset(&asset_id)?;
        if let Some(asset) = local_asset(&asset_id) {
            let basic = rules::default_asset_basic(asset);
            return Ok(AssetFull {
                asset: basic.asset,
                properties: basic.properties,
                score: basic.score,
                tags: Vec::new(),
                links: Vec::new(),
                associations: Vec::new(),
                perpetuals: Vec::new(),
                price: None,
                market: None,
            });
        }
        if asset_id.chain.is_testnet() {
            return Err(BsvHomeApiError::Unsupported {
                msg: format!("unsupported test asset: {asset_id}"),
            });
        }
        let mut asset = self.api.client.get_asset(asset_id.clone()).await?;
        if asset.asset.id != asset_id {
            return Err(BsvHomeApiError::Serialization {
                msg: "asset response does not match the requested asset".into(),
            });
        }
        if self.session.environment.allowed_asset_ids().is_some() {
            asset.associations.retain(|association| self.session.environment.allows_asset(&association.asset_id));
            asset.perpetuals.clear();
        }
        Ok(asset)
    }

    pub async fn get_assets(&self, asset_ids: Vec<AssetId>, currency: Option<String>) -> Result<Vec<AssetBasic>, BsvHomeApiError> {
        for asset_id in &asset_ids {
            self.require_asset(asset_id)?;
            if asset_id.chain.is_testnet() && local_asset(asset_id).is_none() {
                return Err(BsvHomeApiError::Unsupported {
                    msg: format!("unsupported test asset: {asset_id}"),
                });
            }
        }
        let (local, remote): (Vec<_>, Vec<_>) = asset_ids.into_iter().partition(|asset| local_asset(asset).is_some());
        let mut assets: Vec<_> = local.into_iter().filter_map(|asset_id| local_asset(&asset_id)).map(rules::default_asset_basic).collect();
        if !remote.is_empty() {
            let requested = remote.clone();
            assets.extend(
                self.api
                    .client
                    .get_assets(remote, currency)
                    .await?
                    .into_iter()
                    .filter(|asset| requested.contains(&asset.asset.id)),
            );
        }
        Ok(assets)
    }

    pub async fn search_assets(&self, query: String, chains: Vec<Chain>) -> Result<Vec<AssetBasic>, BsvHomeApiError> {
        let chains = self.search_chains(chains);
        for chain in &chains {
            self.session
                .environment
                .require_chain(*chain)
                .map_err(|error| BsvHomeApiError::Unsupported { msg: error.to_string() })?;
        }
        let production = chains
            .iter()
            .copied()
            .filter(|chain| !chain.is_testnet() && (self.session.environment.allowed_asset_ids().is_none() || chain.supports_bsv_home_backend()))
            .collect::<Vec<_>>();
        let mut assets = if !chains.is_empty() && production.is_empty() {
            Vec::new()
        } else {
            self.api.client.get_search_assets(query.clone(), production).await?
        };
        if chains.contains(&Chain::TronShasta) {
            assets.extend(
                matching_assets(vec![Asset::from_chain(Chain::TronShasta), TRON_SHASTA_USDT.clone()], &query)
                    .into_iter()
                    .map(rules::default_asset_basic),
            );
        }
        if self.session.environment.allowed_asset_ids().is_some() && chains.contains(&Chain::BitcoinSv) {
            assets.extend(
                matching_assets(vec![Asset::from_chain(Chain::BitcoinSv)], &query)
                    .into_iter()
                    .map(rules::default_asset_basic),
            );
            assets = unique_by(assets, |asset| asset.asset.id.clone());
        }
        assets.retain(|asset| self.session.environment.allows_asset(&asset.asset.id));
        Ok(assets)
    }

    pub async fn search(&self, query: String, chains: Vec<Chain>, tags: Vec<String>) -> Result<SearchResponse, BsvHomeApiError> {
        let chains = self.search_chains(chains);
        for chain in &chains {
            self.session
                .environment
                .require_chain(*chain)
                .map_err(|error| BsvHomeApiError::Unsupported { msg: error.to_string() })?;
        }
        if !chains.is_empty() && chains.iter().all(|chain| chain.is_testnet()) {
            return Ok(SearchResponse {
                assets: self.search_assets(query, chains).await?,
                perpetuals: Vec::new(),
                nfts: Vec::new(),
                lists: Vec::new(),
            });
        }
        let production = chains
            .iter()
            .copied()
            .filter(|chain| !chain.is_testnet() && (self.session.environment.allowed_asset_ids().is_none() || chain.supports_bsv_home_backend()))
            .collect::<Vec<_>>();
        let mut response = if !chains.is_empty() && production.is_empty() {
            SearchResponse {
                assets: Vec::new(),
                perpetuals: Vec::new(),
                nfts: Vec::new(),
                lists: Vec::new(),
            }
        } else {
            self.api.client.get_search(query.clone(), production, tags).await?
        };
        response.assets.retain(|asset| self.session.environment.allows_asset(&asset.asset.id));
        if self.session.environment.allowed_asset_ids().is_some() {
            if chains.contains(&Chain::BitcoinSv) {
                response.assets.extend(
                    matching_assets(vec![Asset::from_chain(Chain::BitcoinSv)], &query)
                        .into_iter()
                        .map(rules::default_asset_basic),
                );
                response.assets = unique_by(response.assets, |asset| asset.asset.id.clone());
            }
            response.perpetuals.clear();
            response.nfts.clear();
            response.lists.clear();
        }
        Ok(response)
    }

    async fn search_token_asset(&self, asset_id: &AssetId, token_id: String) -> Result<Option<Asset>, BsvHomeApiError> {
        let assets = self.api.client.get_search_assets(token_id, vec![asset_id.chain]).await?;
        Ok(assets.into_iter().map(|basic| basic.asset).find(|asset| &asset.id == asset_id))
    }

    pub async fn sync_availability(&self, versions: ConfigVersions) -> Result<(), BsvHomeServiceError> {
        let results = futures::future::join_all(
            rules::asset_list_versions(&versions)
                .into_iter()
                .map(|(list, remote_version)| self.sync_availability_list(list, remote_version)),
        )
        .await;
        for result in results {
            result?;
        }
        Ok(())
    }

    async fn sync_availability_list(&self, list: AssetList, remote_version: i32) -> Result<(), BsvHomeServiceError> {
        if !rules::is_asset_list_outdated(self.preferences.get_assets_version(list).as_deref(), remote_version) {
            return Ok(());
        }
        let assets = match list {
            AssetList::Buy => self.get_fiat_assets(FiatQuoteType::Buy).await?,
            AssetList::Sell => self.get_fiat_assets(FiatQuoteType::Sell).await?,
            AssetList::Swap => self.get_swap_assets().await?,
        };
        let asset_ids = rules::asset_ids(&assets.asset_ids)
            .into_iter()
            .filter(|asset_id| self.session.environment.allows_asset(asset_id))
            .collect::<Vec<_>>();
        self.sync_missing_assets(asset_ids.clone()).await?;
        match list {
            AssetList::Buy => self.store.set_buyable_assets(asset_ids).await?,
            AssetList::Sell => self.store.set_sellable_assets(asset_ids).await?,
            AssetList::Swap => self.store.set_swappable_assets(asset_ids).await?,
        }
        self.preferences.set_assets_version(list, assets.version.to_string())
    }

    pub async fn search_tokens(&self, token_id: String, chains: Vec<Chain>) -> Vec<AssetBasic> {
        let lookups = chains
            .into_iter()
            .filter(|chain| chain.default_asset_type().is_some() && self.session.environment.allows_asset(&AssetId::from_token(*chain, &token_id)))
            .map(|chain| {
                let token_id = token_id.clone();
                async move {
                    if self.gateway.get_is_token_address(chain, token_id.clone()).await.ok()? {
                        self.gateway.get_token_data(chain, token_id).await.ok().map(rules::default_asset_basic)
                    } else {
                        None
                    }
                }
            });
        futures::future::join_all(lookups)
            .await
            .into_iter()
            .flatten()
            .filter(|asset| self.session.environment.allows_asset(&asset.asset.id))
            .collect()
    }

    pub async fn sync_default_assets(&self) -> Result<(), BsvHomeServiceError> {
        self.ensure_default_assets().await?;
        self.store.set_stakeable_assets(rules::stakeable_asset_ids()).await
    }

    pub async fn ensure_default_assets(&self) -> Result<(), BsvHomeServiceError> {
        let mut assets = rules::default_assets();
        if self.session.environment.allows_chain(Chain::TronShasta) {
            assets.extend([Asset::from_chain(Chain::TronShasta), TRON_SHASTA_USDT.clone()].into_iter().map(rules::default_asset_basic));
        }
        assets.retain(|asset| self.session.environment.allows_asset(&asset.asset.id));
        let existing = self.store.get_asset_ids(assets.iter().map(|asset| asset.asset.id.clone()).collect()).await?;
        let missing = rules::missing_assets(assets, existing);
        if missing.is_empty() {
            return Ok(());
        }
        self.store.save_assets(missing).await
    }

    pub async fn search_assets_and_tokens(&self, query: String, chains: Vec<Chain>) -> Result<Vec<AssetBasic>, BsvHomeServiceError> {
        let token_chains = rules::token_search_chains(&chains);
        let (assets, tokens) = futures::join!(self.search_assets(query.clone(), chains), self.search_tokens(query, token_chains));
        let mut assets = assets?;
        assets.extend(tokens);
        Ok(assets)
    }

    pub async fn sync_swappable_chains(&self) -> Result<(), BsvHomeServiceError> {
        self.store.set_swappable_assets(rules::swappable_chain_asset_ids()).await
    }

    pub async fn get_fiat_assets(&self, quote_type: FiatQuoteType) -> Result<FiatAssets, BsvHomeApiError> {
        Ok(self.api.client.get_fiat_assets(quote_type).await?)
    }

    pub async fn get_swap_assets(&self) -> Result<FiatAssets, BsvHomeApiError> {
        Ok(self.api.client.get_swap_assets().await?)
    }

    async fn stored_asset(&self, asset_id: &AssetId) -> Result<Option<Asset>, BsvHomeServiceError> {
        Ok(self.assets(vec![asset_id.clone()]).await?.into_iter().next())
    }

    fn search_chains(&self, chains: Vec<Chain>) -> Vec<Chain> {
        match self.session.environment.allowed_asset_ids() {
            Some(_) if chains.is_empty() => self.session.environment.available_chains(),
            Some(_) | None => chains,
        }
    }

    fn require_asset(&self, asset_id: &AssetId) -> Result<(), BsvHomeApiError> {
        self.session
            .environment
            .require_asset(asset_id)
            .map_err(|error| BsvHomeApiError::Unsupported { msg: error.to_string() })
    }
}

pub fn popular_asset_ids() -> Vec<AssetId> {
    rules::popular_asset_ids()
}

pub fn default_token_chain(chains: Vec<Chain>) -> Option<Chain> {
    rules::default_token_chain(&chains)
}
