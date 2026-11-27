use bigdecimal::num_traits::ToPrimitive;
use bsv_home_bsv::BsvAddress;
use bsv_home_bsv::multisig::{Funding, FundingAddress, FundingReview, MAX_FUNDING_UTXOS, Policy, funding_transaction_id, member_address, review_funding};
use primitives::{Account, Address, Asset, BsvAddressBranch, BsvAddressLocator, BsvSpendMetadata, Chain, FeePriority, TransactionInputType, UTXO};
use serde::de::DeserializeOwned;
use std::{
    error::Error as StdError,
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
};

use super::{BsvHomeGateway, GatewayError, map_network_error};
use crate::models::BsvHomeBroadcastOptions;

const GAP_LIMIT: u32 = 20;
const MAX_DISCOVERY_STEPS: u32 = 1000;
const MAX_JSON_BYTES: usize = 2_100_000;

#[uniffi::export]
impl BsvHomeGateway {
    pub async fn bsv_multisig_funding_prepare(&self, account: Account, policy_json: String, amount: u64) -> Result<String, GatewayError> {
        if account.chain != Chain::BitcoinSv {
            return Err(invalid("BSV funding requires a BSV account"));
        }
        self.require_chain(Chain::BitcoinSv).map_err(|error| invalid(error.to_string()))?;
        BsvAddress::from_str(&account.address).map_err(|error| invalid(error.to_string()))?;
        let policy: Policy = decode(&policy_json)?;
        policy.configuration().map_err(|error| invalid(error.to_string()))?;
        let addresses = self.funding_addresses(&account).await?;
        let names = addresses.iter().map(|address| address.address.clone()).collect::<Vec<_>>();
        let outputs = self.bsv_client().wallet_native_unspent(&names).await.map_err(map_network_error)?;
        if outputs.len() > MAX_FUNDING_UTXOS {
            return Err(invalid("BSV funding exceeds 1000 UTXOs"));
        }
        let utxos = outputs
            .into_iter()
            .filter(|(_, output)| output.status == "confirmed")
            .map(|(address, output)| {
                Ok(UTXO {
                    transaction_id: output.tx_hash,
                    vout: i32::try_from(output.tx_pos).map_err(|error| invalid(error.to_string()))?,
                    value: output.value.into(),
                    address,
                })
            })
            .collect::<Result<Vec<_>, GatewayError>>()?;
        let fee_rate = self.funding_fee_rate().await?;
        let funding = Funding {
            chain: Chain::BitcoinSv,
            policy: policy.canonical().map_err(|error| invalid(error.to_string()))?,
            sender_address: account.address,
            amount,
            fee_rate,
            utxos,
            addresses,
            change: None,
        };
        review_funding(&funding).map_err(|error| invalid(error.to_string()))?;
        serde_json::to_string(&funding).map_err(|error| invalid(error.to_string()))
    }

    pub async fn bsv_multisig_funding_broadcast(&self, funding_json: String, review_json: String, raw: String) -> Result<String, GatewayError> {
        self.require_chain(Chain::BitcoinSv).map_err(|error| invalid(error.to_string()))?;
        let funding: Funding = decode(&funding_json)?;
        let review: FundingReview = decode(&review_json)?;
        let expected = funding_transaction_id(&funding, &review, &raw).map_err(|error| invalid(error.to_string()))?;
        match self
            .transaction_broadcast(Chain::BitcoinSv, raw.clone(), BsvHomeBroadcastOptions { skip_preflight: false })
            .await
        {
            Ok(returned) if returned == expected => Ok(expected),
            Ok(_) => Err(invalid("BSV funding broadcast returned a different transaction ID")),
            Err(error) => match self.bsv_client().raw_transaction(&expected).await {
                Ok(accepted) if accepted == raw => Ok(expected),
                Ok(_) => Err(invalid("BSV funding accepted transaction differs from signed data")),
                Err(_) => Err(error),
            },
        }
    }
}

