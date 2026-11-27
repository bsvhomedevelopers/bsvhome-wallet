use crate::services::error::BsvHomeServiceError;
use async_trait::async_trait;
use primitives::{AssetId, DelegationBase, DelegationValidator, StakeProviderType, WalletId};

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeStakeStore: Send + Sync {
    async fn get_apr(&self, asset_id: AssetId, provider_type: StakeProviderType) -> Result<Option<f64>, BsvHomeServiceError>;
    async fn get_validators(&self, asset_id: AssetId, provider_type: StakeProviderType) -> Result<Vec<DelegationValidator>, BsvHomeServiceError>;
    async fn save_validators(&self, validators: Vec<DelegationValidator>) -> Result<(), BsvHomeServiceError>;
    async fn deactivate_validators(&self, asset_id: AssetId, validator_ids: Vec<String>) -> Result<(), BsvHomeServiceError>;
    async fn get_delegation_ids(&self, wallet_id: WalletId, asset_id: AssetId, provider_type: StakeProviderType) -> Result<Vec<String>, BsvHomeServiceError>;
    async fn update_delegations(&self, wallet_id: WalletId, delegations: Vec<DelegationBase>, delete_ids: Vec<String>) -> Result<(), BsvHomeServiceError>;
}
