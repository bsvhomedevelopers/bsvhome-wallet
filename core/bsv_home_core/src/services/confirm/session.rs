use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::lock::Mutex;
use primitives::currency::Currency;
use primitives::{BlockExplorerLink, Chain, PerpetualModifyConfirmData, SimulationResult, Wallet};

use super::authorization::{Authorization, ExecutionScope, authorization_error};
use super::rules::preload_simulation;
use super::{
    BsvHomeAcquireAssetFlow, BsvHomeConfirmError, BsvHomeConfirmLoad, BsvHomeConfirmLoadOptions, BsvHomeConfirmScreen, BsvHomeConfirmTransferService, BsvHomeExecuteResult,
    BsvHomeTransactionCheckpoint, BsvHomeTransferAmountResult, SendInput,
};
use crate::services::perpetual::model::BsvHomeAutocloseSummary;
use crate::services::transfer::BsvHomeTransferData;
use crate::services::wallet::BsvHomeKeystoreAuthentication;

#[cfg(test)]
#[path = "authorization_tests.rs"]
mod authorization_tests;

#[derive(uniffi::Object)]
pub struct BsvHomeConfirmSession {
    service: Arc<BsvHomeConfirmTransferService>,
    wallet: Wallet,
    transfer: BsvHomeTransferData,
    simulation: Option<SimulationResult>,
    screen: Mutex<Option<BsvHomeConfirmLoad>>,
    checkpoint: Option<Arc<dyn BsvHomeTransactionCheckpoint>>,
    execution: Mutex<()>,
    revision: Arc<AtomicU64>,
    authorization: Option<Authorization>,
}

impl BsvHomeConfirmSession {
    pub(super) fn new(service: Arc<BsvHomeConfirmTransferService>, wallet: Wallet, transfer: BsvHomeTransferData, simulation: Option<SimulationResult>) -> Self {
        Self {
            service,
            wallet,
            transfer,
            simulation,
            screen: Mutex::new(None),
            checkpoint: None,
            execution: Mutex::new(()),
            revision: Arc::new(AtomicU64::new(0)),
            authorization: None,
        }
    }

    pub(super) fn with_checkpoint(
        service: Arc<BsvHomeConfirmTransferService>,
        wallet: Wallet,
        transfer: BsvHomeTransferData,
        simulation: Option<SimulationResult>,
        checkpoint: Arc<dyn BsvHomeTransactionCheckpoint>,
    ) -> Self {
        Self {
            checkpoint: Some(checkpoint),
            ..Self::new(service, wallet, transfer, simulation)
        }
    }

    pub(super) fn with_authorization(
        service: Arc<BsvHomeConfirmTransferService>,
        wallet: Wallet,
        transfer: BsvHomeTransferData,
        simulation: Option<SimulationResult>,
        checkpoint: Option<Arc<dyn BsvHomeTransactionCheckpoint>>,
        authorization: Authorization,
    ) -> Self {
        Self {
            checkpoint,
            authorization: Some(authorization),
            ..Self::new(service, wallet, transfer, simulation)
        }
    }
}

#[uniffi::export]
impl BsvHomeConfirmSession {
    pub fn screen(&self) -> BsvHomeConfirmScreen {
        BsvHomeConfirmScreen::initial(self.simulation.as_ref())
    }

    pub fn get_currency(&self) -> Currency {
        self.service.get_currency()
    }

    pub fn authentication(&self) -> BsvHomeKeystoreAuthentication {
        self.service.authentication()
    }

    pub fn address_url(&self, chain: Chain, address: String) -> BlockExplorerLink {
        self.service.address_url(chain, address)
    }

    pub fn acquire_asset_flow(&self, chain: Chain) -> BsvHomeAcquireAssetFlow {
        self.service.acquire_asset_flow(chain)
    }

    pub fn insufficient_network_fee_buy_amount(&self) -> i32 {
        self.service.insufficient_network_fee_buy_amount()
    }

    pub fn autoclose_summary(&self, data: PerpetualModifyConfirmData) -> Option<BsvHomeAutocloseSummary> {
        self.service.autoclose_summary(data)
    }

