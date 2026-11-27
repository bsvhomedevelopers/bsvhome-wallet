use std::borrow::Cow;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use bs58;
use bsv_home_bsv::signer::BsvChainSigner;
use bsv_home_evm::eip712::hash_typed_data;
use bsv_home_evm::message::eip191_hash_message;
use bsv_home_solana::{signer::SolanaChainSigner, siws::SiwsMessage};
use bsv_home_sui::signer as sui_signer;
use bsv_home_ton::address::base64_to_hex_address;
use bsv_home_ton::signer::{TonSignDataResponse, TonSignMessageData, TonSignResult, TonSigner};
use primitives::hex::encode_with_0x;
use primitives::unix_seconds;
use signer::{SIGNATURE_LENGTH, Signer, ensure_ethereum_signature_recovery_id_offset};
use sui_types::PersonalMessage;

use super::{
    eip712::BsvHomeEIP712Message,
    payload::MessagePayloadPreview,
    sign_type::{SignDigestType, SignMessage},
};
use crate::{BsvHomeCoreError, keystore::BsvHomeKeystore, siwe::SiweMessage};
use bsv_home_tron::signer::tron_hash_message;
use primitives::{Chain, ChainSigner, SimulationPayloadField};
use std::sync::Arc;
use zeroize::Zeroizing;

fn siwe_or_text_preview(chain: primitives::Chain, data: &[u8]) -> MessagePreview {
    match String::from_utf8(data.to_vec()) {
        Ok(string) => match SiweMessage::try_parse(&string) {
            Some(message) if message.validate(chain).is_ok() => MessagePreview::Siwe(message),
            Some(_) | None => MessagePreview::Text(string),
        },
        Err(_) => MessagePreview::Text(encode_with_0x(data)),
    }
}

#[derive(Debug, PartialEq, uniffi::Enum)]
pub enum MessagePreview {
    Text(String),
    EIP712(BsvHomeEIP712Message),
    Siwe(SiweMessage),
}

#[derive(Debug, uniffi::Object)]
pub struct MessageSigner {
    pub message: SignMessage,
    timestamp: u64,
}

#[uniffi::export]
impl MessageSigner {
    #[uniffi::constructor]
    pub fn new(message: SignMessage) -> Self {
        let timestamp = unix_seconds().unwrap_or(0);
        Self { message, timestamp }
    }

    pub fn preview(&self) -> Result<MessagePreview, BsvHomeCoreError> {
        match self.message.sign_type {
            SignDigestType::Eip191 | SignDigestType::Siwe => Ok(siwe_or_text_preview(self.message.chain, &self.message.data)),
            SignDigestType::SuiPersonal | SignDigestType::TronPersonal => Ok(MessagePreview::Text(self.data_as_utf8_or_hex())),
            SignDigestType::BitcoinMessage => Ok(MessagePreview::Text(self.bitcoin_message()?)),
            SignDigestType::TonPersonal => {
                let string = self.data_as_utf8()?;
                let Ok(ton_data) = TonSignMessageData::from_bytes(string.as_bytes()) else {
                    return Ok(MessagePreview::Text(string));
                };
                Ok(MessagePreview::Text(ton_data.payload.data().to_string()))
            }
            SignDigestType::Eip712 => {
                let string = self.data_as_utf8()?;
                if string.is_empty() {
                    return Err(BsvHomeCoreError::from("Empty EIP712 message string"));
                }
                let message = BsvHomeEIP712Message::from_json(&string).map_err(|e| BsvHomeCoreError::from(format!("Invalid EIP712 message: {e}")))?;
                Ok(MessagePreview::EIP712(message))
            }
            SignDigestType::Base58 => {
                let decoded = bs58::decode(&self.message.data).into_vec().unwrap_or_default();
                Ok(MessagePreview::Text(String::from_utf8_lossy(&decoded).to_string()))
            }
        }
    }

    pub fn payload_preview(&self, simulation_payload: Vec<SimulationPayloadField>) -> Result<Option<MessagePayloadPreview>, BsvHomeCoreError> {
        let payload_preview = match self.preview()? {
            MessagePreview::Text(_) => match self.message.sign_type {
                SignDigestType::Eip191 | SignDigestType::Siwe => self.siwe_payload_preview(simulation_payload),
                SignDigestType::Base58 => self.siws_payload_preview(simulation_payload)?,
                _ => None,
            },
            MessagePreview::EIP712(message) => Some(message.payload_preview(simulation_payload)),
            MessagePreview::Siwe(message) => Some(MessagePayloadPreview::from_siwe(&message, simulation_payload)),
        };

        Ok(payload_preview)
    }

