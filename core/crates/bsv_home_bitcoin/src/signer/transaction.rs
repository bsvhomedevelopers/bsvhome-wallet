use bitcoin::{
    PublicKey, ScriptBuf, Sequence, Transaction, TxIn as TransactionInput, TxOut as TransactionOutput, Witness,
    absolute::LockTime,
    blockdata::{script::Builder, transaction::Version},
    consensus::{deserialize, encode::serialize},
    script::{Instruction, PushBytesBuf},
    secp256k1::{Message, PublicKey as Secp256k1PublicKey, Secp256k1, SecretKey, Signing, ecdsa::Signature},
    sighash::{EcdsaSighashType, SighashCache},
};
use primitives::{BitcoinChain, Chain, SignerError};
use zeroize::Zeroizing;

use crate::signer::{
    address::{UnlockingScript, public_key_hash, script_for_public_key_hash},
    bitcoin_cash::{SighashComponents, sign_plan as sign_bitcoin_cash, sign_plan_with_signatures, signature_hash},
    planner::{PlanInput, SpendPlan},
    zcash::sign_transparent,
};

/// Wipes the secp256k1 `SecretKey` on drop (it is not `ZeroizeOnDrop`), covering early-return paths.
struct ZeroizedSecretKey(SecretKey);

impl Drop for ZeroizedSecretKey {
    fn drop(&mut self) {
        self.0.non_secure_erase();
    }
}

pub(crate) fn sign_plan(chain: BitcoinChain, plan: &SpendPlan, private_key: &[u8], zcash_branch_id: Option<u32>) -> Result<String, SignerError> {
    let secret_key = ZeroizedSecretKey(SecretKey::from_slice(private_key).map_err(|_| SignerError::invalid_input(format!("invalid {} private key", chain.get_chain())))?);
    let secp = Secp256k1::signing_only();
    let public_key = PublicKey::new(Secp256k1PublicKey::from_secret_key(&secp, &secret_key.0));
    validate_chain_input_types(chain, plan)?;
    validate_public_key(chain.get_chain(), plan, &public_key)?;
    validate_plan_amounts(chain.get_chain(), plan)?;

    let transaction = match chain {
        BitcoinChain::BitcoinCash => sign_bitcoin_cash(plan, &secret_key.0, &public_key, &secp)?,
        BitcoinChain::Bitcoin | BitcoinChain::Litecoin | BitcoinChain::Doge => sign_standard(plan, &secret_key.0, &public_key, &secp)?,
        BitcoinChain::Zcash => {
            let branch_id = zcash_branch_id.ok_or_else(|| SignerError::invalid_input("missing Zcash branch id"))?;
            return sign_transparent(plan, branch_id, &secret_key.0, &public_key, &secp);
        }
    };

    Ok(hex::encode(serialize(&transaction)))
}

pub fn build_unsigned_transaction(plan: &SpendPlan) -> Transaction {
    let input = plan
        .inputs
        .iter()
        .map(|input| TransactionInput {
            previous_output: input.previous_output,
            script_sig: ScriptBuf::new(),
            sequence: Sequence(input.sequence),
            witness: Witness::default(),
        })
        .collect();

    let output = plan
        .outputs
        .iter()
        .map(|output| TransactionOutput {
            value: output.value,
            script_pubkey: output.script_pubkey.clone(),
        })
        .collect();

    Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input,
        output,
    }
}

