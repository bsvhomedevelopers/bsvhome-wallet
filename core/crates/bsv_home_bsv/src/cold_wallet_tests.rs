use std::str::FromStr;

use super::*;
use bitcoin::{
    Amount, Network, OutPoint, PublicKey, ScriptBuf, Sequence, TxIn, TxOut, Witness,
    absolute::LockTime,
    bip32::{DerivationPath, Xpriv, Xpub},
    script::{Builder, PushBytesBuf},
    secp256k1::ecdsa::Signature,
    transaction::Version,
};

use bsv_home_bitcoin::signer::fork_id_signature_hash;
use num_bigint::BigUint;

fn fixture() -> (Request, Vec<(String, Vec<u8>)>) {
    let secp = Secp256k1::new();
    let master = Xpriv::new_master(Network::Bitcoin, &[7; 64]).unwrap();
    let root = master.derive_priv(&secp, &DerivationPath::from_str(ACCOUNT_PATH).unwrap()).unwrap();
    let xpub = Xpub::from_priv(&secp, &root).to_string();
    let first = BsvAddress::from_public_key(
        &Xpub::from_priv(&secp, &root)
            .derive_pub(&secp, &[ChildNumber::Normal { index: 0 }, ChildNumber::Normal { index: 0 }])
            .unwrap()
            .public_key
            .serialize(),
    )
    .unwrap()
    .encode();
    let account = Account {
        chain: Chain::BitcoinSv,
        address: first,
        derivation_path: format!("{ACCOUNT_PATH}/0/0"),
        extended_public_key: Some(xpub),
    };
    let pairing = Pairing::from_account(&account).unwrap();
    let addresses = [pairing.address(0, 0).unwrap(), pairing.address(0, 1).unwrap(), pairing.address(1, 0).unwrap()].to_vec();
    let previous = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::from_bytes(vec![1, 7]),
            sequence: Sequence::MAX,
            witness: Witness::default(),
        }],
        output: addresses
            .iter()
            .take(2)
            .map(|child| TxOut {
                value: Amount::from_sat(50_000),
                script_pubkey: BsvAddress::from_str(&child.address).unwrap().script_pubkey(),
            })
            .collect(),
    };
    let utxos = addresses
        .iter()
        .take(2)
        .enumerate()
        .map(|(index, child)| UTXO {
            transaction_id: previous.compute_txid().to_string(),
            vout: index as i32,
            value: 50_000u64.into(),
            address: child.address.clone(),
        })
        .collect();
    let mut request = Request::prepare(&account, pairing.address(0, 8).unwrap().address, 75_000, 1000, utxos, addresses.clone(), 0).unwrap();
    for input in &mut request.inputs {
        input.previous_transaction = hex::encode(serialize(&previous));
    }
    let keys = addresses
        .iter()
        .take(2)
        .map(|child| {
            let key = root
                .derive_priv(&secp, &[ChildNumber::Normal { index: child.branch }, ChildNumber::Normal { index: child.index }])
                .unwrap();
            (child.address.clone(), key.private_key.secret_bytes().to_vec())
        })
        .collect();
    (request, keys)
}

#[test]
fn test_cold_wallet_roundtrip_and_derivation() {
    let (request, keys) = fixture();
    let review = request.review(&request.pairing).unwrap();
    assert_eq!(review.amount + review.change + review.fee, 100_000);
    assert_eq!(review.input_count, 2);
    assert_eq!(review.change_address, Some(request.pairing.address(1, 0).unwrap().address));
    let refs = keys.iter().map(|(address, key)| (address.as_str(), key.as_slice())).collect::<Vec<_>>();
    let signed = request.sign(&review, &refs).unwrap();
    assert_eq!(
        request.transaction_id(&review, &signed).unwrap(),
        decode_transaction(&signed.raw_transaction).unwrap().compute_txid().to_string()
    );
    assert_eq!(request.pairing.account().unwrap().address, request.pairing.first_address);
    assert!(request.pairing.address(2, 0).is_err());
    assert!(request.pairing.address(0, 10_000).is_err());
    for mutate in [
        |p: &mut Pairing| p.network = "testnet".into(),
        |p: &mut Pairing| p.account_path = "m/44'/145'/0'".into(),
        |p: &mut Pairing| p.wallet_id = "00".repeat(32),
        |p: &mut Pairing| p.first_address = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT".into(),
        |p: &mut Pairing| p.version = 2,
    ] {
        let mut pairing = request.pairing.clone();
        mutate(&mut pairing);
        assert!(pairing.validate().is_err());
    }
}

