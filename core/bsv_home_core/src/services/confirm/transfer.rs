use crate::services::simulation::warning_rows;
use std::sync::Arc;

use primitives::currency::Currency;
use primitives::{Asset, Chain, PerpetualModifyConfirmData, SimulationResult, Wallet, WalletId};

use super::BsvHomeConfirmAuthorization;
use super::authorization::{Authorization, ExecutionScope};
use crate::config::fiat_config::get_fiat_config;
use crate::services::assets::config::BsvHomeAssetConfigService;
use crate::services::bsv_cold_wallet::{BsvHomeBsvColdWalletStore, bsv_cold_wallet_require_hot_signing};
use crate::services::confirm::rules::is_insufficient_network_fee;
use crate::services::confirm::{
    BsvHomeAcquireAssetFlow, BsvHomeConfirmError, BsvHomeConfirmFeeLoad, BsvHomeConfirmInput, BsvHomeConfirmLoad, BsvHomeConfirmLoadOptions, BsvHomeConfirmService,
    BsvHomeConfirmSession, BsvHomeConfirmSimulationState, BsvHomeExecuteResult, BsvHomeFeeAsset, BsvHomeTransactionCheckpoint, BsvHomeTransactionCheckpointData,
    BsvHomeTransactionSigner, SendInput,
};
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::name::BsvHomeNameService;
use crate::services::perpetual::model::BsvHomeAutocloseSummary;
use crate::services::perpetual::rules::autoclose_summary;
use crate::services::preferences::BsvHomePreferencesService;
use crate::services::transfer::rules::TransferInput;
use crate::services::transfer::{BsvHomeRecentActivityService, BsvHomeTransferData};
use crate::services::wallet::{BsvHomeKeystoreAuthentication, BsvHomeKeystorePassword};
use crate::services::wallet_session::BsvHomeWalletSessionService;
use primitives::BlockExplorerLink;
use primitives::TransactionInputType;

#[derive(uniffi::Object)]
pub struct BsvHomeConfirmTransferService {
    confirm: Arc<BsvHomeConfirmService>,
    explorer: Arc<BsvHomeExplorerService>,
    names: Arc<BsvHomeNameService>,
    asset_config: Arc<BsvHomeAssetConfigService>,
    signer: Arc<dyn BsvHomeTransactionSigner>,
    password: Arc<dyn BsvHomeKeystorePassword>,
    recent_activity: Arc<BsvHomeRecentActivityService>,
    preferences: Arc<BsvHomePreferencesService>,
}

#[uniffi::export]
impl BsvHomeConfirmTransferService {
    #[uniffi::constructor]
    pub fn new(
        confirm: Arc<BsvHomeConfirmService>,
        explorer: Arc<BsvHomeExplorerService>,
        names: Arc<BsvHomeNameService>,
        asset_config: Arc<BsvHomeAssetConfigService>,
        signer: Arc<dyn BsvHomeTransactionSigner>,
        password: Arc<dyn BsvHomeKeystorePassword>,
        recent_activity: Arc<BsvHomeRecentActivityService>,
        preferences: Arc<BsvHomePreferencesService>,
    ) -> Self {
        Self {
            confirm,
            explorer,
            names,
            asset_config,
            signer,
            password,
            recent_activity,
            preferences,
        }
    }

    pub fn session(self: Arc<Self>, wallet: Wallet, transfer: BsvHomeTransferData, simulation: Option<SimulationResult>) -> Arc<BsvHomeConfirmSession> {
        Arc::new(BsvHomeConfirmSession::new(self, wallet, transfer, simulation))
    }

    pub fn session_with_authorization(
        self: Arc<Self>,
        wallet: Wallet,
        transfer: BsvHomeTransferData,
        simulation: Option<SimulationResult>,
        checkpoint: Option<Arc<dyn BsvHomeTransactionCheckpoint>>,
        wallet_session: Arc<BsvHomeWalletSessionService>,
        authorization: Arc<dyn BsvHomeConfirmAuthorization>,
    ) -> Arc<BsvHomeConfirmSession> {
        Arc::new(BsvHomeConfirmSession::with_authorization(
            self,
            wallet,
            transfer,
            simulation,
            checkpoint,
            Authorization {
                port: authorization,
                session: wallet_session,
            },
        ))
    }

