use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
};

use bitcoin::{
    Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
    absolute::LockTime,
    consensus::{deserialize, serialize},
    script::{Builder, Instruction, PushBytesBuf},
    secp256k1::{Message, PublicKey, Secp256k1, SecretKey, ecdsa::Signature},
    transaction::Version,
};
use bsv_home_bitcoin::signer::fork_id_signature_hash;
use primitives::SignerError;

use super::{
    FinalizedTransaction, InputSignature, MultisigConfig, PartialSignature, PreviousOutput, TransactionPlan,
    script::{canonical_hex, destination_script},
};

const MAX_INPUTS: usize = 100;
const MAX_TRANSACTION_BYTES: usize = 2_000_000;
const MAX_MONEY: u64 = 2_100_000_000_000_000;
const MAX_SIGNATURE_BYTES: usize = 72;
const SIGHASH_ALL_FORKID: u8 = 0x41;

pub(super) struct SigningKey(pub(super) SecretKey);

impl Drop for SigningKey {
    fn drop(&mut self) {
        self.0.non_secure_erase();
    }
}

impl PreviousOutput {
    pub fn validate(&self, locking_script: &ScriptBuf) -> Result<OutPoint, SignerError> {
        let transaction = decode_transaction(&self.raw_transaction)?;
        let transaction_id = Txid::from_str(&self.transaction_id).map_err(SignerError::from_display)?;
        if transaction.compute_txid() != transaction_id || transaction_id.to_string() != self.transaction_id {
            return SignerError::invalid_input_err("BSV previous transaction id mismatch");
        }
        let previous = transaction
            .output
            .get(self.vout as usize)
            .ok_or_else(|| SignerError::invalid_input("BSV previous output missing"))?;
        if self.value == 0 || self.value > MAX_MONEY || previous.value.to_sat() != self.value || previous.script_pubkey != *locking_script {
            return SignerError::invalid_input_err("BSV previous output value or script mismatch");
        }
        if transaction.input.iter().any(|input| !input.witness.is_empty()) {
            return SignerError::invalid_input_err("BSV previous transaction cannot contain witness");
        }
        Ok(OutPoint::new(transaction_id, self.vout))
    }
}

pub fn plan_transfer(
    configuration: &MultisigConfig,
    sources: &[PreviousOutput],
    recipient_script: &str,
    value: Option<u64>,
    fee_rate: u64,
) -> Result<TransactionPlan, SignerError> {
    let locking_script = configuration.locking_script()?;
    let recipient_script = destination_script(recipient_script)?;
    validate_source_count(sources.len())?;
    if fee_rate == 0 || fee_rate > MAX_MONEY || value.is_some_and(|value| value == 0 || value > MAX_MONEY) {
        return SignerError::invalid_input_err("invalid BSV multisig amount or fee rate");
    }
    let mut outpoints = HashSet::new();
    for source in sources {
        if !outpoints.insert(source.validate(&locking_script)?) {
            return SignerError::invalid_input_err("duplicate BSV multisig outpoint");
        }
    }
    let mut candidates = sources.to_vec();
    candidates.sort_by(|left, right| {
        left.value
            .cmp(&right.value)
            .then_with(|| left.transaction_id.cmp(&right.transaction_id))
            .then_with(|| left.vout.cmp(&right.vout))
    });
    let mut selected = Vec::new();
    let mut total = 0u64;
    for source in candidates {
        total = total
            .checked_add(source.value)
            .filter(|value| *value <= MAX_MONEY)
            .ok_or_else(|| SignerError::invalid_input("BSV amount exceeds monetary supply"))?;
        selected.push(source);
        if value.is_none() && selected.len() != sources.len() {
            continue;
        }
        let mut transaction = unsigned_transaction(
            &selected,
            vec![TxOut {
                value: Amount::from_sat(0),
                script_pubkey: recipient_script.clone(),
            }],
        )?;
        let payment = match value {
            None => {
                let fee = estimated_fee(&transaction, configuration.threshold, fee_rate)?;
                total.checked_sub(fee).filter(|value| *value > 0).ok_or(SignerError::InsufficientFunds)?
            }
            Some(payment) => {
                transaction.output.push(TxOut {
                    value: Amount::from_sat(0),
                    script_pubkey: locking_script.clone(),
                });
                let fee = estimated_fee(&transaction, configuration.threshold, fee_rate)?;
                match total.checked_sub(payment).and_then(|remainder| remainder.checked_sub(fee)) {
                    Some(change) if change > 0 => transaction.output[1].value = Amount::from_sat(change),
                    _ => {
                        transaction.output.pop();
                        let fee = estimated_fee(&transaction, configuration.threshold, fee_rate)?;
                        if total.checked_sub(payment).is_none_or(|remainder| remainder < fee) {
                            continue;
                        }
                    }
                }
                payment
            }
        };
        transaction.output[0].value = Amount::from_sat(payment);
        let plan = TransactionPlan {
            unsigned_transaction: hex::encode(serialize(&transaction)),
            previous_outputs: selected,
            fee_rate,
            fee: total - transaction.output.iter().map(|output| output.value.to_sat()).sum::<u64>(),
        };
        validate_plan(configuration, &plan)?;
        return Ok(plan);
    }
    Err(SignerError::InsufficientFunds)
}

