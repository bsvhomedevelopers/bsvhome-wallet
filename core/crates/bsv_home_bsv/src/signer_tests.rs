use super::*;
use crate::multisig::MultisigConfig;
use bitcoin::{
    Transaction,
    consensus::{deserialize, serialize},
};
use num_bigint::{BigInt, BigUint};
use primitives::testkit::signer_mock::TEST_PRIVATE_KEY;
use primitives::{Asset, BsvSpendChange, BsvSpendInput, GasPriceType, TransactionFee, TransactionLoadMetadata, UTXO};

#[test]
fn test_message_hash_compact_size_boundaries() {
    for (length, expected) in [
        (252, "b7b164ef991d52735c6bb888642ad7eb6b6939dc984a7fceff4376be041d142f"),
        (253, "df167ad249ff5837e6acada677118b2ecc6757ab4cdade39caead99ef0220230"),
        (65535, "fade4e6ebe191b9dcf869e37c4ab6a2d5f9ffc1160fbfb84370afb579af7de8d"),
        (65536, "d5db7ae9446693355e5674d5d17e7b0a29f13fc174055077d9613e9ab2b462fe"),
    ] {
        assert_eq!(hex::encode(BsvChainSigner::message_hash(&"a".repeat(length))), expected);
    }
}

#[test]
fn test_message_signature_compressed_p2pkh() {
    let signature = BsvChainSigner::sign_message_compact("Hello Bitcoin", &TEST_PRIVATE_KEY).unwrap();
    assert_eq!(
        hex::encode(signature),
        "1fd203a269c3cb8b6d4338d55f6258d5693722bf37db125329b31a6a50173a754e0405311d87a255e567d1e874aae654b82acf2a8bffc2acd8089090b72bf91af7",
    );
    assert_eq!(
        BsvChainSigner::recover_message_address("Hello Bitcoin", &signature).unwrap(),
        "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD"
    );
    assert_eq!(
        hex::encode(BsvChainSigner::recover_message_public_key("Hello Bitcoin", &signature).unwrap()),
        "031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f"
    );
    assert_ne!(
        BsvChainSigner::recover_message_address("Hello Bitcoin\n", &signature).unwrap(),
        "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD"
    );
    assert!(BsvChainSigner::sign_message_compact("Hello Bitcoin", &[0; 32]).is_err());
}

#[test]
fn test_message_recovery_rejects_invalid_headers_sizes_and_high_s() {
    let signature = BsvChainSigner::sign_message_compact("Hello Bitcoin", &TEST_PRIVATE_KEY).unwrap();
    for header in [0, 27, 30, 35, 39] {
        let mut altered = signature;
        altered[0] = header;
        assert!(BsvChainSigner::recover_message_public_key("Hello Bitcoin", &altered).is_err());
    }
    assert!(BsvChainSigner::recover_message_public_key("Hello Bitcoin", &signature[..64]).is_err());
    let mut high_s_signature = signature;
    high_s_signature[0] = 31 + ((high_s_signature[0] - 31) ^ 1);
    high_s_signature[33..].copy_from_slice(&hex::decode("fbfacee2785daa1a982e178b5519ab468fdfb25aaf85f363b741cdd5a43d264a").unwrap());
    assert_eq!(
        BsvChainSigner::recover_message_public_key("Hello Bitcoin", &high_s_signature).unwrap_err(),
        SignerError::invalid_input("BSV message signature requires low S")
    );
}

fn input(value: u64, is_max: bool) -> TransactionLoadInput {
    let address = "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD".to_string();
    TransactionLoadInput {
        input_type: TransactionInputType::Transfer {
            asset: Asset::from_chain(Chain::BitcoinSv),
        },
        sender_address: address.clone(),
        destination_address: address.clone(),
        value: BigUint::from(value),
        gas_price: GasPriceType::regular(BigInt::from(100)),
        memo: None,
        is_max_value: is_max,
        metadata: TransactionLoadMetadata::Bitcoin {
            utxos: vec![UTXO {
                transaction_id: format!("{:064}", 1),
                vout: 0,
                value: BigUint::from(50000u64),
                address,
            }],
        },
    }
}

