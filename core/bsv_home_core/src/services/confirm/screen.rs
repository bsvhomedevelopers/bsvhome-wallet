use primitives::SimulationResult;

use super::error::BsvHomeConfirmError;
use super::model::{
    BsvHomeConfirmAction, BsvHomeConfirmButton, BsvHomeConfirmButtonKind, BsvHomeConfirmButtonState, BsvHomeConfirmFailure, BsvHomeConfirmFeeRow, BsvHomeConfirmLoad,
    BsvHomeConfirmPhase, BsvHomeConfirmScreen, BsvHomeConfirmStage, BsvHomeTransferAmountResult,
};

impl BsvHomeConfirmScreen {
    pub fn initial(simulation: Option<&SimulationResult>) -> Self {
        Self {
            phase: BsvHomeConfirmPhase::Loading,
            amount_failed: false,
            has_critical_warning: simulation.is_some_and(SimulationResult::has_critical_warning),
            failure: None,
        }
    }

    fn failed(&self, stage: BsvHomeConfirmStage, error: BsvHomeConfirmError) -> Self {
        Self {
            phase: BsvHomeConfirmPhase::Failed,
            failure: Some(BsvHomeConfirmFailure { stage, error }),
            ..self.clone()
        }
    }
}

#[uniffi::export]
impl BsvHomeConfirmScreen {
    pub fn button(&self) -> BsvHomeConfirmButton {
        let button = |kind, state| BsvHomeConfirmButton { kind, state };
        match self.phase {
            BsvHomeConfirmPhase::Loading | BsvHomeConfirmPhase::Confirming => button(BsvHomeConfirmButtonKind::Confirm, BsvHomeConfirmButtonState::Loading),
            BsvHomeConfirmPhase::Failed => button(BsvHomeConfirmButtonKind::Retry, BsvHomeConfirmButtonState::Enabled),
            BsvHomeConfirmPhase::Ready if self.amount_failed => button(BsvHomeConfirmButtonKind::Retry, BsvHomeConfirmButtonState::Enabled),
            BsvHomeConfirmPhase::Ready if self.has_critical_warning => button(BsvHomeConfirmButtonKind::Confirm, BsvHomeConfirmButtonState::Disabled),
            BsvHomeConfirmPhase::Ready => button(BsvHomeConfirmButtonKind::Confirm, BsvHomeConfirmButtonState::Enabled),
        }
    }

    pub fn fee_row(&self) -> BsvHomeConfirmFeeRow {
        match self.phase {
            BsvHomeConfirmPhase::Loading => BsvHomeConfirmFeeRow::Loading,
            BsvHomeConfirmPhase::Failed => BsvHomeConfirmFeeRow::Unavailable,
            BsvHomeConfirmPhase::Ready | BsvHomeConfirmPhase::Confirming => BsvHomeConfirmFeeRow::Ready,
        }
    }

    pub fn action(&self) -> Option<BsvHomeConfirmAction> {
        match self.phase {
            BsvHomeConfirmPhase::Loading | BsvHomeConfirmPhase::Confirming => None,
            BsvHomeConfirmPhase::Failed => match &self.failure {
                Some(failure) if failure.stage == BsvHomeConfirmStage::Execute => Some(BsvHomeConfirmAction::Execute),
                _ => Some(BsvHomeConfirmAction::Load),
            },
            BsvHomeConfirmPhase::Ready if self.amount_failed => Some(BsvHomeConfirmAction::Load),
            BsvHomeConfirmPhase::Ready => Some(BsvHomeConfirmAction::Execute),
        }
    }

    pub fn on_load_started(&self) -> BsvHomeConfirmScreen {
        Self {
            phase: BsvHomeConfirmPhase::Loading,
            amount_failed: false,
            has_critical_warning: self.has_critical_warning,
            failure: None,
        }
    }