    pub async fn execute(&self) -> Result<BsvHomeExecuteResult, BsvHomeConfirmError> {
        let generation = self.revision.load(Ordering::SeqCst);
        let _execution = if self.authorization.is_some() {
            self.execution.try_lock().ok_or(BsvHomeConfirmError::Cancelled)?
        } else {
            self.execution.lock().await
        };
        let scope = Arc::new(ExecutionScope {
            revision: self.revision.clone(),
            generation,
            wallet: self.wallet.clone(),
            chain: self.transfer.input_type.get_asset().chain(),
            session: self.authorization.as_ref().map(|authorization| authorization.session.clone()),
            cold_wallet_store: self.service.cold_wallet_store(),
        });
        scope.check().await?;
        let screen = self.screen.lock().await.clone();
        let preload = screen.as_ref().and_then(|screen| screen.preload.clone()).ok_or_else(|| BsvHomeConfirmError::Load {
            msg: "confirm input is not loaded".to_string(),
        })?;
        let amount = match preload.amount {
            BsvHomeTransferAmountResult::Amount { amount } => amount,
            BsvHomeTransferAmountResult::Error { error } => return Err(error),
        };
        let simulation = screen.and_then(|screen| screen.simulation.result);
        if let Some(authorization) = &self.authorization {
            authorization
                .port
                .authorize(self.wallet.clone(), self.transfer.clone())
                .await
                .map_err(authorization_error)?;
            scope.check().await?;
        }
        self.service
            .execute(
                SendInput {
                    wallet: self.wallet.clone(),
                    confirm: preload.confirm_data,
                    value: amount.value,
                    network_fee: amount.network_fee,
                    simulation,
                },
                self.checkpoint.clone(),
                scope,
            )
            .await
    }

