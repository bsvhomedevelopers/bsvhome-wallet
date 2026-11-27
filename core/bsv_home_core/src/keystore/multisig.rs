use bsv_home_bsv::multisig::{
    Funding, FundingReview, Policy, Proposal, SignatureSet, assemble_proposal, funding_transaction_id, member_address, policy_descriptor, review_funding, review_proposal,
    sign_proposal,
};
use primitives::Chain;
use serde::{Serialize, de::DeserializeOwned};
use signer::secp256k1_public_key;

use super::BsvHomeKeystore;
use crate::BsvHomeCoreError;

const MAX_MULTISIG_JSON_BYTES: usize = 2_100_000;

#[uniffi::export]
impl BsvHomeKeystore {
    pub fn bsv_multisig_public_key(&self, keystore_id: String, password: Vec<u8>) -> Result<String, BsvHomeCoreError> {
        let key = self.signing_key(&keystore_id, Chain::BitcoinSv, password)?;
        Ok(hex::encode(secp256k1_public_key(key.as_slice())?))
    }

    pub fn bsv_multisig_member_address(&self, public_key: String) -> Result<String, BsvHomeCoreError> {
        Ok(member_address(&public_key)?)
    }

    pub fn bsv_multisig_policy(&self, threshold: u32, public_keys: Vec<String>) -> Result<String, BsvHomeCoreError> {
        encode(&policy_descriptor(&Policy { threshold, public_keys })?)
    }

    pub fn bsv_multisig_review(&self, policy_json: String, proposal_json: String) -> Result<String, BsvHomeCoreError> {
        encode(&review_proposal(&decode::<Policy>(&policy_json)?, &decode::<Proposal>(&proposal_json)?)?)
    }

    pub fn bsv_multisig_sign(
        &self,
        keystore_id: String,
        policy_json: String,
        proposal_json: String,
        expected_recipient: String,
        expected_amount: u64,
        maximum_fee: u64,
        password: Vec<u8>,
    ) -> Result<String, BsvHomeCoreError> {
        let policy = decode::<Policy>(&policy_json)?;
        let proposal = decode::<Proposal>(&proposal_json)?;
        review_proposal(&policy, &proposal)?;
        let key = self.signing_key(&keystore_id, Chain::BitcoinSv, password)?;
        encode(&sign_proposal(&policy, &proposal, &expected_recipient, expected_amount, maximum_fee, key.as_slice())?)
    }

    pub fn bsv_multisig_assemble(&self, policy_json: String, proposal_json: String, signatures_json: String) -> Result<String, BsvHomeCoreError> {
        Ok(assemble_proposal(
            &decode::<Policy>(&policy_json)?,
            &decode::<Proposal>(&proposal_json)?,
            &decode::<Vec<SignatureSet>>(&signatures_json)?,
        )?)
    }

    pub fn bsv_multisig_funding_review(&self, funding_json: String) -> Result<String, BsvHomeCoreError> {
        encode(&review_funding(&decode::<Funding>(&funding_json)?)?)
    }

    pub fn bsv_multisig_funding_sign(&self, keystore_id: String, funding_json: String, review_json: String, password: Vec<u8>) -> Result<String, BsvHomeCoreError> {
        self.sign_bsv_multisig_funding_payload(&keystore_id, &decode::<Funding>(&funding_json)?, &decode::<FundingReview>(&review_json)?, password)
    }

    pub fn bsv_multisig_funding_transaction_id(&self, funding_json: String, review_json: String, raw: String) -> Result<String, BsvHomeCoreError> {
        Ok(funding_transaction_id(&decode::<Funding>(&funding_json)?, &decode::<FundingReview>(&review_json)?, &raw)?)
    }
}

fn decode<T: DeserializeOwned>(value: &str) -> Result<T, BsvHomeCoreError> {
    if value.len() > MAX_MULTISIG_JSON_BYTES {
        return Err(BsvHomeCoreError::from("BSV multisig JSON exceeds supported size"));
    }
    Ok(serde_json::from_str(value)?)
}

