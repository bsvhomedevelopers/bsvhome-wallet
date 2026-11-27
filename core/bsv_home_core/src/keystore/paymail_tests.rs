use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use bsv_home_bsv::signer::BsvChainSigner;
use bsv_home_derivation::{derive_account_from_private_key, derive_private_key_from_mnemonic, derive_wallet_id_from_account};
use bsv_home_keystore::{FileKeystore, Keystore};
use primitives::testkit::{ABANDON_PHRASE, signer_mock::TEST_PRIVATE_KEY};
use primitives::{Chain, WalletType};
use tempfile::{TempDir, tempdir};
use zeroize::Zeroizing;

use super::{SECP256K1_ORDER, derive_identity_candidate, derive_identity_key, derive_identity_key_from_candidates, identity_candidate_is_valid};
use crate::BsvHomeCoreError;
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore};

const TEST_PASSWORD: &[u8] = b"paymail-signing-fixture";
const TEST_ADDRESS: &str = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD";
const TEST_IDENTITY_PUBLIC_KEY: &str = "025d484df18ca9407c27f6d7a71067b5c92faa71f90d917d85a42b147fadd72bdb";

struct Context {
    _directory: TempDir,
    keystore: Arc<BsvHomeKeystore>,
    keystore_id: String,
    address: String,
}

impl Context {
    fn new(import: BsvHomeImportType) -> Self {
        let directory = tempdir().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let stored = keystore.create_store(import, TEST_PASSWORD.to_vec()).unwrap();
        Self {
            _directory: directory,
            keystore,
            keystore_id: stored.keystore_id,
            address: stored.accounts[0].address.clone(),
        }
    }

    fn from_private_key(private_key: &[u8]) -> Self {
        let directory = tempdir().unwrap();
        let account = derive_account_from_private_key(private_key, Chain::BitcoinSv).unwrap();
        let wallet_id = derive_wallet_id_from_account(&account, WalletType::PrivateKey).unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let keystore_id = keystore.keystore_id(wallet_id.to_string());
        FileKeystore::open(directory.path().to_path_buf())
            .unwrap()
            .import_private_key(private_key, TEST_PASSWORD, Some(keystore_id.clone()))
            .unwrap();
        Self {
            _directory: directory,
            keystore,
            keystore_id,
            address: account.address,
        }
    }
}

#[test]
fn test_identity_derivation_v1() {
    let identity_key = derive_identity_key(&TEST_PRIVATE_KEY).unwrap();
    assert_eq!(hex::encode(identity_key.as_slice()), "6b82ec88ef73f692c675b303df9696b149702e6088b921a88f20828d7b3a050d");
    assert_eq!(
        hex::encode(derive_identity_candidate(&TEST_PRIVATE_KEY, 1).unwrap().as_slice()),
        "c56287913dbed87890f5c53409cda06a31cf2c6593f2a158b45cfff99d4adf34"
    );
    assert_ne!(identity_key.as_slice(), TEST_PRIVATE_KEY);
    let context = Context::from_private_key(&TEST_PRIVATE_KEY);
    assert_eq!(context.address, TEST_ADDRESS);
    assert_eq!(
        context
            .keystore
            .paymail_identity_public_key(context.keystore_id, &context.address, TEST_PASSWORD.to_vec())
            .unwrap(),
        TEST_IDENTITY_PUBLIC_KEY
    );
}

#[test]
fn test_identity_scalar_boundaries() {
    assert!(!identity_candidate_is_valid(&[0; 32], &TEST_PRIVATE_KEY));
    assert!(!identity_candidate_is_valid(&SECP256K1_ORDER, &TEST_PRIVATE_KEY));
    assert!(!identity_candidate_is_valid(&[0xff; 32], &TEST_PRIVATE_KEY));
    assert!(!identity_candidate_is_valid(&TEST_PRIVATE_KEY, &TEST_PRIVATE_KEY));
    let mut maximum_scalar = SECP256K1_ORDER;
    maximum_scalar[31] -= 1;
    assert!(identity_candidate_is_valid(&maximum_scalar, &TEST_PRIVATE_KEY));
    let mut minimum_scalar = [0; 32];
    minimum_scalar[31] = 1;
    assert!(identity_candidate_is_valid(&minimum_scalar, &TEST_PRIVATE_KEY));
}

#[test]
fn test_identity_derivation_skips_invalid_scalars_and_fails_after_attempt_limit() {
    let mut maximum_scalar = SECP256K1_ORDER;
    maximum_scalar[31] -= 1;
    let candidates = [TEST_PRIVATE_KEY, [0; 32], SECP256K1_ORDER, maximum_scalar];
    let mut counters = Vec::new();
    let identity = derive_identity_key_from_candidates(&TEST_PRIVATE_KEY, |counter| {
        counters.push(counter);
        Ok(Zeroizing::new(candidates[counter as usize]))
    })
    .unwrap();
    assert_eq!(*identity, maximum_scalar);
    assert_eq!(counters, vec![0, 1, 2, 3]);
    let mut counters = Vec::new();
    assert_eq!(
        derive_identity_key_from_candidates(&TEST_PRIVATE_KEY, |counter| {
            counters.push(counter);
            Ok(Zeroizing::new(SECP256K1_ORDER))
        })
        .unwrap_err(),
        BsvHomeCoreError::from("Paymail identity derivation exhausted")
    );
    assert_eq!(counters, (0..1024).collect::<Vec<_>>());
    assert_eq!(
        derive_identity_key_from_candidates(&TEST_PRIVATE_KEY, |_| Err(BsvHomeCoreError::from("HMAC fixture failure"))).unwrap_err(),
        BsvHomeCoreError::from("HMAC fixture failure")
    );
}

