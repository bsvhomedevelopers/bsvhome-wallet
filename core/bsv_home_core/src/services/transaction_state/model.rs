use crate::models::custom_types::BsvHomeBigInt;
use crate::services::failures::StepFailure;
use primitives::{AssetId, Chain, Transaction, TransactionId, TransactionState, Wallet};

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeTransactionStateUpdate {
    pub state: TransactionState,
    pub fee: Option<BsvHomeBigInt>,
    pub block_number: Option<String>,
    pub metadata: Option<String>,
    pub confirmation_eta_seconds: Option<u32>,
}

impl BsvHomeTransactionStateUpdate {
    pub fn new(state: TransactionState) -> Self {
        Self {
            state,
            fee: None,
            block_number: None,
            metadata: None,
            confirmation_eta_seconds: None,
        }
    }

    pub fn has_field_changes(&self) -> bool {
        self.fee.is_some() || self.block_number.is_some() || self.metadata.is_some() || self.confirmation_eta_seconds.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeTransactionStateResult {
    pub transaction_id: TransactionId,
    pub state: TransactionState,
    pub failures: Vec<BsvHomePostProcessingFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomePostProcessingStep {
    Balances,
    Stake,
    Earn,
    Nfts,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomePostProcessingFailure {
    pub step: BsvHomePostProcessingStep,
    pub message: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomePendingTransaction {
    pub wallet: Wallet,
    pub transaction: Transaction,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TransactionPostProcessing {
    pub balance_asset_ids: Vec<AssetId>,
    pub stake_chains: Vec<Chain>,
    pub earn_asset_ids: Vec<AssetId>,
    pub sync_nfts: bool,
}

impl StepFailure for BsvHomePostProcessingFailure {
    type Step = BsvHomePostProcessingStep;

    fn new(step: BsvHomePostProcessingStep, message: String) -> Self {
        Self { step, message }
    }
}
