use std::collections::{BTreeMap, HashSet};

use bitcoin::{
    Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    absolute::LockTime,
    consensus::{deserialize, serialize},
    script::{Builder, PushBytesBuf},
    secp256k1::{Message, PublicKey, Secp256k1, SecretKey},
    transaction::Version,
};
use bsv_home_bitcoin::signer::fork_id_signature_hash;
use primitives::{Address, SignerError};
use serde::{Deserialize, Serialize};

use super::{
    MultisigConfig,
    script::canonical_hex,
    transaction::{SigningKey, decode_signature, validate_unlocking_signatures},
};
use crate::BsvAddress;

pub const MAX_TRANSACTION_BYTES: usize = 1_000_000;
pub const MAX_MONEY: u64 = 2_100_000_000_000_000;
pub const MAX_FEE: u64 = 10_000_000;
pub const MAX_INPUTS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    pub threshold: u32,
    pub public_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Descriptor {
    pub policy: Policy,
    pub locking_script: String,
    pub script_hash: String,
    pub configuration_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposalInput {
    pub transaction_hex: String,
    pub output_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Proposal {
    pub policy: Policy,
    pub inputs: Vec<ProposalInput>,
    pub recipient: String,
    pub amount: u64,
    pub fee: u64,
    pub unsigned_transaction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub recipient: String,
    pub amount: u64,
    pub fee: u64,
    pub change: u64,
    pub input_count: u32,
    pub estimated_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignatureSet {
    pub public_key: String,
    pub signatures: Vec<String>,
}

impl Policy {
    pub fn configuration(&self) -> Result<MultisigConfig, SignerError> {
        let threshold = u8::try_from(self.threshold).map_err(SignerError::from_display)?;
        let mut public_keys = self.public_keys.clone();
        public_keys.sort();
        let configuration = MultisigConfig { threshold, public_keys };
        configuration.validate()?;
        Ok(configuration)
    }

    pub fn canonical(&self) -> Result<Self, SignerError> {
        let configuration = self.configuration()?;
        Ok(Self {
            threshold: u32::from(configuration.threshold),
            public_keys: configuration.public_keys,
        })
    }
}

pub fn member_address(public_key: &str) -> Result<String, SignerError> {
    let bytes = canonical_hex(public_key)?;
    let key = PublicKey::from_slice(&bytes).map_err(SignerError::from_display)?;
    if bytes.len() != 33 || key.serialize().as_slice() != bytes {
        return SignerError::invalid_input_err("BSV member requires a compressed public key");
    }
    Ok(BsvAddress::from_public_key(&bytes).map_err(SignerError::from_display)?.encode())
}

pub fn policy_descriptor(policy: &Policy) -> Result<Descriptor, SignerError> {
    let configuration = policy.configuration()?;
    Ok(Descriptor {
        policy: policy.canonical()?,
        locking_script: hex::encode(configuration.locking_script()?.as_bytes()),
        script_hash: configuration.script_hash()?,
        configuration_hash: configuration.script_id()?,
    })
}

pub(super) fn decode_transaction(raw: &str) -> Result<Transaction, SignerError> {
    if raw.len() > MAX_TRANSACTION_BYTES * 2 {
        return SignerError::invalid_input_err("BSV transaction exceeds supported size");
    }
    let bytes = canonical_hex(raw)?;
    let transaction: Transaction = deserialize(&bytes).map_err(SignerError::from_display)?;
    if serialize(&transaction) != bytes || transaction.input.iter().any(|input| !input.witness.is_empty()) {
        return SignerError::invalid_input_err("noncanonical BSV transaction");
    }
    Ok(transaction)
}

pub(super) fn checked_total(total: u64, value: u64) -> Result<u64, SignerError> {
    total
        .checked_add(value)
        .filter(|value| *value <= MAX_MONEY)
        .ok_or_else(|| SignerError::invalid_input("BSV amount exceeds monetary supply"))
}

fn reconstructed_transaction(policy: &Policy, proposal: &Proposal) -> Result<(Transaction, Vec<u64>, u64), SignerError> {
    if policy.canonical()? != proposal.policy.canonical()?
        || !(1..=MAX_INPUTS).contains(&proposal.inputs.len())
        || proposal.amount == 0
        || proposal.amount > MAX_MONEY
        || proposal.fee > MAX_FEE
    {
        return SignerError::invalid_input_err("invalid BSV multisig proposal policy, amount, fee or input count");
    }
    let configuration = policy.configuration()?;
    let locking_script = configuration.locking_script()?;
    let recipient = BsvAddress::from_str(&proposal.recipient).map_err(SignerError::from_display)?;
    let mut total = 0;
    let mut outpoints = HashSet::new();
    let mut inputs = Vec::new();
    let mut values = Vec::new();
    for source in &proposal.inputs {
        let previous = decode_transaction(&source.transaction_hex)?;
        let output = previous
            .output
            .get(source.output_index as usize)
            .ok_or_else(|| SignerError::invalid_input("BSV source output missing"))?;
        let value = output.value.to_sat();
        let outpoint = OutPoint::new(previous.compute_txid(), source.output_index);
        if output.script_pubkey != locking_script || value == 0 || !outpoints.insert(outpoint) {
            return SignerError::invalid_input_err("invalid BSV multisig source script, value or duplicate input");
        }
        total = checked_total(total, value)?;
        values.push(value);
        inputs.push(TxIn {
            previous_output: outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        });
    }
    let change = total
        .checked_sub(proposal.amount)
        .and_then(|value| value.checked_sub(proposal.fee))
        .ok_or(SignerError::InsufficientFunds)?;
    let mut outputs = vec![TxOut {
        value: Amount::from_sat(proposal.amount),
        script_pubkey: recipient.script_pubkey(),
    }];
    if change > 0 {
        outputs.push(TxOut {
            value: Amount::from_sat(change),
            script_pubkey: locking_script,
        });
    }
    let transaction = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: inputs,
        output: outputs,
    };
    let supplied = decode_transaction(&proposal.unsigned_transaction)?;
    if supplied != transaction {
        return SignerError::invalid_input_err("BSV unsigned transaction differs from independently reconstructed proposal");
    }
    Ok((transaction, values, change))
}

pub fn review_proposal(policy: &Policy, proposal: &Proposal) -> Result<Review, SignerError> {
    let (mut estimated, _, change) = reconstructed_transaction(policy, proposal)?;
    let mut builder = Builder::new().push_int(0);
    for _ in 0..policy.threshold {
        builder = builder.push_slice([0; 72]);
    }
    let unlocking_script = builder.into_script();
    for input in &mut estimated.input {
        input.script_sig = unlocking_script.clone();
    }
    Ok(Review {
        recipient: proposal.recipient.clone(),
        amount: proposal.amount,
        fee: proposal.fee,
        change,
        input_count: proposal.inputs.len() as u32,
        estimated_bytes: serialize(&estimated).len() as u64,
    })
}

pub fn proposal_signature_hashes(policy: &Policy, proposal: &Proposal) -> Result<Vec<String>, SignerError> {
    let (transaction, values, _) = reconstructed_transaction(policy, proposal)?;
    let script = policy.configuration()?.locking_script()?;
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| fork_id_signature_hash(&transaction, index, &script, value).map(hex::encode))
        .collect()
}

pub fn validate_portal_utxo(policy: &Policy, transaction_id: &str, output_index: u32, value: u64, transaction_hex: &str) -> Result<(), SignerError> {
    let transaction = decode_transaction(transaction_hex)?;
    let output = transaction
        .output
        .get(output_index as usize)
        .ok_or_else(|| SignerError::invalid_input("BSV UTXO source output missing"))?;
    if transaction.compute_txid().to_string() != transaction_id
        || value == 0
        || value > MAX_MONEY
        || output.value.to_sat() != value
        || output.script_pubkey != policy.configuration()?.locking_script()?
    {
        return SignerError::invalid_input_err("BSV UTXO source transaction, value or script mismatch");
    }
    Ok(())
}

fn verify_signature_set(transaction: &Transaction, values: &[u64], script: &ScriptBuf, policy: &Policy, signatures: &SignatureSet) -> Result<(), SignerError> {
    if !policy.public_keys.contains(&signatures.public_key) || signatures.signatures.len() != transaction.input.len() {
        return SignerError::invalid_input_err("BSV unknown member or incomplete input signatures");
    }
    let key = PublicKey::from_slice(&canonical_hex(&signatures.public_key)?).map_err(SignerError::from_display)?;
    for (index, signature) in signatures.signatures.iter().enumerate() {
        let signature = decode_signature(&canonical_hex(signature)?)?;
        let digest = fork_id_signature_hash(transaction, index, script, values[index])?;
        Secp256k1::verification_only()
            .verify_ecdsa(&Message::from_digest(digest), &signature, &key)
            .map_err(SignerError::from_display)?;
    }
    Ok(())
}

pub fn validate_signature_set(policy: &Policy, proposal: &Proposal, signatures: &SignatureSet) -> Result<(), SignerError> {
    let (transaction, values, _) = reconstructed_transaction(policy, proposal)?;
    verify_signature_set(&transaction, &values, &policy.configuration()?.locking_script()?, &policy.canonical()?, signatures)
}

pub fn proposal_transaction_id(policy: &Policy, proposal: &Proposal, raw: &str) -> Result<String, SignerError> {
    let (unsigned, values, _) = reconstructed_transaction(policy, proposal)?;
    let signed = decode_transaction(raw)?;
    let mut normalized = signed.clone();
    for input in &mut normalized.input {
        input.script_sig = ScriptBuf::new();
    }
    if normalized != unsigned {
        return SignerError::invalid_input_err("BSV signed transaction differs from confirmed proposal");
    }
    validate_unlocking_signatures(&policy.configuration()?, &signed, &values)?;
    Ok(signed.compute_txid().to_string())
}

pub fn sign_proposal(
    policy: &Policy,
    proposal: &Proposal,
    expected_recipient: &str,
    expected_amount: u64,
    maximum_fee: u64,
    private_key: &[u8],
) -> Result<SignatureSet, SignerError> {
    let review = review_proposal(policy, proposal)?;
    if review.recipient != expected_recipient || review.amount != expected_amount || review.fee > maximum_fee {
        return SignerError::invalid_input_err("BSV multisig proposal differs from authenticated payment intent");
    }
    let key = SigningKey(SecretKey::from_slice(private_key).map_err(SignerError::from_display)?);
    let context = Secp256k1::signing_only();
    let public_key = hex::encode(PublicKey::from_secret_key(&context, &key.0).serialize());
    if !policy.canonical()?.public_keys.contains(&public_key) {
        return SignerError::invalid_input_err("BSV signing wallet is not a policy member");
    }
    let signatures = proposal_signature_hashes(policy, proposal)?
        .into_iter()
        .map(|digest| {
            let digest: [u8; 32] = canonical_hex(&digest)?
                .try_into()
                .map_err(|_| SignerError::invalid_input("invalid BSV signature digest size"))?;
            let mut bytes = context.sign_ecdsa(&Message::from_digest(digest), &key.0).serialize_der().to_vec();
            bytes.push(0x41);
            Ok(hex::encode(bytes))
        })
        .collect::<Result<Vec<_>, SignerError>>()?;
    Ok(SignatureSet { public_key, signatures })
}

pub fn assemble_proposal(policy: &Policy, proposal: &Proposal, signature_sets: &[SignatureSet]) -> Result<String, SignerError> {
    let (mut transaction, values, _) = reconstructed_transaction(policy, proposal)?;
    let canonical = policy.canonical()?;
    let script = policy.configuration()?.locking_script()?;
    if signature_sets.len() < policy.threshold as usize || signature_sets.len() > canonical.public_keys.len() {
        return SignerError::invalid_input_err("BSV signature threshold or member count is invalid");
    }
    let mut members = BTreeMap::new();
    for signatures in signature_sets {
        let rank = canonical
            .public_keys
            .iter()
            .position(|key| *key == signatures.public_key)
            .ok_or_else(|| SignerError::invalid_input("unknown BSV signature member"))?;
        if members.insert(rank, signatures).is_some() {
            return SignerError::invalid_input_err("BSV duplicate signature member");
        }
        verify_signature_set(&transaction, &values, &script, &canonical, signatures)?;
    }
    for (index, input) in transaction.input.iter_mut().enumerate() {
        let mut builder = Builder::new().push_int(0);
        for signatures in members.values().take(policy.threshold as usize) {
            builder = builder.push_slice(PushBytesBuf::try_from(canonical_hex(&signatures.signatures[index])?).map_err(SignerError::from_display)?);
        }
        input.script_sig = builder.into_script();
    }
    let bytes = serialize(&transaction);
    if bytes.len() > MAX_TRANSACTION_BYTES {
        return SignerError::invalid_input_err("BSV signed transaction exceeds supported size");
    }
    Ok(hex::encode(bytes))
}
