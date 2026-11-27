use bitcoin::{
    Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
    absolute::LockTime,
    consensus::{deserialize, serialize},
    hashes::Hash,
    script::{Builder, Instruction, PushBytesBuf},
    secp256k1::{PublicKey, Secp256k1, SecretKey, ecdsa::Signature},
    transaction::Version,
};
use bsv_home_bitcoin::signer::fork_id_signature_hash;
use num_bigint::BigUint;
use serde::Deserialize;

use super::{
    MultisigConfig, PartialSignature, PreviousOutput, finalized_from_raw, merge_signatures, parse_locking_script, plan_transfer, sign_plan, validate_finalized, validate_plan,
    verify_partial,
};

fn configuration(threshold: u8, count: u8) -> MultisigConfig {
    MultisigConfig {
        threshold,
        public_keys: (1..=count)
            .map(|value| hex::encode(PublicKey::from_secret_key(&Secp256k1::new(), &SecretKey::from_slice(&[value; 32]).unwrap()).serialize()))
            .collect(),
    }
}

fn sources(configuration: &MultisigConfig) -> Vec<PreviousOutput> {
    let transaction = Transaction {
        version: Version::ONE,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::new(Txid::from_byte_array([4; 32]), 0),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![
            TxOut {
                value: Amount::from_sat(10_000),
                script_pubkey: configuration.locking_script().unwrap(),
            },
            TxOut {
                value: Amount::from_sat(20_000),
                script_pubkey: configuration.locking_script().unwrap(),
            },
        ],
    };
    transaction
        .output
        .iter()
        .enumerate()
        .map(|(index, output)| PreviousOutput {
            transaction_id: transaction.compute_txid().to_string(),
            vout: index as u32,
            value: output.value.to_sat(),
            raw_transaction: hex::encode(serialize(&transaction)),
        })
        .collect()
}

fn recipient() -> String {
    "76a91479b000887626b294a914501a4cd226b58b23598388ac".to_string()
}

#[test]
fn test_supported_script_rules() {
    for (threshold, count) in [(2, 2), (2, 3), (3, 3), (2, 4), (5, 5)] {
        let configuration = configuration(threshold, count);
        let parsed = parse_locking_script(&hex::encode(configuration.locking_script().unwrap())).unwrap();
        assert_eq!(parsed.threshold, threshold);
        assert_eq!(parsed.locking_script().unwrap(), configuration.locking_script().unwrap());
        let mut reverse = configuration.clone();
        reverse.public_keys.reverse();
        assert_eq!(reverse.script_id().unwrap(), configuration.script_id().unwrap());
    }
    for (threshold, count) in [(1, 2), (2, 1), (3, 2), (2, 6)] {
        assert!(configuration(threshold, count).validate().is_err());
    }
    let mut duplicate = configuration(2, 3);
    duplicate.public_keys[1] = duplicate.public_keys[0].clone();
    assert!(duplicate.validate().is_err());
    let mut uppercase = configuration(2, 3);
    uppercase.public_keys[0] = uppercase.public_keys[0].to_uppercase();
    assert!(uppercase.validate().is_err());
}

#[test]
fn test_sign_merge_all_subsets() {
    for (threshold, count, subsets) in [(2, 2, vec![vec![1, 2]]), (2, 3, vec![vec![1, 2], vec![1, 3], vec![2, 3]]), (3, 3, vec![vec![1, 2, 3]])] {
        let configuration = configuration(threshold, count);
        let plan = plan_transfer(&configuration, &sources(&configuration), &recipient(), Some(25_000), 100).unwrap();
        assert_eq!(plan.previous_outputs.len(), 2);
        let mut ids = Vec::new();
        for subset in subsets {
            let signatures = subset.iter().map(|value| sign_plan(&configuration, &plan, &[*value; 32]).unwrap()).collect::<Vec<_>>();
            let finalized = merge_signatures(&configuration, &plan, &signatures).unwrap();
            validate_finalized(&configuration, &plan, &finalized).unwrap();
            let mut reversed = signatures.clone();
            reversed.reverse();
            assert_eq!(merge_signatures(&configuration, &plan, &reversed).unwrap(), finalized);
            ids.push(finalized.transaction_id);
        }
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), if count == 3 && threshold == 2 { 3 } else { 1 });
    }
}

