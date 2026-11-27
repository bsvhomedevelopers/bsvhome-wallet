use primitives::name::NameRecord;
use primitives::{Asset, Chain};

use super::model::{BsvHomeRecipientError, BsvHomeRecipientNext, BsvHomeRecipientScan, BsvHomeRecipientType, BsvHomeRecipientValidation};
use crate::address::{checksum_address, validate_address};
use crate::models::custom_types::BsvHomeBigInt;
use crate::payment::{BsvHomePaymentConfirmTransfer, BsvHomePaymentDestination, BsvHomePaymentRecipient};
use crate::services::name::BsvHomeNameRecordState;
use crate::services::name::rules::is_name_supported;
use crate::services::transfer::{BsvHomeRecipient, BsvHomeTransferData};
use primitives::TransactionInputType;

pub fn validation(chain: Chain, input: &str, state: &BsvHomeNameRecordState) -> BsvHomeRecipientValidation {
    let name_record = state.record_ref();
    let is_valid = state.can_validate_recipient() && is_valid(chain, input, name_record);
    BsvHomeRecipientValidation {
        is_valid,
        address: address(chain, input, name_record),
        shows_error: shows_error(input, is_valid),
    }
}

pub fn recipient(chain: Chain, input: &str, state: &BsvHomeNameRecordState, memo: Option<String>, references: Vec<String>) -> Result<BsvHomeRecipient, BsvHomeRecipientError> {
    let name_record = state.record_ref();
    if name_record.is_some_and(|record| !matches_input(record, chain, input)) {
        return Err(BsvHomeRecipientError::NameRecordMismatch);
    }
    if !state.can_validate_recipient() || !is_valid(chain, input, name_record) {
        return Err(BsvHomeRecipientError::InvalidAddress);
    }
    Ok(BsvHomeRecipient {
        address: address(chain, input, name_record),
        name: name_record.map(|record| record.name.clone()),
        memo,
        references,
    })
}

fn is_valid(chain: Chain, input: &str, name_record: Option<&NameRecord>) -> bool {
    match name_record {
        Some(record) => matches_input(record, chain, input) && validate_address(&record.address, chain),
        None => !input.trim().is_empty() && validate_address(&checksum_address(input, chain), chain),
    }
}

fn matches_input(record: &NameRecord, chain: Chain, input: &str) -> bool {
    record.name == input && record.chain == chain
}

fn address(chain: Chain, input: &str, name_record: Option<&NameRecord>) -> String {
    let address = name_record.map(|record| record.address.as_str()).filter(|address| !address.is_empty()).unwrap_or(input);
    checksum_address(address, chain)
}

fn shows_error(input: &str, is_valid: bool) -> bool {
    !input.trim().is_empty() && !is_name_supported(input) && !is_valid
}

pub fn scan_route(
    destination: BsvHomePaymentDestination,
    recipient_type: &BsvHomeRecipientType,
    transfer_data: impl FnOnce(BsvHomePaymentConfirmTransfer) -> BsvHomeTransferData,
) -> Result<BsvHomeRecipientScan, BsvHomeRecipientError> {
    match destination {
        BsvHomePaymentDestination::Confirm { transfer } => {
            let transfer = transfer_data(transfer);
            Ok(match recipient_type {
                BsvHomeRecipientType::Asset { .. } => BsvHomeRecipientScan::Confirm { transfer },
                BsvHomeRecipientType::Nft { .. } => BsvHomeRecipientScan::Recipient {
                    payment: BsvHomePaymentRecipient {
                        recipient: transfer.recipient,
                        amount: None,
                    },
                },
            })
        }
        BsvHomePaymentDestination::Recipient { payment, .. } => Ok(BsvHomeRecipientScan::Recipient { payment }),
        BsvHomePaymentDestination::SelectAsset { .. } | BsvHomePaymentDestination::Unsupported => Err(BsvHomeRecipientError::InvalidAddress),
    }
}

