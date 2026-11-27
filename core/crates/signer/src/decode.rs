use crate::{CARDANO_EXTENDED_PRIVATE_KEY_LENGTH, SignatureScheme, SignerError};
use primitives::hex::encode_with_0x;
use primitives::{Chain, ChainType, decode_hex};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy)]
enum KeyEncoding {
    Hex,
    Base58,
    WifCompressed,
    PrefixedBase58(SignatureScheme),
    StellarStrKey,
}

const SECP256K1_PRIVATE_KEY_LENGTH: usize = 32;
const WIF_COMPRESSED_PAYLOAD_LENGTH: usize = SECP256K1_PRIVATE_KEY_LENGTH + 2;
const WIF_COMPRESSED_TEXT_LENGTH: usize = 52;
const BITCOIN_WIF_MAINNET_VERSION: u8 = 0x80;
const WIF_COMPRESSED_FLAG: u8 = 0x01;

fn import_encodings_for_chain(chain: &Chain) -> &'static [KeyEncoding] {
    if *chain == Chain::BitcoinSv {
        return &[KeyEncoding::Hex, KeyEncoding::WifCompressed];
    }
    match chain.chain_type() {
        ChainType::Bitcoin | ChainType::Cardano => &[],
        ChainType::Solana => &[KeyEncoding::Base58, KeyEncoding::Hex],
        ChainType::Stellar => &[KeyEncoding::StellarStrKey, KeyEncoding::Hex],
        ChainType::Near => &[KeyEncoding::PrefixedBase58(SignatureScheme::Ed25519), KeyEncoding::Hex],
        _ => &[KeyEncoding::Hex],
    }
}

fn export_encoding_for_chain(chain: &Chain) -> KeyEncoding {
    if *chain == Chain::BitcoinSv {
        return KeyEncoding::WifCompressed;
    }
    match chain.chain_type() {
        ChainType::Bitcoin | ChainType::Solana => KeyEncoding::Base58,
        _ => KeyEncoding::Hex,
    }
}

pub fn supports_private_key_import(chain: &Chain) -> bool {
    !import_encodings_for_chain(chain).is_empty()
}

fn scheme_for_chain(chain: &Chain) -> SignatureScheme {
    match chain.chain_type() {
        ChainType::Solana | ChainType::Ton | ChainType::Aptos | ChainType::Sui | ChainType::Near | ChainType::Stellar | ChainType::Algorand | ChainType::Polkadot => {
            SignatureScheme::Ed25519
        }
        ChainType::Cardano => SignatureScheme::Ed25519CardanoExtended,
        _ => SignatureScheme::Secp256k1,
    }
}

fn decode_base58(value: &str) -> Option<Vec<u8>> {
    let decoded = Zeroizing::new(bs58::decode(value).into_vec().ok()?);
    match decoded.len() {
        32 => Some(decoded.to_vec()),
        64 => {
            let keypair = decoded.as_slice().try_into().ok()?;
            ed25519_dalek::SigningKey::from_keypair_bytes(keypair).ok()?;
            Some(decoded[..32].to_vec())
        }
        _ => None,
    }
}

fn decode_wif_compressed(value: &str) -> Option<Vec<u8>> {
    if value.len() != WIF_COMPRESSED_TEXT_LENGTH {
        return None;
    }
    let decoded = Zeroizing::new(bs58::decode(value).with_check(None).into_vec().ok()?);
    if decoded.len() != WIF_COMPRESSED_PAYLOAD_LENGTH || decoded[0] != BITCOIN_WIF_MAINNET_VERSION || decoded[WIF_COMPRESSED_PAYLOAD_LENGTH - 1] != WIF_COMPRESSED_FLAG {
        return None;
    }
    Some(decoded[1..=SECP256K1_PRIVATE_KEY_LENGTH].to_vec())
}

fn encode_wif_compressed(private_key: &[u8]) -> Result<String, SignerError> {
    if private_key.len() != SECP256K1_PRIVATE_KEY_LENGTH {
        return Err(SignerError::invalid_input("Invalid BSV private key length for WIF"));
    }
    validate_key(private_key, SignatureScheme::Secp256k1)?;
    let mut payload = Zeroizing::new(Vec::with_capacity(WIF_COMPRESSED_PAYLOAD_LENGTH));
    payload.push(BITCOIN_WIF_MAINNET_VERSION);
    payload.extend_from_slice(private_key);
    payload.push(WIF_COMPRESSED_FLAG);
    Ok(bs58::encode(payload.as_slice()).with_check().into_string())
}

fn decode_prefixed_base58(value: &str, scheme: SignatureScheme) -> Option<Vec<u8>> {
    decode_base58(value.strip_prefix(scheme.as_ref())?.strip_prefix(':')?)
}

