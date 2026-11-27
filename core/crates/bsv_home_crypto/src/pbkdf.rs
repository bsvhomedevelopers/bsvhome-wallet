use zeroize::Zeroizing;

use crate::CryptoError;
use crate::hash::{hmac_sha256, hmac_sha512};

const HMAC_SHA256_OUTPUT_LEN: usize = 32;
const HMAC_SHA512_OUTPUT_LEN: usize = 64;

pub fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: usize, output_len: usize) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    derive::<HMAC_SHA256_OUTPUT_LEN, _>(password, salt, iterations, output_len, hmac_sha256)
}

pub fn pbkdf2_hmac_sha512(password: &[u8], salt: &[u8], iterations: usize, output_len: usize) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    derive::<HMAC_SHA512_OUTPUT_LEN, _>(password, salt, iterations, output_len, hmac_sha512)
}

fn derive<const N: usize, F>(password: &[u8], salt: &[u8], iterations: usize, output_len: usize, hmac: F) -> Result<Zeroizing<Vec<u8>>, CryptoError>
where
    F: Fn(&[u8], &[u8]) -> Result<Zeroizing<[u8; N]>, CryptoError>,
{
    if iterations == 0 {
        return Err(CryptoError::invalid_input("PBKDF2 iterations must be greater than zero"));
    }

    let mut output = Zeroizing::new(vec![0u8; output_len]);
    for (block_index, chunk) in output.chunks_mut(N).enumerate() {
        let mut block_salt = Zeroizing::new(Vec::with_capacity(salt.len() + 4));
        block_salt.extend_from_slice(salt);
        let block_number = u32::try_from(block_index + 1).map_err(|_| CryptoError::invalid_input("PBKDF2 output length is too large"))?;
        block_salt.extend_from_slice(&block_number.to_be_bytes());

        let mut u = hmac(password, block_salt.as_slice())?;
        let mut block = Zeroizing::new(*u);
        for _ in 1..iterations {
            u = hmac(password, u.as_slice())?;
            for (left, right) in block.iter_mut().zip(u.iter()) {
                *left ^= *right;
            }
        }
        chunk.copy_from_slice(&block[..chunk.len()]);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pbkdf2_hmac_sha512_vector() {
        let output = pbkdf2_hmac_sha512(b"password", b"salt", 1, HMAC_SHA512_OUTPUT_LEN).unwrap();

        assert_eq!(
            hex::encode(output.as_slice()),
            "867f70cf1ade02cff3752599a3a53dc4af34c7a669815ae5d513554e1c8cf252\
             c02d470a285a0501bad999bfe943c08f050235d7d68b1da55e63f73b60a57fce"
                .replace(char::is_whitespace, "")
        );
    }

    #[test]
    fn test_pbkdf2_hmac_sha512_rejects_zero_iterations() {
        assert_eq!(
            pbkdf2_hmac_sha512(b"password", b"salt", 0, HMAC_SHA512_OUTPUT_LEN).unwrap_err(),
            CryptoError::invalid_input("PBKDF2 iterations must be greater than zero")
        );
    }

    #[test]
    fn test_pbkdf2_hmac_sha256_matches_public_independent_vectors_including_actual_offline_rounds() {
        for (password, salt, iterations, length, expected) in [
            (
                b"password".as_slice(),
                b"salt".as_slice(),
                1,
                32,
                "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b",
            ),
            (
                b"password".as_slice(),
                b"salt".as_slice(),
                2,
                64,
                "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43830651afcb5c862f0b249bd031f7a67520d136470f5ec271ece91c07773253d9",
            ),
            (
                "lock-\u{1f510}-e\u{301}".as_bytes(),
                b"public-vector".as_slice(),
                600000,
                32,
                "4a64c4004191ca4afc9b9b4ed5c33f14d535bc59923974bca3214b37b656e8b4",
            ),
        ] {
            let output = pbkdf2_hmac_sha256(password, salt, iterations, length).unwrap();
            let expected = hex::decode(expected).unwrap();
            assert!(output.as_slice() == expected.as_slice());
        }
    }

    #[test]
    fn test_pbkdf2_hmac_sha256_rejects_zero_rounds_and_preserves_empty_output() {
        assert!(pbkdf2_hmac_sha256(b"public", b"salt", 0, 32).is_err());
        assert!(pbkdf2_hmac_sha256(b"public", b"salt", 1, 0).unwrap().is_empty());
    }
}