    pub fn cancel(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    pub async fn state(&self) -> Result<BsvHomeConfirmLoad, BsvHomeConfirmError> {
        if let Some(screen) = self.screen.lock().await.clone() {
            return Ok(screen);
        }
        let generation = self.revision.load(Ordering::SeqCst);
        let input = self.service.confirm_input(self.wallet.clone(), self.transfer.clone())?;
        let screen = self.service.state(self.wallet.id.clone(), &input, self.simulation.clone()).await?;
        let mut current = self.screen.lock().await;
        if self.revision.load(Ordering::SeqCst) != generation {
            return Err(BsvHomeConfirmError::Cancelled);
        }
        Ok(current.get_or_insert(screen).clone())
    }

    pub async fn load(&self, options: BsvHomeConfirmLoadOptions) -> Result<BsvHomeConfirmLoad, BsvHomeConfirmError> {
        let generation = self.revision.load(Ordering::SeqCst);
        let _execution = if self.authorization.is_some() {
            Some(self.execution.try_lock().ok_or(BsvHomeConfirmError::Cancelled)?)
        } else {
            None
        };
        let input = self.service.confirm_input(self.wallet.clone(), self.transfer.clone())?;
        let input_type = self.transfer.input_type.clone();
        let requested = async {
            match self.simulation.clone() {
                Some(simulation) => Some(self.service.simulation_state(input_type.clone(), Some(simulation)).await),
                None => None,
            }
        };
        let (screen, fee, requested) = futures::join!(self.state(), self.service.preload(self.wallet.id.clone(), input, options), requested);
        let fee = fee?;
        let simulation = match preload_simulation(self.simulation.as_ref(), &fee.preload) {
            Some(simulation) => Some(self.service.simulation_state(input_type, Some(simulation)).await?),
            None => requested.transpose()?,
        };
        let screen = screen?.with_fee(fee, simulation);
        let mut current = self.screen.lock().await;
        if self.revision.load(Ordering::SeqCst) != generation {
            return Err(BsvHomeConfirmError::Cancelled);
        }
        *current = Some(screen.clone());
        Ok(screen)
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use num_bigint::BigInt;
    use primitives::{Account, Asset, AssetId, Chain, FeePriority, SimulationBalanceChange, SimulationResult, SimulationWarning, TransactionInputType, Wallet, WalletId};

    use super::super::testkit::ConfirmTestkit;
    use crate::services::confirm::{BsvHomeConfirmError, BsvHomeConfirmFeeSelection, BsvHomeConfirmLoadOptions};
    use crate::services::transfer::{BsvHomeRecipient, BsvHomeTransferData};

    #[test]
    fn test_request_wallet_balances_ignore_selected_wallet() {
        block_on(async {
            let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::Tron, "TJRyWwFs9wTFGZg3JbrVriFbNfCug5tDeC")]);
            let selected_wallet = Wallet {
                id: WalletId::Multicoin("other-wallet".into()),
                ..Wallet::mock()
            };
            let testkit = ConfirmTestkit::new(wallet.clone(), selected_wallet);
            let transfer = BsvHomeTransferData {
                input_type: TransactionInputType::Transfer {
                    asset: Asset::from_chain(Chain::Tron),
                },
                recipient: BsvHomeRecipient::address("THTR75o8xXAgCTQqpiot2AFRAjvW1tSbVV".into()),
                value: 0.into(),
                use_max_amount: false,
            };
            let session = testkit.service.session(wallet.clone(), transfer, None);

            let state = session.state().await.unwrap();

            assert_eq!(state.sender.address, wallet.accounts[0].address);
            assert_eq!(state.sender.chain, Chain::Tron);
            assert_eq!(state.metadata.asset_balance.available, 100u32.into());
            assert_eq!(state.metadata.fee_asset_balance.available, 100u32.into());
            assert_eq!(*testkit.balances.requests.lock().unwrap(), vec![wallet.id.clone()]);

            let options = BsvHomeConfirmLoadOptions {
                fee_selection: BsvHomeConfirmFeeSelection::Priority { priority: FeePriority::Normal },
                fee_asset_id: None,
            };
            assert!(session.load(options).await.is_err());
            assert_eq!(*testkit.balances.requests.lock().unwrap(), vec![wallet.id.clone(), wallet.id]);
        });
    }

    #[test]
    fn test_execute_refuses_a_session_without_a_preload() {
        block_on(async {
            let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::Tron, "TJRyWwFs9wTFGZg3JbrVriFbNfCug5tDeC")]);
            let testkit = ConfirmTestkit::new(wallet.clone(), wallet.clone());
            let transfer = BsvHomeTransferData {
                input_type: TransactionInputType::Transfer {
                    asset: Asset::from_chain(Chain::Tron),
                },
                recipient: BsvHomeRecipient::address("THTR75o8xXAgCTQqpiot2AFRAjvW1tSbVV".into()),
                value: 0.into(),
                use_max_amount: false,
            };
            let session = testkit.service.session(wallet, transfer, None);

            assert!(matches!(session.execute().await, Err(BsvHomeConfirmError::Load { .. })));
            session.state().await.unwrap();
            assert!(matches!(session.execute().await, Err(BsvHomeConfirmError::Load { .. })));
        });
    }

    #[test]
    fn test_state_shows_the_request_simulation_without_waiting_for_its_assets_and_names() {
        block_on(async {
            let wallet = Wallet::mock_with_accounts(vec![Account::mock(Chain::Tron, "TJRyWwFs9wTFGZg3JbrVriFbNfCug5tDeC")]);
            let testkit = ConfirmTestkit::new(wallet.clone(), wallet.clone());
            let simulation = SimulationResult {
                warnings: vec![SimulationWarning::validation_error("careful")],
                balance_changes: vec![SimulationBalanceChange::mock(AssetId::from(Chain::Tron, Some("unknown-token".into())), BigInt::from(1), 6)],
                payload: vec![],
                header: None,
            };
            let transfer = BsvHomeTransferData {
                input_type: TransactionInputType::Transfer {
                    asset: Asset::from_chain(Chain::Tron),
                },
                recipient: BsvHomeRecipient::address("THTR75o8xXAgCTQqpiot2AFRAjvW1tSbVV".into()),
                value: 0.into(),
                use_max_amount: false,
            };
            let session = testkit.service.session(wallet, transfer, Some(simulation.clone()));

            let state = session.state().await.unwrap();

            assert_eq!(state.simulation.result, Some(simulation));
            assert_eq!(state.simulation.warnings.len(), 1);
            assert!(state.simulation.simulation.is_none());
            assert!(state.simulation.address_names.is_empty());
        });
    }
}