#[test]
fn test_single_signature_payment_to_multisig_script() {
    let configuration = MultisigConfig {
        threshold: 2,
        public_keys: [
            "024d4b6cd1361032ca9bd2aeb9d900aa4d45d9ead80ac9423374c451a7254d0766",
            "031b84c5567b126440995d3ed5aaba0565d71e1834604819ff9c17f5e9d5dd078f",
        ]
        .map(String::from)
        .to_vec(),
    };
    let script = configuration.locking_script().unwrap();
    let target = format!("bsv-script:{}", hex::encode(script.as_bytes()));
    let mut input = input(10_000, false);
    input.destination_address = target.clone();
    let plan = plan_transfer(&input).unwrap();
    let fee = TransactionFee {
        fee: BigInt::from(plan.fee),
        gas_price_type: input.gas_price.clone(),
        gas_limit: 1.into(),
        options: Default::default(),
        fee_asset: Chain::BitcoinSv.as_asset_id(),
    };
    let signed = BsvChainSigner.sign_transfer(&SignerInput::new(input, fee), &[1; 32]).unwrap();
    let transaction: Transaction = deserialize(&hex::decode(&signed).unwrap()).unwrap();
    assert_eq!(transaction.output[0].script_pubkey, script);
    assert_eq!(
        BsvChainSigner::validate_signed_transfer(&signed, "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD", &target, 10_000).unwrap(),
        transaction.compute_txid().to_string()
    );
    assert!(BsvScriptTarget::parse("bsv-script:51").is_err());
}
#[test]
fn matches_independent_sdk_signing_vector() {
    let input = input(10000, false);
    let plan = plan_transfer(&input).unwrap();
    let fee = TransactionFee {
        fee: BigInt::from(plan.fee),
        gas_price_type: input.gas_price.clone(),
        gas_limit: 1.into(),
        options: Default::default(),
        fee_asset: Chain::BitcoinSv.as_asset_id(),
    };
    let signed = BsvChainSigner.sign_transfer(&SignerInput::new(input, fee), &[1; 32]).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../testdata/sdk_vector.json")).unwrap();
    assert_eq!(signed, fixture["raw"].as_str().unwrap());
    let transaction: Transaction = deserialize(&hex::decode(signed).unwrap()).unwrap();
    assert_eq!(transaction.compute_txid().to_string(), fixture["txid"].as_str().unwrap());
    assert_eq!(transaction.input[0].sequence.0, u32::MAX);
    assert_eq!(plan.fee, 23);
    assert_eq!(plan.outputs[1].value.to_sat(), 39977);
}
#[test]
fn exact_max_and_one_satoshi_change_preserve_amounts() {
    let max = plan_transfer(&input(0, true)).unwrap();
    assert_eq!(max.fee, 20);
    assert_eq!(max.outputs[0].value.to_sat(), 49980);
    let exact = plan_transfer(&input(49976, false)).unwrap();
    assert_eq!(exact.outputs[1].value.to_sat(), 1);
    assert_eq!(exact.fee, 23);
    assert_eq!(plan_transfer(&input(49977, false)).unwrap().fee, 23);
    assert_eq!(plan_transfer(&input(49980, false)).unwrap().fee, 20);
    assert!(plan_transfer(&input(50000, false)).is_err());
    assert!(plan_transfer(&input(0, false)).is_err());
}
#[test]
fn rejects_duplicates_foreign_addresses_and_confirmation_mismatch() {
    let mut source = input(10000, false);
    if let TransactionLoadMetadata::Bitcoin { utxos } = &mut source.metadata {
        utxos.push(utxos[0].clone());
    }
    assert!(plan_transfer(&source).unwrap_err().to_string().contains("duplicate"));
    let mut source = input(10000, false);
    if let TransactionLoadMetadata::Bitcoin { utxos } = &mut source.metadata {
        utxos[0].address = "1K6LZdwpKT5XkEZo2T2kW197aMXYbYMc4f".to_string();
    }
    assert!(plan_transfer(&source).is_err());
    let source = input(10000, false);
    let mut fee = source.default_fee();
    fee.fee = 22.into();
    assert!(BsvChainSigner.sign_transfer(&SignerInput::new(source, fee), &[1; 32]).is_err());
}
#[test]
fn fee_rounding_and_overflow_are_explicit() {
    let source = input(10000, false);
    let plan = plan_transfer(&source).unwrap();
    assert_eq!(BsvSpendPolicy { satoshis_per_kilobyte: 1 }.estimate_fee(&plan.inputs, &plan.outputs).unwrap(), 1);
    assert_eq!(BsvSpendPolicy { satoshis_per_kilobyte: 101 }.estimate_fee(&plan.inputs, &plan.outputs).unwrap(), 23);
    assert!(
        BsvSpendPolicy { satoshis_per_kilobyte: u64::MAX }
            .estimate_fee(&plan.inputs.iter().cycle().take(10).cloned().collect::<Vec<_>>(), &plan.outputs)
            .is_err()
    );
}

