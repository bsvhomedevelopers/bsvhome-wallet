use bitcoin::hashes::{Hash, sha256};
use bitcoin::{
    Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    absolute::LockTime,
    bip32::{ChildNumber, DerivationPath, Xpriv},
    consensus::serialize,
    secp256k1::{PublicKey, Secp256k1, SecretKey},
    transaction::Version,
};
use primitives::{Address, BsvAddressBranch, BsvAddressLocator, BsvSpendChange, BsvSpendInput, Chain, UTXO};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::portal::decode_transaction;
use super::*;
use crate::BsvAddress;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Interop {
    proposal: Proposal,
    signature_hashes: Vec<String>,
    signature_sets: Vec<SignatureSet>,
    signed_transaction: String,
    signed_transaction_id: String,
}

fn fixture() -> Interop {
    serde_json::from_str(include_str!("../../testdata/bsv-multisig-interop.json")).unwrap()
}

fn public_key(value: u8) -> String {
    hex::encode(PublicKey::from_secret_key(&Secp256k1::signing_only(), &SecretKey::from_slice(&[value; 32]).unwrap()).serialize())
}

#[test]
fn test_portal_interop_exact_review_signatures_and_assembly() {
    let fixture = fixture();
    let proposal = &fixture.proposal;
    let review = review_proposal(&proposal.policy, proposal).unwrap();
    assert_eq!((review.amount, review.fee, review.change, review.input_count), (42000, 200, 57800, 2));
    assert_eq!(proposal_signature_hashes(&proposal.policy, proposal).unwrap(), fixture.signature_hashes);
    assert_eq!(assemble_proposal(&proposal.policy, proposal, &fixture.signature_sets).unwrap(), fixture.signed_transaction);
    assert_eq!(
        decode_transaction(&fixture.signed_transaction).unwrap().compute_txid().to_string(),
        fixture.signed_transaction_id
    );
    for value in 1..=3 {
        let signed = sign_proposal(&proposal.policy, proposal, &proposal.recipient, proposal.amount, proposal.fee, &[value; 32]).unwrap();
        assert_eq!(signed, fixture.signature_sets.iter().find(|set| set.public_key == public_key(value)).unwrap().clone());
    }
    assert_eq!(
        proposal_transaction_id(&proposal.policy, proposal, &fixture.signed_transaction).unwrap(),
        fixture.signed_transaction_id
    );
}

#[test]
fn test_portal_proposal_rejects_tampering_and_incomplete_signatures() {
    let fixture = fixture();
    let proposal = &fixture.proposal;
    let mut mutations = Vec::new();
    let mut changed = proposal.clone();
    changed.amount += 1;
    mutations.push(changed);
    let mut changed = proposal.clone();
    changed.fee += 1;
    mutations.push(changed);
    let mut changed = proposal.clone();
    changed.recipient = member_address(&public_key(9)).unwrap();
    mutations.push(changed);
    let mut changed = proposal.clone();
    changed.inputs.reverse();
    mutations.push(changed);
    let mut changed = proposal.clone();
    changed.policy.public_keys[0] = public_key(9);
    mutations.push(changed);
    let mut changed = proposal.clone();
    changed.unsigned_transaction.replace_range(..8, "01000000");
    mutations.push(changed);
    let mut changed = proposal.clone();
    let mut source = decode_transaction(&changed.inputs[0].transaction_hex).unwrap();
    source.output[1].value = Amount::from_sat(1);
    changed.inputs[0].transaction_hex = hex::encode(serialize(&source));
    mutations.push(changed);
    for changed in mutations {
        assert!(review_proposal(&proposal.policy, &changed).is_err());
    }
    assert!(sign_proposal(&proposal.policy, proposal, &proposal.recipient, proposal.amount, proposal.fee - 1, &[1; 32]).is_err());
    assert!(assemble_proposal(&proposal.policy, proposal, &fixture.signature_sets[..1]).is_err());
    let mut duplicate = fixture.signature_sets[..2].to_vec();
    duplicate[1] = duplicate[0].clone();
    assert!(assemble_proposal(&proposal.policy, proposal, &duplicate).is_err());
    let mut incomplete = fixture.signature_sets[..2].to_vec();
    incomplete[0].signatures.pop();
    assert!(assemble_proposal(&proposal.policy, proposal, &incomplete).is_err());
    let mut unknown = fixture.signature_sets[..2].to_vec();
    unknown[0].public_key = public_key(9);
    assert!(assemble_proposal(&proposal.policy, proposal, &unknown).is_err());
}

