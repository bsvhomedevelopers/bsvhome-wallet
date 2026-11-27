use std::collections::HashSet;
use std::str::FromStr;

use bitcoin::NetworkKind;
use bitcoin::bip32::{ChildNumber, Xpub};
use bsv_home_bsv::{BsvAddress, cold_wallet::ColdAddress};
use bsv_home_keystore::{HdDerivationMode, HdRecovery};
use primitives::testkit::ABANDON_PHRASE;
use primitives::{Address, BsvAddressBranch, BsvAddressLocator, Chain};
use serde::Deserialize;
use signer::secp256k1_public_key;

use crate::{AccountDerivationError, derive_accounts_from_mnemonic};

use super::{
    account_private_key, bsv_account_xpub, bsv_address, bsv_address_for_path, bsv_private_key, bsv_private_keys, derive_bsv_account_from_recovery,
    derive_bsv_private_keys_from_recovery, validate_bsv_account,
};

#[derive(Deserialize)]
struct BsvHdVectors {
    account_xpub: String,
    receive: Vec<String>,
    change: Vec<String>,
}

fn vectors() -> BsvHdVectors {
    serde_json::from_str(include_str!("../../testdata/bsv_hd_vectors.json")).unwrap()
}

#[test]
fn test_bsv_hd_address_vectors_and_private_key_consistency() {
    let expected = vectors();
    let account_xpub = bsv_account_xpub(ABANDON_PHRASE).unwrap();
    assert_eq!(account_xpub, expected.account_xpub);
    let mut addresses = HashSet::new();
    for (branch, expected_addresses) in [(BsvAddressBranch::Receive, expected.receive), (BsvAddressBranch::Change, expected.change)] {
        for (index, expected_address) in expected_addresses.into_iter().enumerate() {
            let locator = BsvAddressLocator { branch, index: index as u32 };
            let address = bsv_address(&account_xpub, locator).unwrap();
            let private_key = bsv_private_key(ABANDON_PHRASE, locator).unwrap();
            let public_key = secp256k1_public_key(private_key.as_slice()).unwrap();
            assert_eq!(address, expected_address);
            assert_eq!(BsvAddress::from_public_key(&public_key).unwrap().encode(), address);
            assert!(addresses.insert(address));
        }
    }
    assert_eq!(addresses.len(), 13);
}

#[test]
fn test_bsv_hd_preserves_main_account_identity() {
    let account = derive_accounts_from_mnemonic(ABANDON_PHRASE, vec![Chain::BitcoinSv]).unwrap().remove(0);
    let account_xpub = bsv_account_xpub(ABANDON_PHRASE).unwrap();
    assert_eq!(
        bsv_address(
            &account_xpub,
            BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 0
            }
        )
        .unwrap(),
        account.address
    );
    assert_eq!(account.derivation_path, "m/44'/236'/0'/0/0");
}

#[test]
fn test_bsv_hd_rejects_non_account_xpub() {
    let account_xpub = Xpub::from_str(&vectors().account_xpub).unwrap();
    let locator = BsvAddressLocator {
        branch: BsvAddressBranch::Receive,
        index: 0,
    };
    for invalid_xpub in [
        Xpub {
            network: NetworkKind::Test,
            ..account_xpub
        },
        Xpub { depth: 0, ..account_xpub },
        Xpub { depth: 2, ..account_xpub },
        Xpub { depth: 4, ..account_xpub },
        Xpub {
            child_number: ChildNumber::Normal { index: 0 },
            ..account_xpub
        },
        Xpub {
            child_number: ChildNumber::Hardened { index: 1 },
            ..account_xpub
        },
    ] {
        assert_eq!(
            bsv_address(&invalid_xpub.to_string(), locator).unwrap_err(),
            AccountDerivationError::invalid_input("BSV xpub must identify a mainnet account at depth 3 and hardened account index 0")
        );
    }
    assert!(bsv_address("invalid-xpub", locator).is_err());
}

#[test]
fn test_bsv_hd_locator_boundary_and_invalid_mnemonic() {
    let account_xpub = vectors().account_xpub;
    for branch in [BsvAddressBranch::Receive, BsvAddressBranch::Change] {
        let largest_locator = BsvAddressLocator { branch, index: (1 << 31) - 1 };
        let private_key = bsv_private_key(ABANDON_PHRASE, largest_locator).unwrap();
        let public_key = secp256k1_public_key(private_key.as_slice()).unwrap();
        assert_eq!(
            bsv_address(&account_xpub, largest_locator).unwrap(),
            BsvAddress::from_public_key(&public_key).unwrap().encode()
        );
        for index in [1 << 31, u32::MAX] {
            let invalid_locator = BsvAddressLocator { branch, index };
            assert!(bsv_address(&account_xpub, invalid_locator).is_err());
            assert!(bsv_private_key(ABANDON_PHRASE, invalid_locator).is_err());
        }
    }
    let locator = BsvAddressLocator {
        branch: BsvAddressBranch::Receive,
        index: 0,
    };
    assert!(bsv_account_xpub("invalid mnemonic").is_err());
    assert!(bsv_private_key("invalid mnemonic", locator).is_err());
}

