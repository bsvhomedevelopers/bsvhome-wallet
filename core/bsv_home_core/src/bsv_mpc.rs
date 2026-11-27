use bsv_home_bsv::mpc;

use crate::BsvHomeCoreError;

#[uniffi::export]
pub fn bsv_mpc_review(proposal_json: String, public_key: String, expected_recipient: String, expected_amount: u64, maximum_fee: u64) -> Result<String, BsvHomeCoreError> {
    Ok(mpc::review_json(&proposal_json, &public_key, &expected_recipient, expected_amount, maximum_fee)?)
}

#[uniffi::export]
pub fn bsv_mpc_verify_signed(
    proposal_json: String,
    public_key: String,
    expected_recipient: String,
    expected_amount: u64,
    maximum_fee: u64,
    signed_raw: String,
) -> Result<String, BsvHomeCoreError> {
    Ok(mpc::verify_signed_json(
        &proposal_json,
        &public_key,
        &expected_recipient,
        expected_amount,
        maximum_fee,
        &signed_raw,
    )?)
}
