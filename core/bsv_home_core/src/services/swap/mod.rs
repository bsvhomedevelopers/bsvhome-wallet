pub mod model;
pub mod quote;
pub mod rules;
#[cfg(test)]
mod scope_tests;
pub mod session;
pub mod store;
#[cfg(test)]
pub(crate) mod testkit;

use crate::keystore::decode_password;
use crate::models::custom_types::BsvHomeBigUint;
use primitives::unix_seconds;
use std::sync::Arc;

use primitives::{Asset, Wallet};
use swapper::permit2_data::Permit2Data;
use swapper::{AssetList, FetchQuoteData, Quote, SwapperError};

use crate::bsv_home_swapper::{BsvHomeSwapper, permit2_data_to_eip712_json};
use crate::config::swap_config::get_swap_config;
use crate::keystore::{BsvHomeKeystore, keystore_id_for_wallet};
use crate::message::sign_type::{SignDigestType, SignMessage};
use crate::message::signer::MessageSigner;
use crate::models::swap::BsvHomeSwapQuoteData;
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::BsvHomeKeystorePassword;
pub use model::{
    BsvHomeSwapButtonAction, BsvHomeSwapButtonInput, BsvHomeSwapPair, BsvHomeSwapPairSuggestion, BsvHomeSwapQuoteSummary, BsvHomeSwapTransfer, swap_quote_summary,
    swapper_quote_summary,
};
use primitives::{AssetId, WalletId};
pub use session::{
    BsvHomeSwapButtonState, BsvHomeSwapQuotePhase, BsvHomeSwapQuotesResult, BsvHomeSwapRequest, BsvHomeSwapSession, BsvHomeSwapSessionAction, BsvHomeSwapTransferPhase,
};
pub use store::BsvHomeSwapStore;

#[derive(uniffi::Object)]
pub struct BsvHomeSwapService {
    swapper: Arc<BsvHomeSwapper>,
    keystore: Arc<BsvHomeKeystore>,
    password: Arc<dyn BsvHomeKeystorePassword>,
    store: Arc<dyn BsvHomeSwapStore>,
    environment: BsvHomeRuntimeEnvironment,
}

#[uniffi::export]
impl BsvHomeSwapService {
    #[uniffi::constructor]
    pub fn new(swapper: Arc<BsvHomeSwapper>, keystore: Arc<BsvHomeKeystore>, password: Arc<dyn BsvHomeKeystorePassword>, store: Arc<dyn BsvHomeSwapStore>) -> Self {
        Self::new_with_environment(swapper, keystore, password, store, BsvHomeRuntimeEnvironment::Production)
    }

    #[uniffi::constructor]
    pub fn new_with_environment(
        swapper: Arc<BsvHomeSwapper>,
        keystore: Arc<BsvHomeKeystore>,
        password: Arc<dyn BsvHomeKeystorePassword>,
        store: Arc<dyn BsvHomeSwapStore>,
        environment: BsvHomeRuntimeEnvironment,
    ) -> Self {
        Self {
            swapper,
            keystore,
            password,
            store,
            environment,
        }
    }

    pub async fn get_quotes(
        &self,
        wallet: Wallet,
        from_asset: Asset,
        to_asset: Asset,
        value: BsvHomeBigUint,
        use_max_amount: bool,
        slippage_bps: Option<u32>,
    ) -> Result<Vec<Quote>, SwapperError> {
        self.environment.require_asset(&from_asset.id).map_err(|_| SwapperError::NotSupportedAsset)?;
        self.environment.require_asset(&to_asset.id).map_err(|_| SwapperError::NotSupportedAsset)?;
        let request = rules::quote_request(&wallet, &from_asset, &to_asset, value, use_max_amount, slippage_bps)?;
        self.swapper.preload_routes(from_asset.id, to_asset.id).await;
        Ok(rules::sort_quotes(self.swapper.get_quote(&request).await?))
    }

    pub async fn suggest_pair(&self, wallet_id: WalletId, pay_asset_id: Option<AssetId>) -> Result<Option<BsvHomeSwapPairSuggestion>, BsvHomeServiceError> {
        let pay_asset_id = match pay_asset_id {
            Some(asset_id) => asset_id,
            None => match self
                .store
                .get_pay_asset_ids(wallet_id.clone())
                .await?
                .into_iter()
                .find(|asset_id| self.environment.allows_asset(asset_id))
            {
                Some(asset_id) => asset_id,
                None => return Ok(None),
            },
        };
        self.environment.require_asset(&pay_asset_id)?;
        Ok(Some(BsvHomeSwapPairSuggestion {
            receive_asset_id: self.suggest_receive_asset(&wallet_id, &pay_asset_id).await?,
            pay_asset_id,
        }))
    }

    pub async fn get_transfer(&self, wallet: Wallet, quote: Quote) -> Result<BsvHomeSwapTransfer, SwapperError> {
        for id in [&quote.request.from_asset.id, &quote.request.to_asset.id] {
            let asset_id = AssetId::new(id).ok_or(SwapperError::NotSupportedAsset)?;
            self.environment.require_asset(&asset_id).map_err(|_| SwapperError::NotSupportedAsset)?;
        }
        let data = self.get_quote_data(&wallet, &quote).await?;
        rules::swap_transfer(&wallet, &quote, data)
    }
}

