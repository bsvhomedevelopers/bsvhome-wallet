use crate::{BsvAddress, multisig::BsvScriptTarget};
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Message, Secp256k1, SecretKey};
use bitcoin::sign_message::{MessageSignature, signed_msg_hash};
use bitcoin::{Amount, OutPoint, ScriptBuf, Transaction, Txid, consensus::deserialize, script::Instruction};
use bsv_home_bitcoin::signer::{
    UnlockingScript,
    planner::{PlanInput, PlanOutput, SpendPlan, SpendPolicy, SpendTarget, UtxoPlanner},
    sign_fork_id_plan, sign_fork_id_plan_with_keys, verify_fork_id_plan,
};
use num_traits::ToPrimitive;
use primitives::{
    Address, BsvAddressBranch, BsvAddressLocator, BsvSpendMetadata, Chain, ChainSigner, SignerError, SignerInput, TransactionInputType, TransactionLoadInput,
    TransactionLoadMetadata,
};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};
use zeroize::Zeroizing;

#[cfg(test)]
#[path = "signer_tests.rs"]
mod tests;

pub struct BsvChainSigner;
pub(crate) struct BsvSpendPolicy {
    pub(crate) satoshis_per_kilobyte: u64,
}

impl BsvChainSigner {
    pub fn sign_transfer_with_keys(
        &self,
        input: &SignerInput,
        mut key_for_locator: impl FnMut(BsvAddressLocator) -> Result<Zeroizing<Vec<u8>>, SignerError>,
    ) -> Result<String, SignerError> {
        let plan = confirmed_spend_plan(input)?;
        let TransactionLoadMetadata::BitcoinSv { spend } = &input.metadata else {
            return SignerError::invalid_input_err("BSV multi-address signing requires a frozen spend description");
        };
        let locators = spend
            .inputs
            .iter()
            .map(|source| Ok((spend_outpoint(&source.transaction_id, source.vout)?, source.locator)))
            .collect::<Result<HashMap<_, _>, SignerError>>()?;
        sign_fork_id_plan_with_keys(Chain::BitcoinSv, &plan, |selected| {
            let locator = locators
                .get(&selected.previous_output)
                .ok_or_else(|| SignerError::invalid_input("BSV selected input has no derivation locator"))?;
            key_for_locator(*locator)
        })
    }

    pub fn validate_signed_spend(data: &str, input: &SignerInput) -> Result<String, SignerError> {
        let plan = confirmed_spend_plan(input)?;
        verify_fork_id_plan(Chain::BitcoinSv, &plan, data)
    }

    pub fn validate_cold_signed_spend(data: &str, input: &SignerInput) -> Result<String, SignerError> {
        let mut plan = confirmed_spend_plan(input)?;
        for source in &mut plan.inputs {
            source.sequence = 0xffff_fffd;
        }
        verify_fork_id_plan(Chain::BitcoinSv, &plan, data)
    }

    pub fn validate_signed_transfer(data: &str, sender: &str, recipient: &str, value: u64) -> Result<String, SignerError> {
        let transaction: Transaction = deserialize(&hex::decode(data).map_err(SignerError::from_display)?).map_err(SignerError::from_display)?;
        let sender = BsvAddress::from_str(sender).map_err(SignerError::from_display)?;
        let recipient_script = BsvScriptTarget::parse(recipient)?;
        let output = transaction.output.first().ok_or_else(|| SignerError::invalid_input("missing BSV payment output"))?;
        if value == 0 || output.value.to_sat() != value || output.script_pubkey != recipient_script || transaction.output.len() > 2 {
            return SignerError::invalid_input_err("BSV payment does not match confirmed recipient or value");
        }
        if transaction.output.get(1).is_some_and(|output| output.script_pubkey != sender.script_pubkey()) || transaction.input.is_empty() {
            return SignerError::invalid_input_err("invalid BSV payment change or inputs");
        }
        for input in &transaction.input {
            let instructions = input.script_sig.instructions().collect::<Result<Vec<_>, _>>().map_err(SignerError::from_display)?;
            let [Instruction::PushBytes(signature), Instruction::PushBytes(public_key)] = instructions.as_slice() else {
                return SignerError::invalid_input_err("invalid BSV payment signature script");
            };
            if signature.as_bytes().last() != Some(&0x41)
                || BsvAddress::from_public_key(public_key.as_bytes()).map_err(SignerError::from_display)?.script_pubkey() != sender.script_pubkey()
            {
                return SignerError::invalid_input_err("BSV payment signature owner mismatch");
            }
        }
        Ok(transaction.compute_txid().to_string())
    }

    pub fn message_hash(message: &str) -> [u8; 32] {
        signed_msg_hash(message).to_byte_array()
    }

    pub fn sign_message_compact(message: &str, private_key: &[u8]) -> Result<[u8; 65], SignerError> {
        let mut secret_key = SecretKey::from_slice(private_key).map_err(|_| SignerError::invalid_input("invalid BSV private key"))?;
        let digest = Message::from_digest(Self::message_hash(message));
        let signature = Secp256k1::signing_only().sign_ecdsa_recoverable(&digest, &secret_key);
        secret_key.non_secure_erase();
        Ok(MessageSignature::new(signature, true).serialize())
    }