fn sign_standard<C: Signing>(plan: &SpendPlan, secret_key: &SecretKey, public_key: &PublicKey, secp: &Secp256k1<C>) -> Result<Transaction, SignerError> {
    let mut transaction = build_unsigned_transaction(plan);
    let signed_inputs = {
        let mut sighash_cache = SighashCache::new(&transaction);
        let mut signed_inputs = Vec::with_capacity(plan.inputs.len());

        for (index, input) in plan.inputs.iter().enumerate() {
            match input.unlocking_script {
                UnlockingScript::P2pkh => {
                    let sighash = sighash_cache
                        .legacy_signature_hash(index, &input.script_pubkey, EcdsaSighashType::All.to_u32())
                        .map_err(|e| SignerError::signing_error(e.to_string()))?;
                    let signature = der_signature(secp, secret_key, Message::from(sighash), EcdsaSighashType::All.to_u32() as u8);
                    let script_sig = Builder::new().push_slice(signature_push_bytes(signature)?).push_key(public_key).into_script();
                    signed_inputs.push((script_sig, Witness::default()));
                }
                UnlockingScript::P2wpkh => {
                    let sighash = sighash_cache
                        .p2wpkh_signature_hash(index, &input.script_pubkey, input.value, EcdsaSighashType::All)
                        .map_err(|e| SignerError::signing_error(e.to_string()))?;
                    let signature = der_signature(secp, secret_key, Message::from(sighash), EcdsaSighashType::All.to_u32() as u8);
                    let mut witness = Witness::default();
                    witness.push(signature);
                    witness.push(public_key.to_bytes());
                    signed_inputs.push((ScriptBuf::new(), witness));
                }
            }
        }
        signed_inputs
    };

    for (input, (script_sig, witness)) in transaction.input.iter_mut().zip(signed_inputs) {
        input.script_sig = script_sig;
        input.witness = witness;
    }

    Ok(transaction)
}

fn validate_chain_input_types(chain: BitcoinChain, plan: &SpendPlan) -> Result<(), SignerError> {
    for input in &plan.inputs {
        match chain {
            BitcoinChain::Bitcoin | BitcoinChain::Litecoin | BitcoinChain::Doge => {}
            BitcoinChain::BitcoinCash | BitcoinChain::Zcash => {
                (input.unlocking_script == UnlockingScript::P2pkh)
                    .then_some(())
                    .ok_or_else(|| SignerError::invalid_input(format!("{} UTXO address type is unsupported", chain.get_chain())))?;
            }
        }
    }
    Ok(())
}

fn validate_public_key(chain: primitives::Chain, plan: &SpendPlan, public_key: &PublicKey) -> Result<(), SignerError> {
    for input in &plan.inputs {
        validate_input_public_key(chain, input, public_key)?;
    }
    Ok(())
}

fn validate_input_public_key(chain: Chain, input: &PlanInput, public_key: &PublicKey) -> Result<(), SignerError> {
    let expected = script_for_public_key_hash(input.unlocking_script, public_key_hash(&public_key.to_bytes()));
    (public_key.compressed && expected == input.script_pubkey)
        .then_some(())
        .ok_or_else(|| SignerError::invalid_input(format!("{chain} private key does not match sender address")))
}

fn validate_plan_amounts(chain: primitives::Chain, plan: &SpendPlan) -> Result<(), SignerError> {
    let input_total: u128 = plan.inputs.iter().map(|input| input.value.to_sat() as u128).sum();
    let output_total: u128 = plan.outputs.iter().map(|output| output.value.to_sat() as u128).sum();
    let spent_total = output_total + plan.fee as u128;
    (input_total == spent_total).then_some(()).ok_or_else(|| {
        SignerError::invalid_input(format!(
            "{} plan amount mismatch: inputs {}, outputs {}, fee {}",
            chain, input_total, output_total, plan.fee
        ))
    })?;
    Ok(())
}

pub(super) fn der_signature<C: Signing>(secp: &Secp256k1<C>, secret_key: &SecretKey, message: Message, sighash_byte: u8) -> Vec<u8> {
    let signature = secp.sign_ecdsa(&message, secret_key);
    let mut bytes = signature.serialize_der().to_vec();
    bytes.push(sighash_byte);
    bytes
}

pub(super) fn signature_push_bytes(signature: Vec<u8>) -> Result<PushBytesBuf, SignerError> {
    PushBytesBuf::try_from(signature).map_err(|_| SignerError::signing_error("invalid Bitcoin script push"))
}

pub fn sign_fork_id_plan(chain: primitives::Chain, plan: &SpendPlan, private_key: &[u8]) -> Result<String, SignerError> {
    let secret_key = ZeroizedSecretKey(SecretKey::from_slice(private_key).map_err(|_| SignerError::invalid_input("invalid private key"))?);
    if plan.inputs.is_empty() || plan.inputs.iter().any(|input| input.unlocking_script != UnlockingScript::P2pkh) {
        return SignerError::invalid_input_err("fork-id signer requires P2PKH inputs");
    }
    let secp = Secp256k1::signing_only();
    let public_key = PublicKey::new(Secp256k1PublicKey::from_secret_key(&secp, &secret_key.0));
    validate_public_key(chain, plan, &public_key)?;
    validate_plan_amounts(chain, plan)?;
    Ok(hex::encode(serialize(&sign_bitcoin_cash(plan, &secret_key.0, &public_key, &secp)?)))
}

