use bitcoin::{
    bip32::{ChainCode, ChildNumber, DerivationPath, Xpriv},
    hashes::{
        Hash, HashEngine,
        hmac::{Hmac, HmacEngine},
        sha512,
    },
    secp256k1::{Scalar, Secp256k1},
};
use zeroize::Zeroizing;

use super::{BsvExtendedPrivateKey, map_bip32_error};
use crate::AccountDerivationError;

const ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0,
    0x36, 0x41, 0x41,
];

pub(super) fn derive_account(mut parent: BsvExtendedPrivateKey, path: &DerivationPath) -> Result<BsvExtendedPrivateKey, AccountDerivationError> {
    let secp = Secp256k1::signing_only();
    for child in path.as_ref() {
        let mut index = match child {
            ChildNumber::Hardened { index } => *index,
            ChildNumber::Normal { .. } => {
                parent = parent.derive(&[*child])?;
                continue;
            }
        };
        let mut attempts = 0;
        let next = loop {
            attempts += 1;
            if attempts > 1024 || index >= (1 << 31) {
                return Err(AccountDerivationError::invalid_input("legacy BSV child derivation limit reached"));
            }
            let bytes = Zeroizing::new(parent.0.private_key.secret_bytes());
            let offset = bytes
                .iter()
                .position(|byte| *byte != 0)
                .ok_or_else(|| AccountDerivationError::invalid_input("invalid legacy BSV parent key"))?;
            let mut engine = HmacEngine::<sha512::Hash>::new(parent.0.chain_code.as_bytes());
            engine.input(&[0]);
            engine.input(&bytes[offset..]);
            engine.input(&(index | 0x8000_0000).to_be_bytes());
            let hash = Zeroizing::new(Hmac::from_engine(engine).to_byte_array());
            let mut tweak = Zeroizing::new([0; 32]);
            tweak.copy_from_slice(&hash[..32]);
            if tweak.as_slice() >= ORDER.as_slice() {
                let mut borrow = 0u16;
                for offset in (0..32).rev() {
                    let subtraction = ORDER[offset] as u16 + borrow;
                    let source = tweak[offset] as u16;
                    tweak[offset] = source.wrapping_sub(subtraction) as u8;
                    borrow = u16::from(source < subtraction);
                }
            }
            let scalar = Scalar::from_be_bytes(*tweak).map_err(map_bip32_error)?;
            if let Ok(private_key) = parent.0.private_key.add_tweak(&scalar) {
                let mut code = Zeroizing::new([0; 32]);
                code.copy_from_slice(&hash[32..]);
                break BsvExtendedPrivateKey(Xpriv {
                    network: parent.0.network,
                    depth: parent.0.depth + 1,
                    parent_fingerprint: parent.0.fingerprint(&secp),
                    child_number: ChildNumber::Hardened { index },
                    private_key,
                    chain_code: ChainCode::from(*code),
                });
            }
            index += 1;
        };
        parent = next;
    }
    Ok(parent)
}