    pub fn on_loaded(&self, load: BsvHomeConfirmLoad) -> BsvHomeConfirmScreen {
        Self {
            phase: BsvHomeConfirmPhase::Ready,
            amount_failed: load
                .preload
                .as_ref()
                .is_some_and(|preload| matches!(preload.amount, BsvHomeTransferAmountResult::Error { .. })),
            has_critical_warning: load.simulation.simulation.as_ref().is_some_and(|simulation| simulation.has_critical_warning),
            failure: None,
        }
    }

    pub fn on_load_failed(&self, error: BsvHomeConfirmError) -> BsvHomeConfirmScreen {
        self.failed(BsvHomeConfirmStage::Load, error)
    }

    pub fn on_execute_started(&self) -> BsvHomeConfirmScreen {
        Self {
            phase: BsvHomeConfirmPhase::Confirming,
            failure: None,
            ..self.clone()
        }
    }

    pub fn on_execute_cancelled(&self) -> BsvHomeConfirmScreen {
        Self {
            phase: BsvHomeConfirmPhase::Ready,
            failure: None,
            ..self.clone()
        }
    }

    pub fn on_execute_failed(&self, error: BsvHomeConfirmError) -> BsvHomeConfirmScreen {
        self.failed(BsvHomeConfirmStage::Execute, error)
    }
}

#[cfg(test)]
mod tests {
    use primitives::{Account, Asset, Chain, SimulationResult, SimulationWarning, TransactionInputType};

    use super::super::model::{BsvHomeConfirmMetadata, BsvHomeConfirmPreload, BsvHomeConfirmSimulationState};
    use super::super::testkit::confirm_data;
    use super::*;
    use crate::services::balance::BsvHomeAssetBalance;

    #[test]
    fn test_confirm_button_follows_the_phase_and_the_ready_checks() {
        let screen = |phase, amount_failed, has_critical_warning| BsvHomeConfirmScreen {
            phase,
            amount_failed,
            has_critical_warning,
            failure: None,
        };
        let button = |kind, state| BsvHomeConfirmButton { kind, state };

        assert_eq!(
            screen(BsvHomeConfirmPhase::Loading, false, false).button(),
            button(BsvHomeConfirmButtonKind::Confirm, BsvHomeConfirmButtonState::Loading)
        );
        assert_eq!(
            screen(BsvHomeConfirmPhase::Confirming, false, false).button(),
            button(BsvHomeConfirmButtonKind::Confirm, BsvHomeConfirmButtonState::Loading)
        );
        assert_eq!(
            screen(BsvHomeConfirmPhase::Failed, false, true).button(),
            button(BsvHomeConfirmButtonKind::Retry, BsvHomeConfirmButtonState::Enabled)
        );
        assert_eq!(
            screen(BsvHomeConfirmPhase::Ready, true, true).button(),
            button(BsvHomeConfirmButtonKind::Retry, BsvHomeConfirmButtonState::Enabled)
        );
        assert_eq!(
            screen(BsvHomeConfirmPhase::Ready, false, true).button(),
            button(BsvHomeConfirmButtonKind::Confirm, BsvHomeConfirmButtonState::Disabled)
        );
        assert_eq!(
            screen(BsvHomeConfirmPhase::Ready, false, false).button(),
            button(BsvHomeConfirmButtonKind::Confirm, BsvHomeConfirmButtonState::Enabled)
        );
    }

    #[test]
    fn test_confirm_fee_row_waits_for_the_preload_and_gives_up_on_failure() {
        let screen = |phase| BsvHomeConfirmScreen {
            phase,
            amount_failed: true,
            has_critical_warning: true,
            failure: None,
        };

        assert_eq!(screen(BsvHomeConfirmPhase::Loading).fee_row(), BsvHomeConfirmFeeRow::Loading);
        assert_eq!(screen(BsvHomeConfirmPhase::Ready).fee_row(), BsvHomeConfirmFeeRow::Ready);
        assert_eq!(screen(BsvHomeConfirmPhase::Confirming).fee_row(), BsvHomeConfirmFeeRow::Ready);
        assert_eq!(screen(BsvHomeConfirmPhase::Failed).fee_row(), BsvHomeConfirmFeeRow::Unavailable);
    }