fn base32_decode_char(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'2'..=b'7' => Some(c - b'2' + 26),
        _ => None,
    }
}

fn base32_decode(input: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(input.len() * 5 / 8);
    let mut buffer: u64 = 0;
    let mut bits = 0u8;

    for &c in input {
        if c == b'=' {
            break;
        }
        let val = base32_decode_char(c)?;
        buffer = (buffer << 5) | val as u64;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1u64 << bits) - 1;
        }
    }
    Some(output)
}

fn crc16_xmodem(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

fn decode_stellar_strkey(value: &str) -> Option<Vec<u8>> {
    if value.len() != 56 || !value.starts_with('S') {
        return None;
    }
    let decoded = Zeroizing::new(base32_decode(value.as_bytes())?);
    if decoded.len() != 35 || decoded[0] != 0x90 {
        return None;
    }
    let expected = u16::from_le_bytes([decoded[33], decoded[34]]);
    if crc16_xmodem(&decoded[..33]) != expected {
        return None;
    }
    Some(decoded[1..33].to_vec())
}

fn validate_key(bytes: &[u8], scheme: SignatureScheme) -> Result<(), SignerError> {
    match scheme {
        SignatureScheme::Ed25519 => {
            let arr: &[u8; 32] = bytes.try_into().map_err(|_| SignerError::invalid_input("Invalid ed25519 private key"))?;
            ed25519_dalek::SigningKey::from_bytes(arr);
            Ok(())
        }
        SignatureScheme::Secp256k1 => {
            k256::ecdsa::SigningKey::from_slice(bytes).map_err(|_| SignerError::invalid_input("Invalid secp256k1 private key"))?;
            Ok(())
        }
        SignatureScheme::Ed25519CardanoExtended => {
            if bytes.len() != CARDANO_EXTENDED_PRIVATE_KEY_LENGTH {
                return Err(SignerError::invalid_input("Invalid Cardano extended private key"));
            }
            Ok(())
        }
    }
}

fn decode_key(value: &str, encodings: &[KeyEncoding], scheme: SignatureScheme) -> Result<Zeroizing<Vec<u8>>, SignerError> {
    for encoding in encodings {
        let decoded = match encoding {
            KeyEncoding::Hex => decode_hex(value).ok().map(Zeroizing::new),
            KeyEncoding::Base58 => decode_base58(value).map(Zeroizing::new),
            KeyEncoding::WifCompressed => decode_wif_compressed(value).map(Zeroizing::new),
            KeyEncoding::PrefixedBase58(prefix_scheme) => {
                if *prefix_scheme != scheme {
                    None
                } else {
                    decode_prefixed_base58(value, *prefix_scheme).map(Zeroizing::new)
                }
            }
            KeyEncoding::StellarStrKey => decode_stellar_strkey(value).map(Zeroizing::new),
        };
        if let Some(bytes) = decoded
            && validate_key(&bytes, scheme).is_ok()
        {
            return Ok(bytes);
        }
    }
    Err(SignerError::invalid_input("Invalid private key format"))
}

fn encode_key(bytes: &[u8], encoding: KeyEncoding) -> Result<String, SignerError> {
    match encoding {
        KeyEncoding::Hex => Ok(encode_with_0x(bytes)),
        KeyEncoding::Base58 => Ok(bs58::encode(bytes).into_string()),
        KeyEncoding::WifCompressed => encode_wif_compressed(bytes),
        KeyEncoding::PrefixedBase58(_) | KeyEncoding::StellarStrKey => Err(SignerError::invalid_input("Unsupported private key export encoding")),
    }
}

pub fn decode_private_key(chain: &Chain, value: &str) -> Result<Zeroizing<Vec<u8>>, SignerError> {
    let scheme = scheme_for_chain(chain);
    let key = if *chain == Chain::BitcoinSv {
        let value = value.trim();
        let encoding = if value.starts_with("0x") || value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            KeyEncoding::Hex
        } else {
            KeyEncoding::WifCompressed
        };
        decode_key(value, &[encoding], scheme)?
    } else {
        decode_key(value, import_encodings_for_chain(chain), scheme)?
    };
    if *chain == Chain::BitcoinSv && key.len() != SECP256K1_PRIVATE_KEY_LENGTH {
        return Err(SignerError::invalid_input("BSV private key must contain exactly 32 bytes"));
    }
    Ok(key)
}

