use std::sync::Arc;

use chrono::Utc;
use primitives::rewards::{RedemptionRequest, RedemptionResult};
use primitives::{AuthenticatedRequest, ReferralCode, Rewards, Wallet, WalletId};

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::config::rewards::get_referral_url;
use crate::services::auth::BsvHomeAuthService;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet_session::rules as session_rules;

pub mod model;
pub mod rules;

pub use model::BsvHomeRewardsState;

#[derive(uniffi::Object)]
pub struct BsvHomeRewardsService {
    api: Arc<BsvHomeDeviceApiClient>,
    auth: Arc<BsvHomeAuthService>,
    balance: Arc<BsvHomeBalanceService>,
}

#[uniffi::export]
impl BsvHomeRewardsService {
    #[uniffi::constructor]
    pub fn new(api: Arc<BsvHomeDeviceApiClient>, auth: Arc<BsvHomeAuthService>, balance: Arc<BsvHomeBalanceService>) -> Self {
        Self { api, auth, balance }
    }

    pub fn wallets(&self, wallets: Vec<Wallet>) -> Vec<Wallet> {
        session_rules::rewards_wallets(wallets)
    }

    pub fn selected_wallet(&self, current: Option<Wallet>, wallets: Vec<Wallet>) -> Option<Wallet> {
        session_rules::rewards_wallet(current, &self.wallets(wallets))
    }

    pub fn referral_link(&self, code: String) -> String {
        get_referral_url(&code)
    }

    pub fn state(&self, rewards: Option<Rewards>) -> BsvHomeRewardsState {
        rules::state(rewards.as_ref(), Utc::now())
    }

    pub async fn get_rewards(&self, wallet_id: WalletId) -> Result<Rewards, BsvHomeServiceError> {
        Ok(self.api.client.get_rewards(wallet_id.id()).await.map_err(BsvHomeApiError::from)?)
    }

    pub async fn create_referral(&self, wallet: Wallet, code: String) -> Result<Rewards, BsvHomeServiceError> {
        let wallet_id = wallet.id.id();
        let request = AuthenticatedRequest {
            auth: self.auth.get_auth_payload(wallet).await?,
            data: ReferralCode { code },
        };
        Ok(self.api.client.create_referral(wallet_id, request).await.map_err(BsvHomeApiError::from)?)
    }

    pub async fn use_referral_code(&self, wallet: Wallet, code: String) -> Result<(), BsvHomeServiceError> {
        let wallet_id = wallet.id.id();
        let request = AuthenticatedRequest {
            auth: self.auth.get_auth_payload(wallet).await?,
            data: ReferralCode { code },
        };
        self.api.client.use_referral_code(wallet_id, request).await.map_err(BsvHomeApiError::from)?;
        Ok(())
    }

    pub async fn redeem(&self, wallet: Wallet, redemption_id: String) -> Result<RedemptionResult, BsvHomeServiceError> {
        let wallet_id = wallet.id.clone();
        let request = AuthenticatedRequest {
            auth: self.auth.get_auth_payload(wallet).await?,
            data: RedemptionRequest { id: redemption_id },
        };
        let result = self.api.client.redeem_rewards(wallet_id.id(), request).await.map_err(BsvHomeApiError::from)?;
        if let Some(asset) = &result.redemption.option.asset {
            self.balance.set_assets_enabled(wallet_id, vec![asset.id.clone()], true).await?;
        }
        Ok(result)
    }
}