#[test]
fn test_cold_wallet_rejects_previous_transaction_and_derivation_tampering() {
    let (request, _) = fixture();
    for mutate in [
        |r: &mut Request| r.inputs[0].branch = 1,
        |r: &mut Request| r.inputs[0].index += 1,
        |r: &mut Request| r.change_index = Some(1),
        |r: &mut Request| r.change_index = None,
        |r: &mut Request| r.amount += 1,
        |r: &mut Request| r.recipient = r.pairing.first_address.clone(),
        |r: &mut Request| r.inputs[0].previous_transaction = r.inputs[0].previous_transaction.replace("50c3000000000000", "51c3000000000000"),
        |r: &mut Request| r.inputs.push(r.inputs[0].clone()),
    ] {
        let mut changed = request.clone();
        mutate(&mut changed);
        assert!(changed.review(&request.pairing).is_err());
    }
    let mut changed = request.clone();
    let mut previous = decode_transaction(&changed.inputs[0].previous_transaction).unwrap();
    previous.output[0].script_pubkey = previous.output[1].script_pubkey.clone();
    changed.inputs[0].previous_transaction = hex::encode(serialize(&previous));
    assert!(changed.review(&request.pairing).is_err());
    let mut pairing = request.pairing.clone();
    pairing.xpub = "xpub-invalid".into();
    assert!(request.review(&pairing).is_err());
}

#[test]
fn test_cold_wallet_rejects_review_and_signed_transaction_tampering() {
    let (request, keys) = fixture();
    let review = request.review(&request.pairing).unwrap();
    let refs = keys.iter().map(|(address, key)| (address.as_str(), key.as_slice())).collect::<Vec<_>>();
    let signed = request.sign(&review, &refs).unwrap();
    let mut changed = review.clone();
    changed.fee += 1;
    assert!(request.sign(&changed, &refs).is_err());
    let mut changed = review.clone();
    changed.commitment = "00".repeat(32);
    assert!(request.sign(&changed, &refs).is_err());
    for mutate in [
        |s: &mut Signed| s.transaction_id = "00".repeat(32),
        |s: &mut Signed| s.commitment = "00".repeat(32),
        |s: &mut Signed| s.kind = "other".into(),
    ] {
        let mut changed = signed.clone();
        mutate(&mut changed);
        assert!(request.transaction_id(&review, &changed).is_err());
    }
    for mutate in [
        |t: &mut Transaction| t.output[0].value = Amount::from_sat(75_001),
        |t: &mut Transaction| t.input[0].script_sig = t.input[1].script_sig.clone(),
        |t: &mut Transaction| t.input[0].script_sig = ScriptBuf::new(),
        |t: &mut Transaction| t.input[0].witness.push([1]),
    ] {
        let mut changed = signed.clone();
        let mut transaction = decode_transaction(&changed.raw_transaction).unwrap();
        mutate(&mut transaction);
        changed.transaction_id = transaction.compute_txid().to_string();
        changed.raw_transaction = hex::encode(serialize(&transaction));
        assert!(request.transaction_id(&review, &changed).is_err());
    }
    assert!(request.sign(&review, &[(keys[0].0.as_str(), &[9; 32])]).is_err());
}