#[test]
fn test_portal_policy_all_member_counts_and_script_hashes() {
    for count in 2..=5 {
        for threshold in 2..=count {
            let policy = Policy {
                threshold,
                public_keys: (1..=count).map(|value| public_key(value as u8)).collect(),
            };
            let descriptor = policy_descriptor(&policy).unwrap();
            let parsed = parse_locking_script(&descriptor.locking_script).unwrap();
            assert_eq!(parsed.script_id().unwrap(), descriptor.configuration_hash);
            assert_eq!(parsed.script_hash().unwrap(), descriptor.script_hash);
            assert_eq!(u32::from(parsed.threshold), threshold);
        }
    }
}

#[test]
fn test_portal_utxo_and_individual_signature_validation() {
    let fixture = fixture();
    let proposal = &fixture.proposal;
    let source = &proposal.inputs[0];
    let transaction = decode_transaction(&source.transaction_hex).unwrap();
    let transaction_id = transaction.compute_txid().to_string();
    let value = transaction.output[source.output_index as usize].value.to_sat();
    validate_portal_utxo(&proposal.policy, &transaction_id, source.output_index, value, &source.transaction_hex).unwrap();
    assert!(validate_portal_utxo(&proposal.policy, &"00".repeat(32), source.output_index, value, &source.transaction_hex).is_err());
    assert!(validate_portal_utxo(&proposal.policy, &transaction_id, source.output_index, value + 1, &source.transaction_hex).is_err());
    assert!(validate_portal_utxo(&proposal.policy, &transaction_id, 0, 1000, &source.transaction_hex).is_err());
    validate_signature_set(&proposal.policy, proposal, &fixture.signature_sets[0]).unwrap();
    let mut incomplete = fixture.signature_sets[0].clone();
    incomplete.signatures.pop();
    assert!(validate_signature_set(&proposal.policy, proposal, &incomplete).is_err());
    let mut swapped = fixture.signature_sets[0].clone();
    swapped.signatures.reverse();
    assert!(validate_signature_set(&proposal.policy, proposal, &swapped).is_err());
}

fn funding() -> Funding {
    let addresses = [9, 10]
        .into_iter()
        .enumerate()
        .map(|(index, value)| FundingAddress {
            address: member_address(&public_key(value)).unwrap(),
            locator: BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: index as u32,
            },
        })
        .collect::<Vec<_>>();
    let utxos = addresses
        .iter()
        .enumerate()
        .map(|(index, address)| UTXO {
            transaction_id: format!("{:064x}", index + 10),
            vout: 0,
            value: [50000u64, 70000][index].into(),
            address: address.address.clone(),
        })
        .collect();
    Funding {
        chain: Chain::BitcoinSv,
        policy: fixture().proposal.policy,
        sender_address: addresses[0].address.clone(),
        amount: 100000,
        fee_rate: 100,
        utxos,
        addresses,
        change: None,
    }
}