#[test]
fn max_signing_requires_the_confirmed_net_amount() {
    let source = input(49980, true);
    let fee = TransactionFee {
        fee: 20.into(),
        gas_price_type: source.gas_price.clone(),
        gas_limit: 1.into(),
        options: Default::default(),
        fee_asset: Chain::BitcoinSv.as_asset_id(),
    };
    assert!(BsvChainSigner.sign_transfer(&SignerInput::new(source.clone(), fee.clone()), &[1; 32]).is_ok());
    let mut changed = source;
    changed.value = 49981u64.into();
    assert!(BsvChainSigner.sign_transfer(&SignerInput::new(changed, fee), &[1; 32]).is_err());
}

#[test]
fn multiple_inputs_are_conserved_and_wrong_private_key_is_rejected() {
    let mut source = input(75000, false);
    if let TransactionLoadMetadata::Bitcoin { utxos } = &mut source.metadata {
        let mut second = utxos[0].clone();
        second.transaction_id = format!("{:064}", 2);
        utxos.push(second);
    }
    let plan = plan_transfer(&source).unwrap();
    assert_eq!(plan.inputs.len(), 2);
    assert_eq!(plan.fee, 38);
    assert_eq!(plan.outputs[0].value.to_sat(), 75000);
    assert_eq!(plan.outputs[1].value.to_sat(), 24962);
    let fee = TransactionFee {
        fee: plan.fee.into(),
        gas_price_type: source.gas_price.clone(),
        gas_limit: 1.into(),
        options: Default::default(),
        fee_asset: Chain::BitcoinSv.as_asset_id(),
    };
    let signing = SignerInput::new(source, fee);
    assert!(BsvChainSigner.sign_transfer(&signing, &[1; 32]).is_ok());
    assert!(BsvChainSigner.sign_transfer(&signing, &[2; 32]).is_err());
}

#[test]
fn test_one_satoshi_outputs_are_never_selected_or_signed() {
    let mut source = input(10000, false);
    if let TransactionLoadMetadata::Bitcoin { utxos } = &mut source.metadata {
        let mut protected = utxos[0].clone();
        protected.transaction_id = format!("{:064}", 2);
        protected.value = 1u32.into();
        utxos.push(protected);
    }
    let plan = plan_transfer(&source).unwrap();
    assert_eq!(plan.inputs.len(), 1);
    let mut max = source.clone();
    max.is_max_value = true;
    max.value = 0u32.into();
    let max = plan_transfer(&max).unwrap();
    assert_eq!(max.inputs.len(), 1);
    assert_eq!(max.outputs[0].value.to_sat(), 49980);
    let fee = TransactionFee {
        fee: plan.fee.into(),
        gas_price_type: source.gas_price.clone(),
        gas_limit: 1.into(),
        options: Default::default(),
        fee_asset: Chain::BitcoinSv.as_asset_id(),
    };
    let signed = BsvChainSigner.sign_transfer(&SignerInput::new(source, fee), &[1; 32]).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../testdata/sdk_vector.json")).unwrap();
    assert_eq!(signed, fixture["raw"].as_str().unwrap());
    assert_eq!(
        BsvChainSigner::validate_signed_transfer(&signed, "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD", "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD", 10000).unwrap(),
        fixture["txid"].as_str().unwrap()
    );
    assert!(BsvChainSigner::validate_signed_transfer(&signed, "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD", "1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD", 10001).is_err());
}

fn derived_input(value: u64, is_max: bool) -> TransactionLoadInput {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../testdata/multiple_keys_vector.json")).unwrap();
    let addresses = fixture["input_addresses"].as_array().unwrap();
    let inputs = addresses
        .iter()
        .enumerate()
        .map(|(index, address)| {
            let address = address.as_str().unwrap().to_string();
            BsvSpendInput {
                transaction_id: format!("{:064}", index + 1),
                vout: 0,
                value: if index == 0 { 50000 } else { 30000 },
                script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey()),
                address,
                locator: BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index: index as u32,
                },
            }
        })
        .collect();
    let change = fixture["change"].as_str().unwrap().to_string();
    TransactionLoadInput {
        sender_address: addresses[0].as_str().unwrap().to_string(),
        destination_address: fixture["recipient"].as_str().unwrap().to_string(),
        metadata: TransactionLoadMetadata::BitcoinSv {
            spend: BsvSpendMetadata {
                inputs,
                change: Some(BsvSpendChange {
                    script_pubkey: hex::encode(BsvAddress::from_str(&change).unwrap().script_pubkey()),
                    address: change,
                    locator: BsvAddressLocator {
                        branch: BsvAddressBranch::Change,
                        index: 0,
                    },
                }),
            },
        },
        ..input(value, is_max)
    }
}

fn confirmed_derived_input(value: u64, is_max: bool) -> SignerInput {
    let mut input = derived_input(value, is_max);
    let plan = plan_transfer(&input).unwrap();
    input.metadata = finalize_spend_metadata(&input, &plan).unwrap();
    SignerInput::new(
        input.clone(),
        TransactionFee {
            fee: plan.fee.into(),
            gas_price_type: input.gas_price.clone(),
            gas_limit: 1.into(),
            options: Default::default(),
            fee_asset: Chain::BitcoinSv.as_asset_id(),
        },
    )
}

