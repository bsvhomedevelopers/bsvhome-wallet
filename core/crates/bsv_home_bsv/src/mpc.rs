use std::collections::HashSet;

use bitcoin::{
    PublicKey, ScriptBuf, Transaction,
    blockdata::script::Instruction,
    consensus::encode::{deserialize, serialize},
    hashes::{Hash, sha256},
    secp256k1::{Message, Secp256k1, ecdsa::Signature},
};
use bsv_home_bitcoin::signer::fork_id_signature_hash;
use primitives::{Address, SignerError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{BsvAddress, multisig::ProposalInput};

const MAX_MONEY: u64 = 2_100_000_000_000_000;
const MAX_FEE: u64 = 1_000_000;
const MAX_HEX_BYTES: usize = 2_000_000;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Proposal {
    policy: Option<Value>,
    inputs: Vec<ProposalInput>,
    recipient: String,
    amount: u64,
    fee: u64,
    unsigned_transaction: String,
    application: Option<Value>,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Review {
    recipient: String,
    amount: u64,
    fee: u64,
    change: u64,
    digests: Vec<String>,
    unsigned_transaction: String,
    commitment: String,
}

struct Validated {
    transaction: Transaction,
    public_key: PublicKey,
    digests: Vec<[u8; 32]>,
    review: Review,
}

pub fn review_json(proposal_json: &str, public_key: &str, expected_recipient: &str, expected_amount: u64, maximum_fee: u64) -> Result<String, SignerError> {
    Ok(serde_json::to_string(
        &validate(proposal_json, public_key, expected_recipient, expected_amount, maximum_fee)?.review,
    )?)
}

pub fn verify_signed_json(
    proposal_json: &str,
    public_key: &str,
    expected_recipient: &str,
    expected_amount: u64,
    maximum_fee: u64,
    signed_raw: &str,
) -> Result<String, SignerError> {
    let validated = validate(proposal_json, public_key, expected_recipient, expected_amount, maximum_fee)?;
    let transaction = decode_transaction(signed_raw)?;
    let mut unsigned = transaction.clone();
    for input in &mut unsigned.input {
        input.script_sig = ScriptBuf::new();
    }
    if unsigned != validated.transaction {
        return SignerError::invalid_input_err("MPC signed transaction differs from reviewed transaction");
    }
    let secp = Secp256k1::verification_only();
    let public_bytes = validated.public_key.to_bytes();
    for (input, digest) in transaction.input.iter().zip(validated.digests) {
        let instructions = input.script_sig.instructions_minimal().collect::<Result<Vec<_>, _>>().map_err(SignerError::from_display)?;
        let pushes = instructions
            .iter()
            .map(|instruction| match instruction {
                Instruction::PushBytes(bytes) => Ok(bytes.as_bytes()),
                Instruction::Op(_) => SignerError::invalid_input_err("Invalid MPC signature script"),
            })
            .collect::<Result<Vec<_>, SignerError>>()?;
        if pushes.len() != 2 || pushes[0].last() != Some(&0x41) || pushes[1] != public_bytes {
            return SignerError::invalid_input_err("Invalid MPC signature scope or public key");
        }
        let der = &pushes[0][..pushes[0].len() - 1];
        let signature = Signature::from_der(der).map_err(SignerError::from_display)?;
        let mut normalized = signature;
        normalized.normalize_s();
        if signature != normalized || signature.serialize_der().as_ref() != der {
            return SignerError::invalid_input_err("Noncanonical MPC signature");
        }
        secp.verify_ecdsa(&Message::from_digest(digest), &signature, &validated.public_key.inner)
            .map_err(SignerError::from_display)?;
    }
    Ok(transaction.compute_txid().to_string())
}

fn validate(proposal_json: &str, public_key: &str, expected_recipient: &str, expected_amount: u64, maximum_fee: u64) -> Result<Validated, SignerError> {
    if proposal_json.len() > 2_100_000 || public_key.len() != 66 {
        return SignerError::invalid_input_err("MPC proposal exceeds limit");
    }
    let proposal: Proposal = serde_json::from_str(proposal_json)?;
    if proposal.policy.is_some()
        || proposal.application.is_some()
        || proposal.inputs.is_empty()
        || proposal.inputs.len() > 100
        || proposal.recipient != expected_recipient
        || proposal.amount != expected_amount
        || proposal.amount == 0
        || proposal.amount > MAX_MONEY
        || proposal.fee == 0
        || proposal.fee > MAX_FEE.min(maximum_fee)
    {
        return SignerError::invalid_input_err("MPC proposal does not match transfer intent");
    }
    let public_key = PublicKey::from_slice(&hex::decode(public_key).map_err(SignerError::from_display)?).map_err(SignerError::from_display)?;
    let own_script = BsvAddress::from_public_key(&public_key.to_bytes()).map_err(SignerError::from_display)?.script_pubkey();
    let recipient_script = BsvAddress::from_str(expected_recipient).map_err(SignerError::from_display)?.script_pubkey();
    let transaction = decode_transaction(&proposal.unsigned_transaction)?;
    if !recipient_script.is_p2pkh() || transaction.version.0 != 2 || transaction.lock_time.to_consensus_u32() != 0 || transaction.input.len() != proposal.inputs.len() {
        return SignerError::invalid_input_err("Unsupported MPC transaction");
    }
    let mut encoded_bytes = proposal.unsigned_transaction.len();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    let mut amounts = Vec::with_capacity(proposal.inputs.len());
    for (input, source) in transaction.input.iter().zip(&proposal.inputs) {
        encoded_bytes += source.transaction_hex.len();
        if encoded_bytes > MAX_HEX_BYTES || !seen.insert(input.previous_output) || input.sequence.0 != u32::MAX || !input.script_sig.is_empty() {
            return SignerError::invalid_input_err("Invalid MPC transaction input");
        }
        let previous = decode_transaction(&source.transaction_hex)?;
        let output = previous
            .output
            .get(source.output_index as usize)
            .ok_or_else(|| SignerError::invalid_input("MPC previous output missing"))?;
        if previous.compute_txid() != input.previous_output.txid
            || input.previous_output.vout != source.output_index
            || output.script_pubkey != own_script
            || output.value.to_sat() == 0
            || output.value.to_sat() > MAX_MONEY
        {
            return SignerError::invalid_input_err("MPC input does not belong to wallet");
        }
        total = total
            .checked_add(output.value.to_sat())
            .filter(|amount| *amount <= MAX_MONEY)
            .ok_or_else(|| SignerError::invalid_input("MPC amount overflow"))?;
        amounts.push(output.value);
    }
    let change = total.checked_sub(proposal.amount + proposal.fee).ok_or(SignerError::InsufficientFunds)?;
    if transaction.output.len() != if change > 0 { 2 } else { 1 }
        || transaction.output[0].value.to_sat() != proposal.amount
        || transaction.output[0].script_pubkey != recipient_script
        || (change > 0 && (transaction.output[1].value.to_sat() != change || transaction.output[1].script_pubkey != own_script))
    {
        return SignerError::invalid_input_err("MPC outputs differ from transfer intent");
    }
    let digests = amounts
        .iter()
        .enumerate()
        .map(|(index, amount)| fork_id_signature_hash(&transaction, index, &own_script, amount.to_sat()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut commitment = b"WowPay/MPC/review/v1\0".to_vec();
    commitment.extend_from_slice(&serialize(&transaction));
    commitment.extend_from_slice(&public_key.to_bytes());
    for amount in amounts {
        commitment.extend_from_slice(&amount.to_sat().to_le_bytes());
    }
    let review = Review {
        recipient: proposal.recipient,
        amount: proposal.amount,
        fee: proposal.fee,
        change,
        digests: digests.iter().map(hex::encode).collect(),
        unsigned_transaction: hex::encode(serialize(&transaction)),
        commitment: sha256::Hash::hash(&commitment).to_string(),
    };
    Ok(Validated {
        transaction,
        public_key,
        digests,
        review,
    })
}

fn decode_transaction(raw: &str) -> Result<Transaction, SignerError> {
    if raw.len() > MAX_HEX_BYTES {
        return SignerError::invalid_input_err("MPC transaction exceeds limit");
    }
    let bytes = hex::decode(raw).map_err(SignerError::from_display)?;
    let transaction: Transaction = deserialize(&bytes).map_err(SignerError::from_display)?;
    if transaction.input.iter().any(|input| !input.witness.is_empty()) || serialize(&transaction) != bytes {
        return SignerError::invalid_input_err("Invalid BSV transaction encoding");
    }
    Ok(transaction)
}

#[cfg(test)]
mod tests {
    use bitcoin::{
        Amount, OutPoint, Sequence, TxIn, TxOut, Witness,
        absolute::LockTime,
        blockdata::{script::Builder, transaction::Version},
        secp256k1::SecretKey,
    };

    use super::*;
    use bitcoin::script::PushBytesBuf;

    const TEST_PRIVATE_KEY: [u8; 32] = [1; 32];

    fn fixture() -> (Proposal, PublicKey, SecretKey) {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&TEST_PRIVATE_KEY).unwrap();
        let public = PublicKey::new(bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &secret));
        let own = BsvAddress::from_public_key(&public.to_bytes()).unwrap().encode();
        let own_script = BsvAddress::from_str(&own).unwrap().script_pubkey();
        let recipient = bitcoin::Address::p2pkh(bitcoin::PubkeyHash::from_byte_array([2; 20]), bitcoin::Network::Bitcoin).to_string();
        let previous = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::from_bytes(vec![1, 7]),
                sequence: Sequence::MAX,
                witness: Witness::default(),
            }],
            output: vec![
                TxOut {
                    value: Amount::from_sat(50_000),
                    script_pubkey: own_script.clone()
                };
                2
            ],
        };
        let transaction = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: (0..2)
                .map(|index| TxIn {
                    previous_output: OutPoint::new(previous.compute_txid(), index),
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::default(),
                })
                .collect(),
            output: vec![
                TxOut {
                    value: Amount::from_sat(75_000),
                    script_pubkey: BsvAddress::from_str(&recipient).unwrap().script_pubkey(),
                },
                TxOut {
                    value: Amount::from_sat(24_500),
                    script_pubkey: own_script,
                },
            ],
        };
        (
            Proposal {
                policy: None,
                inputs: (0..2)
                    .map(|index| ProposalInput {
                        transaction_hex: hex::encode(serialize(&previous)),
                        output_index: index,
                    })
                    .collect(),
                recipient,
                amount: 75_000,
                fee: 500,
                unsigned_transaction: hex::encode(serialize(&transaction)),
                application: None,
            },
            public,
            secret,
        )
    }

    fn signed(proposal: &Proposal, public: &PublicKey, secret: &SecretKey) -> Transaction {
        let validated = validate(
            &serde_json::to_string(proposal).unwrap(),
            &public.to_string(),
            &proposal.recipient,
            proposal.amount,
            proposal.fee,
        )
        .unwrap();
        let mut transaction = validated.transaction;
        for (input, digest) in transaction.input.iter_mut().zip(validated.digests) {
            let mut bytes = Secp256k1::new().sign_ecdsa(&Message::from_digest(digest), secret).serialize_der().to_vec();
            bytes.push(0x41);
            input.script_sig = Builder::new().push_slice(PushBytesBuf::try_from(bytes).unwrap()).push_key(public).into_script();
        }
        transaction
    }

    #[test]
    fn test_review_json_and_verify_signed_json() {
        let (proposal, public, secret) = fixture();
        let json = serde_json::to_string(&proposal).unwrap();
        let review: Review = serde_json::from_str(&review_json(&json, &public.to_string(), &proposal.recipient, 75_000, 500).unwrap()).unwrap();
        assert_eq!((review.amount, review.fee, review.change), (75_000, 500, 24_500));
        assert_eq!(
            review.digests,
            vec![
                "109300ae667a6f714f7f2b331c42a85eebbaedea1a0f36c46986a5b966b307f3",
                "a2086a7d299a8be00c1ffde8e6ec912211a1239dbd4e03d5c3682a6e569ec3ff"
            ]
        );
        let sdk_signed = "02000000023ee1d3f3efec9fa761918df6cbd141012a3539a36b9afc174afd7827feb76bef000000006a47304402201a79a33a38455336e4b2e3acce28dfcfe176f12d80560ce87e6517847a7138aa02207614940a17b0aad1dd5be0f7221bdae8e82fa4c05ec8c80f9503c365e6bf8c2d4121031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078fffffffff3ee1d3f3efec9fa761918df6cbd141012a3539a36b9afc174afd7827feb76bef010000006a47304402200ced06ab8026f36fdf58f444dd076569f15dc03b3aa250d93b3e17a6552cf57702205131b50df2d7d9a204bea7393874eea22683d0633130c4d5113dae0915e599214121031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078fffffffff02f8240100000000001976a914020202020202020202020202020202020202020288acb45f0000000000001976a91479b000887626b294a914501a4cd226b58b23598388ac00000000";
        assert_eq!(
            verify_signed_json(&json, &public.to_string(), &proposal.recipient, 75_000, 500, sdk_signed).unwrap(),
            "ee5468f7041b603166c7e07cef9a41739a0520fa6b5462fd97351c070e9299fd"
        );
        let signed = signed(&proposal, &public, &secret);
        assert_eq!(hex::encode(serialize(&signed)), sdk_signed);
        assert_eq!(
            verify_signed_json(&json, &public.to_string(), &proposal.recipient, 75_000, 500, &hex::encode(serialize(&signed))).unwrap(),
            signed.compute_txid().to_string()
        );
    }

    #[test]
    fn test_review_rejects_intent_input_output_and_fee_tampering() {
        let (proposal, public, _) = fixture();
        let review = |p: &Proposal| review_json(&serde_json::to_string(p).unwrap(), &public.to_string(), &proposal.recipient, proposal.amount, 500);
        let changes: [fn(&mut Proposal); 8] = [
            |p| p.amount += 1,
            |p| p.fee += 1,
            |p| p.recipient = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT".into(),
            |p| p.inputs[0].output_index = 1,
            |p| p.inputs[0].transaction_hex.replace_range(0..2, "01"),
            |p| p.inputs.push(p.inputs[0].clone()),
            |p| p.policy = Some(serde_json::json!({"threshold":2})),
            |p| p.application = Some(serde_json::json!({"version":1})),
        ];
        for change in changes {
            let mut tampered = proposal.clone();
            change(&mut tampered);
            assert!(review(&tampered).is_err());
        }
        let mut transaction = decode_transaction(&proposal.unsigned_transaction).unwrap();
        let changes: [fn(&mut Transaction); 6] = [
            |t| t.input[1].previous_output = t.input[0].previous_output,
            |t| t.input[0].sequence = Sequence::ENABLE_RBF_NO_LOCKTIME,
            |t| t.output[1].script_pubkey = t.output[0].script_pubkey.clone(),
            |t| t.output[0].value = Amount::from_sat(75_001),
            |t| t.output.push(t.output[1].clone()),
            |t| t.output[1].value = Amount::from_sat(24_499),
        ];
        for change in changes {
            let mut tampered = transaction.clone();
            change(&mut tampered);
            let mut changed = proposal.clone();
            changed.unsigned_transaction = hex::encode(serialize(&tampered));
            assert!(review(&changed).is_err());
        }
        transaction.version = Version::ONE;
        let mut changed = proposal.clone();
        changed.unsigned_transaction = hex::encode(serialize(&transaction));
        assert!(review(&changed).is_err());
    }

    #[test]
    fn test_verify_rejects_wrong_scope_key_signature_and_outputs() {
        let (proposal, public, secret) = fixture();
        let json = serde_json::to_string(&proposal).unwrap();
        let transaction = signed(&proposal, &public, &secret);
        let verify = |t: &Transaction| verify_signed_json(&json, &public.to_string(), &proposal.recipient, proposal.amount, 500, &hex::encode(serialize(t)));
        let changes: [fn(&mut Transaction); 5] = [
            |t| t.input[0].script_sig = ScriptBuf::new(),
            |t| {
                let mut bytes = t.input[0].script_sig.as_bytes().to_vec();
                let end = bytes[0] as usize;
                bytes[end] = 1;
                t.input[0].script_sig = ScriptBuf::from_bytes(bytes);
            },
            |t| {
                let mut bytes = t.input[0].script_sig.as_bytes().to_vec();
                bytes[8] ^= 1;
                t.input[0].script_sig = ScriptBuf::from_bytes(bytes);
            },
            |t| {
                let mut bytes = t.input[0].script_sig.as_bytes().to_vec();
                let last = bytes.len() - 1;
                bytes[last] ^= 1;
                t.input[0].script_sig = ScriptBuf::from_bytes(bytes);
            },
            |t| t.output[0].value = Amount::from_sat(75_001),
        ];
        for change in changes {
            let mut tampered = transaction.clone();
            change(&mut tampered);
            assert!(verify(&tampered).is_err());
        }
    }
    #[test]
    fn test_verify_rejects_high_s_nonminimal_push_and_witness() {
        let (proposal, public, secret) = fixture();
        let json = serde_json::to_string(&proposal).unwrap();
        let transaction = signed(&proposal, &public, &secret);
        let verify = |t: &Transaction| verify_signed_json(&json, &public.to_string(), &proposal.recipient, proposal.amount, 500, &hex::encode(serialize(t)));
        let script = transaction.input[0].script_sig.as_bytes();
        let signature = Signature::from_der(&script[1..script[0] as usize]).unwrap();
        let mut compact = signature.serialize_compact();
        let order = num_bigint::BigUint::parse_bytes(b"fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141", 16).unwrap();
        let high_s = (order - num_bigint::BigUint::from_bytes_be(&compact[32..])).to_bytes_be();
        compact[32..].fill(0);
        compact[64 - high_s.len()..].copy_from_slice(&high_s);
        let mut high_der = Signature::from_compact(&compact).unwrap().serialize_der().to_vec();
        high_der.push(0x41);
        let mut high = transaction.clone();
        high.input[0].script_sig = Builder::new().push_slice(PushBytesBuf::try_from(high_der).unwrap()).push_key(&public).into_script();
        assert!(verify(&high).is_err());
        let mut nonminimal = transaction.clone();
        nonminimal.input[0].script_sig = ScriptBuf::from_bytes([vec![0x4c, script[0]], script[1..].to_vec()].concat());
        assert!(verify(&nonminimal).is_err());
        let mut witness = transaction;
        witness.input[0].witness = Witness::from_slice(&[vec![0]]);
        assert!(verify(&witness).is_err());
    }

    #[test]
    fn test_review_rejects_unknown_fields_limits_and_unaffordable_fees() {
        let (proposal, public, _) = fixture();
        let json = serde_json::to_string(&proposal).unwrap();
        assert!(review_json(&json, &public.to_string(), &proposal.recipient, proposal.amount, 0).is_err());
        assert!(review_json(&json, &public.to_string(), &proposal.recipient, proposal.amount + 1, 500).is_err());
        let mut unknown = serde_json::to_value(&proposal).unwrap();
        unknown["extra"] = serde_json::json!(true);
        assert!(review_json(&unknown.to_string(), &public.to_string(), &proposal.recipient, proposal.amount, 500).is_err());
        let mut empty = proposal.clone();
        empty.inputs.clear();
        assert!(review_json(&serde_json::to_string(&empty).unwrap(), &public.to_string(), &proposal.recipient, proposal.amount, 500).is_err());
        let mut too_large = proposal.clone();
        too_large.amount = MAX_MONEY;
        assert!(review_json(&serde_json::to_string(&too_large).unwrap(), &public.to_string(), &proposal.recipient, too_large.amount, 500).is_err());
    }
}