#[test]
fn test_funding_multiple_addresses_change_and_followup_multisig_spend() {
    let funding = funding();
    let review = review_funding(&funding).unwrap();
    assert_eq!(review.input_count, 2);
    assert_eq!(review.amount + review.change + review.fee, 120000);
    let raw = sign_funding_with_keys(&funding, &review, |locator| Ok(Zeroizing::new(vec![9 + locator.index as u8; 32]))).unwrap();
    let transaction = decode_transaction(&raw).unwrap();
    assert_eq!(transaction.output[0].script_pubkey, funding.policy.configuration().unwrap().locking_script().unwrap());
    assert_eq!(transaction.output[1].script_pubkey, BsvAddress::from_str(&funding.sender_address).unwrap().script_pubkey());
    assert_eq!(funding_transaction_id(&funding, &review, &raw).unwrap(), transaction.compute_txid().to_string());
    let unsigned = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::new(transaction.compute_txid(), 0),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![
            TxOut {
                value: Amount::from_sat(1000),
                script_pubkey: BsvAddress::from_str(&funding.sender_address).unwrap().script_pubkey(),
            },
            TxOut {
                value: Amount::from_sat(98800),
                script_pubkey: funding.policy.configuration().unwrap().locking_script().unwrap(),
            },
        ],
    };
    let proposal = Proposal {
        policy: funding.policy.clone(),
        inputs: vec![ProposalInput {
            transaction_hex: raw,
            output_index: 0,
        }],
        recipient: funding.sender_address.clone(),
        amount: 1000,
        fee: 200,
        unsigned_transaction: hex::encode(serialize(&unsigned)),
    };
    let sets = [1, 2]
        .into_iter()
        .map(|value| sign_proposal(&funding.policy, &proposal, &proposal.recipient, 1000, 200, &[value; 32]).unwrap())
        .collect::<Vec<_>>();
    assert!(assemble_proposal(&funding.policy, &proposal, &sets).is_ok());
}

#[test]
fn test_funding_rejects_review_and_input_mutations() {
    let funding = funding();
    let review = review_funding(&funding).unwrap();
    let mut changed = review.clone();
    changed.amount += 1;
    assert!(sign_funding_with_keys(&funding, &changed, |_| Ok(Zeroizing::new(vec![9; 32]))).is_err());
    let mut changed = funding.clone();
    changed.utxos[0].value = 1u64.into();
    assert!(review_funding(&changed).is_err());
    let mut changed = funding.clone();
    changed.utxos[0].address = member_address(&public_key(11)).unwrap();
    assert!(review_funding(&changed).is_err());
    let mut changed = funding.clone();
    changed.utxos.push(changed.utxos[0].clone());
    assert!(review_funding(&changed).is_err());
    let mut changed = funding.clone();
    changed.utxos = vec![changed.utxos[0].clone(); 1001];
    assert!(review_funding(&changed).is_err());
    let mut changed = funding.clone();
    changed.chain = Chain::Bitcoin;
    assert!(review_funding(&changed).is_err());
    let raw = sign_funding_with_keys(&funding, &review, |locator| Ok(Zeroizing::new(vec![9 + locator.index as u8; 32]))).unwrap();
    let mut transaction = decode_transaction(&raw).unwrap();
    transaction.output[0].value = Amount::from_sat(1);
    assert!(funding_transaction_id(&funding, &review, &hex::encode(serialize(&transaction))).is_err());
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyFunding<'a> {
    chain: Chain,
    policy: &'a Policy,
    sender_address: &'a str,
    amount: u64,
    fee_rate: u64,
    utxos: &'a [UTXO],
    addresses: &'a [FundingAddress],
}

fn derived_key(locator: BsvAddressLocator) -> Zeroizing<Vec<u8>> {
    let master = Xpriv::new_master(Network::Bitcoin, &[0u8; 64]).unwrap();
    let path = DerivationPath::from(vec![
        ChildNumber::from_hardened_idx(44).unwrap(),
        ChildNumber::from_hardened_idx(236).unwrap(),
        ChildNumber::from_hardened_idx(0).unwrap(),
        ChildNumber::from_normal_idx(locator.branch.index()).unwrap(),
        ChildNumber::from_normal_idx(locator.index).unwrap(),
    ]);
    let derived = master.derive_priv(&Secp256k1::new(), &path).unwrap();
    Zeroizing::new(derived.private_key.secret_bytes().to_vec())
}

fn derived_address(locator: BsvAddressLocator) -> String {
    let key = SecretKey::from_slice(derived_key(locator).as_slice()).unwrap();
    BsvAddress::from_public_key(&PublicKey::from_secret_key(&Secp256k1::new(), &key).serialize())
        .unwrap()
        .encode()
}

