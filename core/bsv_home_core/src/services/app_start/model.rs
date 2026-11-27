use crate::services::failures::StepFailure;
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeAppStartStep {
    UpdateConfig,
    SetupBanners,
    SyncAssets,
    SyncDevice,
    SetupChains,
    SetupWalletAssets,
    SetupWalletBanners,
    SyncWalletConfiguration,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeAppStartFailure {
    pub step: BsvHomeAppStartStep,
    pub message: String,
}

impl StepFailure for BsvHomeAppStartFailure {
    type Step = BsvHomeAppStartStep;

    fn new(step: BsvHomeAppStartStep, message: String) -> Self {
        Self { step, message }
    }
}