#[test]
fn test_cold_wallet_qr_reassembly_bounds_duplicates_and_integrity() {
    let (request, _) = fixture();
    let payload = serde_json::to_string(&request).unwrap();
    let frames = qr_frames(&payload).unwrap();
    assert_eq!(qr_assemble(&frames[..1]).unwrap(), None);
    let mut repeated = frames.clone();
    repeated.reverse();
    repeated.push(frames[0].clone());
    assert_eq!(qr_assemble(&repeated).unwrap(), Some(payload));
    let other = qr_frames("{\"other\":true}").unwrap();
    let mut mixed = frames.clone();
    mixed.push(other[0].clone());
    assert!(qr_assemble(&mixed).is_err());
    for mutate in [
        |f: &mut Frame| f.data = "A".repeat(FRAME_BYTES + 1),
        |f: &mut Frame| f.total = MAX_FRAMES + 1,
        |f: &mut Frame| f.index = f.total,
        |f: &mut Frame| f.version = 2,
        |f: &mut Frame| f.data.replace_range(0..1, "!"),
    ] {
        let mut corrupted = frames.clone();
        let mut frame: Frame = serde_json::from_str(&corrupted[0]).unwrap();
        mutate(&mut frame);
        corrupted[0] = serde_json::to_string(&frame).unwrap();
        assert!(qr_assemble(&corrupted).is_err());
    }
    let mut conflict = frames.clone();
    let mut frame: Frame = serde_json::from_str(&frames[0]).unwrap();
    frame.data.replace_range(0..1, "A");
    conflict.push(serde_json::to_string(&frame).unwrap());
    assert!(qr_assemble(&conflict).is_err());
    assert!(decode_json::<Request>(&" ".repeat(MAX_PAYLOAD + 1)).is_err());
}

