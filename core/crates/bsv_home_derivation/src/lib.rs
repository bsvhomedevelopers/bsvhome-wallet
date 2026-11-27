mod bsv;
mod cardano;
mod error;
mod mnemonic;
mod private_key;
mod wallet_id;

pub use bsv::{
    BSV_ACCOUNT_PATH, BSV_MAX_ADDRESSES, SIMPLY_CASH_ACCOUNT_PATH, bsv_account_xpub, bsv_address, bsv_address_for_path, bsv_private_key, bsv_private_keys,
    derive_bsv_account_from_recovery, derive_bsv_private_keys_from_recovery, validate_bsv_account,
};
pub use error::AccountDerivationError;
pub use mnemonic::{derive_accounts_from_mnemonic, derive_private_key_from_mnemonic};
pub use private_key::{ImportedPrivateKeyAccount, derive_account_from_private_key, derive_account_from_private_key_value, import_account_from_private_key};
pub use wallet_id::derive_wallet_id_from_account;

pub(crate) fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    let mut array = [0u8; N];
    array.copy_from_slice(&bytes[offset..offset + N]);
    array
}