fn hd_funding() -> Funding {
    let mut funding = funding();
    funding.addresses = (0..2)
        .map(|index| {
            let locator = BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index,
            };
            FundingAddress {
                address: derived_address(locator),
                locator,
            }
        })
        .collect();
    funding.sender_address = funding.addresses[0].address.clone();
    for (source, address) in funding.utxos.iter_mut().zip(&funding.addresses) {
        source.address = address.address.clone();
    }
    funding
}

fn allocated_change(index: u32) -> BsvSpendChange {
    let locator = BsvAddressLocator {
        branch: BsvAddressBranch::Change,
        index,
    };
    let address = derived_address(locator);
    BsvSpendChange {
        script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
        address,
        locator,
    }
}

#[test]
fn test_funding_legacy_serialization_commitment_and_signed_round_trip() {
    let funding = funding();
    let legacy = serde_json::to_vec(&LegacyFunding {
        chain: funding.chain,
        policy: &funding.policy,
        sender_address: &funding.sender_address,
        amount: funding.amount,
        fee_rate: funding.fee_rate,
        utxos: &funding.utxos,
        addresses: &funding.addresses,
    })
    .unwrap();
    assert!(serde_json::to_vec(&funding).unwrap() == legacy);
    let restored: Funding = serde_json::from_slice(&legacy).unwrap();
    assert_eq!(restored.change, None);
    let review = review_funding(&funding).unwrap();
    assert_eq!(review.commitment, hex::encode(sha256::Hash::hash(&legacy).to_byte_array()));
    assert_eq!(review_funding(&restored).unwrap(), review);
    assert_eq!(
        hex::encode(sha256::Hash::hash(&legacy).to_byte_array()),
        "897500fd7c55cdc450372d385b77815a155ca39c6c90cee4352d268adcd121fb"
    );
    assert_eq!(
        hex::encode(sha256::Hash::hash(serde_json::to_string(&review).unwrap().as_bytes()).to_byte_array()),
        "11aba3f056cc548bcfb3906531612477a2839a2eb2443319354612ea560c0f45"
    );
    let mut requested = Vec::new();
    let raw = sign_funding_with_keys(&funding, &review, |locator| {
        requested.push(locator);
        Ok(Zeroizing::new(vec![9 + locator.index as u8; 32]))
    })
    .unwrap();
    assert_eq!(
        hex::encode(sha256::Hash::hash(raw.as_bytes()).to_byte_array()),
        "320aa96514e197cf8404441bce8a89446741db9d35014c0f9445e6521f4144f3"
    );
    requested.sort_by_key(|locator| locator.index);
    assert_eq!(
        requested,
        vec![
            BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 0
            },
            BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 1
            },
        ]
    );
    assert!(sign_funding_with_keys(&restored, &review, |locator| Ok(Zeroizing::new(vec![9 + locator.index as u8; 32]))).unwrap() == raw);
    assert_eq!(
        decode_transaction(&raw).unwrap().output[1].script_pubkey,
        BsvAddress::from_str(&funding.sender_address).unwrap().script_pubkey()
    );
    assert_eq!(
        funding_transaction_id(&funding, &review, &raw).unwrap(),
        funding_transaction_id(&restored, &review, &raw).unwrap()
    );
}