pub fn next_step(recipient_type: BsvHomeRecipientType, payment: BsvHomePaymentRecipient) -> BsvHomeRecipientNext {
    match recipient_type {
        BsvHomeRecipientType::Asset { .. } => BsvHomeRecipientNext::Amount { payment },
        BsvHomeRecipientType::Nft { nft_asset } => BsvHomeRecipientNext::Confirm {
            transfer: BsvHomeTransferData {
                input_type: TransactionInputType::TransferNft {
                    asset: Asset::from_chain(nft_asset.chain),
                    nft_asset,
                },
                recipient: payment.recipient,
                value: BsvHomeBigInt::from(0),
                use_max_amount: false,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::name::NameProvider;

    const ADDRESS: &str = "0x1f9090aae28b8a3dceadf281b0f12828e676c326";
    const CHECKSUMMED: &str = "0x1f9090aaE28b8a3dCeaDf281B0F12828e676c326";

    fn record(name: &str, address: &str, chain: Chain) -> NameRecord {
        NameRecord {
            name: name.to_string(),
            chain,
            address: address.to_string(),
            provider: NameProvider::Ens,
        }
    }

    fn complete(record: NameRecord) -> BsvHomeNameRecordState {
        BsvHomeNameRecordState::Complete { record }
    }

    #[test]
    fn test_a_pending_or_failed_name_lookup_is_never_a_valid_recipient() {
        let loading = BsvHomeNameRecordState::Loading { name: "h3rman.near".into() };
        let pending = validation(Chain::Near, "h3rman.near", &loading);
        assert!(!pending.is_valid);
        assert!(!pending.shows_error);
        assert_eq!(pending.address, "h3rman.near");
        assert!(!validation(Chain::Near, "h3rman.near", &BsvHomeNameRecordState::Error).is_valid);
        assert!(validation(Chain::Near, "h3rman.near", &BsvHomeNameRecordState::None).is_valid);
        assert_eq!(recipient(Chain::Near, "h3rman.near", &loading, None, vec![]), Err(BsvHomeRecipientError::InvalidAddress));
    }

    #[test]
    fn test_address_input_is_checksummed_and_validated() {
        let valid = validation(Chain::Ethereum, ADDRESS, &BsvHomeNameRecordState::None);
        assert!(valid.is_valid);
        assert_eq!(valid.address, CHECKSUMMED);
        assert!(!valid.shows_error);

        let invalid = validation(Chain::Ethereum, "0xinvalid", &BsvHomeNameRecordState::None);
        assert!(!invalid.is_valid);
        assert!(invalid.shows_error);
        assert!(!validation(Chain::Ethereum, "", &BsvHomeNameRecordState::None).shows_error);
        assert!(!validation(Chain::Ethereum, "vitalik.eth", &BsvHomeNameRecordState::None).shows_error);
    }

    #[test]
    fn test_name_record_must_match_input_and_chain() {
        let ens = record("vitalik.eth", ADDRESS, Chain::Ethereum);
        let valid = validation(Chain::Ethereum, "vitalik.eth", &complete(ens.clone()));
        assert!(valid.is_valid);
        assert_eq!(valid.address, CHECKSUMMED);

        assert!(!validation(Chain::Ethereum, "other.eth", &complete(ens.clone())).is_valid);
        assert!(!validation(Chain::Polygon, "vitalik.eth", &complete(ens.clone())).is_valid);
        assert!(!validation(Chain::Ethereum, "vitalik.eth", &complete(record("vitalik.eth", "0xbad", Chain::Ethereum))).is_valid);
    }

    #[test]
    fn test_recipient_builds_from_record_or_address() {
        let ens = record("vitalik.eth", ADDRESS, Chain::Ethereum);
        let named = recipient(Chain::Ethereum, "vitalik.eth", &complete(ens.clone()), Some("memo".into()), vec!["ref".into()]).unwrap();
        assert_eq!(named.address, CHECKSUMMED);
        assert_eq!(named.name.as_deref(), Some("vitalik.eth"));
        assert_eq!(named.memo.as_deref(), Some("memo"));
        assert_eq!(named.references, vec!["ref".to_string()]);

        let plain = recipient(Chain::Ethereum, ADDRESS, &BsvHomeNameRecordState::None, None, vec![]).unwrap();
        assert_eq!(plain.address, CHECKSUMMED);
        assert_eq!(plain.name, None);
        assert_eq!(
            recipient(Chain::Ethereum, "other.eth", &complete(ens.clone()), None, vec![]),
            Err(BsvHomeRecipientError::NameRecordMismatch)
        );
        assert_eq!(
            recipient(Chain::Ethereum, "0xinvalid", &BsvHomeNameRecordState::None, None, vec![]),
            Err(BsvHomeRecipientError::InvalidAddress)
        );
    }

    #[test]
    fn test_input_is_trimmed_and_prefix_is_case_sensitive() {
        let padded = format!("  {ADDRESS} \n");
        assert!(validation(Chain::Ethereum, &padded, &BsvHomeNameRecordState::None).is_valid);
        assert_eq!(
            recipient(Chain::Ethereum, &padded, &BsvHomeNameRecordState::None, None, vec![]).unwrap().address,
            CHECKSUMMED
        );

        let upper_prefix = ADDRESS.replacen("0x", "0X", 1);
        assert!(!validation(Chain::Ethereum, &upper_prefix, &BsvHomeNameRecordState::None).is_valid);
        assert_eq!(
            recipient(Chain::Ethereum, &upper_prefix, &BsvHomeNameRecordState::None, None, vec![]),
            Err(BsvHomeRecipientError::InvalidAddress)
        );
    }

    #[test]
    fn test_name_record_matching_is_exact() {
        let ens = record("vitalik.eth", ADDRESS, Chain::Ethereum);
        assert!(!validation(Chain::Ethereum, "Vitalik.eth", &complete(ens.clone())).is_valid);
        assert_eq!(
            recipient(Chain::Ethereum, "Vitalik.eth", &complete(ens.clone()), None, vec![]),
            Err(BsvHomeRecipientError::NameRecordMismatch)
        );
        assert_eq!(
            recipient(Chain::Ethereum, " vitalik.eth", &complete(ens.clone()), None, vec![]),
            Err(BsvHomeRecipientError::NameRecordMismatch)
        );
        assert_eq!(
            recipient(Chain::Polygon, "vitalik.eth", &complete(ens.clone()), None, vec![]),
            Err(BsvHomeRecipientError::NameRecordMismatch)
        );

        let empty = record("vitalik.eth", "", Chain::Ethereum);
        assert_eq!(
            recipient(Chain::Ethereum, "vitalik.eth", &complete(empty.clone()), None, vec![]),
            Err(BsvHomeRecipientError::InvalidAddress)
        );
        let fallback = validation(Chain::Ethereum, "vitalik.eth", &complete(empty.clone()));
        assert!(!fallback.is_valid);
        assert_eq!(fallback.address, "vitalik.eth");
    }

    #[test]
    fn test_non_evm_addresses_keep_their_case() {
        let near = recipient(Chain::Near, "h3rman.near", &BsvHomeNameRecordState::None, None, vec![]).unwrap();
        assert_eq!(near.address, "h3rman.near");
        assert_eq!(near.name, None);

        let solana = "GvhwZwtV32kYUXUw965CUM3KGPdtBsDwPVpi92brY5R2";
        assert_eq!(validation(Chain::Solana, solana, &BsvHomeNameRecordState::None).address, solana);
        assert!(validation(Chain::Solana, solana, &BsvHomeNameRecordState::None).is_valid);

        let tron = "TJRyWwFs9wTFGZg3JbrVriFbNfCug5tDeC";
        let tron_recipient = recipient(Chain::Tron, tron, &BsvHomeNameRecordState::None, Some("  memo ".into()), vec!["a".into(), "b".into()]).unwrap();
        assert_eq!(tron_recipient.address, tron);
        assert_eq!(tron_recipient.memo.as_deref(), Some("  memo "));
        assert_eq!(tron_recipient.references, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(
            recipient(Chain::Tron, &tron.to_lowercase(), &BsvHomeNameRecordState::None, None, vec![]),
            Err(BsvHomeRecipientError::InvalidAddress)
        );
    }

    fn confirm_transfer(address: &str) -> BsvHomePaymentConfirmTransfer {
        BsvHomePaymentConfirmTransfer {
            asset_id: primitives::AssetId::from_chain(Chain::Ethereum),
            address: address.to_string(),
            value: 5u32.into(),
            memo: None,
            references: vec![],
        }
    }

    fn transfer_data(transfer: BsvHomePaymentConfirmTransfer) -> BsvHomeTransferData {
        BsvHomeTransferData {
            input_type: TransactionInputType::Transfer {
                asset: Asset::from_chain(Chain::Ethereum),
            },
            recipient: BsvHomeRecipient::address(transfer.address),
            value: BsvHomeBigInt::from(5),
            use_max_amount: false,
        }
    }

    #[test]
    fn test_scan_confirms_an_asset_payment_and_only_fills_an_nft_recipient() {
        let destination = BsvHomePaymentDestination::Confirm {
            transfer: confirm_transfer(ADDRESS),
        };
        let asset = BsvHomeRecipientType::Asset {
            asset: Asset::from_chain(Chain::Ethereum),
        };
        let nft = BsvHomeRecipientType::Nft {
            nft_asset: primitives::NFTAsset::mock(),
        };

        assert!(matches!(scan_route(destination.clone(), &asset, transfer_data), Ok(BsvHomeRecipientScan::Confirm { transfer }) if transfer.recipient.address == ADDRESS));
        assert!(
            matches!(scan_route(destination, &nft, transfer_data), Ok(BsvHomeRecipientScan::Recipient { payment }) if payment.recipient.address == ADDRESS && payment.amount.is_none())
        );
    }

    #[test]
    fn test_scan_fills_a_recipient_and_rejects_the_rest() {
        let asset = BsvHomeRecipientType::Asset {
            asset: Asset::from_chain(Chain::Ethereum),
        };
        let payment = BsvHomePaymentRecipient {
            recipient: BsvHomeRecipient::address(ADDRESS.to_string()),
            amount: Some("1.5".to_string()),
        };
        let recipient = BsvHomePaymentDestination::Recipient {
            asset_id: primitives::AssetId::from_chain(Chain::Ethereum),
            payment: payment.clone(),
        };

        assert!(matches!(scan_route(recipient, &asset, transfer_data), Ok(BsvHomeRecipientScan::Recipient { payment: found }) if found == payment));
        assert!(matches!(
            scan_route(BsvHomePaymentDestination::Unsupported, &asset, transfer_data),
            Err(BsvHomeRecipientError::InvalidAddress)
        ));
        assert!(matches!(
            scan_route(BsvHomePaymentDestination::SelectAsset { payment, chains: vec![] }, &asset, transfer_data),
            Err(BsvHomeRecipientError::InvalidAddress)
        ));
    }

    #[test]
    fn test_next_goes_to_the_amount_for_an_asset_and_to_confirm_for_an_nft() {
        let payment = BsvHomePaymentRecipient {
            recipient: BsvHomeRecipient::address(ADDRESS.to_string()),
            amount: Some("2".to_string()),
        };
        let nft_asset = primitives::NFTAsset::mock();

        assert!(matches!(
            next_step(BsvHomeRecipientType::Asset { asset: Asset::from_chain(Chain::Ethereum) }, payment.clone()),
            BsvHomeRecipientNext::Amount { payment: found } if found == payment
        ));
        match next_step(BsvHomeRecipientType::Nft { nft_asset: nft_asset.clone() }, payment) {
            BsvHomeRecipientNext::Confirm { transfer } => {
                assert!(
                    matches!(transfer.input_type, TransactionInputType::TransferNft { asset, nft_asset: found } if asset == Asset::from_chain(nft_asset.chain) && found.id == nft_asset.id)
                );
                assert_eq!(transfer.recipient.address, ADDRESS);
                assert_eq!(transfer.value, BsvHomeBigInt::from(0));
                assert!(!transfer.use_max_amount);
            }
            BsvHomeRecipientNext::Amount { .. } => panic!("an nft recipient goes straight to confirm"),
        }
    }
}