fn encode(value: &impl Serialize) -> Result<String, BsvHomeCoreError> {
    Ok(serde_json::to_string(value)?)
}

#[cfg(test)]
mod tests {
    use bsv_home_bsv::BsvAddress;
    use bsv_home_bsv::multisig::FundingAddress;
    use bsv_home_derivation::{BSV_ACCOUNT_PATH, SIMPLY_CASH_ACCOUNT_PATH, bsv_account_xpub, bsv_address, bsv_address_for_path};
    use primitives::{Address, BsvAddressBranch, BsvAddressLocator, BsvSpendChange, UTXO, testkit::ABANDON_PHRASE};
    use serde::Deserialize;
    use tempfile::TempDir;

    use super::*;
    use crate::keystore::BsvHomeImportType;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Interop {
        proposal: Proposal,
        signature_sets: Vec<SignatureSet>,
        signed_transaction: String,
    }

    fn interop() -> Interop {
        serde_json::from_str(include_str!("../../../crates/bsv_home_bsv/testdata/bsv-multisig-interop.json")).unwrap()
    }

    fn funding(addresses: Vec<FundingAddress>) -> Funding {
        let utxos = addresses
            .iter()
            .enumerate()
            .map(|(index, address)| UTXO {
                transaction_id: format!("{:064x}", index + 20),
                vout: 0,
                value: 150000u64.into(),
                address: address.address.clone(),
            })
            .collect();
        Funding {
            chain: Chain::BitcoinSv,
            policy: interop().proposal.policy,
            sender_address: addresses[0].address.clone(),
            amount: 200000,
            fee_rate: 100,
            utxos,
            addresses,
            change: None,
        }
    }