    pub fn session_with_checkpoint(
        self: Arc<Self>,
        wallet: Wallet,
        transfer: BsvHomeTransferData,
        simulation: Option<SimulationResult>,
        checkpoint: Arc<dyn BsvHomeTransactionCheckpoint>,
    ) -> Arc<BsvHomeConfirmSession> {
        Arc::new(BsvHomeConfirmSession::with_checkpoint(self, wallet, transfer, simulation, checkpoint))
    }

    pub async fn broadcast_saved_transaction(
        &self,
        wallet: Wallet,
        transfer: BsvHomeTransferData,
        saved: BsvHomeTransactionCheckpointData,
        checkpoint: Arc<dyn BsvHomeTransactionCheckpoint>,
    ) -> Result<String, BsvHomeConfirmError> {
        self.confirm.broadcast_saved_transaction(wallet, transfer, saved, checkpoint).await
    }

    pub fn address_url(&self, chain: Chain, address: String) -> BlockExplorerLink {
        self.explorer.get_address_url(chain, address)
    }
}

fn simulation_seed(chain: Chain, simulation: Option<SimulationResult>) -> BsvHomeConfirmSimulationState {
    BsvHomeConfirmSimulationState {
        chain,
        warnings: simulation.as_ref().map(|result| warning_rows(&result.warnings)).unwrap_or_default(),
        result: simulation,
        simulation: None,
        address_names: Vec::new(),
    }
}

fn is_broadcast(result: &BsvHomeExecuteResult) -> bool {
    match result {
        BsvHomeExecuteResult::Sent { .. } => true,
        BsvHomeExecuteResult::Signed { .. } => false,
    }
}

impl BsvHomeConfirmTransferService {
    pub(super) fn cold_wallet_store(&self) -> Arc<dyn BsvHomeBsvColdWalletStore> {
        self.confirm.cold_wallet_store.clone()
    }

