use bsv_home_client::{Client, ClientError, ClientExt};
use primitives::{AssetBasic, AssetFull, AssetId, Chain, ChartPeriod, Charts, ConfigResponse, FiatAssets, FiatQuoteType, SearchResponse};

use crate::target::BsvHomeApiTarget;

#[derive(Debug, Clone)]
pub struct BsvHomeApiClient<C: Client> {
    client: C,
}

impl<C: Client> BsvHomeApiClient<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub async fn get_config(&self) -> Result<ConfigResponse, ClientError> {
        self.client.get(BsvHomeApiTarget::GetConfig).await
    }

    pub async fn get_charts(&self, asset_id: AssetId, period: ChartPeriod) -> Result<Charts, ClientError> {
        self.client.get(BsvHomeApiTarget::GetCharts { asset_id, period }).await
    }

    pub async fn get_asset(&self, asset_id: AssetId) -> Result<AssetFull, ClientError> {
        self.client.get(BsvHomeApiTarget::GetAsset { asset_id }).await
    }

    pub async fn get_assets(&self, asset_ids: Vec<AssetId>, currency: Option<String>) -> Result<Vec<AssetBasic>, ClientError> {
        let identifiers = asset_ids.iter().map(AssetId::to_string).collect::<Vec<_>>();
        self.client.post(BsvHomeApiTarget::GetAssets { currency }, &identifiers).await
    }

    pub async fn get_search_assets(&self, query: String, chains: Vec<Chain>) -> Result<Vec<AssetBasic>, ClientError> {
        self.client.get(BsvHomeApiTarget::GetSearchAssets { query, chains }).await
    }

    pub async fn get_search(&self, query: String, chains: Vec<Chain>, tags: Vec<String>) -> Result<SearchResponse, ClientError> {
        self.client.get(BsvHomeApiTarget::GetSearch { query, chains, tags }).await
    }

    pub async fn get_fiat_assets(&self, quote_type: FiatQuoteType) -> Result<FiatAssets, ClientError> {
        self.client.get(BsvHomeApiTarget::GetFiatAssets { quote_type }).await
    }

    pub async fn get_swap_assets(&self) -> Result<FiatAssets, ClientError> {
        self.client.get(BsvHomeApiTarget::GetSwapAssets).await
    }
}