    pub fn hash(&self) -> Result<Vec<u8>, BsvHomeCoreError> {
        match &self.message.sign_type {
            SignDigestType::SuiPersonal => {
                let message = PersonalMessage(Cow::Borrowed(&self.message.data));
                Ok(message.signing_digest().to_vec())
            }
            SignDigestType::TonPersonal => {
                let string = self.data_as_utf8()?;
                let ton_data = TonSignMessageData::from_bytes(string.as_bytes())?;
                Ok(ton_data.hash(self.timestamp)?)
            }
            SignDigestType::TronPersonal => Ok(tron_hash_message(&self.message.data).to_vec()),
            SignDigestType::BitcoinMessage => Ok(BsvChainSigner::message_hash(&self.bitcoin_message()?).to_vec()),
            SignDigestType::Eip191 | SignDigestType::Siwe => Ok(eip191_hash_message(&self.message.data).to_vec()),
            SignDigestType::Eip712 => {
                let json = self.data_as_utf8()?;
                let digest = hash_typed_data(&json)?;
                Ok(digest.to_vec())
            }
            SignDigestType::Base58 => bs58::decode(&self.message.data).into_vec().map_err(|e| BsvHomeCoreError::from(e.to_string())),
        }
    }

    pub fn sign_with_keystore(&self, keystore: Arc<BsvHomeKeystore>, keystore_id: String, password: Vec<u8>) -> Result<String, BsvHomeCoreError> {
        let private_key = keystore.signing_key(&keystore_id, self.message.chain, password)?;
        self.sign(private_key)
    }
}

impl MessageSigner {
    pub fn plain_preview(&self) -> String {
        match self.message.sign_type {
            SignDigestType::SuiPersonal | SignDigestType::Eip191 | SignDigestType::TronPersonal | SignDigestType::BitcoinMessage => self.data_as_utf8_or_hex(),
            SignDigestType::Base58 => match self.preview() {
                Ok(MessagePreview::Text(preview)) => preview,
                _ => "".to_string(),
            },
            SignDigestType::TonPersonal => match self.preview() {
                Ok(MessagePreview::Text(preview)) => preview,
                _ => self.data_as_utf8_or_hex(),
            },
            SignDigestType::Siwe => self.data_as_utf8_or_hex(),
            SignDigestType::Eip712 => {
                let value: serde_json::Value = serde_json::from_slice(&self.message.data).unwrap_or_default();
                serde_json::to_string_pretty(&value).unwrap_or_default()
            }
        }
    }
}

impl MessageSigner {
    pub fn get_result(&self, data: &[u8]) -> String {
        match &self.message.sign_type {
            SignDigestType::Eip191 | SignDigestType::Eip712 | SignDigestType::Siwe | SignDigestType::TronPersonal => {
                if data.len() < SIGNATURE_LENGTH {
                    return encode_with_0x(data);
                }
                let mut signature = data.to_vec();
                ensure_ethereum_signature_recovery_id_offset(&mut signature);
                encode_with_0x(&signature)
            }
            SignDigestType::SuiPersonal | SignDigestType::TonPersonal | SignDigestType::BitcoinMessage => BASE64.encode(data),
            SignDigestType::Base58 => bs58::encode(data).into_string(),
        }
    }
}

impl MessageSigner {
    pub fn sign(&self, private_key: Zeroizing<Vec<u8>>) -> Result<String, BsvHomeCoreError> {
        match &self.message.sign_type {
            SignDigestType::SuiPersonal => {
                let hash = self.hash()?;
                sui_signer::sign_digest(&hash, &private_key).map_err(BsvHomeCoreError::from)
            }
            SignDigestType::TonPersonal => {
                let result = TonSigner::new(&private_key)?.sign_personal(&self.message.data, self.timestamp)?;
                self.get_ton_result(&result)
            }
            SignDigestType::BitcoinMessage => {
                let signature = BsvChainSigner::sign_message_compact(&self.bitcoin_message()?, &private_key)?;
                Ok(BASE64.encode(signature))
            }
            SignDigestType::Eip191 | SignDigestType::Eip712 | SignDigestType::Siwe | SignDigestType::TronPersonal => {
                let hash = self.hash()?;
                let signature = Signer::sign_ethereum_digest(&hash, &private_key)?;
                Ok(encode_with_0x(&signature))
            }
            SignDigestType::Base58 => {
                if self.message.chain != Chain::Solana {
                    return Err(BsvHomeCoreError::from(format!("Base58 sign message is not supported for {:?}", self.message.chain)));
                }
                let message = self.hash()?;
                SolanaChainSigner.sign_message(&message, private_key.as_slice()).map_err(BsvHomeCoreError::from)
            }
        }
    }