    pub(super) fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }
    pub(super) fn authentication(&self) -> BsvHomeKeystoreAuthentication {
        self.password.authentication().unwrap_or(BsvHomeKeystoreAuthentication::None)
    }
    pub(super) fn autoclose_summary(&self, data: PerpetualModifyConfirmData) -> Option<BsvHomeAutocloseSummary> {
        autoclose_summary(&data)
    }
    pub(super) fn acquire_asset_flow(&self, chain: Chain) -> BsvHomeAcquireAssetFlow {
        self.asset_config.acquire_flow(chain)
    }
    pub(super) fn insufficient_network_fee_buy_amount(&self) -> i32 {
        get_fiat_config().insufficient_network_fee_buy_amount
    }

    pub(super) async fn execute(
        &self,
        input: SendInput,
        checkpoint: Option<Arc<dyn BsvHomeTransactionCheckpoint>>,
        scope: Arc<ExecutionScope>,
    ) -> Result<BsvHomeExecuteResult, BsvHomeConfirmError> {
        let wallet_id = input.wallet.id.clone();
        let input_type = input.confirm.input.transfer.input_type.clone();
        let result = self.confirm.execute_scoped(input, self.signer.clone(), checkpoint, Some(scope)).await?;
        if is_broadcast(&result) {
            let _ = self.recent_activity.add(input_type, wallet_id).await;
        }
        Ok(result)
    }

    pub(super) fn confirm_input(&self, wallet: Wallet, transfer: BsvHomeTransferData) -> Result<BsvHomeConfirmInput, BsvHomeConfirmError> {
        self.confirm.require_input_assets(&transfer.input_type)?;
        let chain = transfer.input_type.get_asset().chain();
        bsv_cold_wallet_require_hot_signing(wallet.clone(), chain, self.confirm.cold_wallet_store.clone())?;
        let from = wallet.account(chain).cloned().ok_or(BsvHomeConfirmError::AccountMissing { chain })?;
        Ok(BsvHomeConfirmInput { from, transfer })
    }

    async fn fee_assets(&self, wallet_id: WalletId, chain: Chain) -> Result<Vec<BsvHomeFeeAsset>, BsvHomeConfirmError> {
        self.confirm.fee_assets(wallet_id, chain).await
    }

    pub(super) async fn preload(&self, wallet_id: WalletId, input: BsvHomeConfirmInput, options: BsvHomeConfirmLoadOptions) -> Result<BsvHomeConfirmFeeLoad, BsvHomeConfirmError> {
        let input_type = input.transfer.input_type.clone();
        match self.confirm.preload(wallet_id.clone(), input, options).await {
            Ok(fee) => Ok(fee),
            Err(error) => Err(self.missing_network_fee(wallet_id, input_type).await.unwrap_or(error)),
        }
    }

    pub(super) async fn state(&self, wallet_id: WalletId, input: &BsvHomeConfirmInput, simulation: Option<SimulationResult>) -> Result<BsvHomeConfirmLoad, BsvHomeConfirmError> {
        let input_type = input.transfer.input_type.clone();
        let chain = input_type.transaction_asset().chain();
        let (metadata, fee_assets, address_name) = futures::join!(
            self.confirm.input_metadata(wallet_id.clone(), &input_type, input_type.fee_asset().id),
            self.fee_assets(wallet_id, chain),
            self.names.address_name(chain, input.transfer.recipient.address.clone()),
        );
        Ok(BsvHomeConfirmLoad {
            sender: input.from.clone(),
            fee_asset: input_type.fee_asset(),
            metadata: metadata?,
            fee_assets: fee_assets?,
            simulation: simulation_seed(chain, simulation),
            address_name: address_name.unwrap_or_default(),
            preload: None,
        })
    }

    pub(super) async fn simulation_state(
        &self,
        input_type: TransactionInputType,
        simulation: Option<SimulationResult>,
    ) -> Result<BsvHomeConfirmSimulationState, BsvHomeConfirmError> {
        let chain = input_type.transaction_asset().chain();
        let assets = match &simulation {
            Some(simulation) => self.confirm.ensure_simulation_assets(simulation.asset_ids()).await?,
            None => Vec::new(),
        };
        let Ok(details) = self.confirm.simulation(input_type, simulation.clone(), assets) else {
            return Ok(simulation_seed(chain, simulation));
        };
        let requests = details.address_requests(chain);
        let address_names = self.names.get_address_names(requests).await.unwrap_or_default();
        Ok(BsvHomeConfirmSimulationState {
            chain,
            warnings: simulation.as_ref().map(|result| warning_rows(&result.warnings)).unwrap_or_default(),
            result: simulation,
            simulation: Some(details),
            address_names,
        })
    }

    async fn missing_network_fee(&self, wallet_id: WalletId, input_type: TransactionInputType) -> Option<BsvHomeConfirmError> {
        let balance = self.confirm.input_metadata(wallet_id, &input_type, input_type.fee_asset().id).await.ok()?.fee_asset_balance;
        is_insufficient_network_fee(&balance.asset_id, &balance.available).then(|| BsvHomeConfirmError::InsufficientNetworkFee {
            asset: Asset::from_chain(balance.asset_id.chain),
            requirement: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_only_a_broadcast_send_records_recent_activity() {
        let sent = BsvHomeExecuteResult::Sent {
            hashes: vec!["0xhash".to_string()],
            transactions: vec![],
        };
        let signed = BsvHomeExecuteResult::Signed {
            data: vec!["0xsigned".to_string()],
        };

        assert!(is_broadcast(&sent));
        assert!(!is_broadcast(&signed));
    }
}