    #[test]
    fn test_multisig_keystore_exact_public_fixture_and_wrong_identity() {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let stored = keystore
            .create_store(
                BsvHomeImportType::PrivateKey {
                    value: hex::encode([1; 32]),
                    chain: Chain::BitcoinSv,
                },
                b"multisig-offline-test".to_vec(),
            )
            .unwrap();
        let fixture = interop();
        let policy_json = encode(&fixture.proposal.policy).unwrap();
        let proposal_json = encode(&fixture.proposal).unwrap();
        let public_key = keystore.bsv_multisig_public_key(stored.keystore_id.clone(), b"multisig-offline-test".to_vec()).unwrap();
        assert_eq!(keystore.bsv_multisig_member_address(public_key.clone()).unwrap(), stored.accounts[0].address);
        let signed: SignatureSet = decode(
            &keystore
                .bsv_multisig_sign(
                    stored.keystore_id.clone(),
                    policy_json.clone(),
                    proposal_json.clone(),
                    fixture.proposal.recipient.clone(),
                    42000,
                    200,
                    b"multisig-offline-test".to_vec(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(signed, fixture.signature_sets.iter().find(|set| set.public_key == public_key).unwrap().clone());
        assert_eq!(
            keystore
                .bsv_multisig_assemble(policy_json.clone(), proposal_json.clone(), encode(&fixture.signature_sets).unwrap())
                .unwrap(),
            fixture.signed_transaction
        );
        assert!(
            keystore
                .bsv_multisig_sign(
                    stored.keystore_id.clone(),
                    policy_json.clone(),
                    proposal_json.clone(),
                    fixture.proposal.recipient.clone(),
                    42001,
                    200,
                    b"multisig-offline-test".to_vec()
                )
                .is_err()
        );
        assert!(
            keystore
                .bsv_multisig_sign(
                    stored.keystore_id,
                    policy_json,
                    proposal_json,
                    fixture.proposal.recipient,
                    42000,
                    200,
                    b"wrong-password".to_vec()
                )
                .is_err()
        );
    }

    #[test]
    fn test_multisig_funding_keystore_hd_and_private_key() {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let hd = keystore
            .create_store(
                BsvHomeImportType::SinglePhrase {
                    words: ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                    chain: Chain::BitcoinSv,
                },
                b"multisig-offline-test".to_vec(),
            )
            .unwrap();
        let xpub = bsv_account_xpub(ABANDON_PHRASE).unwrap();
        let addresses = (0..2)
            .map(|index| {
                let locator = BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index,
                };
                FundingAddress {
                    address: bsv_address(&xpub, locator).unwrap(),
                    locator,
                }
            })
            .collect();
        let hd_funding = funding(addresses);
        assert_eq!(hd_funding.sender_address, hd.accounts[0].address);
        let private = keystore
            .create_store(
                BsvHomeImportType::PrivateKey {
                    value: hex::encode([9; 32]),
                    chain: Chain::BitcoinSv,
                },
                b"multisig-offline-test".to_vec(),
            )
            .unwrap();
        let mut private_funding = funding(vec![FundingAddress {
            address: private.accounts[0].address.clone(),
            locator: BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 0,
            },
        }]);
        private_funding.amount = 100000;
        for (id, funding) in [(hd.keystore_id.clone(), hd_funding.clone()), (private.keystore_id.clone(), private_funding)] {
            let funding_json = encode(&funding).unwrap();
            let review_json = keystore.bsv_multisig_funding_review(funding_json.clone()).unwrap();
            let raw = keystore
                .bsv_multisig_funding_sign(id, funding_json.clone(), review_json.clone(), b"multisig-offline-test".to_vec())
                .unwrap();
            assert_eq!(
                keystore
                    .bsv_multisig_funding_transaction_id(funding_json.clone(), review_json.clone(), raw.clone())
                    .unwrap(),
                funding_transaction_id(&funding, &decode::<FundingReview>(&review_json).unwrap(), &raw).unwrap()
            );
            let mut changed = decode::<FundingReview>(&review_json).unwrap();
            changed.fee += 1;
            assert!(keystore.bsv_multisig_funding_transaction_id(funding_json, encode(&changed).unwrap(), raw).is_err());
        }
        let funding_json = encode(&hd_funding).unwrap();
        let review_json = keystore.bsv_multisig_funding_review(funding_json.clone()).unwrap();
        assert!(
            keystore
                .bsv_multisig_funding_sign(private.keystore_id, funding_json.clone(), review_json.clone(), b"multisig-offline-test".to_vec())
                .is_err()
        );
        assert!(
            keystore
                .bsv_multisig_funding_sign("missing-wallet".into(), funding_json, review_json, b"multisig-offline-test".to_vec())
                .is_err()
        );
    }

    #[test]
    fn test_multisig_funding_keystore_validates_allocated_change_against_actual_recovery() {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let hd = keystore
            .create_store(
                BsvHomeImportType::SinglePhrase {
                    words: ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                    chain: Chain::BitcoinSv,
                },
                b"multisig-offline-test".to_vec(),
            )
            .unwrap();
        let xpub = bsv_account_xpub(ABANDON_PHRASE).unwrap();
        let addresses = (0..2)
            .map(|index| {
                let locator = BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index,
                };
                FundingAddress {
                    address: bsv_address(&xpub, locator).unwrap(),
                    locator,
                }
            })
            .collect();
        let mut funding = funding(addresses);
        let change_locator = BsvAddressLocator {
            branch: BsvAddressBranch::Change,
            index: 7,
        };
        let change_address = bsv_address(&xpub, change_locator).unwrap();
        funding.change = Some(BsvSpendChange {
            script_pubkey: hex::encode(BsvAddress::from_str(&change_address).unwrap().script_pubkey().as_bytes()),
            address: change_address,
            locator: change_locator,
        });
        let review = review_funding(&funding).unwrap();
        let raw = keystore
            .bsv_multisig_funding_sign(
                hd.keystore_id.clone(),
                encode(&funding).unwrap(),
                encode(&review).unwrap(),
                b"multisig-offline-test".to_vec(),
            )
            .unwrap();
        assert_eq!(
            keystore
                .bsv_multisig_funding_transaction_id(encode(&funding).unwrap(), encode(&review).unwrap(), raw.clone())
                .unwrap(),
            funding_transaction_id(&funding, &review, &raw).unwrap()
        );
        let foreign = bsv_address(
            &xpub,
            BsvAddressLocator {
                branch: BsvAddressBranch::Change,
                index: 8,
            },
        )
        .unwrap();
        let change = funding.change.as_mut().unwrap();
        change.address = foreign.clone();
        change.script_pubkey = hex::encode(BsvAddress::from_str(&foreign).unwrap().script_pubkey().as_bytes());
        let changed_review = review_funding(&funding).unwrap();
        assert!(
            keystore
                .bsv_multisig_funding_sign(
                    hd.keystore_id,
                    encode(&funding).unwrap(),
                    encode(&changed_review).unwrap(),
                    b"multisig-offline-test".to_vec()
                )
                .is_err()
        );
    }