#[test]
fn test_previous_evidence_and_plan_tampering() {
    let configuration = configuration(2, 3);
    let original = plan_transfer(&configuration, &sources(&configuration), &recipient(), Some(5000), 100).unwrap();
    let mut amount = original.clone();
    amount.previous_outputs[0].value += 1;
    assert!(validate_plan(&configuration, &amount).is_err());
    let mut transaction_id = original.clone();
    transaction_id.previous_outputs[0].transaction_id = "01".repeat(32);
    assert!(validate_plan(&configuration, &transaction_id).is_err());
    let mut output = original.clone();
    let mut transaction: Transaction = deserialize(&hex::decode(&output.unsigned_transaction).unwrap()).unwrap();
    transaction.output[1].script_pubkey = ScriptBuf::from_bytes(hex::decode(recipient()).unwrap());
    output.unsigned_transaction = hex::encode(serialize(&transaction));
    assert!(validate_plan(&configuration, &output).is_err());
    let mut version = original.clone();
    transaction = deserialize(&hex::decode(&version.unsigned_transaction).unwrap()).unwrap();
    transaction.version = Version::TWO;
    version.unsigned_transaction = hex::encode(serialize(&transaction));
    assert!(validate_plan(&configuration, &version).is_err());
    let mut fee = original.clone();
    fee.fee += 1;
    assert!(validate_plan(&configuration, &fee).is_err());
}

#[test]
fn test_threshold_is_required_per_input() {
    let configuration = configuration(2, 3);
    let plan = plan_transfer(&configuration, &sources(&configuration), &recipient(), Some(25_000), 100).unwrap();
    let first = sign_plan(&configuration, &plan, &[1; 32]).unwrap();
    let mut second = sign_plan(&configuration, &plan, &[2; 32]).unwrap();
    second.signatures.pop();
    verify_partial(&configuration, &plan, &second).unwrap();
    assert!(merge_signatures(&configuration, &plan, &[first.clone(), second]).is_err());
    assert!(merge_signatures(&configuration, &plan, &[first.clone(), first]).is_err());
    assert!(sign_plan(&configuration, &plan, &[4; 32]).is_err());
}

#[test]
fn test_partial_signature_rejects_wrong_context_and_sighash() {
    let configuration = configuration(2, 3);
    let plan = plan_transfer(&configuration, &sources(&configuration), &recipient(), Some(25_000), 100).unwrap();
    let original = sign_plan(&configuration, &plan, &[1; 32]).unwrap();
    let mut wrong_input = original.clone();
    wrong_input.signatures[0].input_index = 1;
    wrong_input.signatures.truncate(1);
    assert!(verify_partial(&configuration, &plan, &wrong_input).is_err());
    let mut wrong_hash = original.clone();
    let signature = &mut wrong_hash.signatures[0].signature;
    signature.truncate(signature.len() - 2);
    signature.push_str("01");
    assert!(verify_partial(&configuration, &plan, &wrong_hash).is_err());
    let alternative = plan_transfer(&configuration, &sources(&configuration), &recipient(), Some(26_000), 100).unwrap();
    assert!(verify_partial(&configuration, &alternative, &original).is_err());
}

