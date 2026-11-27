pub mod model;
#[cfg(test)]
mod tests;

use std::future::Future;
use std::sync::Arc;

use primitives::Wallet;

pub use model::{BsvHomeAppStartFailure, BsvHomeAppStartStep};

use crate::services::assets::BsvHomeAssetsService;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::banner::BsvHomeBannerService;
use crate::services::config::BsvHomeConfigService;
use crate::services::device::BsvHomeDeviceService;
use crate::services::error::BsvHomeServiceError;
use crate::services::failures::{StepFailure, record};
use crate::services::wallet::BsvHomeWalletService;
use crate::services::wallet_configuration::BsvHomeWalletConfigurationService;

#[derive(uniffi::Object)]
pub struct BsvHomeAppStartService {
    config: Arc<BsvHomeConfigService>,
    banners: Arc<BsvHomeBannerService>,
    assets: Arc<BsvHomeAssetsService>,
    balance: Arc<BsvHomeBalanceService>,
    wallet_configuration: Arc<BsvHomeWalletConfigurationService>,
    wallet: Arc<BsvHomeWalletService>,
    device: Arc<BsvHomeDeviceService>,
}

#[uniffi::export]
impl BsvHomeAppStartService {
    #[uniffi::constructor]
    pub fn new(
        config: Arc<BsvHomeConfigService>,
        banners: Arc<BsvHomeBannerService>,
        assets: Arc<BsvHomeAssetsService>,
        balance: Arc<BsvHomeBalanceService>,
        wallet_configuration: Arc<BsvHomeWalletConfigurationService>,
        wallet: Arc<BsvHomeWalletService>,
        device: Arc<BsvHomeDeviceService>,
    ) -> Self {
        Self {
            config,
            banners,
            assets,
            balance,
            wallet_configuration,
            wallet,
            device,
        }
    }

    pub async fn setup_wallets(&self) -> Vec<BsvHomeAppStartFailure> {
        let mut failures = Vec::new();
        record(&mut failures, BsvHomeAppStartStep::SetupWalletAssets, self.assets.sync_default_assets()).await;
        match self.wallet.setup_chains_outcome(self.wallet.available_chains()).await {
            Ok(outcome) => {
                for (wallet_id, error) in outcome.failures {
                    failures.push(BsvHomeAppStartFailure::new(BsvHomeAppStartStep::SetupChains, format!("wallet {}: {error}", wallet_id.id())));
                }
            }
            Err(error) => failures.push(BsvHomeAppStartFailure::new(BsvHomeAppStartStep::SetupChains, error.to_string())),
        }
        match self.wallet.wallets().await {
            Ok(wallets) => {
                for wallet in wallets {
                    let wallet_id = wallet.id.clone();
                    if let Err(error) = self.balance.setup_wallet(wallet).await {
                        failures.push(BsvHomeAppStartFailure::new(
                            BsvHomeAppStartStep::SetupWalletAssets,
                            format!("wallet {}: {error}", wallet_id.id()),
                        ));
                    }
                }
            }
            Err(error) => failures.push(BsvHomeAppStartFailure::new(BsvHomeAppStartStep::SetupWalletAssets, error.to_string())),
        }
        failures
    }

    pub async fn run(&self) -> Vec<BsvHomeAppStartFailure> {
        if !self.assets.environment().uses_bsv_home_hosted_services() {
            return recorded(BsvHomeAppStartStep::SetupBanners, self.banners.setup()).await;
        }
        let (banners, assets, device) = futures::join!(
            recorded(BsvHomeAppStartStep::SetupBanners, self.banners.setup()),
            self.sync_config_and_assets(),
            recorded(BsvHomeAppStartStep::SyncDevice, async { self.device.synchronize().await.map(|_| ()) }),
        );
        [banners, assets, device].concat()
    }

    pub async fn initialize_wallet_assets(&self, wallet: Wallet) -> Result<(), BsvHomeServiceError> {
        self.assets.ensure_default_assets().await?;
        self.balance.initialize_wallet_balances(wallet).await.map(|_| ())
    }

    pub async fn setup_wallet(&self, wallet: Wallet) -> Vec<BsvHomeAppStartFailure> {
        let mut failures = Vec::new();
        record(&mut failures, BsvHomeAppStartStep::SetupWalletBanners, self.banners.setup_wallet(wallet.clone())).await;
        record(&mut failures, BsvHomeAppStartStep::SetupWalletAssets, async {
            self.assets.ensure_default_assets().await?;
            self.balance.setup_wallet(wallet.clone()).await
        })
        .await;
        record(&mut failures, BsvHomeAppStartStep::SyncWalletConfiguration, self.wallet_configuration.sync(wallet.id)).await;
        failures
    }
}

impl BsvHomeAppStartService {
    async fn sync_config_and_assets(&self) -> Vec<BsvHomeAppStartFailure> {
        let mut failures = recorded(BsvHomeAppStartStep::UpdateConfig, async { self.config.update_config().await.map(|_| ()) }).await;
        record(&mut failures, BsvHomeAppStartStep::SyncAssets, self.sync_assets()).await;
        failures
    }

    async fn sync_assets(&self) -> Result<(), BsvHomeServiceError> {
        self.assets.sync_swappable_chains().await?;
        let config = self.config.get_config().await?;
        self.assets.sync_availability(config.versions).await
    }
}

async fn recorded<F>(step: BsvHomeAppStartStep, future: F) -> Vec<BsvHomeAppStartFailure>
where
    F: Future<Output = Result<(), BsvHomeServiceError>>,
{
    let mut failures = Vec::new();
    record(&mut failures, step, future).await;
    failures
}