    #[test]
    fn test_confirm_screen_transitions_follow_the_load_and_execute_outcomes() {
        let failed_load = || BsvHomeConfirmError::Load { msg: "down".to_string() };
        let started = BsvHomeConfirmScreen::initial(None).on_load_started();
        assert_eq!(started.phase, BsvHomeConfirmPhase::Loading);
        assert_eq!(started.action(), None);

        let mut load = load_without_preload();
        let ready = started.on_loaded(load.clone());
        assert_eq!(ready.phase, BsvHomeConfirmPhase::Ready);
        assert!(!ready.amount_failed);
        assert_eq!(ready.action(), Some(BsvHomeConfirmAction::Execute));

        load.preload = Some(BsvHomeConfirmPreload {
            confirm_data: confirm_data(Chain::Ethereum, TransactionInputType::Transfer { asset: Asset::mock_eth() }, "sender"),
            amount: BsvHomeTransferAmountResult::Error { error: failed_load() },
        });
        let amount_failed = started.on_loaded(load);
        assert!(amount_failed.amount_failed);
        assert_eq!(amount_failed.action(), Some(BsvHomeConfirmAction::Load));

        let load_failed = started.on_load_failed(failed_load());
        assert_eq!(load_failed.phase, BsvHomeConfirmPhase::Failed);
        assert_eq!(load_failed.failure.as_ref().map(|failure| failure.stage), Some(BsvHomeConfirmStage::Load));
        assert_eq!(load_failed.action(), Some(BsvHomeConfirmAction::Load));
        assert!(load_failed.on_load_started().failure.is_none());

        let confirming = ready.on_execute_started();
        assert_eq!(confirming.phase, BsvHomeConfirmPhase::Confirming);
        assert_eq!(confirming.action(), None);
        assert_eq!(confirming.on_execute_cancelled().phase, BsvHomeConfirmPhase::Ready);

        let execute_failed = confirming.on_execute_failed(BsvHomeConfirmError::Broadcast {
            hashes: vec![],
            msg: "rejected".to_string(),
        });
        assert_eq!(execute_failed.phase, BsvHomeConfirmPhase::Failed);
        assert_eq!(execute_failed.failure.as_ref().map(|failure| failure.stage), Some(BsvHomeConfirmStage::Execute));
        assert_eq!(execute_failed.action(), Some(BsvHomeConfirmAction::Execute));
        assert_eq!(execute_failed.button().kind, BsvHomeConfirmButtonKind::Retry);
    }

    #[test]
    fn test_initial_screen_reads_the_critical_warning_off_the_request_simulation() {
        let critical = SimulationResult {
            warnings: vec![SimulationWarning::validation_error("bad")],
            balance_changes: vec![],
            payload: vec![],
            header: None,
        };
        assert_eq!(BsvHomeConfirmScreen::initial(Some(&critical)).has_critical_warning, critical.has_critical_warning());
        assert!(!BsvHomeConfirmScreen::initial(None).has_critical_warning);
        assert_eq!(BsvHomeConfirmScreen::initial(None).phase, BsvHomeConfirmPhase::Loading);
    }

    fn load_without_preload() -> BsvHomeConfirmLoad {
        let eth = Asset::mock_eth();
        BsvHomeConfirmLoad {
            sender: Account::mock(Chain::Ethereum, "sender"),
            fee_asset: eth.clone(),
            metadata: BsvHomeConfirmMetadata {
                asset_balance: BsvHomeAssetBalance::mock(),
                fee_asset_balance: BsvHomeAssetBalance::mock(),
                prices: vec![],
            },
            fee_assets: vec![],
            simulation: BsvHomeConfirmSimulationState {
                chain: Chain::Ethereum,
                result: None,
                warnings: vec![],
                simulation: None,
                address_names: vec![],
            },
            address_name: None,
            preload: None,
        }
    }
}