impl BsvHomeGateway {
    pub(crate) async fn bsv_multisig_funding_broadcast_checked<F, Fut>(
        &self,
        funding_json: String,
        review_json: String,
        raw: String,
        check: &(dyn Fn() -> Result<(), GatewayError> + Sync),
        before_send: F,
        before_submission: &(dyn Fn() -> Result<(), GatewayError> + Sync),
    ) -> Result<String, GatewayError>
    where
        F: Fn() -> Fut + Sync,
        Fut: Future<Output = Result<(), GatewayError>> + Send,
    {
        self.require_chain(Chain::BitcoinSv).map_err(|_| invalid("BSV funding is unavailable"))?;
        let funding: Funding = decode(&funding_json)?;
        let review: FundingReview = decode(&review_json)?;
        let expected = funding_transaction_id(&funding, &review, &raw).map_err(|_| invalid("invalid signed BSV funding"))?;
        let sdk_check = || check().map_err(|error| Box::new(error) as Box<dyn StdError + Send + Sync>);
        let sdk_before_send = || async { before_send().await.map_err(|error| Box::new(error) as Box<dyn StdError + Send + Sync>) };
        let submitted = AtomicBool::new(false);
        let sdk_submission = || {
            before_submission().map_err(|error| Box::new(error) as Box<dyn StdError + Send + Sync>)?;
            submitted.store(true, Ordering::SeqCst);
            Ok(())
        };
        check()?;
        let client = self.bsv_client();
        match client.broadcast_checked(raw.clone(), &sdk_check, sdk_before_send, &sdk_submission).await {
            Ok(returned) if returned == expected => Ok(expected),
            Ok(_) => Err(invalid("BSV funding broadcast returned a different transaction ID")),
            Err(error) => {
                if let Some(error) = error.downcast_ref::<GatewayError>() {
                    return Err(error.clone());
                }
                check()?;
                if !submitted.load(Ordering::SeqCst) {
                    return Err(map_network_error(error));
                }
                match client.raw_transaction_checked(&expected, &sdk_check).await {
                    Ok(accepted) if accepted == raw => Ok(expected),
                    Ok(_) => Err(invalid("BSV funding accepted transaction differs from signed data")),
                    Err(_) => {
                        check()?;
                        Err(map_network_error(error))
                    }
                }
            }
        }
    }

    pub(crate) async fn bsv_multisig_funding_prepare_with_spend(
        &self,
        sender_address: String,
        policy: Policy,
        amount: u64,
        spend: BsvSpendMetadata,
    ) -> Result<String, GatewayError> {
        self.require_chain(Chain::BitcoinSv).map_err(|error| invalid(error.to_string()))?;
        let fee_rate = self.funding_fee_rate().await?;
        let funding = Funding::from_spend(policy, sender_address, amount, fee_rate, spend).map_err(|error| invalid(error.to_string()))?;
        serde_json::to_string(&funding).map_err(|error| invalid(error.to_string()))
    }

    async fn funding_fee_rate(&self) -> Result<u64, GatewayError> {
        let rates = self
            .get_fee_rates(
                Chain::BitcoinSv,
                TransactionInputType::Transfer {
                    asset: Asset::from_chain(Chain::BitcoinSv),
                },
            )
            .await?;
        let rate = rates
            .into_iter()
            .find(|rate| rate.priority == FeePriority::Normal)
            .ok_or_else(|| invalid("BSV Normal fee rate missing"))?;
        let fee_rate = rate
            .gas_price_type
            .gas_price()
            .to_u64()
            .filter(|rate| *rate > 0)
            .ok_or_else(|| invalid("invalid BSV Normal fee rate"))?;
        Ok(fee_rate)
    }

    async fn funding_addresses(&self, account: &Account) -> Result<Vec<FundingAddress>, GatewayError> {
        let default_locator = BsvAddressLocator {
            branch: BsvAddressBranch::Receive,
            index: 0,
        };
        let Some(xpub) = &account.extended_public_key else {
            return Ok(vec![FundingAddress {
                address: account.address.clone(),
                locator: default_locator,
            }]);
        };
        if !xpub.starts_with("xpub") {
            if member_address(xpub).map_err(|error| invalid(error.to_string()))? != account.address {
                return Err(invalid("BSV funding account public key differs from default identity"));
            }
            return Ok(vec![FundingAddress {
                address: account.address.clone(),
                locator: default_locator,
            }]);
        }
        if bsv_home_derivation::bsv_address(xpub, default_locator).map_err(|error| invalid(error.to_string()))? != account.address {
            return Err(invalid("BSV funding account public key differs from default identity"));
        }
        let client = self.bsv_client();
        let mut addresses = Vec::new();
        for branch in [BsvAddressBranch::Receive, BsvAddressBranch::Change] {
            let mut empty_count = 0;
            for index in 0..MAX_DISCOVERY_STEPS {
                let locator = BsvAddressLocator { branch, index };
                let address = bsv_home_derivation::bsv_address(xpub, locator).map_err(|error| invalid(error.to_string()))?;
                let (has_history, _) = client.address_activity(&address).await.map_err(map_network_error)?;
                empty_count = if has_history { 0 } else { empty_count + 1 };
                addresses.push(FundingAddress { address, locator });
                if empty_count == GAP_LIMIT {
                    break;
                }
            }
            if empty_count != GAP_LIMIT {
                return Err(invalid("BSV funding address discovery is incomplete"));
            }
        }
        Ok(addresses)
    }
}

