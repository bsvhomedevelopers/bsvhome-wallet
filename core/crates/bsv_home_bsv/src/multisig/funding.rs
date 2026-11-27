use std::collections::{HashMap, HashSet};

use bitcoin::hashes::{Hash, sha256};
use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
use bsv_home_bitcoin::signer::{planner::SpendPlan, sign_fork_id_plan_with_keys, verify_fork_id_plan};
use num_traits::ToPrimitive;
use primitives::{
    Address, Asset, BsvAddressBranch, BsvAddressLocator, BsvSpendChange, BsvSpendInput, BsvSpendMetadata, Chain, GasPriceType, SignerError, TransactionInputType,
    TransactionLoadInput, TransactionLoadMetadata, UTXO,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::{
    Policy, policy_descriptor,
    portal::{MAX_FEE, MAX_MONEY, decode_transaction},
    transaction::SigningKey,
};
use crate::{BsvAddress, signer::plan_transfer};

pub const MAX_FUNDING_UTXOS: usize = 1000;
const MAX_FUNDING_ADDRESSES: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FundingAddress {
    pub address: String,
    pub locator: BsvAddressLocator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Funding {
    pub chain: Chain,
    pub policy: Policy,
    pub sender_address: String,
    pub amount: u64,
    pub fee_rate: u64,
    pub utxos: Vec<UTXO>,
    pub addresses: Vec<FundingAddress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<BsvSpendChange>,
}

impl Funding {
    pub fn from_spend(policy: Policy, sender_address: String, amount: u64, fee_rate: u64, spend: BsvSpendMetadata) -> Result<Self, SignerError> {
        if spend.inputs.is_empty() || spend.inputs.len() > MAX_FUNDING_UTXOS {
            return SignerError::invalid_input_err("invalid BSV funding UTXO count");
        }
        let mut addresses = vec![FundingAddress {
            address: sender_address.clone(),
            locator: BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 0,
            },
        }];
        let mut utxos = Vec::with_capacity(spend.inputs.len());
        for input in spend.inputs {
            let owner = BsvAddress::from_str(&input.address).map_err(SignerError::from_display)?;
            if hex::decode(&input.script_pubkey).map_err(SignerError::from_display)? != owner.script_pubkey().as_bytes() {
                return SignerError::invalid_input_err("BSV funding source script does not match address");
            }
            let address = FundingAddress {
                address: input.address.clone(),
                locator: input.locator,
            };
            if !addresses.contains(&address) {
                addresses.push(address);
            }
            utxos.push(UTXO {
                transaction_id: input.transaction_id.to_ascii_lowercase(),
                vout: i32::try_from(input.vout).map_err(SignerError::from_display)?,
                value: input.value.into(),
                address: input.address,
            });
        }
        if let Some(change) = &spend.change {
            let address = FundingAddress {
                address: change.address.clone(),
                locator: change.locator,
            };
            if !addresses.contains(&address) {
                addresses.push(address);
            }
        }
        let mut funding = Self {
            chain: Chain::BitcoinSv,
            policy: policy.canonical()?,
            sender_address,
            amount,
            fee_rate,
            utxos,
            addresses,
            change: spend.change,
        };
        let selected = funding_spend_metadata(&funding)?;
        let outpoints = selected.inputs.iter().map(|input| (input.transaction_id.as_str(), input.vout)).collect::<HashSet<_>>();
        funding.utxos.retain(|source| outpoints.contains(&(source.transaction_id.as_str(), source.vout as u32)));
        funding.addresses.retain(|owner| {
            owner.address == funding.sender_address
                || selected.inputs.iter().any(|input| input.address == owner.address && input.locator == owner.locator)
                || selected
                    .change
                    .as_ref()
                    .is_some_and(|change| change.address == owner.address && change.locator == owner.locator)
        });
        funding.change = selected.change;
        review_funding(&funding)?;
        Ok(funding)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FundingReview {
    pub commitment: String,
    pub configuration_hash: String,
    pub sender_address: String,
    pub amount: u64,
    pub fee: u64,
    pub change: u64,
    pub input_count: u32,
}

fn funding_plan_with_spend(funding: &Funding) -> Result<(SpendPlan, BsvSpendMetadata), SignerError> {
    if funding.chain != Chain::BitcoinSv
        || funding.amount == 0
        || funding.amount > MAX_MONEY
        || funding.utxos.is_empty()
        || funding.utxos.len() > MAX_FUNDING_UTXOS
        || funding.addresses.len() > MAX_FUNDING_ADDRESSES
        || funding.fee_rate == 0
    {
        return SignerError::invalid_input_err("invalid BSV funding chain, amount, fee rate or UTXO count");
    }
    BsvAddress::from_str(&funding.sender_address).map_err(SignerError::from_display)?;
    let mut addresses = HashMap::new();
    let mut locators = HashSet::new();
    for address in &funding.addresses {
        address.locator.validate()?;
        BsvAddress::from_str(&address.address).map_err(SignerError::from_display)?;
        if addresses.insert(&address.address, address.locator).is_some() || !locators.insert(address.locator) {
            return SignerError::invalid_input_err("duplicate BSV funding address or locator");
        }
    }
    if addresses.get(&funding.sender_address)
        != Some(&BsvAddressLocator {
            branch: BsvAddressBranch::Receive,
            index: 0,
        })
    {
        return SignerError::invalid_input_err("BSV funding sender does not match default account identity");
    }
    let mut inputs = Vec::new();
    for source in &funding.utxos {
        let value = source.value.to_u64().ok_or_else(|| SignerError::invalid_input("BSV funding value exceeds integer range"))?;
        if value <= 1 {
            return SignerError::invalid_input_err("BSV funding cannot spend token or zero-value outputs");
        }
        let locator = *addresses
            .get(&source.address)
            .ok_or_else(|| SignerError::invalid_input("BSV funding source address missing"))?;
        let owner = BsvAddress::from_str(&source.address).map_err(SignerError::from_display)?;
        inputs.push(BsvSpendInput {
            transaction_id: source.transaction_id.clone(),
            vout: u32::try_from(source.vout).map_err(SignerError::from_display)?,
            value,
            script_pubkey: hex::encode(owner.script_pubkey().as_bytes()),
            address: source.address.clone(),
            locator,
        });
    }
    let descriptor = policy_descriptor(&funding.policy)?;
    let spend = BsvSpendMetadata {
        inputs,
        change: funding.change.clone(),
    };
    let plan = plan_transfer(&TransactionLoadInput {
        input_type: TransactionInputType::Transfer {
            asset: Asset::from_chain(Chain::BitcoinSv),
        },
        sender_address: funding.sender_address.clone(),
        destination_address: format!("bsv-script:{}", descriptor.locking_script),
        value: funding.amount.into(),
        gas_price: GasPriceType::regular(funding.fee_rate),
        memo: None,
        is_max_value: false,
        metadata: TransactionLoadMetadata::BitcoinSv { spend: spend.clone() },
    })?;
    if plan.fee > MAX_FEE {
        return SignerError::invalid_input_err("BSV funding fee exceeds supported limit");
    }
    Ok((plan, spend))
}

fn funding_plan(funding: &Funding) -> Result<SpendPlan, SignerError> {
    funding_plan_with_spend(funding).map(|(plan, _)| plan)
}

pub fn funding_spend_metadata(funding: &Funding) -> Result<BsvSpendMetadata, SignerError> {
    let (plan, mut spend) = funding_plan_with_spend(funding)?;
    let selected = plan
        .inputs
        .iter()
        .map(|input| (input.previous_output.txid.to_string(), input.previous_output.vout))
        .collect::<HashSet<_>>();
    spend.inputs.retain(|input| selected.contains(&(input.transaction_id.to_ascii_lowercase(), input.vout)));
    if plan.outputs.len() == 1 {
        spend.change = None;
    }
    Ok(spend)
}

pub fn review_funding(funding: &Funding) -> Result<FundingReview, SignerError> {
    let plan = funding_plan(funding)?;
    let descriptor = policy_descriptor(&funding.policy)?;
    let commitment = hex::encode(sha256::Hash::hash(&serde_json::to_vec(funding).map_err(SignerError::from_display)?).to_byte_array());
    Ok(FundingReview {
        commitment,
        configuration_hash: descriptor.configuration_hash,
        sender_address: funding.sender_address.clone(),
        amount: funding.amount,
        fee: plan.fee,
        change: plan.outputs.get(1).map_or(0, |output| output.value.to_sat()),
        input_count: plan.inputs.len() as u32,
    })
}

pub fn sign_funding_with_keys(
    funding: &Funding,
    review: &FundingReview,
    mut key_for_locator: impl FnMut(BsvAddressLocator) -> Result<Zeroizing<Vec<u8>>, SignerError>,
) -> Result<String, SignerError> {
    if &review_funding(funding)? != review {
        return SignerError::invalid_input_err("BSV funding differs from confirmed review");
    }
    let plan = funding_plan(funding)?;
    if let Some(change) = &funding.change {
        let key = SigningKey(SecretKey::from_slice(key_for_locator(change.locator)?.as_slice()).map_err(SignerError::from_display)?);
        let public_key = PublicKey::from_secret_key(&Secp256k1::signing_only(), &key.0).serialize();
        if BsvAddress::from_public_key(&public_key).map_err(SignerError::from_display)?.encode() != change.address {
            return SignerError::invalid_input_err("BSV funding change does not belong to the selected payment wallet");
        }
    }
    let locators = funding.addresses.iter().map(|address| (&address.address, address.locator)).collect::<HashMap<_, _>>();
    let sources = funding
        .utxos
        .iter()
        .map(|source| ((source.transaction_id.clone(), source.vout), &source.address))
        .collect::<HashMap<_, _>>();
    let raw = sign_fork_id_plan_with_keys(Chain::BitcoinSv, &plan, |input| {
        let address = sources
            .get(&(input.previous_output.txid.to_string(), input.previous_output.vout as i32))
            .ok_or_else(|| SignerError::invalid_input("BSV funding selected input missing"))?;
        let locator = *locators.get(*address).ok_or_else(|| SignerError::invalid_input("BSV funding derivation locator missing"))?;
        key_for_locator(locator)
    })?;
    funding_transaction_id(funding, review, &raw)?;
    Ok(raw)
}

pub fn funding_transaction_id(funding: &Funding, review: &FundingReview, raw: &str) -> Result<String, SignerError> {
    if &review_funding(funding)? != review {
        return SignerError::invalid_input_err("BSV funding differs from confirmed review");
    }
    decode_transaction(raw)?;
    verify_fork_id_plan(Chain::BitcoinSv, &funding_plan(funding)?, raw)
}