pub fn validate_plan(configuration: &MultisigConfig, plan: &TransactionPlan) -> Result<(), SignerError> {
    validated_transaction(configuration, plan).map(|_| ())
}

pub fn payment_value(plan: &TransactionPlan) -> Result<u64, SignerError> {
    decode_transaction(&plan.unsigned_transaction)?
        .output
        .first()
        .map(|output| output.value.to_sat())
        .ok_or_else(|| SignerError::invalid_input("BSV multisig payment output missing"))
}

pub fn validate_payment_intent(configuration: &MultisigConfig, plan: &TransactionPlan, recipient_script: &str, value: u64) -> Result<(), SignerError> {
    let transaction = validated_transaction(configuration, plan)?;
    let recipient = destination_script(recipient_script)?;
    if transaction.output[0].script_pubkey != recipient || transaction.output[0].value.to_sat() != value {
        return SignerError::invalid_input_err("BSV multisig payment differs from authenticated intent");
    }
    Ok(())
}

pub fn plan_outputs(plan: &TransactionPlan) -> Result<Vec<(String, u64)>, SignerError> {
    Ok(decode_transaction(&plan.unsigned_transaction)?
        .output
        .iter()
        .map(|output| (hex::encode(output.script_pubkey.as_bytes()), output.value.to_sat()))
        .collect())
}

pub fn finalized_from_raw(configuration: &MultisigConfig, plan: &TransactionPlan, raw: &str) -> Result<FinalizedTransaction, SignerError> {
    let transaction = decode_transaction(raw)?;
    let finalized = FinalizedTransaction {
        raw_transaction: raw.to_string(),
        transaction_id: transaction.compute_txid().to_string(),
        fee: plan.fee,
        serialized_size: serialize(&transaction).len() as u64,
    };
    validate_finalized(configuration, plan, &finalized)?;
    Ok(finalized)
}