    pub fn recover_message_address(message: &str, signature: &[u8]) -> Result<String, SignerError> {
        Ok(BsvAddress::from_public_key(&Self::recover_message_public_key(message, signature)?)?.encode())
    }

    pub fn recover_message_public_key(message: &str, signature: &[u8]) -> Result<[u8; 33], SignerError> {
        if signature.len() != 65 || !(31..=34).contains(&signature[0]) {
            return SignerError::invalid_input_err("invalid BSV compressed P2PKH message signature");
        }
        let signature = MessageSignature::from_slice(signature).map_err(SignerError::from_display)?;
        let standard_signature = signature.signature.to_standard();
        let mut normalized_signature = standard_signature;
        normalized_signature.normalize_s();
        if normalized_signature != standard_signature {
            return SignerError::invalid_input_err("BSV message signature requires low S");
        }
        let public_key = signature
            .recover_pubkey(&Secp256k1::verification_only(), signed_msg_hash(message))
            .map_err(SignerError::from_display)?;
        Ok(public_key.inner.serialize())
    }
}

impl SpendPolicy for BsvSpendPolicy {
    fn estimate_fee(&self, inputs: &[PlanInput], outputs: &[PlanOutput]) -> Result<u64, SignerError> {
        let variable_length = |count: usize| match count {
            0..=252 => 1,
            253..=65535 => 3,
            65536..=4294967295 => 5,
            _ => 9,
        };
        let bytes = 8u128
            + variable_length(inputs.len())
            + variable_length(outputs.len())
            + inputs.len() as u128 * 148
            + outputs.iter().map(|output| output.serialized_len() as u128).sum::<u128>();
        let fee = (bytes * self.satoshis_per_kilobyte as u128).div_ceil(1000);
        u64::try_from(fee).map_err(|_| SignerError::invalid_input("BSV fee overflow"))
    }
    fn dust_threshold(&self, _script: &ScriptBuf) -> u64 {
        1
    }
}

pub fn plan_transfer(input: &TransactionLoadInput) -> Result<SpendPlan, SignerError> {
    let native_transfer = match &input.input_type {
        TransactionInputType::Transfer { asset } => asset.id == Chain::BitcoinSv.as_asset_id(),
        TransactionInputType::Deposit { .. }
        | TransactionInputType::Withdrawal { .. }
        | TransactionInputType::Swap { .. }
        | TransactionInputType::Stake { .. }
        | TransactionInputType::TokenApprove { .. }
        | TransactionInputType::Generic { .. }
        | TransactionInputType::TransferNft { .. }
        | TransactionInputType::Account { .. }
        | TransactionInputType::Perpetual { .. }
        | TransactionInputType::Earn { .. } => false,
    };
    if !native_transfer || input.get_memo().is_some() {
        return SignerError::invalid_input_err("BSV supports native transfers without memo only");
    }
    let sender = BsvAddress::from_str(&input.sender_address).map_err(|error| SignerError::invalid_input(error.to_string()))?;
    let destination_script = BsvScriptTarget::parse(&input.destination_address)?;
    let rate = input
        .gas_price
        .gas_price()
        .to_u64()
        .filter(|rate| *rate > 0)
        .ok_or_else(|| SignerError::invalid_input("invalid BSV sat/KB fee rate"))?;
    let inputs = spend_inputs(input, &sender)?;
    let mut outpoints = HashSet::new();
    let mut input_total = 0u128;
    for source in &inputs {
        if !outpoints.insert(source.previous_output) {
            return SignerError::invalid_input_err("duplicate BSV outpoint");
        }
        input_total += source.value.to_sat() as u128;
        if input_total > 2_100_000_000_000_000 {
            return SignerError::invalid_input_err("BSV input total exceeds monetary supply");
        }
    }
    let change_script = match &input.metadata {
        TransactionLoadMetadata::BitcoinSv { spend } => match &spend.change {
            Some(change) => {
                change.locator.validate()?;
                if change.locator.branch != BsvAddressBranch::Change {
                    return SignerError::invalid_input_err("BSV change must use the change branch");
                }
                validated_address_script(&change.address, &change.script_pubkey)?
            }
            None => sender.script_pubkey(),
        },
        _ => sender.script_pubkey(),
    };
    UtxoPlanner::plan_spend(
        &BsvSpendPolicy { satoshis_per_kilobyte: rate },
        if input.is_max_value {
            SpendTarget::Max
        } else {
            SpendTarget::Exact(input.value_as_u64()?)
        },
        destination_script,
        change_script,
        None,
        inputs,
    )
}