fn decode<T: DeserializeOwned>(value: &str) -> Result<T, GatewayError> {
    if value.len() > MAX_JSON_BYTES {
        return Err(invalid("BSV funding JSON exceeds supported size"));
    }
    serde_json::from_str(value).map_err(|error| invalid(error.to_string()))
}

fn invalid(message: impl ToString) -> GatewayError {
    GatewayError::PlatformError { msg: message.to_string() }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bsv_home_bsv::multisig::{Proposal, SignatureSet, funding_spend_metadata, sign_funding_with_keys};
    use futures::executor::block_on;
    use serde::Deserialize;
    use zeroize::Zeroizing;

    use super::*;
    use crate::alien::AlienResponse;
    use crate::gateway::EmptyPreferences;
    use crate::testkit::TestAlienProvider;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Interop {
        proposal: Proposal,
        signature_sets: Vec<SignatureSet>,
    }

    fn funding() -> Funding {
        let fixture: Interop = serde_json::from_str(include_str!("../../../crates/bsv_home_bsv/testdata/bsv-multisig-interop.json")).unwrap();
        let address = member_address(&fixture.signature_sets[0].public_key).unwrap();
        Funding {
            chain: Chain::BitcoinSv,
            policy: fixture.proposal.policy,
            sender_address: address.clone(),
            amount: 100000,
            fee_rate: 100,
            utxos: vec![UTXO {
                transaction_id: "11".repeat(32),
                vout: 0,
                value: 200000u64.into(),
                address: address.clone(),
            }],
            addresses: vec![FundingAddress {
                address,
                locator: BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index: 0,
                },
            }],
            change: None,
        }
    }

    #[test]
    fn test_funding_broadcast_unknown_result_checks_original_transaction_and_blocks_tampering() {
        block_on(async {
            let funding = funding();
            let review = review_funding(&funding).unwrap();
            let raw = sign_funding_with_keys(&funding, &review, |_| Ok(Zeroizing::new(vec![2; 32]))).unwrap();
            let transaction_id = funding_transaction_id(&funding, &review, &raw).unwrap();
            let origin = "https://api.whatsonchain.com/v1/bsv/main";
            let provider = Arc::new(TestAlienProvider::with_responses(vec![
                (format!("{origin}/tx/raw"), AlienResponse::new(Some(503), Vec::new())),
                (format!("{origin}/tx/{transaction_id}/hex"), AlienResponse::new(Some(200), raw.as_bytes().to_vec())),
            ]));
            let gateway = BsvHomeGateway::new(provider.clone(), Arc::new(EmptyPreferences), Arc::new(EmptyPreferences));
            let funding_json = serde_json::to_string(&funding).unwrap();
            let review_json = serde_json::to_string(&review).unwrap();
            assert_eq!(
                gateway
                    .bsv_multisig_funding_broadcast(funding_json.clone(), review_json.clone(), raw.clone())
                    .await
                    .unwrap(),
                transaction_id
            );
            let paths = provider.requested_paths();
            assert_eq!(paths.iter().filter(|path| *path == "/v1/bsv/main/tx/raw").count(), 1);
            assert_eq!(paths.last().unwrap(), &format!("/v1/bsv/main/tx/{transaction_id}/hex"));
            let mut changed = review.clone();
            changed.amount += 1;
            assert!(
                gateway
                    .bsv_multisig_funding_broadcast(funding_json.clone(), serde_json::to_string(&changed).unwrap(), raw.clone())
                    .await
                    .is_err()
            );
            assert_eq!(provider.requested_paths(), paths);
            let missing_provider = Arc::new(TestAlienProvider::with_json(503, ""));
            let missing_gateway = BsvHomeGateway::new(missing_provider, Arc::new(EmptyPreferences), Arc::new(EmptyPreferences));
            assert!(missing_gateway.bsv_multisig_funding_broadcast(funding_json, review_json, raw).await.is_err());
        });
    }

    #[test]
    fn test_funding_account_compressed_public_key_and_wrong_identity() {
        block_on(async {
            let funding = funding();
            let fixture: Interop = serde_json::from_str(include_str!("../../../crates/bsv_home_bsv/testdata/bsv-multisig-interop.json")).unwrap();
            let provider = Arc::new(TestAlienProvider::with_status(404));
            let gateway = BsvHomeGateway::new(provider.clone(), Arc::new(EmptyPreferences), Arc::new(EmptyPreferences));
            let mut account = Account {
                chain: Chain::BitcoinSv,
                address: funding.sender_address.clone(),
                derivation_path: String::new(),
                extended_public_key: Some(fixture.signature_sets[0].public_key.clone()),
            };
            assert_eq!(gateway.funding_addresses(&account).await.unwrap(), funding.addresses);
            account.extended_public_key = Some(fixture.signature_sets[1].public_key.clone());
            assert!(gateway.funding_addresses(&account).await.is_err());
            assert_eq!(provider.requested_paths(), Vec::<String>::new());
        });
    }

    #[test]
    fn test_funding_prepare_with_spend_uses_only_normal_fee_and_prunes_unused_candidates() {
        block_on(async {
            let original = funding();
            let mut spend = funding_spend_metadata(&original).unwrap();
            let mut unused = spend.inputs[0].clone();
            unused.transaction_id = "ff".repeat(32);
            unused.vout = 3;
            unused.value = 300000;
            spend.inputs.push(unused);
            let provider = Arc::new(TestAlienProvider::with_responses(vec![(
                "https://api.whatsonchain.com/v1/bsv/main/feerecommendation".into(),
                AlienResponse::new(Some(200), br#"{"fee_unit":"sat/KB","fee":125,"mempool_min_fee":250}"#.to_vec()),
            )]));
            let gateway = BsvHomeGateway::new(provider.clone(), Arc::new(EmptyPreferences), Arc::new(EmptyPreferences));
            let prepared: Funding = serde_json::from_str(
                &gateway
                    .bsv_multisig_funding_prepare_with_spend(original.sender_address.clone(), original.policy.clone(), original.amount, spend)
                    .await
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(prepared.fee_rate, 250);
            assert_eq!(prepared.sender_address, original.sender_address);
            assert!(prepared.policy == original.policy);
            assert_eq!(prepared.utxos.len(), 1);
            assert_eq!(prepared.addresses.len(), 1);
            assert_eq!(prepared.change, None);
            let review = review_funding(&prepared).unwrap();
            let raw = sign_funding_with_keys(&prepared, &review, |_| Ok(Zeroizing::new(vec![2; 32]))).unwrap();
            assert_eq!(funding_transaction_id(&prepared, &review, &raw).unwrap().len(), 64);
            assert_eq!(provider.requested_paths(), vec!["/v1/bsv/main/feerecommendation"]);
        });
    }

    #[test]
    fn test_funding_prepare_with_spend_invalid_script_has_no_scanner_fallback() {
        block_on(async {
            let original = funding();
            let mut spend = funding_spend_metadata(&original).unwrap();
            spend.inputs[0].script_pubkey = "00".into();
            let provider = Arc::new(TestAlienProvider::with_responses(vec![(
                "https://api.whatsonchain.com/v1/bsv/main/feerecommendation".into(),
                AlienResponse::new(Some(200), include_bytes!("../../../crates/bsv_home_bsv/testdata/fee.json").to_vec()),
            )]));
            let gateway = BsvHomeGateway::new(provider.clone(), Arc::new(EmptyPreferences), Arc::new(EmptyPreferences));
            assert!(
                gateway
                    .bsv_multisig_funding_prepare_with_spend(original.sender_address, original.policy, original.amount, spend)
                    .await
                    .is_err()
            );
            assert_eq!(provider.requested_paths(), vec!["/v1/bsv/main/feerecommendation"]);
        });
    }

    #[test]
    fn test_funding_prepare_with_spend_single_root_keeps_existing_none_payload() {
        block_on(async {
            let original = funding();
            let spend = funding_spend_metadata(&original).unwrap();
            let provider = Arc::new(TestAlienProvider::with_responses(vec![(
                "https://api.whatsonchain.com/v1/bsv/main/feerecommendation".into(),
                AlienResponse::new(Some(200), include_bytes!("../../../crates/bsv_home_bsv/testdata/fee.json").to_vec()),
            )]));
            let gateway = BsvHomeGateway::new(provider.clone(), Arc::new(EmptyPreferences), Arc::new(EmptyPreferences));
            let prepared = gateway
                .bsv_multisig_funding_prepare_with_spend(original.sender_address.clone(), original.policy.clone(), original.amount, spend)
                .await
                .unwrap();
            assert!(prepared == serde_json::to_string(&original).unwrap());
            assert_eq!(provider.requested_paths(), vec!["/v1/bsv/main/feerecommendation"]);
        });
    }
}