fn validated_transaction(configuration: &MultisigConfig, plan: &TransactionPlan) -> Result<Transaction, SignerError> {
    let locking_script = configuration.locking_script()?;
    validate_source_count(plan.previous_outputs.len())?;
    let transaction = decode_transaction(&plan.unsigned_transaction)?;
    if transaction.version != Version::ONE
        || transaction.lock_time != LockTime::ZERO
        || transaction.input.len() != plan.previous_outputs.len()
        || !(1..=2).contains(&transaction.output.len())
    {
        return SignerError::invalid_input_err("invalid BSV multisig transaction shape");
    }
    let mut outpoints = HashSet::new();
    let mut input_total = 0u64;
    for (input, previous) in transaction.input.iter().zip(&plan.previous_outputs) {
        let outpoint = previous.validate(&locking_script)?;
        if !outpoints.insert(outpoint) || input.previous_output != outpoint || input.sequence != Sequence::MAX || !input.script_sig.is_empty() || !input.witness.is_empty() {
            return SignerError::invalid_input_err("BSV multisig input differs from frozen previous output");
        }
        input_total = input_total
            .checked_add(previous.value)
            .filter(|value| *value <= MAX_MONEY)
            .ok_or_else(|| SignerError::invalid_input("BSV input amount exceeds monetary supply"))?;
    }
    let mut output_total = 0u64;
    for (index, output) in transaction.output.iter().enumerate() {
        if output.value == Amount::ZERO || (index == 1 && output.script_pubkey != locking_script) {
            return SignerError::invalid_input_err("invalid BSV multisig payment or change");
        }
        destination_script(&hex::encode(output.script_pubkey.as_bytes()))?;
        output_total = output_total
            .checked_add(output.value.to_sat())
            .filter(|value| *value <= MAX_MONEY)
            .ok_or_else(|| SignerError::invalid_input("BSV output amount exceeds monetary supply"))?;
    }
    let fee = input_total.checked_sub(output_total).ok_or(SignerError::InsufficientFunds)?;
    if fee != plan.fee || plan.fee_rate == 0 || plan.fee_rate > MAX_MONEY || fee < estimated_fee(&transaction, configuration.threshold, plan.fee_rate)? {
        return SignerError::invalid_input_err("BSV multisig fee differs from frozen plan or is insufficient");
    }
    let mut upper = transaction.clone();
    if upper.output.len() == 1 {
        upper.output.push(TxOut {
            value: Amount::from_sat(1),
            script_pubkey: locking_script,
        });
    }
    if fee > estimated_fee(&upper, configuration.threshold, plan.fee_rate)? {
        return SignerError::invalid_input_err("BSV multisig fee exceeds the permitted serialized fee bound");
    }
    Ok(transaction)
}

pub fn sign_plan(configuration: &MultisigConfig, plan: &TransactionPlan, private_key: &[u8]) -> Result<PartialSignature, SignerError> {
    let transaction = validated_transaction(configuration, plan)?;
    let locking_script = configuration.locking_script()?;
    let key = SigningKey(SecretKey::from_slice(private_key).map_err(SignerError::from_display)?);
    let secp = Secp256k1::signing_only();
    let public_key = PublicKey::from_secret_key(&secp, &key.0);
    if !configuration.sorted_public_keys()?.contains(&public_key) {
        return SignerError::invalid_input_err("BSV signing wallet is not a configuration member");
    }
    let signatures = plan
        .previous_outputs
        .iter()
        .enumerate()
        .map(|(index, previous)| {
            let digest = fork_id_signature_hash(&transaction, index, locking_script.as_script(), previous.value)?;
            let signature = secp.sign_ecdsa(&Message::from_digest(digest), &key.0);
            let mut bytes = signature.serialize_der().to_vec();
            bytes.push(SIGHASH_ALL_FORKID);
            Ok(InputSignature {
                input_index: index as u32,
                signature: hex::encode(bytes),
            })
        })
        .collect::<Result<Vec<_>, SignerError>>()?;
    Ok(PartialSignature {
        public_key: hex::encode(public_key.serialize()),
        signatures,
    })
}

pub fn verify_partial(configuration: &MultisigConfig, plan: &TransactionPlan, partial: &PartialSignature) -> Result<(), SignerError> {
    let transaction = validated_transaction(configuration, plan)?;
    verify_partial_transaction(configuration, plan, &transaction, partial)
}

fn verify_partial_transaction(configuration: &MultisigConfig, plan: &TransactionPlan, transaction: &Transaction, partial: &PartialSignature) -> Result<(), SignerError> {
    let public_key = PublicKey::from_slice(&canonical_hex(&partial.public_key)?).map_err(SignerError::from_display)?;
    if partial.public_key != hex::encode(public_key.serialize())
        || !configuration.sorted_public_keys()?.contains(&public_key)
        || partial.signatures.is_empty()
        || partial.signatures.len() > transaction.input.len()
    {
        return SignerError::invalid_input_err("invalid BSV multisig partial signature member or count");
    }
    let locking_script = configuration.locking_script()?;
    let secp = Secp256k1::verification_only();
    let mut indexes = HashSet::new();
    for input_signature in &partial.signatures {
        let index = input_signature.input_index as usize;
        if !indexes.insert(index) {
            return SignerError::invalid_input_err("duplicate BSV multisig partial signature input");
        }
        let previous = plan
            .previous_outputs
            .get(index)
            .ok_or_else(|| SignerError::invalid_input("BSV signature input out of bounds"))?;
        let signature = decode_signature(&canonical_hex(&input_signature.signature)?)?;
        let digest = fork_id_signature_hash(transaction, index, locking_script.as_script(), previous.value)?;
        secp.verify_ecdsa(&Message::from_digest(digest), &signature, &public_key)
            .map_err(SignerError::from_display)?;
    }
    Ok(())
}