#[test]
fn test_funding_allocated_change_is_bound_to_review_and_signed_output() {
    let mut funding = hd_funding();
    let legacy_review = review_funding(&funding).unwrap();
    let change = allocated_change(7);
    funding.change = Some(change.clone());
    let review = review_funding(&funding).unwrap();
    assert_ne!(review.commitment, legacy_review.commitment);
    let raw = sign_funding_with_keys(&funding, &review, |locator| Ok(derived_key(locator))).unwrap();
    let mut transaction = decode_transaction(&raw).unwrap();
    assert_eq!(transaction.output[1].script_pubkey, BsvAddress::from_str(&change.address).unwrap().script_pubkey());
    assert_eq!(transaction.output[1].value.to_sat(), review.change);
    assert_eq!(funding_transaction_id(&funding, &review, &raw).unwrap(), transaction.compute_txid().to_string());
    transaction.output[1].script_pubkey = BsvAddress::from_str(&funding.sender_address).unwrap().script_pubkey();
    assert!(funding_transaction_id(&funding, &review, &hex::encode(serialize(&transaction))).is_err());
    funding.change = Some(allocated_change(8));
    assert!(sign_funding_with_keys(&funding, &review, |_| Ok(Zeroizing::new(vec![12; 32]))).is_err());
}

#[test]
fn test_funding_allocated_change_rejects_unowned_key_before_signing_inputs() {
    let mut funding = hd_funding();
    funding.change = Some(allocated_change(8));
    let review = review_funding(&funding).unwrap();
    let mut input_requests = 0;
    let result = sign_funding_with_keys(&funding, &review, |locator| {
        if locator.branch == BsvAddressBranch::Receive {
            input_requests += 1;
        }
        let requested = if locator.branch == BsvAddressBranch::Change {
            BsvAddressLocator {
                index: locator.index + 1,
                ..locator
            }
        } else {
            locator
        };
        Ok(derived_key(requested))
    });
    assert!(result.is_err());
    assert_eq!(input_requests, 0);
}

#[test]
fn test_funding_allocated_change_requires_real_change_branch_and_address_script() {
    let mut funding = hd_funding();
    let mut change = allocated_change(7);
    change.locator.branch = BsvAddressBranch::Receive;
    funding.change = Some(change);
    assert!(review_funding(&funding).is_err());
    let mut change = allocated_change(7);
    change.locator.index = 1 << 31;
    funding.change = Some(change);
    assert!(review_funding(&funding).is_err());
    let mut change = allocated_change(7);
    change.script_pubkey = allocated_change(8).script_pubkey;
    funding.change = Some(change);
    assert!(review_funding(&funding).is_err());
}

#[test]
fn test_funding_spend_metadata_reserves_only_signed_inputs_and_keeps_legacy_root_change() {
    let mut funding = funding();
    let unused = UTXO {
        transaction_id: "ff".repeat(32),
        vout: 7,
        value: 200000u64.into(),
        address: funding.sender_address.clone(),
    };
    funding.utxos.push(unused.clone());
    let review = review_funding(&funding).unwrap();
    let spend = funding_spend_metadata(&funding).unwrap();
    assert_eq!(spend.inputs.len(), 2);
    assert_eq!(spend.inputs.len(), review.input_count as usize);
    assert_eq!(spend.change, None);
    assert_eq!(
        spend
            .inputs
            .iter()
            .filter(|input| input.transaction_id == unused.transaction_id && input.vout == unused.vout as u32)
            .count(),
        0
    );
    let raw = sign_funding_with_keys(&funding, &review, |locator| Ok(Zeroizing::new(vec![9 + locator.index as u8; 32]))).unwrap();
    let signed = decode_transaction(&raw).unwrap();
    let mut actual = signed
        .input
        .iter()
        .map(|input| (input.previous_output.txid.to_string(), input.previous_output.vout))
        .collect::<Vec<_>>();
    let mut reserved = spend.inputs.iter().map(|input| (input.transaction_id.clone(), input.vout)).collect::<Vec<_>>();
    actual.sort();
    reserved.sort();
    assert_eq!(reserved, actual);
    assert_eq!(signed.output[1].script_pubkey, BsvAddress::from_str(&funding.sender_address).unwrap().script_pubkey());
    assert_eq!(
        funding_spend_metadata(&serde_json::from_str(&serde_json::to_string(&funding).unwrap()).unwrap()).unwrap(),
        spend
    );
}

