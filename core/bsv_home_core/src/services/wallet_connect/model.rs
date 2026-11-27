use crate::services::transfer::BsvHomeTransferData;
use primitives::{Account, Asset, Chain, SimulationResult, Wallet, WalletConnectionSession, WalletConnectionSessionProposal, WalletConnectionVerificationStatus};

use crate::message::sign_type::SignMessage;
use crate::wallet_connect::WalletConnectResponseType;

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeWalletConnectSessionRequest {
    pub topic: String,
    pub request_id: String,
    pub method: String,
    pub params: String,
    pub chain_id: Option<String>,
    pub origin: Option<String>,
    pub validation: WalletConnectionVerificationStatus,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeWalletConnectRpcError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeWalletConnectResponse {
    Response { value: WalletConnectResponseType },
    Null,
    Error { error: BsvHomeWalletConnectRpcError },
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum BsvHomeWalletConnectFailure {
    MaliciousOrigin,
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeWalletConnectOutcome {
    pub response: Option<BsvHomeWalletConnectResponse>,
    pub failure: Option<BsvHomeWalletConnectFailure>,
}

impl BsvHomeWalletConnectOutcome {
    pub fn rejected(failure: Option<BsvHomeWalletConnectFailure>) -> Self {
        Self {
            response: Some(BsvHomeWalletConnectResponse::Error {
                error: crate::services::wallet_connect::rules::user_rejected_error(),
            }),
            failure,
        }
    }

    pub fn ignored() -> Self {
        Self { response: None, failure: None }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeSessionProposal {
    pub proposal: WalletConnectionSessionProposal,
    pub verification_status: WalletConnectionVerificationStatus,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeSessionApproval {
    pub chains: Vec<Chain>,
    pub accounts: Vec<Account>,
    pub methods: Vec<String>,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeWalletConnectMessageRequest {
    pub session_id: String,
    pub chain: Chain,
    pub wallet: Wallet,
    pub account: Account,
    pub session: WalletConnectionSession,
    pub simulation: SimulationResult,
    pub message: SignMessage,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeWalletConnectTransactionRequest {
    pub session_id: String,
    pub chain: Chain,
    pub wallet: Wallet,
    pub account: Account,
    pub session: WalletConnectionSession,
    pub simulation: SimulationResult,
    pub transfer: BsvHomeTransferData,
    pub action: BsvHomeWalletConnectTransactionAction,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomeWalletConnectTransactionAction {
    Sign,
    Send,
}