pub fn sign_fork_id_plan_with_keys(
    chain: Chain,
    plan: &SpendPlan,
    mut key_for_input: impl FnMut(&PlanInput) -> Result<Zeroizing<Vec<u8>>, SignerError>,
) -> Result<String, SignerError> {
    validate_fork_id_plan(chain, plan)?;
    let secp = Secp256k1::signing_only();
    let transaction = sign_plan_with_signatures(plan, |input, digest| {
        let private_key = key_for_input(input)?;
        let secret_key = ZeroizedSecretKey(SecretKey::from_slice(&private_key).map_err(|_| SignerError::invalid_input("invalid private key"))?);
        let public_key = PublicKey::new(Secp256k1PublicKey::from_secret_key(&secp, &secret_key.0));
        validate_input_public_key(chain, input, &public_key)?;
        Ok((der_signature(&secp, &secret_key.0, Message::from_digest(digest), 0x41), public_key))
    })?;
    Ok(hex::encode(serialize(&transaction)))
}

pub fn verify_fork_id_plan(chain: Chain, plan: &SpendPlan, data: &str) -> Result<String, SignerError> {
    validate_fork_id_plan(chain, plan)?;
    let transaction: Transaction = deserialize(&hex::decode(data).map_err(SignerError::from_display)?).map_err(SignerError::from_display)?;
    let unsigned = build_unsigned_transaction(plan);
    if transaction.version != unsigned.version
        || transaction.lock_time != unsigned.lock_time
        || transaction.output != unsigned.output
        || transaction.input.len() != plan.inputs.len()
    {
        return SignerError::invalid_input_err("fork-id transaction differs from confirmed plan");
    }
    let components = SighashComponents::new(&transaction);
    let secp = Secp256k1::verification_only();
    for (index, (signed, input)) in transaction.input.iter().zip(&plan.inputs).enumerate() {
        if signed.previous_output != input.previous_output || signed.sequence != Sequence(input.sequence) || !signed.witness.is_empty() {
            return SignerError::invalid_input_err("fork-id transaction input differs from confirmed plan");
        }
        let instructions = signed.script_sig.instructions_minimal().collect::<Result<Vec<_>, _>>().map_err(SignerError::from_display)?;
        let [Instruction::PushBytes(signature), Instruction::PushBytes(public_key)] = instructions.as_slice() else {
            return SignerError::invalid_input_err("invalid fork-id signature script");
        };
        let (sighash, signature) = signature.as_bytes().split_last().ok_or_else(|| SignerError::invalid_input("missing fork-id signature"))?;
        if *sighash != 0x41 || public_key.len() != 33 {
            return SignerError::invalid_input_err("invalid fork-id signature type or public key");
        }
        let public_key = PublicKey::from_slice(public_key.as_bytes()).map_err(SignerError::from_display)?;
        validate_input_public_key(chain, input, &public_key)?;
        let signature = Signature::from_der(signature).map_err(SignerError::from_display)?;
        let mut canonical = signature;
        canonical.normalize_s();
        if canonical != signature {
            return SignerError::invalid_input_err("noncanonical fork-id signature");
        }
        let digest = signature_hash(&transaction, plan, &components, index)?;
        secp.verify_ecdsa(&Message::from_digest(digest), &signature, &public_key.inner)
            .map_err(SignerError::from_display)?;
    }
    Ok(transaction.compute_txid().to_string())
}

fn validate_fork_id_plan(chain: Chain, plan: &SpendPlan) -> Result<(), SignerError> {
    if plan.inputs.is_empty() || plan.inputs.iter().any(|input| input.unlocking_script != UnlockingScript::P2pkh) {
        return SignerError::invalid_input_err("fork-id signer requires P2PKH inputs");
    }
    validate_plan_amounts(chain, plan)
}