#[test]
fn test_paymail_dual_signatures_preserve_bitcoin_wire_bytes() {
    let context = Context::from_private_key(&TEST_PRIVATE_KEY);
    for (message, expected_address_signature, expected_identity_signature) in [
        (
            "Hello Bitcoin",
            "H9IDomnDy4ttQzjVX2JY1Wk3Ir832xJTKbMaalAXOnVOBAUxHYeiVeVn0eh0quZUuCrPKov/wqzYCJCQtyv5Gvc=",
            "H00TKfVLcSp05DXJlQ2FU4f/I2F5xD+zx7MZShsJoNRyfufGTXkNuhKDDmD3v4ltyicPjZUPes+o+TnS59hLUjs=",
        ),
        (
            "Paymail 收款\n",
            "IH6qGd8P7MBNsZDuTWOw3HdP+JdzqiWkpbojpF2gVfv4C1p4uWOEA4gwY4HsR7GPlQZ2rzSq3SsKSiiYLFna3Pc=",
            "ICaTNWDwTzGEDQxws1LcjNNxs7WICN7ggl2dO3QWzLW2I+GnBYJVin++0vrwVjZV/ms1NjwbuOvIAS8lvTx+82Y=",
        ),
    ] {
        let signatures = context
            .keystore
            .sign_paymail_binding(context.keystore_id.clone(), &context.address, TEST_IDENTITY_PUBLIC_KEY, message, TEST_PASSWORD.to_vec())
            .unwrap();
        assert_eq!(signatures.address_signature, expected_address_signature);
        assert_eq!(signatures.identity_signature, expected_identity_signature);
        let address_signature = BASE64.decode(signatures.address_signature).unwrap();
        let identity_signature = BASE64.decode(signatures.identity_signature).unwrap();
        assert_eq!(BsvChainSigner::recover_message_address(message, &address_signature).unwrap(), TEST_ADDRESS);
        assert_eq!(
            hex::encode(BsvChainSigner::recover_message_public_key(message, &identity_signature).unwrap()),
            TEST_IDENTITY_PUBLIC_KEY
        );
        assert_ne!(
            hex::encode(BsvChainSigner::recover_message_public_key(&format!("{message}\n"), &identity_signature).unwrap()),
            TEST_IDENTITY_PUBLIC_KEY
        );
    }
}

#[test]
fn test_paymail_identity_restores_from_default_bsv_source() {
    let context = Context::new(BsvHomeImportType::SinglePhrase {
        words: ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
        chain: Chain::BitcoinSv,
    });
    assert_eq!(context.address, "1K6LZdwpKT5XkEZo2T2kW197aMXYbYMc4f");
    let private_key = derive_private_key_from_mnemonic(ABANDON_PHRASE, Chain::BitcoinSv).unwrap();
    let recovered = Context::from_private_key(private_key.as_slice());
    let expected_public_key = "03c1c19e9d8e0838c466ed0b2c44b170e80ed257e609abcf08c188253d418e2c77";
    for wallet in [&context, &recovered] {
        assert_eq!(
            wallet
                .keystore
                .paymail_identity_public_key(wallet.keystore_id.clone(), &wallet.address, TEST_PASSWORD.to_vec())
                .unwrap(),
            expected_public_key
        );
        assert_eq!(
            wallet
                .keystore
                .paymail_identity_public_key(wallet.keystore_id.clone(), "1DhquSu6ky8QQnf88b1d3tRYeUkMLASZg9", TEST_PASSWORD.to_vec())
                .unwrap_err(),
            BsvHomeCoreError::from("Paymail default address does not match signing wallet")
        );
    }
    let original_signatures = context
        .keystore
        .sign_paymail_binding(context.keystore_id, &context.address, expected_public_key, "Paymail recovery", TEST_PASSWORD.to_vec())
        .unwrap();
    let recovered_signatures = recovered
        .keystore
        .sign_paymail_binding(recovered.keystore_id, &recovered.address, expected_public_key, "Paymail recovery", TEST_PASSWORD.to_vec())
        .unwrap();
    assert_eq!(original_signatures.address_signature, recovered_signatures.address_signature);
    assert_eq!(original_signatures.identity_signature, recovered_signatures.identity_signature);
}

#[test]
fn test_paymail_signing_rejects_wrong_password_address_and_identity() {
    let context = Context::from_private_key(&TEST_PRIVATE_KEY);
    assert!(
        context
            .keystore
            .paymail_identity_public_key(context.keystore_id.clone(), &context.address, b"wrong-password".to_vec())
            .is_err()
    );
    assert_eq!(
        context
            .keystore
            .sign_paymail_binding(
                context.keystore_id.clone(),
                "1K6LZdwpKT5XkEZo2T2kW197aMXYbYMc4f",
                TEST_IDENTITY_PUBLIC_KEY,
                "message",
                TEST_PASSWORD.to_vec()
            )
            .err()
            .unwrap(),
        BsvHomeCoreError::from("Paymail default address does not match signing wallet")
    );
    assert_eq!(
        context
            .keystore
            .sign_paymail_binding(
                context.keystore_id.clone(),
                &context.address,
                "03c1c19e9d8e0838c466ed0b2c44b170e80ed257e609abcf08c188253d418e2c77",
                "message",
                TEST_PASSWORD.to_vec()
            )
            .err()
            .unwrap(),
        BsvHomeCoreError::from("Paymail identity does not match signing wallet")
    );
    assert!(
        context
            .keystore
            .sign_paymail_binding(context.keystore_id, &context.address, TEST_IDENTITY_PUBLIC_KEY, "message", b"wrong-password".to_vec())
            .is_err()
    );
}