pub fn merge_signatures(configuration: &MultisigConfig, plan: &TransactionPlan, partials: &[PartialSignature]) -> Result<FinalizedTransaction, SignerError> {
    let mut transaction = validated_transaction(configuration, plan)?;
    if partials.len() > configuration.public_keys.len() {
        return SignerError::invalid_input_err("too many BSV multisig signature members");
    }
    let keys = configuration.sorted_public_keys()?;
    let mut signatures = vec![BTreeMap::new(); transaction.input.len()];
    let mut members = HashSet::new();
    for partial in partials {
        verify_partial_transaction(configuration, plan, &transaction, partial)?;
        if !members.insert(&partial.public_key) {
            return SignerError::invalid_input_err("duplicate BSV multisig signer");
        }
        let public_key = PublicKey::from_slice(&canonical_hex(&partial.public_key)?).map_err(SignerError::from_display)?;
        let rank = keys
            .iter()
            .position(|key| *key == public_key)
            .ok_or_else(|| SignerError::invalid_input("unknown BSV multisig signer"))?;
        for signature in &partial.signatures {
            signatures[signature.input_index as usize].insert(rank, canonical_hex(&signature.signature)?);
        }
    }
    for (input, signatures) in transaction.input.iter_mut().zip(signatures) {
        if signatures.len() < configuration.threshold as usize {
            return SignerError::invalid_input_err("each BSV multisig input requires its signature threshold");
        }
        let mut builder = Builder::new().push_int(0);
        for signature in signatures.values().take(configuration.threshold as usize) {
            builder = builder.push_slice(PushBytesBuf::try_from(signature.clone()).map_err(SignerError::from_display)?);
        }
        input.script_sig = builder.into_script();
    }
    let bytes = serialize(&transaction);
    let finalized = FinalizedTransaction {
        raw_transaction: hex::encode(&bytes),
        transaction_id: transaction.compute_txid().to_string(),
        fee: plan.fee,
        serialized_size: bytes.len() as u64,
    };
    validate_finalized(configuration, plan, &finalized)?;
    Ok(finalized)
}

pub fn validate_finalized(configuration: &MultisigConfig, plan: &TransactionPlan, finalized: &FinalizedTransaction) -> Result<(), SignerError> {
    let unsigned = validated_transaction(configuration, plan)?;
    let signed = decode_transaction(&finalized.raw_transaction)?;
    let mut normalized = signed.clone();
    for input in &mut normalized.input {
        input.script_sig = ScriptBuf::new();
    }
    if normalized != unsigned
        || signed.compute_txid().to_string() != finalized.transaction_id
        || serialize(&signed).len() as u64 != finalized.serialized_size
        || finalized.fee != plan.fee
    {
        return SignerError::invalid_input_err("BSV finalized transaction differs from frozen plan");
    }
    validate_unlocking_signatures(configuration, &signed, &plan.previous_outputs.iter().map(|output| output.value).collect::<Vec<_>>())?;
    if finalized.fee < fee_for_bytes(finalized.serialized_size, plan.fee_rate)? {
        return SignerError::invalid_input_err("BSV finalized fee is below the requested rate");
    }
    Ok(())
}

