use crate::services::bsv_multisig::BsvHomeBsvMultisigPaymentRequest;
use serde::{Deserialize, Serialize};

use super::model::deserialize_u64;

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomePaymailBindingAvailability {
    Available,
    DomainRemoved,
    AddressMismatch,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePaymailBinding {
    pub handle: String,
    pub address: String,
    pub identity_public_key: String,
    pub active: bool,
    pub version: u64,
    pub availability: BsvHomePaymailBindingAvailability,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePaymailSnapshot {
    pub wallet_id: String,
    pub address: String,
    pub public_origin: String,
    pub domains: Vec<String>,
    pub registration_domains: Vec<String>,
    pub bindings: Vec<BsvHomePaymailBinding>,
    pub active_handle: Option<String>,
    pub verification: BsvHomePaymailVerification,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomePaymailVerification {
    Clear,
    Pending,
    Reconciled,
    Changed,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, uniffi::Record)]
pub struct BsvHomePaymailBindingIntent {
    pub wallet_id: String,
    pub address: String,
    pub public_origin: String,
    pub handle: String,
    pub active: bool,
    pub previous_version: u64,
}

#[derive(Deserialize, Serialize)]
pub(super) struct PendingBinding {
    pub intent: BsvHomePaymailBindingIntent,
    pub identity_public_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum BsvHomePaymailMutationStatus {
    Confirmed,
    Reconciled,
    PendingVerification,
    Changed,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePaymailMutation {
    pub status: BsvHomePaymailMutationStatus,
    pub snapshot: Option<BsvHomePaymailSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomePaymailOutput {
    P2pkh {
        address: String,
        output_script: String,
    },
    P2ms {
        request: BsvHomeBsvMultisigPaymentRequest,
        output_script: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomePaymailResolvedDestination {
    pub handle: String,
    pub output: BsvHomePaymailOutput,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomePaymailDestination {
    pub handle: String,
    pub address: String,
    pub reference: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Configuration {
    pub domains: Vec<String>,
    pub public_base_url: String,
    pub network: String,
    pub address_type: String,
    pub identity_key_rotation: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct Binding {
    pub handle: String,
    pub address: String,
    pub identity_public_key: String,
    pub active: bool,
    #[serde(deserialize_with = "deserialize_u64")]
    pub version: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BindingChallenge {
    pub challenge_id: String,
    pub message: String,
    pub expires_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChallengeRequest<'a> {
    pub alias: &'a str,
    pub domain: &'a str,
    pub address: &'a str,
    pub identity_public_key: &'a str,
    pub active: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BindingRequest<'a> {
    pub challenge_id: &'a str,
    pub address_signature: &'a str,
    pub identity_signature: &'a str,
}