#[test]
fn test_cold_wallet_rejects_duplicate_outpoints_extra_outputs_invalid_amounts_and_excessive_fees() {
    let (request, _) = fixture();
    for mutate in [
        |t: &mut Transaction| t.input[1].previous_output = t.input[0].previous_output,
        |t: &mut Transaction| t.input[0].previous_output.vout = 999,
        |t: &mut Transaction| t.output.push(t.output[0].clone()),
        |t: &mut Transaction| t.output[0].value = Amount::from_sat(100_001),
        |t: &mut Transaction| t.output[1].value = Amount::ZERO,
        |t: &mut Transaction| t.output[1].script_pubkey = t.output[0].script_pubkey.clone(),
        |t: &mut Transaction| t.lock_time = LockTime::from_consensus(1),
    ] {
        let mut changed = request.clone();
        let mut transaction = decode_transaction(&changed.unsigned_transaction).unwrap();
        mutate(&mut transaction);
        changed.unsigned_transaction = hex::encode(serialize(&transaction));
        assert!(changed.review(&request.pairing).is_err());
    }
    let mut excessive = request.clone();
    let mut previous = decode_transaction(&excessive.inputs[0].previous_transaction).unwrap();
    previous.output[0].value = Amount::from_sat(MAX_FEE + 100_000);
    let hash = previous.compute_txid();
    for input in &mut excessive.inputs {
        input.previous_transaction = hex::encode(serialize(&previous));
    }
    let mut transaction = decode_transaction(&excessive.unsigned_transaction).unwrap();
    for input in &mut transaction.input {
        input.previous_output.txid = hash;
    }
    excessive.unsigned_transaction = hex::encode(serialize(&transaction));
    assert!(excessive.review(&request.pairing).is_err());
    let mut unknown = serde_json::to_value(&request).unwrap();
    unknown.as_object_mut().unwrap().insert("unknown".into(), serde_json::Value::Bool(true));
    assert!(decode_json::<Request>(&serde_json::to_string(&unknown).unwrap()).is_err());
    let mut hardened = request.clone();
    hardened.inputs[0].index = 0x8000_0000;
    assert!(hardened.review(&request.pairing).is_err());
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SdkFixture {
    vectors: Vec<SdkVector>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SdkVector {
    account_path: String,
    pairing: Pairing,
    request: Request,
    review: Review,
    signed: Signed,
    digests: Vec<String>,
    qr_frames: Vec<String>,
}

#[test]
fn test_cold_wallet_sdk_exact_pairing_commitment_qr_fork_id_digests_and_signed_bytes() {
    let fixture: SdkFixture = serde_json::from_str(include_str!("../testdata/bsv-cold-sdk-vectors.json")).unwrap();
    for vector in fixture.vectors {
        let account = Xpriv::new_master(Network::Bitcoin, &[7; 64])
            .unwrap()
            .derive_priv(&Secp256k1::new(), AccountPath::parse(&vector.account_path).unwrap().derivation())
            .unwrap();
        assert_eq!(Xpub::from_priv(&Secp256k1::new(), &account).to_string(), vector.pairing.xpub);
        assert_eq!(vector.request.pairing, vector.pairing);
        assert_eq!(Pairing::from_account(&vector.pairing.account().unwrap()).unwrap(), vector.pairing);
        assert_eq!(vector.request.review(&vector.pairing).unwrap(), vector.review);
        let payload = serde_json::to_string(&vector.request).unwrap();
        assert_eq!(qr_frames(&payload).unwrap(), vector.qr_frames);
        assert_eq!(qr_assemble(&vector.qr_frames).unwrap(), Some(payload));
        let transaction = decode_transaction(&vector.request.unsigned_transaction).unwrap();
        let plan = vector.request.plan(&vector.pairing).unwrap();
        let digests = plan
            .inputs
            .iter()
            .enumerate()
            .map(|(index, input)| hex::encode(fork_id_signature_hash(&transaction, index, &input.script_pubkey, input.value.to_sat()).unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(digests, vector.digests);
        let keys = vector
            .request
            .addresses()
            .unwrap()
            .into_iter()
            .map(|address| {
                let key = account
                    .derive_priv(
                        &Secp256k1::new(),
                        &[ChildNumber::Normal { index: address.branch }, ChildNumber::Normal { index: address.index }],
                    )
                    .unwrap();
                (address.address, key.private_key.secret_bytes())
            })
            .collect::<Vec<_>>();
        let keys = keys.iter().map(|(address, key)| (address.as_str(), key.as_slice())).collect::<Vec<_>>();
        let signed = vector.request.sign(&vector.review, &keys).unwrap();
        assert_eq!(serde_json::to_string(&signed).unwrap(), serde_json::to_string(&vector.signed).unwrap());
        assert_eq!(vector.request.transaction_id(&vector.review, &vector.signed).unwrap(), vector.signed.transaction_id);
    }
}

#[test]
fn test_cold_wallet_rejects_high_s_even_when_transaction_id_is_updated() {
    let (request, keys) = fixture();
    let review = request.review(&request.pairing).unwrap();
    let keys = keys.iter().map(|(address, key)| (address.as_str(), key.as_slice())).collect::<Vec<_>>();
    let mut signed = request.sign(&review, &keys).unwrap();
    let mut transaction = decode_transaction(&signed.raw_transaction).unwrap();
    let instructions = transaction.input[0].script_sig.instructions_minimal().collect::<Result<Vec<_>, _>>().unwrap();
    let signature = instructions[0].push_bytes().unwrap().as_bytes();
    let public_key = PublicKey::from_slice(instructions[1].push_bytes().unwrap().as_bytes()).unwrap();
    let mut compact = Signature::from_der(&signature[..signature.len() - 1]).unwrap().serialize_compact();
    let order = BigUint::parse_bytes(b"fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141", 16).unwrap();
    let high = (order - BigUint::from_bytes_be(&compact[32..])).to_bytes_be();
    compact[32..].fill(0);
    compact[64 - high.len()..].copy_from_slice(&high);
    let mut signature = Signature::from_compact(&compact).unwrap().serialize_der().to_vec();
    signature.push(0x41);
    transaction.input[0].script_sig = Builder::new().push_slice(PushBytesBuf::try_from(signature).unwrap()).push_key(&public_key).into_script();
    signed.transaction_id = transaction.compute_txid().to_string();
    signed.raw_transaction = hex::encode(serialize(&transaction));
    assert!(request.transaction_id(&review, &signed).is_err());
}
