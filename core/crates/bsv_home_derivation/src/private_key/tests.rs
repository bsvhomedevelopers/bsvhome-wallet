use primitives::testkit::signer_mock::TEST_PRIVATE_KEY;
use primitives::{
    Chain,
    hex::{self, decode_hex},
};
use signer::{Ed25519KeyPair, secp256k1_public_key};
use zeroize::Zeroizing;

use super::*;
use crate::AccountDerivationError;

#[test]
fn test_private_key_account_derivation_selected_chains() {
    let secp_private_key = decode_hex("30df0ffc2b43717f4653c2a1e827e9dfb3d9364e019cc60092496cd4997d5d6e").unwrap();
    assert_eq!(
        derive_account_from_private_key(&secp_private_key, Chain::Ethereum).unwrap().address,
        "0x4ce31c0b2114abe61Ac123E1E6254E961C18D10B"
    );
    assert_eq!(
        derive_account_from_private_key(&secp_private_key, Chain::HyperCore).unwrap().address,
        "0x4ce31c0b2114abe61Ac123E1E6254E961C18D10B"
    );
    assert_eq!(
        derive_account_from_private_key(&secp_private_key, Chain::Tron).unwrap().address,
        "TGykRV8uNxzjgbAKz9zWaaniwXT4nRcWaf"
    );

    let ed_private_key = decode_hex("3d769e8a65b9002a470e9aecf2587ef848e2a0b483320e24c493a5913d594eb9").unwrap();
    assert_eq!(
        derive_account_from_private_key(&ed_private_key, Chain::Solana).unwrap().address,
        "QszdTXmSeM88WET7RqJbE51SWNWLG3975rVHT5bSyAP"
    );
    assert_eq!(
        derive_account_from_private_key(&ed_private_key, Chain::Near).unwrap().address,
        "061e046a9b89bf2fd06e92b43dad4fc997cb1d0a2aab5d76c65687ddb0b22308"
    );
    assert_eq!(
        derive_account_from_private_key(&ed_private_key, Chain::Stellar).unwrap().address,
        "GADB4BDKTOE36L6QN2JLIPNNJ7EZPSY5BIVKWXLWYZLIPXNQWIRQQZKT"
    );
    assert_eq!(
        derive_account_from_private_key(&ed_private_key, Chain::Algorand).unwrap().address,
        "AYPAI2U3RG7S7UDOSK2D3LKPZGL4WHIKFKVV25WGK2D53MFSEMEEEPTJWY"
    );
    assert_eq!(
        derive_account_from_private_key(&ed_private_key, Chain::Aptos).unwrap().address,
        "0x82494016df078d6f9041319694f0973cab5e31cc9787d2b21a6303dd23fbb07e"
    );
    assert_eq!(
        derive_account_from_private_key(&ed_private_key, Chain::Sui).unwrap().address,
        "0xd1fd4baa9c58d54f561ba224506a572b02d0f8ed7fe2f7e70b5a93cc19c8e308"
    );

    let aptos_private_key = decode_hex("0000000000000000000000000000000000000000000000000000000000000022").unwrap();
    assert_eq!(
        derive_account_from_private_key(&aptos_private_key, Chain::Aptos).unwrap().address,
        "0x0dc4a9af7d0b8e5811bf326bf49c44f8685d4ee5b4fd3606e06987c5056f61bb"
    );
}

#[test]
fn test_import_private_key_rejects_bitcoin_family() {
    assert_eq!(
        import_account_from_private_key("0x30df0ffc2b43717f4653c2a1e827e9dfb3d9364e019cc60092496cd4997d5d6e", Chain::Bitcoin)
            .err()
            .unwrap(),
        AccountDerivationError::InvalidPrivateKey
    );
}

#[test]
fn test_import_near_prefixed_private_key() {
    let private_key = decode_hex("3d769e8a65b9002a470e9aecf2587ef848e2a0b483320e24c493a5913d594eb9").unwrap();
    let public_key = Ed25519KeyPair::from_private_key(&private_key).unwrap().public_key_bytes;
    let encoded = format!(
        "ed25519:{}",
        bs58::encode(private_key.iter().chain(public_key.iter()).copied().collect::<Vec<_>>()).into_string()
    );

    assert_eq!(
        import_account_from_private_key(&encoded, Chain::Near).unwrap().account.address,
        "061e046a9b89bf2fd06e92b43dad4fc997cb1d0a2aab5d76c65687ddb0b22308"
    );
    assert_eq!(
        import_account_from_private_key(&encoded, Chain::Solana).err().unwrap(),
        AccountDerivationError::InvalidPrivateKey
    );
}

#[test]
fn test_bsv_wif_and_legacy_hex_import_derive_the_same_public_key_account_and_address() {
    let hex = Zeroizing::new(hex::encode(TEST_PRIVATE_KEY));
    let mut payload = Zeroizing::new(Vec::with_capacity(34));
    payload.push(0x80);
    payload.extend_from_slice(&TEST_PRIVATE_KEY);
    payload.push(0x01);
    let wif = Zeroizing::new(bs58::encode(payload.as_slice()).with_check().into_string());
    let legacy = import_account_from_private_key(&hex, Chain::BitcoinSv).unwrap();
    let imported = import_account_from_private_key(&wif, Chain::BitcoinSv).unwrap();
    assert!(imported.account == legacy.account);
    assert!(!imported.account.address.is_empty());
    assert!(imported.private_key.as_slice() == legacy.private_key.as_slice());
    assert!(secp256k1_public_key(&imported.private_key).unwrap() == secp256k1_public_key(&legacy.private_key).unwrap());
    assert!(derive_account_from_private_key_value(&wif, Chain::BitcoinSv).unwrap() == imported.account);
    assert!(import_account_from_private_key(&wif, Chain::Bitcoin).is_err());
}
