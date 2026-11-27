use bitcoin::{
    PublicKey, Script, Transaction,
    blockdata::script::Builder,
    consensus::encode::serialize,
    secp256k1::{Message, Secp256k1, SecretKey, Signing},
};
use primitives::SignerError;

use crate::{
    hash::double_sha256,
    signer::{
        planner::{PlanInput, SpendPlan},
        transaction::{build_unsigned_transaction, der_signature, signature_push_bytes},
    },
};

const SIGHASH_ALL_FORKID: u32 = 0x41;

pub(super) struct SighashComponents {
    hash_prevouts: [u8; 32],
    hash_sequence: [u8; 32],
    hash_outputs: [u8; 32],
}

impl SighashComponents {
    pub(super) fn new(transaction: &Transaction) -> Self {
        Self {
            hash_prevouts: double_sha256(&transaction.input.iter().flat_map(|input| serialize(&input.previous_output)).collect::<Vec<_>>()),
            hash_sequence: double_sha256(&transaction.input.iter().flat_map(|input| input.sequence.0.to_le_bytes()).collect::<Vec<_>>()),
            hash_outputs: double_sha256(&transaction.output.iter().flat_map(serialize).collect::<Vec<_>>()),
        }
    }
}

pub(crate) fn sign_plan<C: Signing>(plan: &SpendPlan, secret_key: &SecretKey, public_key: &PublicKey, secp: &Secp256k1<C>) -> Result<Transaction, SignerError> {
    sign_plan_with_signatures(plan, |_, sighash| {
        Ok((der_signature(secp, secret_key, Message::from_digest(sighash), SIGHASH_ALL_FORKID as u8), *public_key))
    })
}

pub(super) fn sign_plan_with_signatures(
    plan: &SpendPlan,
    mut signature_for_input: impl FnMut(&PlanInput, [u8; 32]) -> Result<(Vec<u8>, PublicKey), SignerError>,
) -> Result<Transaction, SignerError> {
    let mut transaction = build_unsigned_transaction(plan);
    let components = SighashComponents::new(&transaction);
    for (index, input) in plan.inputs.iter().enumerate() {
        let sighash = signature_hash(&transaction, plan, &components, index)?;
        let (signature, public_key) = signature_for_input(input, sighash)?;
        transaction.input[index].script_sig = Builder::new().push_slice(signature_push_bytes(signature)?).push_key(&public_key).into_script();
    }
    Ok(transaction)
}

pub(super) fn signature_hash(transaction: &Transaction, plan: &SpendPlan, components: &SighashComponents, input_index: usize) -> Result<[u8; 32], SignerError> {
    let input = plan
        .inputs
        .get(input_index)
        .ok_or_else(|| SignerError::signing_error("Bitcoin Cash input index out of bounds"))?;

    signature_hash_with_components(transaction, input_index, input.script_pubkey.as_script(), input.value.to_sat(), components)
}

pub fn fork_id_signature_hash(transaction: &Transaction, input_index: usize, previous_script: &Script, value: u64) -> Result<[u8; 32], SignerError> {
    let components = SighashComponents::new(transaction);
    signature_hash_with_components(transaction, input_index, previous_script, value, &components)
}

fn signature_hash_with_components(
    transaction: &Transaction,
    input_index: usize,
    previous_script: &Script,
    value: u64,
    components: &SighashComponents,
) -> Result<[u8; 32], SignerError> {
    let input = transaction
        .input
        .get(input_index)
        .ok_or_else(|| SignerError::invalid_input("fork-id input index out of bounds"))?;
    let mut preimage = Vec::new();
    preimage.extend_from_slice(&transaction.version.0.to_le_bytes());
    preimage.extend_from_slice(&components.hash_prevouts);
    preimage.extend_from_slice(&components.hash_sequence);
    preimage.extend_from_slice(&serialize(&input.previous_output));
    preimage.extend_from_slice(&serialize(previous_script));
    preimage.extend_from_slice(&value.to_le_bytes());
    preimage.extend_from_slice(&input.sequence.0.to_le_bytes());
    preimage.extend_from_slice(&components.hash_outputs);
    preimage.extend_from_slice(&transaction.lock_time.to_consensus_u32().to_le_bytes());
    preimage.extend_from_slice(&SIGHASH_ALL_FORKID.to_le_bytes());

    Ok(double_sha256(&preimage))
}
