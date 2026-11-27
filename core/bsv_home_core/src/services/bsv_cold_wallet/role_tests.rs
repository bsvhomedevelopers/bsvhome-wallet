use std::sync::Arc;

use bsv_home_derivation::{bsv_account_xpub, derive_accounts_from_mnemonic};
use futures::executor::block_on;
use primitives::{Chain, Wallet, WalletId, WalletType, testkit::ABANDON_PHRASE};

use super::testkit::{Context, MemoryColdWalletStore};
use super::{BsvHomeBsvColdWalletRole, BsvHomeBsvColdWalletStore, BsvHomeBsvColdWalletStoredState, bsv_cold_wallet_require_hot_signing, bsv_cold_wallet_role};

fn public_wallet(view: bool) -> Wallet {
    let mut account = derive_accounts_from_mnemonic(ABANDON_PHRASE, vec![Chain::BitcoinSv]).unwrap().remove(0);
    account.extended_public_key = Some(bsv_account_xpub(ABANDON_PHRASE).unwrap());
    Wallet {
        id: if view {
            WalletId::View(Chain::BitcoinSv, account.address.clone())
        } else {
            WalletId::Single(Chain::BitcoinSv, account.address.clone())
        },
        wallet_type: if view { WalletType::View } else { WalletType::Single },
        accounts: vec![account],
        ..Wallet::mock()
    }
}

#[test]
fn test_hot_account_xpub_does_not_imply_cold_role_and_presence_never_downgrades_corruption_to_hot() {
    let wallet = public_wallet(false);
    let store = Arc::new(MemoryColdWalletStore::default());
    assert_eq!(bsv_cold_wallet_role(wallet.clone(), store.clone()).unwrap(), BsvHomeBsvColdWalletRole::None);
    bsv_cold_wallet_require_hot_signing(wallet.clone(), Chain::BitcoinSv, store.clone()).unwrap();
    store.records.lock().unwrap().insert(
        wallet.id.clone(),
        BsvHomeBsvColdWalletStoredState {
            revision: 0,
            value: "malformed public state".into(),
        },
    );
    assert_eq!(bsv_cold_wallet_role(wallet.clone(), store.clone()).unwrap(), BsvHomeBsvColdWalletRole::Signer);
    assert!(bsv_cold_wallet_require_hot_signing(wallet, Chain::BitcoinSv, store).is_err());
}

#[test]
fn test_observer_classification_requires_a_verified_public_pairing_but_never_enables_hot_keys() {
    let mut wallet = public_wallet(true);
    let store = Arc::new(MemoryColdWalletStore::default());
    assert_eq!(bsv_cold_wallet_role(wallet.clone(), store.clone()).unwrap(), BsvHomeBsvColdWalletRole::Observer);
    assert!(bsv_cold_wallet_require_hot_signing(wallet.clone(), Chain::BitcoinSv, store.clone()).is_err());
    wallet.accounts[0].extended_public_key = None;
    assert_eq!(bsv_cold_wallet_role(wallet.clone(), store.clone()).unwrap(), BsvHomeBsvColdWalletRole::None);
    store.records.lock().unwrap().insert(
        wallet.id.clone(),
        BsvHomeBsvColdWalletStoredState {
            revision: 0,
            value: "malformed public state".into(),
        },
    );
    assert_eq!(bsv_cold_wallet_role(wallet.clone(), store.clone()).unwrap(), BsvHomeBsvColdWalletRole::Observer);
    assert!(bsv_cold_wallet_require_hot_signing(wallet, Chain::BitcoinSv, store).is_err());
}

#[test]
fn test_role_and_hot_key_guard_propagate_actual_presence_read_errors() {
    let wallet = public_wallet(false);
    let store = Arc::new(MemoryColdWalletStore::default());
    *store.read_fail.lock().unwrap() = true;
    let expected = store.has_state(wallet.id.clone()).unwrap_err();
    assert_eq!(bsv_cold_wallet_role(wallet.clone(), store.clone()).unwrap_err(), expected);
    assert_eq!(bsv_cold_wallet_require_hot_signing(wallet, Chain::BitcoinSv, store).unwrap_err(), expected);
}

#[test]
fn test_cold_metadata_routing_does_not_replace_full_sdk_state_verification() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        context.store.records.lock().unwrap().get_mut(&wallet.id).unwrap().value = "malformed public state".into();
        assert_eq!(bsv_cold_wallet_role(wallet.clone(), context.store.clone()).unwrap(), BsvHomeBsvColdWalletRole::Signer);
        assert!(context.service.snapshot(wallet.id).await.is_err());
    });
}

#[test]
fn test_wallets_without_a_bsv_account_have_no_cold_role_without_reading_unrelated_state() {
    for view in [false, true] {
        let mut wallet = public_wallet(view);
        wallet.accounts.clear();
        let store = Arc::new(MemoryColdWalletStore::default());
        store.records.lock().unwrap().insert(
            wallet.id.clone(),
            BsvHomeBsvColdWalletStoredState {
                revision: 0,
                value: "unrelated malformed metadata".into(),
            },
        );
        *store.read_fail.lock().unwrap() = true;
        assert_eq!(bsv_cold_wallet_role(wallet, store).unwrap(), BsvHomeBsvColdWalletRole::None);
    }
}