fn derived_key(locator: BsvAddressLocator) -> Result<Zeroizing<Vec<u8>>, SignerError> {
    Ok(Zeroizing::new(vec![locator.index as u8 + 1; 32]))
}

#[test]
fn test_multiple_child_keys_match_independent_bytes_and_verify_each_signature() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../testdata/multiple_keys_vector.json")).unwrap();
    let input = confirmed_derived_input(60000, false);
    let mut signed_locators = Vec::new();
    let signed = BsvChainSigner
        .sign_transfer_with_keys(&input, |locator| {
            signed_locators.push(locator.index);
            derived_key(locator)
        })
        .unwrap();
    assert_eq!(signed_locators, vec![1, 0]);
    assert_eq!(signed, fixture["raw"].as_str().unwrap());
    assert_eq!(BsvChainSigner::validate_signed_spend(&signed, &input).unwrap(), fixture["txid"].as_str().unwrap());
    assert!(BsvChainSigner.sign_transfer_with_keys(&input, |_| Ok(Zeroizing::new(vec![1; 32]))).is_err());
    assert!(BsvChainSigner.sign_transfer(&input, &[1; 32]).is_err());

    let mut changed_amounts = input.clone();
    if let TransactionLoadMetadata::BitcoinSv { spend } = &mut changed_amounts.input.metadata {
        spend.inputs[0].value += 1;
        spend.inputs[1].value -= 1;
    }
    assert!(BsvChainSigner::validate_signed_spend(&signed, &changed_amounts).is_err());
    let mut transaction: Transaction = deserialize(&hex::decode(&signed).unwrap()).unwrap();
    transaction.output.push(transaction.output[0].clone());
    assert!(BsvChainSigner::validate_signed_spend(&hex::encode(serialize(&transaction)), &input).is_err());
}

#[test]
fn test_derived_spend_freezes_selection_and_clears_unused_change() {
    let mut input = confirmed_derived_input(10000, false);
    let spend = derived_spend(&input);
    assert_eq!(spend.inputs.len(), 1);
    assert_eq!(spend.inputs[0].locator.index, 1);
    let signed = BsvChainSigner.sign_transfer_with_keys(&input, derived_key).unwrap();
    assert!(BsvChainSigner::validate_signed_spend(&signed, &input).is_ok());
    input.input.metadata = derived_input(10000, false).metadata;
    assert!(BsvChainSigner.sign_transfer_with_keys(&input, derived_key).is_err());

    let input = confirmed_derived_input(79966, true);
    let spend = derived_spend(&input);
    assert_eq!(spend.change, None);
    let signed = BsvChainSigner.sign_transfer_with_keys(&input, derived_key).unwrap();
    assert!(BsvChainSigner::validate_signed_spend(&signed, &input).is_ok());
    let mut changed = input;
    changed.input.value = 79967u32.into();
    assert!(BsvChainSigner.sign_transfer_with_keys(&changed, derived_key).is_err());
}

fn derived_spend(input: &SignerInput) -> &BsvSpendMetadata {
    match &input.metadata {
        TransactionLoadMetadata::BitcoinSv { spend } => spend,
        _ => panic!(),
    }
}

#[test]
fn test_derived_spend_accepts_owned_change_branch_inputs() {
    let mut input = confirmed_derived_input(60000, false);
    if let TransactionLoadMetadata::BitcoinSv { spend } = &mut input.input.metadata {
        spend.inputs[1].locator.branch = BsvAddressBranch::Change;
    }
    let signed = BsvChainSigner.sign_transfer_with_keys(&input, derived_key).unwrap();
    assert!(BsvChainSigner::validate_signed_spend(&signed, &input).is_ok());
}

#[test]
fn test_derived_spend_rejects_script_locator_and_change_mismatches() {
    let source = confirmed_derived_input(60000, false);
    for alteration in 0..5 {
        let mut input = source.clone();
        if let TransactionLoadMetadata::BitcoinSv { spend } = &mut input.input.metadata {
            match alteration {
                0 => spend.inputs[0].locator.index = 1 << 31,
                1 => spend.inputs[0].script_pubkey = spend.inputs[1].script_pubkey.clone(),
                2 => spend.change.as_mut().unwrap().locator.branch = BsvAddressBranch::Receive,
                3 => spend.change = None,
                4 => spend.inputs.push(spend.inputs[0].clone()),
                _ => panic!(),
            }
        }
        assert!(BsvChainSigner.sign_transfer_with_keys(&input, derived_key).is_err());
    }
}
