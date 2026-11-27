mod funding;
mod models;
mod portal;
mod script;
mod transaction;

#[cfg(test)]
mod portal_tests;
#[cfg(test)]
mod tests;

pub use funding::{Funding, FundingAddress, FundingReview, MAX_FUNDING_UTXOS, funding_spend_metadata, funding_transaction_id, review_funding, sign_funding_with_keys};
pub use models::{FinalizedTransaction, InputSignature, MultisigConfig, PartialSignature, PreviousOutput, TransactionPlan};
pub use portal::{
    Descriptor, Policy, Proposal, ProposalInput, Review, SignatureSet, assemble_proposal, member_address, policy_descriptor, proposal_signature_hashes, proposal_transaction_id,
    review_proposal, sign_proposal, validate_portal_utxo, validate_signature_set,
};
pub use script::{BsvScriptTarget, parse_locking_script};
pub use transaction::{
    finalized_from_raw, merge_signatures, payment_value, plan_outputs, plan_transfer, sign_plan, validate_finalized, validate_payment_intent, validate_plan, verify_partial,
};