pub fn encode_private_key(chain: &Chain, private_key: &[u8]) -> Result<String, SignerError> {
    encode_key(private_key, export_encoding_for_chain(chain))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::TEST_PRIVATE_KEY;

    fn bsv_wif_fixture(private_key: &[u8], version: u8, compressed: Option<u8>) -> String {
        let mut payload = Zeroizing::new(Vec::with_capacity(34));
        payload.push(version);
        payload.extend_from_slice(private_key);
        if let Some(compressed) = compressed {
            payload.push(compressed);
        }
        bs58::encode(payload.as_slice()).with_check().into_string()
    }

    #[test]
    fn test_bsv_wif_rejects_other_network_uncompressed_invalid_flag_checksum_and_scalars() {
        let key = Zeroizing::new(decode_hex(TEST_PRIVATE_KEY).unwrap());
        let valid = Zeroizing::new(bsv_wif_fixture(&key, 0x80, Some(0x01)));
        assert!(decode_private_key(&Chain::BitcoinSv, &valid).unwrap().as_slice() == key.as_slice());
        for (version, compressed) in [(0xef, Some(0x01)), (0x81, Some(0x01)), (0x80, None), (0x80, Some(0x00)), (0x80, Some(0x02))] {
            assert!(decode_private_key(&Chain::BitcoinSv, &bsv_wif_fixture(&key, version, compressed)).is_err());
        }
        let mut invalid_checksum = Zeroizing::new(bs58::decode(valid.as_str()).into_vec().unwrap());
        *invalid_checksum.last_mut().unwrap() ^= 1;
        let invalid_checksum = Zeroizing::new(bs58::encode(invalid_checksum.as_slice()).into_string());
        assert!(decode_private_key(&Chain::BitcoinSv, &invalid_checksum).is_err());
        for invalid_scalar in [vec![0; 32], vec![0xff; 32], vec![1; 31], vec![1; 33]] {
            let invalid_scalar = Zeroizing::new(invalid_scalar);
            assert!(decode_private_key(&Chain::BitcoinSv, &bsv_wif_fixture(&invalid_scalar, 0x80, Some(0x01))).is_err());
            assert!(encode_private_key(&Chain::BitcoinSv, &invalid_scalar).is_err());
        }
        assert!(decode_private_key(&Chain::BitcoinSv, &format!("0x{}", valid.as_str())).is_err());
        assert!(decode_private_key(&Chain::BitcoinSv, &bs58::encode(key.as_slice()).into_string()).is_err());
        assert!(decode_private_key(&Chain::Bitcoin, &valid).is_err());
        assert!(decode_private_key(&Chain::BitcoinCash, &valid).is_err());
    }

    #[test]
    fn test_bsv_private_key_import_export_roundtrip_and_invalid_keys() {
        let private_key = Zeroizing::new(vec![9; 32]);
        let exported = encode_private_key(&Chain::BitcoinSv, &private_key).unwrap();
        let payload = Zeroizing::new(bs58::decode(&exported).with_check(None).into_vec().unwrap());
        assert_eq!((payload.len(), payload[0], payload[33]), (34, 0x80, 0x01));
        assert!(&payload[1..33] == private_key.as_slice());
        let old_export = Zeroizing::new(format!("0x{}", hex::encode(private_key.as_slice())));
        assert!(decode_private_key(&Chain::BitcoinSv, &old_export).unwrap().as_slice() == private_key.as_slice());
        assert_eq!(decode_private_key(&Chain::BitcoinSv, &exported).unwrap().as_slice(), private_key.as_slice());
        assert_eq!(
            decode_private_key(&Chain::BitcoinSv, &hex::encode(private_key.as_slice())).unwrap().as_slice(),
            private_key.as_slice()
        );
        assert!(decode_private_key(&Chain::BitcoinSv, &hex::encode([0; 32])).is_err());
        assert!(decode_private_key(&Chain::BitcoinSv, &hex::encode([9; 31])).is_err());
        assert!(decode_private_key(&Chain::Bitcoin, &exported).is_err());
    }

    #[test]
    fn test_decode_solana_base58() {
        let bytes = decode_private_key(&Chain::Solana, "4ha2npeRkDXipjgGJ3L5LhZ9TK9dRjP2yktydkFBhAzXj3N8ytpYyTS24kxcYGEefy4WKWRcog2zSPvpPZoGmxCC").unwrap();
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn test_decode_ethereum_hex() {
        let bytes = decode_private_key(&Chain::Ethereum, "0x30df0ffc2b43717f4653c2a1e827e9dfb3d9364e019cc60092496cd4997d5d6e").unwrap();
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn test_decode_stellar_strkey() {
        let bytes = decode_private_key(&Chain::Stellar, "SA6XNHUKMW4QAKSHB2NOZ4SYP34ERYVAWSBTEDREYSJ2LEJ5LFHLTIRJ").unwrap();
        assert_eq!(hex::encode(bytes.as_slice()), "3d769e8a65b9002a470e9aecf2587ef848e2a0b483320e24c493a5913d594eb9");
    }

    #[test]
    fn test_decode_near_prefixed_base58() {
        let private_key = [7u8; 32];
        let keypair = ed25519_dalek::SigningKey::from_bytes(&private_key).to_keypair_bytes();
        let values = [
            format!("ed25519:{}", bs58::encode(private_key).into_string()),
            format!("ed25519:{}", bs58::encode(keypair).into_string()),
        ];

        for value in values {
            assert_eq!(decode_private_key(&Chain::Near, &value).unwrap().as_slice(), private_key);
            assert!(decode_private_key(&Chain::Solana, &value).is_err());
            assert!(decode_private_key(&Chain::Ethereum, &value).is_err());
        }
    }

    #[test]
    fn test_decode_invalid() {
        let mut mismatched_keypair = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]).to_keypair_bytes();
        mismatched_keypair[63] ^= 1;

        assert!(decode_private_key(&Chain::Ethereum, "not_valid").is_err());
        assert!(decode_private_key(&Chain::Stellar, "GA6XNHUKMW4QAKSHB2NOZ4SYP34ERYVAWSBTEDREYSJ2LEJ5LFHLTIRJ").is_err());
        assert!(decode_private_key(&Chain::Near, "ed25519:").is_err());
        assert!(decode_private_key(&Chain::Near, "secp256k1:11111111111111111111111111111111").is_err());
        assert!(decode_private_key(&Chain::Near, "ed25519:1111111111111111111111111111111").is_err());
        assert!(decode_private_key(&Chain::Near, &format!("ed25519:{}", bs58::encode(mismatched_keypair).into_string())).is_err());
    }

    #[test]
    fn test_encode_solana_base58() {
        let bytes = decode_private_key(&Chain::Solana, "4ha2npeRkDXipjgGJ3L5LhZ9TK9dRjP2yktydkFBhAzXj3N8ytpYyTS24kxcYGEefy4WKWRcog2zSPvpPZoGmxCC").unwrap();
        let encoded = encode_private_key(&Chain::Solana, &bytes).unwrap();

        assert_eq!(encoded, "DTJi5pMtSKZHdkLX4wxwvjGjf2xwXx1LSuuUZhugYWDV");
    }

    #[test]
    fn test_encode_ethereum_hex() {
        let bytes = decode_private_key(&Chain::Ethereum, "0x30df0ffc2b43717f4653c2a1e827e9dfb3d9364e019cc60092496cd4997d5d6e").unwrap();
        let encoded = encode_private_key(&Chain::Ethereum, &bytes).unwrap();

        assert_eq!(encoded, "0x30df0ffc2b43717f4653c2a1e827e9dfb3d9364e019cc60092496cd4997d5d6e");
    }

    #[test]
    fn test_encode_bitcoin_base58() {
        let bytes = decode_private_key(&Chain::Ethereum, "0x30df0ffc2b43717f4653c2a1e827e9dfb3d9364e019cc60092496cd4997d5d6e").unwrap();
        let encoded = encode_private_key(&Chain::Bitcoin, &bytes).unwrap();

        assert_eq!(encoded, "4Hmr8TxnwVB7m6fzfPRcASMn2hLRgzUz3gDGwF4ZnpVK");
    }

    #[test]
    fn test_encode_does_not_revalidate_bytes() {
        assert_eq!(encode_private_key(&Chain::Ethereum, &[1u8; 16]).unwrap(), "0x01010101010101010101010101010101");
        assert_eq!(encode_private_key(&Chain::Solana, &[1u8; 64]).unwrap(), bs58::encode([1u8; 64]).into_string());
    }

    #[test]
    fn test_stellar_bad_checksum() {
        // Valid stellar key with last char changed to corrupt checksum
        assert!(decode_private_key(&Chain::Stellar, "SA6XNHUKMW4QAKSHB2NOZ4SYP34ERYVAWSBTEDREYSJ2LEJ5LFHLTIRA").is_err());
    }

    #[test]
    fn test_supports_private_key_import() {
        assert!(supports_private_key_import(&Chain::Ethereum));
        assert!(supports_private_key_import(&Chain::Solana));
        assert!(supports_private_key_import(&Chain::Stellar));
        assert!(!supports_private_key_import(&Chain::Cardano));
        assert!(!supports_private_key_import(&Chain::Bitcoin));
        assert!(!supports_private_key_import(&Chain::Litecoin));
        assert!(!supports_private_key_import(&Chain::Doge));
    }

    #[test]
    fn test_base58_rejects_unexpected_length() {
        // 96-byte base58 payload should not be accepted
        let long_key = bs58::encode(vec![1u8; 96]).into_string();
        assert!(decode_base58(&long_key).is_none());
    }
}
