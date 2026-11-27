use std::collections::{BTreeMap, BTreeSet};

use bsv_home_bsv::multisig::{Policy, Proposal as TransactionProposal, SignatureSet};
use serde::{Deserialize, Serialize};

use super::funding_model::BsvHomeBsvMultisigFundingTarget;
pub(crate) use super::funding_model::FundingCheckpoint;
use super::payment::BsvHomeBsvMultisigPaymentRequest;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvMultisigWalletStatus {
    WaitingForMembers,
    WaitingForConfirmations,
    Active,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvMultisigConfigurationStatus {
    Unavailable,
    NeedsConfirmation,
    NeedsLocalReview,
    WaitingForMembers,
    Confirmed,
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvMultisigMemberStatus {
    Invited,
    AwaitingApproval,
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvMultisigMember {
    pub slot: u32,
    pub name: String,
    pub public_key: Option<String>,
    pub address: Option<String>,
    pub status: BsvHomeBsvMultisigMemberStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvMultisigWallet {
    pub id: String,
    pub name: String,
    pub threshold: u32,
    pub member_count: u32,
    pub version: u64,
    pub status: BsvHomeBsvMultisigWalletStatus,
    pub configuration_status: BsvHomeBsvMultisigConfigurationStatus,
    pub members: Vec<BsvHomeBsvMultisigMember>,
    pub configuration_id: Option<String>,
    pub locking_script: Option<String>,
    pub local_signer_wallet_id: Option<String>,
    pub local_signer_public_key: Option<String>,
    pub balance_satoshis: Option<String>,
    pub balance_updated_at: Option<u64>,
    pub balance_is_stale: bool,
    pub pending_count: u32,
    pub can_invite: bool,
    pub can_transact: bool,
    pub personal_configuration_pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvMultisigProposalState {
    CollectingSignatures,
    ReadyToBroadcast,
    PendingConfirmation,
    Confirmed,
    Stopped,
    Expired,
    Unknown,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvMultisigProposalReview {
    pub review_id: String,
    pub proposal: BsvHomeBsvMultisigProposal,
    pub fields: Vec<BsvHomeBsvMultisigField>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvMultisigProposal {
    pub id: String,
    pub wallet_id: String,
    pub version: u64,
    pub destination: String,
    pub value_satoshis: String,
    pub fee_satoshis: String,
    pub memo: String,
    pub state: BsvHomeBsvMultisigProposalState,
    pub remote_status: String,
    pub input_count: u32,
    pub signed_members: Vec<String>,
    pub required_signatures: u32,
    pub transaction_id: Option<String>,
    pub created_at: u64,
    pub expires_at: u64,
    pub confirmations: Option<u64>,
    pub can_sign: bool,
    pub can_broadcast: bool,
    pub can_withdraw: bool,
    pub personal_pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct BsvHomeBsvMultisigSignerBinding {
    pub wallet_id: String,
    pub public_key: String,
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvMultisigCreationStatus {
    Ready,
    Submitted,
    Resolved { wallet_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvMultisigCreationDraft {
    pub id: String,
    pub name: String,
    pub threshold: u32,
    pub member_names: Vec<String>,
    pub signing_identity: BsvHomeBsvMultisigSignerBinding,
    pub status: BsvHomeBsvMultisigCreationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CreationPayload {
    pub name: String,
    pub threshold: u32,
    pub member_names: Vec<String>,
    pub signing_identity: BsvHomeBsvMultisigSignerBinding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct StoredCreation {
    pub payload: CreationPayload,
    pub baseline_wallet_ids: BTreeSet<String>,
    pub wallet_id: Option<String>,
}

pub(super) struct ReadyCreation {
    pub id: String,
    pub payload: CreationPayload,
    pub baseline_wallet_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvMultisigStoredState {
    pub revision: u64,
    pub json: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvMultisigInvitation {
    pub wallet_id: String,
    pub token: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvMultisigField {
    pub title: String,
    pub value: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvMultisigInvitePreview {
    pub wallet: BsvHomeBsvMultisigWallet,
    pub token: String,
    pub expires_at: u64,
    pub signing_identity: BsvHomeBsvMultisigSignerBinding,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvMultisigFundingPayer {
    pub wallet_id: String,
    pub name: String,
    pub address: String,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum BsvHomeBsvMultisigFundingPreparationTarget {
    Member {
        wallet_id: String,
        name: String,
        members: Vec<BsvHomeBsvMultisigMember>,
        threshold: u32,
    },
    Public {
        request: BsvHomeBsvMultisigPaymentRequest,
    },
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvMultisigFundingPreparation {
    pub id: String,
    pub target: BsvHomeBsvMultisigFundingPreparationTarget,
    pub payer_wallet_id: String,
    pub payer_wallet_name: String,
    pub sender_address: String,
    pub amount_satoshis: String,
    pub fee_satoshis: String,
    pub total_satoshis: String,
    pub change_satoshis: String,
    pub input_count: u32,
    pub configuration_hash: String,
    pub remaining_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum BsvHomeBsvMultisigFundingStatus {
    Pending,
    Broadcast,
    SyncFailed,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvMultisigFundingResult {
    pub target: BsvHomeBsvMultisigFundingTarget,
    pub operation_id: String,
    pub payer_wallet_id: String,
    pub transaction_id: String,
    pub payer_wallet_name: String,
    pub amount_satoshis: String,
    pub fee_satoshis: String,
    pub total_satoshis: String,
    pub status: BsvHomeBsvMultisigFundingStatus,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeBsvMultisigUnsubmittedFunding {
    pub operation_id: String,
    pub target: BsvHomeBsvMultisigFundingTarget,
    pub payer_wallet_id: String,
    pub payer_wallet_name: String,
    pub sender_address: String,
    pub amount_satoshis: String,
    pub fee_satoshis: String,
    pub total_satoshis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteMember {
    pub user_id: i64,
    pub address: String,
    pub public_key: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteWallet {
    pub id: String,
    pub name: String,
    pub status: String,
    pub threshold: u32,
    pub member_count: u32,
    pub version: i64,
    pub members: Vec<RemoteMember>,
    pub locking_script: Option<String>,
    pub configuration_hash: Option<String>,
    pub script_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteProposal {
    pub id: String,
    pub wallet_id: String,
    pub status: String,
    pub creator_id: Option<i64>,
    pub creator_address: Option<String>,
    pub note: Option<String>,
    pub version: i64,
    pub created_at: i64,
    pub expires_at: i64,
    pub txid: Option<String>,
    pub confirmations: Option<i64>,
    pub proposal: TransactionProposal,
    pub signatures: Vec<SignatureSet>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProposalPage {
    pub items: Vec<RemoteProposal>,
    pub total: i64,
    pub offset: u32,
    pub limit: u32,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteInvite {
    pub token: String,
    pub expires_at: i64,
    pub wallet_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteInvitePreview {
    pub wallet: RemoteWallet,
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteUtxo {
    pub txid: String,
    pub output_index: i64,
    pub satoshis: i64,
    pub transaction_hex: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct State {
    pub trusted: BTreeMap<String, Policy>,
    pub member_names: BTreeMap<String, Vec<String>>,
    pub intentions: BTreeMap<String, PrepareIntent>,
    #[serde(deserialize_with = "super::funding_model::deserialize_funding_checkpoints")]
    pub funding: BTreeMap<String, FundingCheckpoint>,
    #[serde(default)]
    pub creations: BTreeMap<String, StoredCreation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PrepareIntent {
    pub key: String,
    pub recipient: String,
    pub amount: u64,
    pub note: String,
}