#[test]
fn test_high_s_and_unlocking_script_mutations_are_rejected() {
    let configuration = configuration(2, 3);
    let plan = plan_transfer(&configuration, &sources(&configuration), &recipient(), Some(25_000), 100).unwrap();
    let mut first = sign_plan(&configuration, &plan, &[1; 32]).unwrap();
    let second = sign_plan(&configuration, &plan, &[2; 32]).unwrap();
    let finalized = merge_signatures(&configuration, &plan, &[first.clone(), second]).unwrap();
    let bytes = hex::decode(&first.signatures[0].signature).unwrap();
    let signature = Signature::from_der(&bytes[..bytes.len() - 1]).unwrap();
    let mut compact = signature.serialize_compact();
    let order = BigUint::from_bytes_be(&hex::decode("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141").unwrap());
    let high = (order - BigUint::from_bytes_be(&compact[32..])).to_bytes_be();
    compact[32..].copy_from_slice(&high);
    let mut high_signature = Signature::from_compact(&compact).unwrap().serialize_der().to_vec();
    high_signature.push(0x41);
    first.signatures[0].signature = hex::encode(high_signature);
    assert!(verify_partial(&configuration, &plan, &first).is_err());
    let original: Transaction = deserialize(&hex::decode(&finalized.raw_transaction).unwrap()).unwrap();
    let signatures = original.input[0]
        .script_sig
        .instructions()
        .skip(1)
        .map(|instruction| match instruction.unwrap() {
            Instruction::PushBytes(bytes) => bytes.as_bytes().to_vec(),
            Instruction::Op(_) => panic!(),
        })
        .collect::<Vec<_>>();
    for dummy in [None, Some(1)] {
        let mut transaction = original.clone();
        let mut builder = match dummy {
            None => Builder::new(),
            Some(value) => Builder::new().push_int(value),
        };
        for signature in &signatures {
            builder = builder.push_slice(PushBytesBuf::try_from(signature.clone()).unwrap());
        }
        transaction.input[0].script_sig = builder.into_script();
        assert!(finalized_from_raw(&configuration, &plan, &hex::encode(serialize(&transaction))).is_err());
    }
    let mut reversed = original.clone();
    let mut builder = Builder::new().push_int(0);
    for signature in signatures.iter().rev() {
        builder = builder.push_slice(PushBytesBuf::try_from(signature.clone()).unwrap());
    }
    reversed.input[0].script_sig = builder.into_script();
    assert!(finalized_from_raw(&configuration, &plan, &hex::encode(serialize(&reversed))).is_err());
}

#[derive(Deserialize)]
struct SdkVectors {
    sdk: String,
    vectors: Vec<SdkVector>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SdkVector {
    threshold: u8,
    count: u8,
    public_keys: Vec<String>,
    locking_script: String,
    previous: String,
    unsigned: String,
    fee: u64,
    digests: Vec<String>,
    signatures: Vec<PartialSignature>,
    finalized: String,
    transaction_id: String,
    validation: Vec<bool>,
    subset_validation: Vec<Vec<bool>>,
    mutation_validation: Vec<bool>,
}

#[test]
fn test_official_sdk_script_signature_and_finalization_vectors() {
    let vectors: SdkVectors = serde_json::from_str(include_str!("../../testdata/multisig_sdk_vectors.json")).unwrap();
    assert_eq!(vectors.sdk, "@bsv/sdk@2.0.0");
    for vector in vectors.vectors {
        let configuration = configuration(vector.threshold, vector.count);
        assert_eq!(configuration.public_keys, vector.public_keys);
        let script = configuration.locking_script().unwrap();
        assert_eq!(hex::encode(script.as_bytes()), vector.locking_script);
        let previous_outputs = sources(&configuration);
        assert_eq!(previous_outputs[0].raw_transaction, vector.previous);
        let plan = plan_transfer(&configuration, &previous_outputs, &recipient(), Some(25_000), 100).unwrap();
        assert_eq!(plan.unsigned_transaction, vector.unsigned);
        assert_eq!(plan.fee, vector.fee);
        let transaction: Transaction = deserialize(&hex::decode(&plan.unsigned_transaction).unwrap()).unwrap();
        for (index, previous) in previous_outputs.iter().enumerate() {
            assert_eq!(
                hex::encode(fork_id_signature_hash(&transaction, index, script.as_script(), previous.value).unwrap()),
                vector.digests[index]
            );
        }
        for value in 1..=vector.count {
            let signature = sign_plan(&configuration, &plan, &[value; 32]).unwrap();
            assert_eq!(vector.signatures.iter().find(|partial| partial.public_key == signature.public_key), Some(&signature));
        }
        let finalized = merge_signatures(&configuration, &plan, &vector.signatures).unwrap();
        assert_eq!(finalized.raw_transaction, vector.finalized);
        assert_eq!(finalized.transaction_id, vector.transaction_id);
        assert_eq!(vector.validation, [true, true]);
        assert_eq!(
            vector.subset_validation,
            vec![vec![true, true]; if vector.threshold == 2 && vector.count == 3 { 3 } else { 1 }]
        );
        assert_eq!(vector.mutation_validation, [false; 5]);
    }
}