fn spend_inputs(input: &TransactionLoadInput, sender: &BsvAddress) -> Result<Vec<PlanInput>, SignerError> {
    if let TransactionLoadMetadata::BitcoinSv { spend } = &input.metadata {
        return spend
            .inputs
            .iter()
            .map(|source| {
                source.locator.validate()?;
                if source.value <= 1 {
                    return SignerError::invalid_input_err("BSV protected or zero-value input cannot be spent");
                }
                Ok(PlanInput {
                    previous_output: spend_outpoint(&source.transaction_id, source.vout)?,
                    value: Amount::from_sat(source.value),
                    script_pubkey: validated_address_script(&source.address, &source.script_pubkey)?,
                    unlocking_script: UnlockingScript::P2pkh,
                    sequence: u32::MAX,
                })
            })
            .collect();
    }
    let mut inputs = Vec::new();
    let mut outpoints = HashSet::new();
    for utxo in input.metadata.get_utxos().map_err(SignerError::from_display)? {
        let vout = u32::try_from(utxo.vout).map_err(|_| SignerError::invalid_input("negative BSV output index"))?;
        let previous_output = spend_outpoint(&utxo.transaction_id, vout)?;
        if !outpoints.insert(previous_output) {
            return SignerError::invalid_input_err("duplicate BSV outpoint");
        }
        let value = utxo
            .value
            .to_u64()
            .filter(|value| *value > 0)
            .ok_or_else(|| SignerError::invalid_input("invalid BSV UTXO amount"))?;
        if value == 1 {
            continue;
        }
        if utxo.address != input.sender_address {
            return SignerError::invalid_input_err("BSV UTXO address does not match sender");
        }
        inputs.push(PlanInput {
            previous_output,
            value: Amount::from_sat(value),
            script_pubkey: sender.script_pubkey(),
            unlocking_script: UnlockingScript::P2pkh,
            sequence: u32::MAX,
        });
    }
    Ok(inputs)
}

fn spend_outpoint(transaction_id: &str, vout: u32) -> Result<OutPoint, SignerError> {
    Ok(OutPoint::new(Txid::from_str(transaction_id).map_err(SignerError::from_display)?, vout))
}

fn validated_address_script(address: &str, script: &str) -> Result<ScriptBuf, SignerError> {
    let owner = BsvAddress::from_str(address).map_err(SignerError::from_display)?;
    let script = ScriptBuf::from_bytes(hex::decode(script).map_err(SignerError::from_display)?);
    if script != owner.script_pubkey() {
        return SignerError::invalid_input_err("BSV source script does not match address");
    }
    Ok(script)
}

pub fn finalize_spend_metadata(input: &TransactionLoadInput, plan: &SpendPlan) -> Result<TransactionLoadMetadata, SignerError> {
    let TransactionLoadMetadata::BitcoinSv { spend } = &input.metadata else {
        return Ok(input.metadata.clone());
    };
    let selected = plan.inputs.iter().map(|source| source.previous_output).collect::<HashSet<_>>();
    let mut inputs = Vec::with_capacity(selected.len());
    for source in &spend.inputs {
        if selected.contains(&spend_outpoint(&source.transaction_id, source.vout)?) {
            inputs.push(source.clone());
        }
    }
    let change = if plan.outputs.len() > 1 {
        Some(
            spend
                .change
                .clone()
                .ok_or_else(|| SignerError::invalid_input("BSV change locator must be allocated before confirmation"))?,
        )
    } else {
        None
    };
    Ok(TransactionLoadMetadata::BitcoinSv {
        spend: BsvSpendMetadata { inputs, change },
    })
}

fn confirmed_spend_plan(input: &SignerInput) -> Result<SpendPlan, SignerError> {
    let plan = plan_transfer(input)?;
    validate_confirmation(input, &plan)?;
    if let TransactionLoadMetadata::BitcoinSv { spend } = &input.metadata
        && (spend.inputs.len() != plan.inputs.len() || spend.change.is_some() != (plan.outputs.len() > 1))
    {
        return SignerError::invalid_input_err("BSV spend description differs from confirmed selection");
    }
    Ok(plan)
}

fn validate_confirmation(input: &SignerInput, plan: &SpendPlan) -> Result<(), SignerError> {
    if input.is_max_value && plan.outputs.first().map(|output| output.value.to_sat()) != Some(input.value_as_u64()?) {
        return SignerError::invalid_input_err("BSV available balance changed since confirmation");
    }
    if plan.fee != input.fee.fee()? || input.fee.fee_asset != Chain::BitcoinSv.as_asset_id() || input.gas_price != input.fee.gas_price_type {
        return SignerError::invalid_input_err("BSV confirmation fee does not match signing plan");
    }
    Ok(())
}

impl ChainSigner for BsvChainSigner {
    fn sign_transfer(&self, input: &SignerInput, private_key: &[u8]) -> Result<String, SignerError> {
        if let TransactionLoadMetadata::BitcoinSv { .. } = input.metadata {
            return SignerError::invalid_input_err("BSV derived inputs require the wallet key provider");
        }
        let plan = plan_transfer(input)?;
        validate_confirmation(input, &plan)?;
        sign_fork_id_plan(Chain::BitcoinSv, &plan, private_key)
    }
}