    fn data_as_utf8(&self) -> Result<String, BsvHomeCoreError> {
        Ok(String::from_utf8(self.message.data.clone())?)
    }

    fn bitcoin_message(&self) -> Result<String, BsvHomeCoreError> {
        if self.message.chain != Chain::BitcoinSv {
            return Err(BsvHomeCoreError::from("Bitcoin Message signing is only supported for BSV"));
        }
        self.data_as_utf8()
    }

    fn data_as_utf8_or_hex(&self) -> String {
        String::from_utf8(self.message.data.clone()).unwrap_or_else(|_| encode_with_0x(&self.message.data))
    }

    fn siwe_payload_preview(&self, simulation_payload: Vec<SimulationPayloadField>) -> Option<MessagePayloadPreview> {
        let string = String::from_utf8(self.message.data.clone()).ok()?;
        let message = SiweMessage::try_parse(&string)?;
        Some(MessagePayloadPreview::from_siwe(&message, simulation_payload))
    }

    fn siws_payload_preview(&self, simulation_payload: Vec<SimulationPayloadField>) -> Result<Option<MessagePayloadPreview>, BsvHomeCoreError> {
        let Ok(string) = String::from_utf8(self.hash()?) else {
            return Ok(None);
        };
        Ok(SiwsMessage::parse(&string)
            .map_err(BsvHomeCoreError::from)?
            .map(|message| MessagePayloadPreview::from_siws(&message, simulation_payload)))
    }

    fn get_ton_result(&self, result: &TonSignResult) -> Result<String, BsvHomeCoreError> {
        let string = self.data_as_utf8()?;
        let data = TonSignMessageData::from_bytes(string.as_bytes())?;
        let raw_address = base64_to_hex_address(&data.address).ok_or_else(|| BsvHomeCoreError::from("Invalid TON address"))?;

        let response = TonSignDataResponse {
            signature: BASE64.encode(&result.signature),
            address: raw_address,
            timestamp: result.timestamp,
            domain: data.domain,
            payload: data.payload,
        };

        serde_json::to_string(&response).map_err(|e| BsvHomeCoreError::from(e.to_string()))
    }
}