#[test]
fn test_funding_spend_metadata_retains_only_real_allocated_hd_change() {
    let mut funding = hd_funding();
    let change = allocated_change(7);
    funding.change = Some(change.clone());
    let spend = funding_spend_metadata(&funding).unwrap();
    assert_eq!(spend.change, Some(change.clone()));
    let review = review_funding(&funding).unwrap();
    let raw = sign_funding_with_keys(&funding, &review, |locator| Ok(derived_key(locator))).unwrap();
    assert_eq!(
        decode_transaction(&raw).unwrap().output[1].script_pubkey,
        BsvAddress::from_str(&change.address).unwrap().script_pubkey()
    );
    let descriptor = policy_descriptor(&funding.policy).unwrap();
    let output_size = 8 + 1 + hex::decode(descriptor.locking_script).unwrap().len() as u64;
    let fee_without_change = ((8 + 1 + 1 + 2 * 148 + output_size) * funding.fee_rate).div_ceil(1000);
    funding.amount = 120000 - fee_without_change;
    assert_eq!(review_funding(&funding).unwrap().change, 0);
    assert_eq!(funding_spend_metadata(&funding).unwrap().change, None);
    assert_eq!(funding.change, Some(change));
}

#[test]
fn test_funding_spend_metadata_uses_the_same_invalid_source_boundary_as_signing() {
    let mut funding = funding();
    funding.utxos[0].vout = -1;
    assert!(funding_spend_metadata(&funding).is_err());
    let mut funding = hd_funding();
    let mut change = allocated_change(7);
    change.script_pubkey = allocated_change(8).script_pubkey;
    funding.change = Some(change);
    assert!(funding_spend_metadata(&funding).is_err());
}

#[test]
fn test_funding_spend_metadata_single_address_keeps_real_receive_zero_and_root_change() {
    let mut funding = funding();
    funding.addresses.truncate(1);
    funding.utxos.truncate(1);
    funding.utxos[0].value = 200000u64.into();
    let spend = funding_spend_metadata(&funding).unwrap();
    assert_eq!(spend.inputs.len(), 1);
    assert_eq!(spend.change, None);
    assert_eq!(spend.inputs[0].address, funding.sender_address);
    assert_eq!(
        spend.inputs[0].locator,
        BsvAddressLocator {
            branch: BsvAddressBranch::Receive,
            index: 0
        }
    );
    let review = review_funding(&funding).unwrap();
    let raw = sign_funding_with_keys(&funding, &review, |locator| {
        assert_eq!(
            locator,
            BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 0
            }
        );
        Ok(Zeroizing::new(vec![9; 32]))
    })
    .unwrap();
    let signed = decode_transaction(&raw).unwrap();
    assert_eq!(signed.input.len(), spend.inputs.len());
    assert_eq!(signed.input[0].previous_output.txid.to_string(), spend.inputs[0].transaction_id);
    assert_eq!(signed.output[1].script_pubkey, BsvAddress::from_str(&funding.sender_address).unwrap().script_pubkey());
}

#[test]
fn test_funding_from_spend_keeps_only_selected_owners_and_real_allocated_change() {
    let original = hd_funding();
    let mut spend = funding_spend_metadata(&original).unwrap();
    spend.inputs[0].transaction_id.make_ascii_uppercase();
    let unused_locator = BsvAddressLocator {
        branch: BsvAddressBranch::Receive,
        index: 8,
    };
    let unused_address = derived_address(unused_locator);
    spend.inputs.push(BsvSpendInput {
        transaction_id: "ee".repeat(32),
        vout: 4,
        value: 200000,
        address: unused_address.clone(),
        script_pubkey: hex::encode(BsvAddress::from_str(&unused_address).unwrap().script_pubkey().as_bytes()),
        locator: unused_locator,
    });
    let change = allocated_change(7);
    spend.change = Some(change.clone());
    let funding = Funding::from_spend(original.policy, original.sender_address, original.amount, original.fee_rate, spend).unwrap();
    assert_eq!(funding.utxos.len(), 2);
    assert_eq!(funding.addresses.len(), 3);
    assert_eq!(funding.addresses.iter().filter(|owner| owner.address == unused_address).count(), 0);
    assert_eq!(funding.change, Some(change.clone()));
    let review = review_funding(&funding).unwrap();
    let raw = sign_funding_with_keys(&funding, &review, |locator| Ok(derived_key(locator))).unwrap();
    let signed = decode_transaction(&raw).unwrap();
    assert_eq!(signed.input.len(), 2);
    assert_eq!(signed.output[1].script_pubkey, BsvAddress::from_str(&change.address).unwrap().script_pubkey());
    assert_eq!(funding_spend_metadata(&funding).unwrap().inputs.len(), signed.input.len());
}

