use std::collections::HashMap;

use bsv_home_client::{Client, ClientError};
use chrono::DateTime;
use primitives::{AssetId, Price, PriceProvider};
use serde::Deserialize;

use super::rules::{BSV_COIN_ID, defillama_coin_id};
use crate::services::collections::unique;

pub(crate) const DEFILLAMA_URL: &str = "https://coins.llama.fi";

pub(crate) fn bsv_price_url() -> String {
    format!("{DEFILLAMA_URL}{}", current_prices_path(BSV_COIN_ID))
}

fn current_prices_path(coins: &str) -> String {
    format!("/prices/current/{coins}")
}

#[derive(Debug)]
pub(crate) struct DefiLlamaClient<C: Client> {
    client: C,
}

#[derive(Deserialize)]
struct CurrentPrices {
    coins: HashMap<String, CoinPrice>,
}

#[derive(Deserialize)]
struct CoinPrice {
    price: f64,
    timestamp: i64,
}

impl<C: Client> DefiLlamaClient<C> {
    pub(crate) fn new(client: C) -> Self {
        Self { client }
    }

    pub(crate) async fn get_prices(&self, asset_ids: &[AssetId]) -> Result<HashMap<AssetId, Price>, ClientError> {
        let requested = unique(asset_ids.iter().filter_map(|asset_id| defillama_coin_id(asset_id).map(|coin_id| (asset_id, coin_id))));
        if requested.is_empty() {
            return Ok(HashMap::new());
        }
        let coins = requested.iter().map(|(_, coin_id)| *coin_id).collect::<Vec<_>>().join(",");
        let response: CurrentPrices = serde_json::from_str(&self.client.get_text(&current_prices_path(&coins)).await?)?;
        requested
            .into_iter()
            .map(|(asset_id, coin_id)| {
                let coin = response.coins.get(coin_id).ok_or_else(|| ClientError::Serialization(format!("missing price: {coin_id}")))?;
                if !coin.price.is_finite() || coin.price <= 0.0 || coin.timestamp <= 0 {
                    return Err(ClientError::Serialization(format!("invalid price: {coin_id}")));
                }
                let updated_at = DateTime::from_timestamp(coin.timestamp, 0).ok_or_else(|| ClientError::Serialization(format!("invalid price timestamp: {coin_id}")))?;
                Ok((asset_id.clone(), Price::new(coin.price, 0.0, updated_at, PriceProvider::DefiLlama)))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::executor::block_on;
    use primitives::{Chain, asset_constants::TRON_USDT_ASSET_ID};

    use crate::alien::new_alien_client;
    use crate::testkit::TestAlienProvider;

    use super::*;

    const PRICES: &str = r#"{"coins":{"coingecko:tether":{"price":0.9997,"symbol":"USDT","timestamp":1790672860,"confidence":0.99},"coingecko:bitcoin-cash-sv":{"price":20.31,"symbol":"BSV","timestamp":1790671610,"confidence":0.99}}}"#;

    #[test]
    fn test_get_prices_maps_only_requested_assets_and_preserves_provider_timestamp() {
        let provider = Arc::new(TestAlienProvider::with_json(200, PRICES));
        let client = DefiLlamaClient::new(new_alien_client(DEFILLAMA_URL.into(), provider.clone()));
        let bsv = Chain::BitcoinSv.as_asset_id();
        let usdt = TRON_USDT_ASSET_ID.clone();
        let prices = block_on(client.get_prices(&[bsv.clone(), usdt.clone(), Chain::Tron.as_asset_id()])).unwrap();
        assert_eq!(
            prices,
            HashMap::from([
                (
                    bsv.clone(),
                    Price::new(20.31, 0.0, DateTime::from_timestamp(1790671610, 0).unwrap(), PriceProvider::DefiLlama)
                ),
                (usdt, Price::new(0.9997, 0.0, DateTime::from_timestamp(1790672860, 0).unwrap(), PriceProvider::DefiLlama)),
            ])
        );
        assert_eq!(
            provider.requested_paths(),
            vec![format!("{DEFILLAMA_URL}/prices/current/coingecko:bitcoin-cash-sv,coingecko:tether")]
        );

        let prices = block_on(client.get_prices(&[bsv])).unwrap();
        assert_eq!(prices.len(), 1);
        let unsupported = DefiLlamaClient::new(new_alien_client(String::new(), Arc::new(TestAlienProvider::with_status(500))));
        assert_eq!(block_on(unsupported.get_prices(&[Chain::Tron.as_asset_id()])).unwrap(), HashMap::new());
    }

    #[test]
    fn test_get_prices_requests_only_supported_unique_coin_ids() {
        block_on(async {
            for (assets, path, count) in [
                (
                    vec![Chain::BitcoinSv.as_asset_id(), Chain::BitcoinSv.as_asset_id()],
                    "/prices/current/coingecko:bitcoin-cash-sv",
                    1,
                ),
                (vec![TRON_USDT_ASSET_ID.clone()], "/prices/current/coingecko:tether", 1),
                (
                    vec![Chain::BitcoinSv.as_asset_id(), TRON_USDT_ASSET_ID.clone(), Chain::Tron.as_asset_id()],
                    "/prices/current/coingecko:bitcoin-cash-sv,coingecko:tether",
                    2,
                ),
            ] {
                let provider = Arc::new(TestAlienProvider::with_json(200, PRICES));
                let client = DefiLlamaClient::new(new_alien_client(DEFILLAMA_URL.into(), provider.clone()));
                let prices = client.get_prices(&assets).await.unwrap();
                assert_eq!(prices.len(), count);
                assert_eq!(provider.requested_paths(), vec![format!("{DEFILLAMA_URL}{path}")]);
            }
        });
    }

    #[test]
    fn test_get_prices_rejects_missing_invalid_and_failed_responses() {
        for body in [
            r#"{"coins":{}}"#,
            r#"{"coins":{"coingecko:bitcoin-cash-sv":{"price":0,"timestamp":1790671610}}}"#,
            r#"{"coins":{"coingecko:bitcoin-cash-sv":{"price":-1,"timestamp":1790671610}}}"#,
            r#"{"coins":{"coingecko:bitcoin-cash-sv":{"price":20,"timestamp":0}}}"#,
            r#"{"coins":{"coingecko:bitcoin-cash-sv":{"price":20,"timestamp":9223372036854775807}}}"#,
            r#"{"coins":{"coingecko:bitcoin-cash-sv":{"price":"20","timestamp":1790671610}}}"#,
        ] {
            let client = DefiLlamaClient::new(new_alien_client(String::new(), Arc::new(TestAlienProvider::with_json(200, body))));
            assert!(block_on(client.get_prices(&[Chain::BitcoinSv.as_asset_id()])).is_err());
        }
        for status in [429, 500] {
            let client = DefiLlamaClient::new(new_alien_client(DEFILLAMA_URL.into(), Arc::new(TestAlienProvider::with_json(status, PRICES))));
            assert!(matches!(
                block_on(client.get_prices(&[Chain::BitcoinSv.as_asset_id()])),
                Err(ClientError::Http { status: actual, .. }) if actual == status
            ));
        }
    }

    #[test]
    fn test_bsv_status_url_matches_the_real_single_asset_price_request() {
        let provider = Arc::new(TestAlienProvider::with_json(200, PRICES));
        let client = DefiLlamaClient::new(new_alien_client(DEFILLAMA_URL.into(), provider.clone()));
        let bsv = Chain::BitcoinSv.as_asset_id();

        let prices = block_on(client.get_prices(std::slice::from_ref(&bsv))).unwrap();

        assert_eq!(prices.keys().cloned().collect::<Vec<_>>(), vec![bsv]);
        assert_eq!(provider.requested_paths(), vec![bsv_price_url()]);
    }
}