pub(super) fn validate_unlocking_signatures(configuration: &MultisigConfig, signed: &Transaction, values: &[u64]) -> Result<(), SignerError> {
    if values.len() != signed.input.len() {
        return SignerError::invalid_input_err("BSV signed transaction input values are incomplete");
    }
    let keys = configuration.sorted_public_keys()?;
    let locking_script = configuration.locking_script()?;
    let secp = Secp256k1::verification_only();
    for (index, input) in signed.input.iter().enumerate() {
        let instructions = input.script_sig.instructions_minimal().collect::<Result<Vec<_>, _>>().map_err(SignerError::from_display)?;
        if instructions.len() != configuration.threshold as usize + 1
            || instructions.first() != Some(&Instruction::PushBytes((&[][..]).try_into().map_err(SignerError::from_display)?))
        {
            return SignerError::invalid_input_err("BSV multisig unlocking script requires empty dummy and exactly M signatures");
        }
        let digest = fork_id_signature_hash(signed, index, locking_script.as_script(), values[index])?;
        let message = Message::from_digest(digest);
        let mut key_index = 0;
        for instruction in &instructions[1..] {
            let Instruction::PushBytes(bytes) = instruction else {
                return SignerError::invalid_input_err("BSV multisig unlocking script must contain minimal signature pushes");
            };
            let signature = decode_signature(bytes.as_bytes())?;
            let mut verified = false;
            while let Some(key) = keys.get(key_index) {
                key_index += 1;
                if secp.verify_ecdsa(&message, &signature, key).is_ok() {
                    verified = true;
                    break;
                }
            }
            if !verified {
                return SignerError::invalid_input_err("invalid BSV multisig signature or public key order");
            }
        }
    }
    Ok(())
}

pub(super) fn decode_signature(bytes: &[u8]) -> Result<Signature, SignerError> {
    let (sighash, der) = bytes.split_last().ok_or_else(|| SignerError::invalid_input("missing BSV signature"))?;
    if *sighash != SIGHASH_ALL_FORKID || bytes.len() > MAX_SIGNATURE_BYTES {
        return SignerError::invalid_input_err("BSV multisig only permits SIGHASH_ALL FORKID");
    }
    let signature = Signature::from_der(der).map_err(SignerError::from_display)?;
    let mut normalized = signature;
    normalized.normalize_s();
    if normalized != signature || signature.serialize_der().as_ref() != der {
        return SignerError::invalid_input_err("BSV multisig requires canonical DER and low S");
    }
    Ok(signature)
}

fn decode_transaction(raw: &str) -> Result<Transaction, SignerError> {
    if raw.len() > MAX_TRANSACTION_BYTES * 2 {
        return SignerError::invalid_input_err("BSV transaction exceeds supported size");
    }
    let bytes = canonical_hex(raw)?;
    let transaction: Transaction = deserialize(&bytes).map_err(SignerError::from_display)?;
    if serialize(&transaction) != bytes {
        return SignerError::invalid_input_err("noncanonical BSV transaction serialization");
    }
    Ok(transaction)
}

fn unsigned_transaction(previous: &[PreviousOutput], output: Vec<TxOut>) -> Result<Transaction, SignerError> {
    let input = previous
        .iter()
        .map(|source| {
            Ok(TxIn {
                previous_output: OutPoint::new(Txid::from_str(&source.transaction_id).map_err(SignerError::from_display)?, source.vout),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            })
        })
        .collect::<Result<Vec<_>, SignerError>>()?;
    Ok(Transaction {
        version: Version::ONE,
        lock_time: LockTime::ZERO,
        input,
        output,
    })
}

fn estimated_fee(transaction: &Transaction, threshold: u8, fee_rate: u64) -> Result<u64, SignerError> {
    let mut estimated = transaction.clone();
    let maximum_signature = [0; MAX_SIGNATURE_BYTES];
    let mut builder = Builder::new().push_int(0);
    for _ in 0..threshold {
        builder = builder.push_slice(maximum_signature);
    }
    let unlocking_script = builder.into_script();
    for input in &mut estimated.input {
        input.script_sig = unlocking_script.clone();
    }
    fee_for_bytes(serialize(&estimated).len() as u64, fee_rate)
}

fn fee_for_bytes(size: u64, rate: u64) -> Result<u64, SignerError> {
    u64::try_from((size as u128 * rate as u128).div_ceil(1000)).map_err(|_| SignerError::invalid_input("BSV multisig fee overflow"))
}

fn validate_source_count(count: usize) -> Result<(), SignerError> {
    if !(1..=MAX_INPUTS).contains(&count) {
        return SignerError::invalid_input_err("BSV multisig requires between 1 and 100 inputs");
    }
    Ok(())
}