    #[test]
    fn test_multisig_funding_private_key_rejects_fabricated_derived_change_locator() {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let private = keystore
            .create_store(
                BsvHomeImportType::PrivateKey {
                    value: hex::encode([9; 32]),
                    chain: Chain::BitcoinSv,
                },
                b"multisig-offline-test".to_vec(),
            )
            .unwrap();
        let address = private.accounts[0].address.clone();
        let mut funding = funding(vec![FundingAddress {
            address: address.clone(),
            locator: BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 0,
            },
        }]);
        funding.amount = 100000;
        funding.change = Some(BsvSpendChange {
            script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
            address,
            locator: BsvAddressLocator {
                branch: BsvAddressBranch::Change,
                index: 0,
            },
        });
        let review = review_funding(&funding).unwrap();
        let unsupported = keystore
            .bsv_multisig_funding_sign(
                private.keystore_id.clone(),
                encode(&funding).unwrap(),
                encode(&review).unwrap(),
                b"not-the-wallet-password".to_vec(),
            )
            .unwrap_err();
        assert_eq!(unsupported.to_string(), "BSV funding sender does not match the selected private-key wallet");

        assert!(
            keystore
                .bsv_multisig_funding_sign(private.keystore_id, encode(&funding).unwrap(), encode(&review).unwrap(), b"multisig-offline-test".to_vec())
                .is_err()
        );
    }

    #[test]
    fn test_multisig_funding_allocated_change_uses_actual_hd_recovery_profile() {
        for (account_path, legacy) in [(BSV_ACCOUNT_PATH, false), (SIMPLY_CASH_ACCOUNT_PATH, false), (SIMPLY_CASH_ACCOUNT_PATH, true)] {
            let directory = TempDir::new().unwrap();
            let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
            let hd = keystore
                .import_bsv_hd(ABANDON_PHRASE, Some(account_path), Some(" public:Pass "), None, legacy, b"multisig-offline-test")
                .unwrap();
            let xpub = hd.accounts[0].public_key.as_ref().unwrap();
            let addresses = (0..2)
                .map(|index| {
                    let locator = BsvAddressLocator {
                        branch: BsvAddressBranch::Receive,
                        index,
                    };
                    FundingAddress {
                        address: bsv_address_for_path(xpub, account_path, locator).unwrap(),
                        locator,
                    }
                })
                .collect();
            let mut funding = funding(addresses);
            let locator = BsvAddressLocator {
                branch: BsvAddressBranch::Change,
                index: 7,
            };
            let address = bsv_address_for_path(xpub, account_path, locator).unwrap();
            funding.change = Some(BsvSpendChange {
                script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
                address,
                locator,
            });
            let review = review_funding(&funding).unwrap();
            let raw = keystore
                .bsv_multisig_funding_sign(hd.keystore_id, encode(&funding).unwrap(), encode(&review).unwrap(), b"multisig-offline-test".to_vec())
                .unwrap();
            assert_eq!(
                keystore
                    .bsv_multisig_funding_transaction_id(encode(&funding).unwrap(), encode(&review).unwrap(), raw.clone())
                    .unwrap(),
                funding_transaction_id(&funding, &review, &raw).unwrap()
            );
        }
    }
}
