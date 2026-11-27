pub mod model;
pub mod rules;
pub mod store;

use std::sync::Arc;

use primitives::currency::Currency;
use primitives::{Chain, ChartPeriod, PortfolioAssets, PortfolioAssetsRequest, PortfolioChartType, PortfolioData, PortfolioType, Wallet, WalletId};

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::chart::BsvHomeChartData;
use crate::services::error::BsvHomeServiceError;
use crate::services::perpetual::BsvHomePerpetualService;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::price::BsvHomePriceService;
use crate::services::stream::rules::hyperliquid_account;

pub use model::BsvHomePortfolioValues;
pub use store::BsvHomePortfolioStore;

#[uniffi::export]
pub fn portfolio_chart_data(data: PortfolioData, portfolio_type: PortfolioType, chart_type: PortfolioChartType) -> Option<BsvHomeChartData> {
    rules::portfolio_chart_data(data, portfolio_type, chart_type)
}

#[derive(uniffi::Object)]
pub struct BsvHomePortfolioService {
    api: Arc<BsvHomeDeviceApiClient>,
    store: Arc<dyn BsvHomePortfolioStore>,
    price: Arc<BsvHomePriceService>,
    perpetual: Arc<BsvHomePerpetualService>,
    preferences: Arc<BsvHomePreferencesService>,
    environment: BsvHomeRuntimeEnvironment,
}

#[uniffi::export]
impl BsvHomePortfolioService {
    #[uniffi::constructor]
    pub fn new(
        api: Arc<BsvHomeDeviceApiClient>,
        store: Arc<dyn BsvHomePortfolioStore>,
        price: Arc<BsvHomePriceService>,
        perpetual: Arc<BsvHomePerpetualService>,
        preferences: Arc<BsvHomePreferencesService>,
    ) -> Self {
        Self::new_with_environment(api, store, price, perpetual, preferences, BsvHomeRuntimeEnvironment::Production)
    }

    #[uniffi::constructor]
    pub fn new_with_environment(
        api: Arc<BsvHomeDeviceApiClient>,
        store: Arc<dyn BsvHomePortfolioStore>,
        price: Arc<BsvHomePriceService>,
        perpetual: Arc<BsvHomePerpetualService>,
        preferences: Arc<BsvHomePreferencesService>,
        environment: BsvHomeRuntimeEnvironment,
    ) -> Self {
        Self {
            api,
            store,
            price,
            perpetual,
            preferences,
            environment,
        }
    }

    pub async fn portfolio_data(&self, wallet: Wallet, portfolio_type: PortfolioType, period: ChartPeriod) -> Result<PortfolioData, BsvHomeServiceError> {
        match portfolio_type {
            PortfolioType::Wallet => Ok(rules::wallet_portfolio_data(
                self.sync_wallet_values(wallet.id, period, self.preferences.get_currency()).await?,
            )),
            PortfolioType::Perpetuals => {
                self.environment.require_chain(Chain::HyperCore)?;
                let account = hyperliquid_account(&wallet.accounts).ok_or(BsvHomeServiceError::NotFound {
                    msg: "wallet has no perpetual account".to_string(),
                })?;
                Ok(rules::perpetual_portfolio_data(
                    self.perpetual.get_portfolio(Chain::HyperCore, account.address.clone()).await?,
                    period,
                ))
            }
        }
    }
}

impl BsvHomePortfolioService {
    async fn sync_wallet_values(&self, wallet_id: WalletId, period: ChartPeriod, currency: Currency) -> Result<BsvHomePortfolioValues, BsvHomeServiceError> {
        let portfolio = self.get_wallet_assets(wallet_id, period).await?;
        let rate = self.price.rate(currency.clone()).await?.ok_or(BsvHomeServiceError::InvalidInput {
            msg: format!("unknown currency: {currency}"),
        })?;
        Ok(rules::converted_portfolio(portfolio, rate.rate))
    }

    async fn get_assets(&self, period: ChartPeriod, request: PortfolioAssetsRequest) -> Result<PortfolioAssets, BsvHomeApiError> {
        Ok(self.api.client.get_portfolio_assets(period, request).await?)
    }

    async fn get_wallet_assets(&self, wallet_id: WalletId, period: ChartPeriod) -> Result<PortfolioAssets, BsvHomeServiceError> {
        let assets = scoped_assets(self.environment, self.store.get_wallet_assets(wallet_id).await?);
        Ok(self.get_assets(period, PortfolioAssetsRequest { assets }).await?)
    }
}

fn scoped_assets(environment: BsvHomeRuntimeEnvironment, assets: Vec<primitives::PortfolioAsset>) -> Vec<primitives::PortfolioAsset> {
    assets.into_iter().filter(|asset| environment.allows_asset(&asset.asset_id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::{AssetId, PortfolioAsset, known_assets::TRON_USDT};

    #[test]
    fn test_wowpay_portfolio_preserves_exact_bsv_amounts_and_excludes_hidden_assets() {
        let allowed = [Chain::BitcoinSv.as_asset_id()];
        let assets = allowed
            .iter()
            .enumerate()
            .map(|(index, asset_id)| PortfolioAsset {
                asset_id: asset_id.clone(),
                value: (index as u64 + 1).into(),
            })
            .chain([
                PortfolioAsset {
                    asset_id: Chain::Tron.as_asset_id(),
                    value: 300u32.into(),
                },
                PortfolioAsset {
                    asset_id: TRON_USDT.id.clone(),
                    value: 400u32.into(),
                },
                PortfolioAsset {
                    asset_id: AssetId::from_token(Chain::Tron, "counterfeit-usdt"),
                    value: 100u32.into(),
                },
                PortfolioAsset {
                    asset_id: Chain::Ethereum.as_asset_id(),
                    value: 200u32.into(),
                },
            ])
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_value(scoped_assets(BsvHomeRuntimeEnvironment::Production, assets.clone())).unwrap(),
            serde_json::to_value(&assets).unwrap()
        );
        let scoped = scoped_assets(BsvHomeRuntimeEnvironment::WowPay, assets.clone());
        assert_eq!(scoped.iter().map(|asset| asset.asset_id.clone()).collect::<Vec<_>>(), allowed);
        assert_eq!(scoped.iter().map(|asset| asset.value.clone()).collect::<Vec<_>>(), vec![1u32.into()]);
        assert_eq!(assets.len(), 5);
    }
}