#[test]
fn test_funding_from_spend_rejects_corrupt_candidate_metadata_without_rewriting_it() {
    let original = hd_funding();
    let spend = funding_spend_metadata(&original).unwrap();
    let mut wrong_script = spend.clone();
    wrong_script.inputs[0].script_pubkey = "00".into();
    let mut duplicate_locator = spend.clone();
    duplicate_locator.inputs[1].locator = duplicate_locator.inputs[0].locator;
    let mut duplicate_outpoint = spend.clone();
    duplicate_outpoint.inputs.push(spend.inputs[0].clone());
    let mut unrepresentable_output = spend.clone();
    unrepresentable_output.inputs[0].vout = u32::MAX;
    let mut hardened_locator = spend.clone();
    hardened_locator.inputs[0].locator.index = 1 << 31;
    let mut wrong_change_script = spend;
    let mut change = allocated_change(7);
    change.script_pubkey = allocated_change(8).script_pubkey;
    wrong_change_script.change = Some(change);
    for corrupted in [
        wrong_script,
        duplicate_locator,
        duplicate_outpoint,
        unrepresentable_output,
        hardened_locator,
        wrong_change_script,
    ] {
        assert!(Funding::from_spend(original.policy.clone(), original.sender_address.clone(), original.amount, original.fee_rate, corrupted).is_err());
    }
}

#[test]
fn test_funding_from_spend_single_address_none_preserves_legacy_payload_review_and_raw() {
    let mut original = funding();
    original.addresses.truncate(1);
    original.utxos.truncate(1);
    original.utxos[0].value = 200000u64.into();
    let spend = funding_spend_metadata(&original).unwrap();
    let funding = Funding::from_spend(original.policy.clone(), original.sender_address.clone(), original.amount, original.fee_rate, spend).unwrap();
    assert!(serde_json::to_vec(&funding).unwrap() == serde_json::to_vec(&original).unwrap());
    let original_review = review_funding(&original).unwrap();
    let review = review_funding(&funding).unwrap();
    assert_eq!(review, original_review);
    let original_raw = sign_funding_with_keys(&original, &original_review, |_| Ok(Zeroizing::new(vec![9; 32]))).unwrap();
    let raw = sign_funding_with_keys(&funding, &review, |_| Ok(Zeroizing::new(vec![9; 32]))).unwrap();
    assert!(raw == original_raw);
    assert_eq!(funding.change, None);
    assert_eq!(funding.addresses.len(), 1);
}

#[test]
fn test_funding_from_spend_does_not_allocate_an_unused_change_candidate() {
    let original = hd_funding();
    let mut spend = funding_spend_metadata(&original).unwrap();
    let change = allocated_change(7);
    spend.change = Some(change.clone());
    let descriptor = policy_descriptor(&original.policy).unwrap();
    let output_size = 8 + 1 + hex::decode(descriptor.locking_script).unwrap().len() as u64;
    let fee_without_change = ((8 + 1 + 1 + 2 * 148 + output_size) * original.fee_rate).div_ceil(1000);
    let funding = Funding::from_spend(original.policy, original.sender_address, 120000 - fee_without_change, original.fee_rate, spend).unwrap();
    assert_eq!(funding.change, None);
    assert_eq!(funding.addresses.len(), 2);
    assert_eq!(funding.addresses.iter().filter(|owner| owner.address == change.address).count(), 0);
    assert_eq!(review_funding(&funding).unwrap().change, 0);
}
