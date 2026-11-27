use std::str::FromStr;

use bitcoin::bip32::{ChildNumber, Xpriv, Xpub};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{Network, NetworkKind};
use bsv_home_bsv::BsvAddress;
use bsv_home_keystore::Mnemonic;
use primitives::{Address, BsvAddressLocator};
use zeroize::{Zeroize, Zeroizing};

use crate::AccountDerivationError;

#[cfg(test)]
mod tests;

mod legacy;
mod recovery;
pub use recovery::{
    BSV_ACCOUNT_PATH, BSV_MAX_ADDRESSES, SIMPLY_CASH_ACCOUNT_PATH, bsv_address_for_path, derive_bsv_account_from_recovery, derive_bsv_private_keys_from_recovery,
    validate_bsv_account,
};

const ACCOUNT_DEPTH: u8 = 3;
const ACCOUNT_PATH: [ChildNumber; 3] = [
    ChildNumber::Hardened { index: 44 },
    ChildNumber::Hardened { index: 236 },
    ChildNumber::Hardened { index: 0 },
];

struct BsvExtendedPrivateKey(Xpriv);

impl BsvExtendedPrivateKey {
    fn derive(mut self, path: &[ChildNumber]) -> Result<Self, AccountDerivationError> {
        let context = Secp256k1::signing_only();
        for child in path {
            let next = Self(self.0.derive_priv(&context, &[*child]).map_err(map_bip32_error)?);
            self = next;
        }
        Ok(self)
    }

    fn erase(&mut self) {
        self.0.private_key.non_secure_erase();
        let chain_code: &mut [u8] = self.0.chain_code.as_mut();
        chain_code.zeroize();
    }
}

impl Drop for BsvExtendedPrivateKey {
    fn drop(&mut self) {
        self.erase();
    }
}

pub fn bsv_account_xpub(words: &str) -> Result<String, AccountDerivationError> {
    let account_private_key = account_private_key(words)?;
    Ok(Xpub::from_priv(&Secp256k1::signing_only(), &account_private_key.0).to_string())
}

pub fn bsv_address(account_xpub: &str, locator: BsvAddressLocator) -> Result<String, AccountDerivationError> {
    locator.validate()?;
    let account_public_key = Xpub::from_str(account_xpub).map_err(map_bip32_error)?;
    if account_public_key.network != NetworkKind::Main || account_public_key.depth != ACCOUNT_DEPTH || account_public_key.child_number != (ChildNumber::Hardened { index: 0 }) {
        return Err(AccountDerivationError::invalid_input(
            "BSV xpub must identify a mainnet account at depth 3 and hardened account index 0",
        ));
    }
    let public_key = account_public_key
        .derive_pub(&Secp256k1::verification_only(), &address_path(locator))
        .map_err(map_bip32_error)?;
    Ok(BsvAddress::from_public_key(&public_key.public_key.serialize())
        .map_err(|error| AccountDerivationError::invalid_input(error.to_string()))?
        .encode())
}

pub fn bsv_private_key(words: &str, locator: BsvAddressLocator) -> Result<Zeroizing<Vec<u8>>, AccountDerivationError> {
    locator.validate()?;
    let account_private_key = account_private_key(words)?;
    address_private_key(&account_private_key, locator)
}

pub fn bsv_private_keys(words: &str, locators: &[BsvAddressLocator]) -> Result<Vec<Zeroizing<Vec<u8>>>, AccountDerivationError> {
    if locators.is_empty() {
        return Ok(Vec::new());
    }
    for locator in locators {
        locator.validate()?;
    }
    let account_private_key = account_private_key(words)?;
    locators.iter().map(|locator| address_private_key(&account_private_key, *locator)).collect()
}

fn address_private_key(account_private_key: &BsvExtendedPrivateKey, locator: BsvAddressLocator) -> Result<Zeroizing<Vec<u8>>, AccountDerivationError> {
    let path = address_path(locator);
    let branch = BsvExtendedPrivateKey(account_private_key.0.derive_priv(&Secp256k1::signing_only(), &[path[0]]).map_err(map_bip32_error)?);
    let private_key = branch.derive(&path[1..])?;
    let private_key_bytes = Zeroizing::new(private_key.0.private_key.secret_bytes());
    Ok(Zeroizing::new(private_key_bytes.to_vec()))
}

fn account_private_key(words: &str) -> Result<BsvExtendedPrivateKey, AccountDerivationError> {
    let seed = Mnemonic::seed(words)?;
    let master = BsvExtendedPrivateKey(Xpriv::new_master(Network::Bitcoin, seed.as_slice()).map_err(map_bip32_error)?);
    master.derive(&ACCOUNT_PATH)
}

fn address_path(locator: BsvAddressLocator) -> [ChildNumber; 2] {
    [ChildNumber::Normal { index: locator.branch.index() }, ChildNumber::Normal { index: locator.index }]
}

fn map_bip32_error(error: impl std::fmt::Display) -> AccountDerivationError {
    AccountDerivationError::invalid_input(format!("invalid BSV account derivation: {error}"))
}
