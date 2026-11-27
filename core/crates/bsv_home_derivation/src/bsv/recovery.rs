use bitcoin::{
    Network,
    bip32::{Xpriv, Xpub},
    secp256k1::Secp256k1,
};
use bsv_home_bitcoin::signer::account_path::AccountPath;
use bsv_home_bsv::{BsvAddress, cold_wallet::ColdAddress};
use bsv_home_keystore::{HdDerivationMode, HdRecovery, Mnemonic};
use primitives::{Account, Address, BsvAddressBranch, BsvAddressLocator, Chain};
use zeroize::Zeroizing;

use super::{BsvExtendedPrivateKey, address_path, legacy, map_bip32_error};
use crate::{AccountDerivationError, derive_account_from_private_key};

pub const BSV_ACCOUNT_PATH: &str = "m/44'/236'/0'";
pub const SIMPLY_CASH_ACCOUNT_PATH: &str = "m/44'/145'/0'";
pub const BSV_MAX_ADDRESSES: u32 = 10_000;

fn recovery_account(recovery: &HdRecovery) -> Result<BsvExtendedPrivateKey, AccountDerivationError> {
    let path = AccountPath::parse(&recovery.account_path).map_err(map_bip32_error)?;
    let seed = Mnemonic::seed_with_passphrase(&recovery.phrase, &recovery.passphrase)?;
    let root = BsvExtendedPrivateKey(Xpriv::new_master(Network::Bitcoin, seed.as_slice()).map_err(map_bip32_error)?);
    match recovery.mode {
        HdDerivationMode::Standard => root.derive(path.derivation().as_ref()),
        HdDerivationMode::LegacyUnpadded => legacy::derive_account(root, path.derivation()),
    }
}

pub fn derive_bsv_account_from_recovery(recovery: &HdRecovery) -> Result<Account, AccountDerivationError> {
    let account = recovery_account(recovery)?;
    let xpub = Xpub::from_priv(&Secp256k1::signing_only(), &account.0).to_string();
    let path = AccountPath::parse(&recovery.account_path).map_err(map_bip32_error)?.canonical();
    let address = bsv_address_for_path(
        &xpub,
        &path,
        BsvAddressLocator {
            branch: BsvAddressBranch::Receive,
            index: 0,
        },
    )?;
    Ok(Account {
        chain: Chain::BitcoinSv,
        address,
        derivation_path: format!("{path}/0/0"),
        extended_public_key: Some(xpub),
    })
}

pub fn bsv_address_for_path(xpub: &str, account_path: &str, locator: BsvAddressLocator) -> Result<String, AccountDerivationError> {
    locator.validate()?;
    let account = AccountPath::parse(account_path).and_then(|path| path.validate_xpub(xpub)).map_err(map_bip32_error)?;
    let child = account.derive_pub(&Secp256k1::verification_only(), &address_path(locator)).map_err(map_bip32_error)?;
    Ok(BsvAddress::from_public_key(&child.public_key.serialize()).map_err(map_bip32_error)?.encode())
}

pub fn validate_bsv_account(account: &Account) -> Result<&str, AccountDerivationError> {
    let path = account
        .derivation_path
        .strip_suffix("/0/0")
        .ok_or_else(|| AccountDerivationError::invalid_input("unsupported BSV account derivation"))?;
    if account.chain != Chain::BitcoinSv {
        return Err(AccountDerivationError::invalid_input("expected a BSV account"));
    }
    let xpub = account
        .extended_public_key
        .as_deref()
        .ok_or_else(|| AccountDerivationError::invalid_input("BSV account has no xpub"))?;
    let address = bsv_address_for_path(
        xpub,
        path,
        BsvAddressLocator {
            branch: BsvAddressBranch::Receive,
            index: 0,
        },
    )?;
    if address != account.address {
        return Err(AccountDerivationError::invalid_input("BSV account xpub does not match the first address"));
    }
    Ok(xpub)
}

pub fn derive_bsv_private_keys_from_recovery(recovery: &HdRecovery, addresses: &[ColdAddress]) -> Result<Vec<(String, Zeroizing<Vec<u8>>)>, AccountDerivationError> {
    let account = recovery_account(recovery)?;
    addresses
        .iter()
        .map(|address| {
            let branch = match address.branch {
                0 => BsvAddressBranch::Receive,
                1 => BsvAddressBranch::Change,
                _ => return Err(AccountDerivationError::invalid_input("unsupported BSV address branch")),
            };
            let locator = BsvAddressLocator { branch, index: address.index };
            locator.validate()?;
            let path = address_path(locator);
            let child = BsvExtendedPrivateKey(account.0.derive_priv(&Secp256k1::signing_only(), &[path[0]]).map_err(map_bip32_error)?).derive(&path[1..])?;
            let bytes = Zeroizing::new(child.0.private_key.secret_bytes());
            let key = Zeroizing::new(bytes.to_vec());
            if derive_account_from_private_key(&key, Chain::BitcoinSv)?.address != address.address {
                return Err(AccountDerivationError::invalid_input("BSV input address does not belong to the recovery profile"));
            }
            Ok((address.address.clone(), key))
        })
        .collect()
}