pub(crate) fn recover_bitcoin_message_address(message: &[u8], signature: &str) -> Result<String, BsvHomeCoreError> {
    let message = String::from_utf8(message.to_vec())?;
    let signature = BASE64.decode(signature).map_err(|_| BsvHomeCoreError::from("Invalid BSV message signature Base64"))?;
    BsvChainSigner::recover_message_address(&message, &signature).map_err(BsvHomeCoreError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystore::BsvHomeImportType;
    use crate::message::{
        eip712::{BsvHomeEIP712Section, BsvHomeEIP712Value, BsvHomeEIP712ValueType},
        sign_type::{MessageType, SignDigestType},
    };
    use crate::signer::ChainTransactionSigner;
    use bsv_home_evm::EIP712Domain;
    use primitives::Address;
    use primitives::testkit::ABANDON_PHRASE;
    use primitives::testkit::signer_mock::TEST_PRIVATE_KEY;
    use signer::Ed25519KeyPair;
    use tempfile::TempDir;

    #[test]
    fn test_bitcoin_message_preserves_utf8_and_compact_signature() {
        for (message, expected_hash, expected_signature) in [
            (
                "Hello Bitcoin",
                "93a4e556613458adb2019c52d7dbaff7a7261da4bc4b8b3f8b9c5f098209de37",
                "H9IDomnDy4ttQzjVX2JY1Wk3Ir832xJTKbMaalAXOnVOBAUxHYeiVeVn0eh0quZUuCrPKov/wqzYCJCQtyv5Gvc=",
            ),
            (
                "WowPay 登录\n地址: 1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD\nNonce: nonce-123\n",
                "19bb32dbbbaf2cadde0f033558ca60dbf42171cc00315df64174fa2851a0e806",
                "IJF76z5dyTwfRTLKV8nGRiMKjXgPPgZoLPkO+tUld1AoNvn0pe2KSpBxZJxzCHpAJgvE2/VIqujRqu+lNvgBv6o=",
            ),
        ] {
            let signer = MessageSigner::new(SignMessage {
                chain: Chain::BitcoinSv,
                sign_type: SignDigestType::BitcoinMessage,
                data: message.as_bytes().to_vec(),
            });
            assert_eq!(signer.preview().unwrap(), MessagePreview::Text(message.to_string()));
            assert_eq!(signer.plain_preview(), message);
            assert_eq!(hex::encode(signer.hash().unwrap()), expected_hash);
            assert_eq!(signer.sign(Zeroizing::new(TEST_PRIVATE_KEY.to_vec())).unwrap(), expected_signature);
            assert_eq!(
                recover_bitcoin_message_address(message.as_bytes(), expected_signature).unwrap(),
                "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD"
            );
            if message.ends_with('\n') {
                assert_ne!(
                    recover_bitcoin_message_address(message.trim_end().as_bytes(), expected_signature).unwrap(),
                    "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD"
                );
            }
        }
    }

    #[test]
    fn test_bitcoin_message_recovers_standard_signature_and_rejects_invalid_input() {
        let message = b"rust-bitcoin MessageSignature test";
        let signature = "IAM2qX24tYx/bdBTIgVLhD8QEAjrPlJpmjB4nZHdRYGIBa4DmVulAcwjPnWe6Q5iEwXH6F0pUCJP/ZeHPWS1h1o=";
        assert_eq!(recover_bitcoin_message_address(message, signature).unwrap(), "1DUm3t5rF5yMD6r1VbCmgbPDe6LAVYVGrQ");
        assert_ne!(
            recover_bitcoin_message_address(b"different message", signature).unwrap(),
            "1DUm3t5rF5yMD6r1VbCmgbPDe6LAVYVGrQ"
        );
        assert!(recover_bitcoin_message_address(&[0xff], signature).is_err());
        assert!(recover_bitcoin_message_address(message, "not Base64").is_err());
        let mut compact = BASE64.decode(signature).unwrap();
        assert!(recover_bitcoin_message_address(message, &BASE64.encode(&compact[..64])).is_err());
        for header in [0, 27, 30, 35, 39, 255] {
            compact[0] = header;
            assert!(recover_bitcoin_message_address(message, &BASE64.encode(&compact)).is_err());
        }
        for (chain, data) in [(Chain::Bitcoin, message.to_vec()), (Chain::BitcoinSv, vec![0xff])] {
            let signer = MessageSigner::new(SignMessage {
                chain,
                sign_type: SignDigestType::BitcoinMessage,
                data,
            });
            assert!(signer.preview().is_err());
            assert!(signer.hash().is_err());
            assert!(signer.sign(Zeroizing::new(TEST_PRIVATE_KEY.to_vec())).is_err());
        }
    }

    #[test]
    fn test_bitcoin_message_sign_with_keystore_uses_bsv_account() {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().to_string()).unwrap();
        let stored = keystore
            .create_store(
                BsvHomeImportType::MulticoinPhrase {
                    words: ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                    chains: vec![Chain::BitcoinSv],
                },
                b"password".to_vec(),
            )
            .unwrap();
        let message = b"WowPay Login\nNonce: test-keystore-challenge\n";
        let signer = MessageSigner::new(SignMessage {
            chain: Chain::BitcoinSv,
            sign_type: SignDigestType::BitcoinMessage,
            data: message.to_vec(),
        });
        let signature = signer.sign_with_keystore(keystore.clone(), stored.keystore_id.clone(), b"password".to_vec()).unwrap();
        assert_eq!(recover_bitcoin_message_address(message, &signature).unwrap(), stored.accounts[0].address);
        assert!(signer.sign_with_keystore(keystore, stored.keystore_id, b"wrong-password".to_vec()).is_err());
    }

    #[test]
    fn test_eip712_chain_signer_matches_message_signer() {
        let json = include_str!("./test/eip712_seaport.json");
        let via_chain_signer = ChainTransactionSigner::new(Chain::Ethereum)
            .sign_message(json.as_bytes().to_vec(), TEST_PRIVATE_KEY.to_vec())
            .unwrap();
        let via_message_signer = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip712,
            data: json.as_bytes().to_vec(),
        })
        .sign(Zeroizing::new(TEST_PRIVATE_KEY.to_vec()))
        .unwrap();
        assert_eq!(via_chain_signer, via_message_signer);
    }

    #[test]
    fn test_eip191() {
        let data = b"hello world".to_vec();
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip191,
            data,
        });
        match decoder.preview() {
            Ok(MessagePreview::Text(preview)) => assert_eq!(preview, "hello world"),
            _ => panic!("Unexpected preview result"),
        }

        let hash = decoder.hash().unwrap();
        assert_eq!(encode_with_0x(&hash), "0xd9eba16ed0ecae432b71fe008c98cc872bb4cc214d3220a36f365326cf807d68");
    }

    #[test]
    fn test_eip191_hex_value() {
        // 0x74657374 corresponds to "test" in UTF-8
        let data = hex::decode("74657374").expect("Invalid hex string");
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip191,
            data,
        });
        match decoder.preview() {
            Ok(MessagePreview::Text(preview)) => assert_eq!(preview, "test"),
            _ => panic!("Unexpected preview result"),
        }
    }

    #[test]
    fn test_eip191_non_utf8_hex_value() {
        // 0xdeadbeef is not valid UTF-8
        let data = hex::decode("deadbeef").expect("Invalid hex string");
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip191,
            data,
        });
        match decoder.preview() {
            // Since 0xdeadbeef is not valid UTF-8, preview should show the hex representation
            Ok(MessagePreview::Text(preview)) => assert_eq!(preview, "0xdeadbeef"),
            _ => panic!("Unexpected preview result"),
        }
    }

    #[test]
    fn test_eip191_plain_preview_for_siwe() {
        let message = r#"thepoc.xyz wants you to sign in with your Ethereum account:
0xBA4D1d35bCe0e8F28E5a3403e7a0b996c5d50AC4

Sign in with different chain ID

URI: https://thepoc.xyz
Version: 1
Chain ID: 137
Nonce: byjof9dwrao97skautdxhb
Issued At: 2026-03-09T15:48:34.458Z"#;
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip191,
            data: message.as_bytes().to_vec(),
        });

        assert_eq!(decoder.plain_preview(), message);
    }

    #[test]
    fn test_get_result_eip191() {
        let data = hex::decode("d80c5ffe75fcbac0706c5c5d3b8884ae3588c30065a95075e07fa6ebc24e56433e5030992ef438b1d23437ec8d66d3197b1ad92f85222af1624d8f295907a65800")
            .expect("Invalid hex string");
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip191,
            data: data.clone(),
        });
        let result = decoder.get_result(data.as_slice());
        assert_eq!(
            result,
            "0xd80c5ffe75fcbac0706c5c5d3b8884ae3588c30065a95075e07fa6ebc24e56433e5030992ef438b1d23437ec8d66d3197b1ad92f85222af1624d8f295907a6581b"
        );
    }

    #[test]
    fn test_get_result_recovery_id_conversion() {
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip191,
            data: b"test".to_vec(),
        });

        // Raw recovery ID 0 -> 27 (0x1b)
        let mut sig = vec![0u8; 65];
        sig[64] = 0;
        assert!(decoder.get_result(&sig).ends_with("1b"));

        // Raw recovery ID 1 -> 28 (0x1c)
        sig[64] = 1;
        assert!(decoder.get_result(&sig).ends_with("1c"));

        // Already converted IDs stay unchanged
        sig[64] = 27;
        assert!(decoder.get_result(&sig).ends_with("1b"));

        sig[64] = 28;
        assert!(decoder.get_result(&sig).ends_with("1c"));
    }

    #[test]
    fn test_sui_personal_message_hash() {
        let data = b"Hello, world!".to_vec();
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Sui,
            sign_type: SignDigestType::SuiPersonal,
            data: data.clone(),
        });

        let hash = decoder.hash().unwrap();
        let expected_hash = PersonalMessage(Cow::Owned(data)).signing_digest().to_vec();
        assert_eq!(hash, expected_hash);

        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Sui,
            sign_type: SignDigestType::SuiPersonal,
            data: b"Hello, world!".to_vec(),
        });
        let mut signature = vec![0u8; 97];
        signature[0] = 0;
        signature[96] = 1;
        let expected = BASE64.encode(&signature);
        assert_eq!(decoder.get_result(&signature), expected);
    }

    #[test]
    fn test_base58() {
        let message = "X3CUgCGzyn43DTAbUKnTMDzcGWMooJT2hPSZinjfN1QUgVNYYfeoJ5zg6i4Nd5coKGUrNpEYVoD";
        let data = message.as_bytes().to_vec();
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Solana,
            sign_type: SignDigestType::Base58,
            data: data.clone(),
        });

        match decoder.preview() {
            Ok(MessagePreview::Text(preview)) => assert_eq!(preview, "This is an example message to be signed - 1747125759060"),
            _ => panic!("Unexpected preview result for base58"),
        }
        let hash = decoder.hash().unwrap();

        assert_eq!(
            hex::encode(&hash),
            "5468697320697320616e206578616d706c65206d65737361676520746f206265207369676e6564202d2031373437313235373539303630"
        );

        assert_eq!(decoder.payload_preview(vec![]).unwrap(), None);

        let result_data = b"StV1DL6CwTryKyV"; // Data to pass to get_result, mimicking Swift test
        let result = decoder.get_result(result_data);

        assert_eq!(result, "3LRFsmWKLfsR7G5PqjytR");
    }

    #[test]
    fn test_siws_preview_preserves_signed_message() {
        let key_pair = Ed25519KeyPair::from_private_key(&TEST_PRIVATE_KEY).unwrap();
        let message = include_str!("../../../crates/bsv_home_solana/testdata/siws_sign_in.txt");
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Solana,
            sign_type: SignDigestType::Base58,
            data: bs58::encode(message.as_bytes()).into_string().into_bytes(),
        });

        assert_eq!(decoder.preview().unwrap(), MessagePreview::Text(message.to_string()));
        assert_eq!(decoder.plain_preview(), message);
        assert_eq!(decoder.hash().unwrap(), message.as_bytes());
        assert_eq!(decoder.payload_preview(vec![]).unwrap().unwrap().message_type, MessageType::Siws);
        assert_eq!(
            decoder.sign(Zeroizing::new(TEST_PRIVATE_KEY.to_vec())).unwrap(),
            bs58::encode(key_pair.sign(message.as_bytes())).into_string(),
        );
    }

    #[test]
    fn test_siws_payload_preview_rejects_malformed_message() {
        let message = format!("{}\nUnknown: hidden field", include_str!("../../../crates/bsv_home_solana/testdata/siws_sign_in.txt"));
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Solana,
            sign_type: SignDigestType::Base58,
            data: bs58::encode(message.as_bytes()).into_string().into_bytes(),
        });

        assert!(decoder.payload_preview(vec![]).is_err());
        assert_eq!(decoder.preview().unwrap(), MessagePreview::Text(message.clone()));
        assert_eq!(decoder.hash().unwrap(), message.as_bytes());
    }

    #[test]
    fn test_base58_sign_rejects_non_solana_chain() {
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Base58,
            data: b"hello".to_vec(),
        });

        assert_eq!(
            decoder.sign(Zeroizing::new(TEST_PRIVATE_KEY.to_vec())).unwrap_err().to_string(),
            "Base58 sign message is not supported for ethereum"
        );
    }

    #[test]
    fn test_eip712_hash() {
        let json_str = include_str!("./test/eip712_seaport.json");
        let hash = hash_typed_data(json_str).unwrap();

        assert_eq!(hex::encode(hash), "0b8aa9f3712df0034bc29fe5b24dd88cfdba02c7f499856ab24632e2969709a8",);

        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip712,
            data: json_str.as_bytes().to_vec(),
        });
        let preview = decoder.preview().unwrap();
        assert_eq!(
            preview,
            MessagePreview::EIP712(BsvHomeEIP712Message {
                domain: EIP712Domain {
                    name: Some("Seaport".to_string()),
                    version: Some("1.1".to_string()),
                    chain_id: Some(1),
                    verifying_contract: Some("0x00000000006c3852cbEf3e08E8dF289169EdE581".to_string()),
                    salts: None,
                },
                message: vec![BsvHomeEIP712Section {
                    name: "OrderComponents".to_string(),
                    values: vec![
                        BsvHomeEIP712Value {
                            name: "offerer".to_string(),
                            value: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_string(),
                            value_type: BsvHomeEIP712ValueType::Address,
                        },
                        BsvHomeEIP712Value {
                            name: "zone".to_string(),
                            value: "0x004C00500000aD104D7DBd00e3ae0A5C00560C00".to_string(),
                            value_type: BsvHomeEIP712ValueType::Address,
                        },
                        BsvHomeEIP712Value {
                            name: "offer.token".to_string(),
                            value: "0xA604060890923Ff400e8c6f5290461A83AEDACec".to_string(),
                            value_type: BsvHomeEIP712ValueType::Address,
                        },
                        BsvHomeEIP712Value {
                            name: "startTime".to_string(),
                            value: "1658645591".to_string(),
                            value_type: BsvHomeEIP712ValueType::Text,
                        },
                        BsvHomeEIP712Value {
                            name: "endTime".to_string(),
                            value: "1659250386".to_string(),
                            value_type: BsvHomeEIP712ValueType::Text,
                        },
                        BsvHomeEIP712Value {
                            name: "zoneHash".to_string(),
                            value: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                            value_type: BsvHomeEIP712ValueType::Text,
                        },
                        BsvHomeEIP712Value {
                            name: "salt".to_string(),
                            value: "16178208897136618".to_string(),
                            value_type: BsvHomeEIP712ValueType::Text,
                        },
                        BsvHomeEIP712Value {
                            name: "conduitKey".to_string(),
                            value: "0x0000007b02230091a7ed01230072f7006a004d60a8d4e71d599b8104250f0000".to_string(),
                            value_type: BsvHomeEIP712ValueType::Text,
                        },
                        BsvHomeEIP712Value {
                            name: "counter".to_string(),
                            value: "0".to_string(),
                            value_type: BsvHomeEIP712ValueType::Text,
                        },
                    ],
                }],
            })
        );
    }

    #[test]
    fn test_eip712_hyperliquid_approve_agent_hash() {
        let json_str = include_str!("../../../crates/bsv_home_hypercore/testdata/hl_eip712_approve_agent.json");
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip712,
            data: json_str.as_bytes().to_vec(),
        });

        let digest = decoder.hash().unwrap();
        assert_eq!(hex::encode(digest), "480af9fd3cdc70c2f8a521388be13620d16a0f643d9cffdfbb65cd019cc27537");
    }

    #[test]
    fn test_eip712_ploymarket_hash() {
        let json_str = include_str!("./test/eip712_polymarket.json");

        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Polygon,
            sign_type: SignDigestType::Eip712,
            data: json_str.as_bytes().to_vec(),
        });
        let preview = decoder.preview().unwrap();
        assert_eq!(
            preview,
            MessagePreview::EIP712(BsvHomeEIP712Message {
                domain: EIP712Domain {
                    name: Some("ClobAuthDomain".to_string()),
                    version: Some("1".to_string()),
                    chain_id: Some(137),
                    verifying_contract: None,
                    salts: None,
                },
                message: vec![BsvHomeEIP712Section {
                    name: "ClobAuth".to_string(),
                    values: vec![
                        BsvHomeEIP712Value {
                            name: "address".to_string(),
                            value: "0x514BCb1F9AAbb904e6106Bd1052B66d2706dBbb7".to_string(),
                            value_type: BsvHomeEIP712ValueType::Address,
                        },
                        BsvHomeEIP712Value {
                            name: "timestamp".to_string(),
                            value: "1752326774".to_string(),
                            value_type: BsvHomeEIP712ValueType::Timestamp,
                        },
                        BsvHomeEIP712Value {
                            name: "nonce".to_string(),
                            value: "0".to_string(),
                            value_type: BsvHomeEIP712ValueType::Text,
                        },
                        BsvHomeEIP712Value {
                            name: "message".to_string(),
                            value: "This message attests that I control the given wallet".to_string(),
                            value_type: BsvHomeEIP712ValueType::Text,
                        },
                    ],
                }],
            })
        );
    }

    #[test]
    fn test_siwe_preview() {
        let message = [
            "login.xyz wants you to sign in with your Ethereum account:",
            "0x6dD7802E6d44bE89a789C4bD60bD511B68F41c7c",
            "",
            "URI: https://login.xyz",
            "Version: 1",
            "Chain ID: 1",
            "Nonce: 8hK9pX32",
            "Issued At: 2024-04-01T12:00:00Z",
        ]
        .join("\n");

        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Siwe,
            data: message.as_bytes().to_vec(),
        });

        match decoder.preview() {
            Ok(MessagePreview::Siwe(siwe)) => {
                assert_eq!(siwe.domain, "login.xyz");
                assert_eq!(siwe.chain_id, 1);
            }
            other => panic!("Unexpected result: {other:?}"),
        }

        assert_eq!(decoder.plain_preview(), message);
    }

    #[test]
    fn test_siwe_preview_falls_back_to_text_when_validation_fails() {
        let message = [
            "login.xyz wants you to sign in with your Ethereum account:",
            "0x6dD7802E6d44bE89a789C4bD60bD511B68F41c7c",
            "",
            "URI: https://login.xyz",
            "Version: 1",
            "Chain ID: 137",
            "Nonce: 8hK9pX32",
            "Issued At: 2024-04-01T12:00:00Z",
        ]
        .join("\n");

        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Siwe,
            data: message.as_bytes().to_vec(),
        });

        match decoder.preview() {
            Ok(MessagePreview::Text(preview)) => assert_eq!(preview, message),
            other => panic!("Unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_eip191_siwe_preview_falls_back_to_text_on_chain_mismatch() {
        let message = [
            "thepoc.xyz wants you to sign in with your Ethereum account:",
            "0xBA4D1d35bCe0e8F28E5a3403e7a0b996c5d50AC4",
            "",
            "Sign in with different chain ID",
            "",
            "URI: https://thepoc.xyz",
            "Version: 1",
            "Chain ID: 137",
            "Nonce: byjof9dwrao97skautdxhb",
            "Issued At: 2026-03-09T15:48:34.458Z",
        ]
        .join("\n");

        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip191,
            data: message.as_bytes().to_vec(),
        });

        match decoder.preview() {
            Ok(MessagePreview::Text(preview)) => assert_eq!(preview, message),
            other => panic!("Expected text preview, got {other:?}"),
        }
    }

    #[test]
    fn test_eip191_siwe_payload_preview_keeps_structured_fields_on_chain_mismatch() {
        let message = [
            "thepoc.xyz wants you to sign in with your Ethereum account:",
            "0xBA4D1d35bCe0e8F28E5a3403e7a0b996c5d50AC4",
            "",
            "Sign in with different chain ID",
            "",
            "URI: https://thepoc.xyz",
            "Version: 1",
            "Chain ID: 137",
            "Nonce: byjof9dwrao97skautdxhb",
            "Issued At: 2026-03-09T15:48:34.458Z",
        ]
        .join("\n");

        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ethereum,
            sign_type: SignDigestType::Eip191,
            data: message.as_bytes().to_vec(),
        });

        let payload_preview = decoder.payload_preview(vec![]).unwrap().expect("expected SIWE payload preview");
        assert_eq!(payload_preview.message_type, MessageType::Siwe);
        assert_eq!(payload_preview.primary.len(), 2);
        assert_eq!(payload_preview.primary[0].label.as_deref(), Some("domain"));
        assert_eq!(payload_preview.primary[1].label.as_deref(), Some("address"));
    }

    #[test]
    fn test_ton_personal_preview() {
        let ton_data = TonSignMessageData::from_value(
            serde_json::json!({"type": "text", "text": "Hello TON"}),
            "example.com".to_string(),
            "UQBY1cVPu4SIr36q0M3HWcqPb_efyVVRBsEzmwN-wKQDR6zg".to_string(),
        )
        .unwrap();
        let data = String::from_utf8(ton_data.to_bytes()).unwrap();
        let decoder = MessageSigner::new(SignMessage {
            chain: Chain::Ton,
            sign_type: SignDigestType::TonPersonal,
            data: data.as_bytes().to_vec(),
        });

        match decoder.preview() {
            Ok(MessagePreview::Text(preview)) => assert_eq!(preview, "Hello TON"),
            other => panic!("Unexpected result: {other:?}"),
        }

        assert_eq!(decoder.plain_preview(), "Hello TON");
    }

    #[test]
    fn test_ton_hash_and_sign_share_timestamp() {
        let sender_address = TonSigner::new(&TEST_PRIVATE_KEY).unwrap().address().encode();
        let ton_data = TonSignMessageData::from_value(
            serde_json::json!({"type": "text", "text": "timestamp consistency"}),
            "example.com".to_string(),
            sender_address,
        )
        .unwrap();
        let data = ton_data.to_bytes();
        let signer = MessageSigner::new(SignMessage {
            chain: Chain::Ton,
            sign_type: SignDigestType::TonPersonal,
            data: data.clone(),
        });

        let previewed = signer.hash().unwrap();
        let response: serde_json::Value = serde_json::from_str(&signer.sign(Zeroizing::new(TEST_PRIVATE_KEY.to_vec())).unwrap()).unwrap();
        let signed_timestamp = response["timestamp"].as_u64().unwrap();

        let re_hashed = TonSignMessageData::from_bytes(&data).unwrap().hash(signed_timestamp).unwrap();
        assert_eq!(previewed, re_hashed);
    }
}
