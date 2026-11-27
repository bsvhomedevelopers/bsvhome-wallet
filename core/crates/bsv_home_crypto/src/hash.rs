use zeroize::Zeroizing;

use crate::CryptoError;

pub fn hmac_sha512(key: &[u8], data: &[u8]) -> Result<Zeroizing<[u8; 64]>, CryptoError> {
    bsv_home_hash::sha2::hmac_sha512(key, data)
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::invalid_input("invalid HMAC-SHA512 key"))
}

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    bsv_home_hash::sha2::hmac_sha256(key, data)
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::invalid_input("invalid HMAC-SHA256 key"))
}