#[test]
fn test_bsv_private_keys_preserves_input_order_and_shared_addresses() {
    let expected = vectors();
    let locators = [
        BsvAddressLocator {
            branch: BsvAddressBranch::Change,
            index: 2,
        },
        BsvAddressLocator {
            branch: BsvAddressBranch::Receive,
            index: 9,
        },
        BsvAddressLocator {
            branch: BsvAddressBranch::Change,
            index: 2,
        },
    ];
    let addresses = bsv_private_keys(ABANDON_PHRASE, &locators)
        .unwrap()
        .into_iter()
        .map(|private_key| BsvAddress::from_public_key(&secp256k1_public_key(private_key.as_slice()).unwrap()).unwrap().encode())
        .collect::<Vec<_>>();
    assert_eq!(addresses, vec![expected.change[2].clone(), expected.receive[9].clone(), expected.change[2].clone()]);
    assert_eq!(bsv_private_keys(ABANDON_PHRASE, &[]).unwrap().len(), 0);
    assert!(
        bsv_private_keys(
            ABANDON_PHRASE,
            &[BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 1 << 31
            }]
        )
        .is_err()
    );
    assert!(bsv_private_keys("invalid mnemonic", &locators).is_err());
}

#[test]
fn test_bsv_extended_private_key_erases_owned_key_and_chain_code() {
    let mut private_key = account_private_key(ABANDON_PHRASE).unwrap();
    private_key.erase();
    assert!(private_key.0.private_key.secret_bytes() == [1u8; 32]);
    let chain_code: &[u8] = private_key.0.chain_code.as_ref();
    assert!(chain_code == [0u8; 32]);
}

#[derive(Clone, Deserialize)]
struct ExpectedAddress {
    address: String,
    branch: u32,
    index: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryVector {
    mnemonic: String,
    passphrase: String,
    account_path: String,
    compliant: bool,
    account_xpub: String,
    first_address: Option<String>,
    derived: Option<Vec<ExpectedAddress>>,
    receive: Option<Vec<ExpectedAddress>>,
    change: Option<Vec<ExpectedAddress>>,
}

#[derive(Deserialize)]
struct RecoveryVectors {
    cases: Vec<RecoveryVector>,
}

fn assert_recovery_vectors(input: &str, expected_count: usize) {
    let vectors: RecoveryVectors = serde_json::from_str(input).unwrap();
    assert_eq!(vectors.cases.len(), expected_count);
    for vector in vectors.cases {
        let mode = if vector.compliant {
            HdDerivationMode::Standard
        } else {
            HdDerivationMode::LegacyUnpadded
        };
        let recovery = HdRecovery::new(&vector.mnemonic, &vector.passphrase, &vector.account_path, mode).unwrap();
        let account = derive_bsv_account_from_recovery(&recovery).unwrap();
        let addresses = match vector.derived {
            Some(addresses) => addresses,
            None => vector.receive.unwrap().into_iter().chain(vector.change.unwrap()).collect(),
        };
        let addresses = addresses
            .into_iter()
            .map(|address| ColdAddress {
                address: address.address,
                branch: address.branch,
                index: address.index,
            })
            .collect::<Vec<_>>();
        assert_eq!(account.address, vector.first_address.unwrap_or_else(|| addresses[0].address.clone()));
        assert_eq!(account.extended_public_key.as_deref(), Some(vector.account_xpub.as_str()));
        assert_eq!(account.derivation_path, format!("{}/0/0", vector.account_path));
        assert_eq!(validate_bsv_account(&account).unwrap(), vector.account_xpub);
        for address in &addresses {
            let locator = BsvAddressLocator {
                branch: if address.branch == 0 { BsvAddressBranch::Receive } else { BsvAddressBranch::Change },
                index: address.index,
            };
            assert_eq!(bsv_address_for_path(&vector.account_xpub, &vector.account_path, locator).unwrap(), address.address);
        }
        let keys = derive_bsv_private_keys_from_recovery(&recovery, &addresses).unwrap();
        assert_eq!(keys.len(), addresses.len());
        for ((address, key), expected) in keys.iter().zip(&addresses) {
            assert_eq!(address, &expected.address);
            assert_eq!(BsvAddress::from_public_key(&secp256k1_public_key(key).unwrap()).unwrap().encode(), expected.address);
        }
        let mut wrong = account.clone();
        wrong.derivation_path = format!("{}/1/0/0", vector.account_path);
        assert!(validate_bsv_account(&wrong).is_err());
        let mut foreign = addresses.clone();
        foreign[0].address = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT".into();
        assert!(derive_bsv_private_keys_from_recovery(&recovery, &foreign).is_err());
    }
}

#[test]
fn test_custom_recovery_matches_independent_sdk_vectors() {
    assert_recovery_vectors(include_str!("../../testdata/bsv_custom_path_vectors.json"), 20);
}

#[test]
fn test_legacy_and_standard_recovery_match_independent_sdk_vectors() {
    assert_recovery_vectors(include_str!("../../testdata/simply_cash_vectors.json"), 6);
}

#[test]
fn test_recovery_passphrase_nfkd_preserves_meaningful_whitespace() {
    let recovery = HdRecovery::new(ABANDON_PHRASE, " é:Pass ", "m/44'/236'/0'", HdDerivationMode::Standard).unwrap();
    let normalized = HdRecovery::new(ABANDON_PHRASE, " e\u{301}:Pass ", "m/44h/236H/0'", HdDerivationMode::Standard).unwrap();
    let account = derive_bsv_account_from_recovery(&recovery).unwrap();
    let normalized = derive_bsv_account_from_recovery(&normalized).unwrap();
    assert_eq!(
        (account.address, account.derivation_path, account.extended_public_key),
        (normalized.address, normalized.derivation_path, normalized.extended_public_key)
    );
    let trimmed = HdRecovery::new(ABANDON_PHRASE, "é:Pass", "m/44'/236'/0'", HdDerivationMode::Standard).unwrap();
    assert_ne!(
        derive_bsv_account_from_recovery(&recovery).unwrap().address,
        derive_bsv_account_from_recovery(&trimmed).unwrap().address
    );
}