impl BsvHomeSwapService {
    pub fn pair_for_asset(&self, asset_id: AssetId, has_balance: bool) -> Result<BsvHomeSwapPairSuggestion, BsvHomeServiceError> {
        self.environment.require_asset(&asset_id)?;
        let pair = rules::pair_for_asset(asset_id, has_balance);
        self.environment.require_asset(&pair.pay_asset_id)?;
        if let Some(asset_id) = &pair.receive_asset_id {
            self.environment.require_asset(asset_id)?;
            if self.environment.allowed_asset_ids().is_some() && !self.supported_assets(pair.pay_asset_id.clone()).asset_ids.contains(asset_id) {
                return Err(BsvHomeServiceError::Unsupported {
                    msg: "swap pair unavailable in this environment".into(),
                });
            }
        }
        Ok(pair)
    }

    pub fn supported_assets(&self, asset_id: AssetId) -> AssetList {
        if !self.environment.allows_asset(&asset_id) {
            return AssetList {
                chains: Vec::new(),
                asset_ids: Vec::new(),
            };
        }
        scoped_supported_assets(self.environment, self.swapper.supported_chains_for_from_asset(&asset_id))
    }
}

impl BsvHomeSwapService {
    async fn suggest_receive_asset(&self, wallet_id: &WalletId, pay_asset_id: &AssetId) -> Result<Option<AssetId>, BsvHomeServiceError> {
        let pairs = self
            .store
            .get_swap_pairs(wallet_id.clone())
            .await?
            .into_iter()
            .filter(|pair| self.environment.allows_asset(&pair.from_asset_id) && self.environment.allows_asset(&pair.to_asset_id))
            .collect::<Vec<_>>();
        if let Some(asset_id) = rules::most_swapped_receive_asset(&pairs, pay_asset_id) {
            return Ok(Some(asset_id));
        }
        let recents = self
            .store
            .get_recent_asset_ids(wallet_id.clone())
            .await?
            .into_iter()
            .filter(|asset_id| self.environment.allows_asset(asset_id))
            .collect();
        if let Some(asset_id) = rules::first_other_asset(recents, pay_asset_id) {
            return Ok(Some(asset_id));
        }
        let supported = self.supported_assets(pay_asset_id.clone());
        let candidates = self
            .store
            .get_receive_asset_ids(wallet_id.clone(), supported.chains, supported.asset_ids)
            .await?
            .into_iter()
            .filter(|asset_id| self.environment.allows_asset(asset_id))
            .collect();
        Ok(rules::first_other_asset(candidates, pay_asset_id))
    }

    async fn get_quote_data(&self, wallet: &Wallet, quote: &Quote) -> Result<BsvHomeSwapQuoteData, SwapperError> {
        let data = match self.swapper.get_permit2_for_quote(quote).await? {
            Some(approval) => FetchQuoteData::Permit2(self.permit2_data(wallet, quote, &approval)?),
            None => FetchQuoteData::None,
        };
        self.swapper.get_quote_data(quote, data).await
    }

    fn permit2_data(&self, wallet: &Wallet, quote: &Quote, approval: &swapper::Permit2ApprovalData) -> Result<Permit2Data, SwapperError> {
        let chain = AssetId::new(&quote.request.from_asset.id).ok_or(SwapperError::NotSupportedAsset)?.chain;
        let now = unix_seconds().map_err(|error| SwapperError::TransactionError(error.to_string()))?;
        let permit_single = rules::permit_single(approval, now, &get_swap_config());
        let json = permit2_data_to_eip712_json(chain, permit_single.clone(), &approval.permit2_contract)?;
        let signer = MessageSigner::new(SignMessage {
            chain,
            sign_type: SignDigestType::Eip712,
            data: json.into_bytes(),
        });
        let password = self
            .password
            .get_password(false)
            .map(|password| decode_password(&password))
            .map_err(|error| SwapperError::TransactionError(error.to_string()))?;
        let signature = signer
            .sign_with_keystore(self.keystore.clone(), keystore_id_for_wallet(wallet.id.id()), password)
            .map_err(|error| SwapperError::TransactionError(error.to_string()))?;
        let signature = primitives::hex::decode_hex(&signature).map_err(|error| SwapperError::TransactionError(error.to_string()))?;
        Ok(Permit2Data { permit_single, signature })
    }
}

fn scoped_supported_assets(environment: BsvHomeRuntimeEnvironment, supported: AssetList) -> AssetList {
    match environment.allowed_asset_ids() {
        Some(assets) => AssetList {
            chains: Vec::new(),
            asset_ids: assets
                .into_iter()
                .filter(|asset| supported.chains.contains(&asset.chain) || supported.asset_ids.contains(asset))
                .collect(),
        },
        None => supported,
    }
}
