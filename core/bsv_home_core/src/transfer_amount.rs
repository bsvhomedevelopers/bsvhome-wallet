use crate::models::custom_types::BsvHomeBigInt;
use primitives::{AssetId, TransactionInputType, TransferAmount, TransferAmountError, TransferAmountInput};

pub type BsvHomeTransferAmount = TransferAmount;
pub type BsvHomeTransferAmountError = TransferAmountError;

#[uniffi::remote(Record)]
pub struct BsvHomeTransferAmount {
    pub value: BsvHomeBigInt,
    pub network_fee: BsvHomeBigInt,
    pub is_max_amount: bool,
}

pub(crate) struct BsvHomeTransferAmountInput {
    pub(crate) input_type: TransactionInputType,
    pub(crate) value: BsvHomeBigInt,
    pub(crate) available_value: BsvHomeBigInt,
    pub(crate) fee_asset: AssetId,
    pub(crate) fee_asset_balance: BsvHomeBigInt,
    pub(crate) fee: BsvHomeBigInt,
    pub(crate) is_max_amount: bool,
}

impl From<BsvHomeTransferAmountInput> for TransferAmountInput {
    fn from(value: BsvHomeTransferAmountInput) -> Self {
        Self {
            input_type: value.input_type,
            value: value.value,
            available_value: value.available_value,
            fee_asset: value.fee_asset,
            fee_asset_balance: value.fee_asset_balance,
            fee: value.fee,
            is_max_amount: value.is_max_amount,
        }
    }
}

impl BsvHomeTransferAmountInput {
    pub(crate) fn calculate(self) -> Result<BsvHomeTransferAmount, BsvHomeTransferAmountError> {
        TransferAmountInput::from(self).calculate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;
    use primitives::{Asset, Chain};

    fn input(value: u64, available_value: u64, fee_asset_balance: u64) -> BsvHomeTransferAmountInput {
        let asset = Asset::from_chain(Chain::Solana);
        BsvHomeTransferAmountInput {
            input_type: TransactionInputType::Transfer { asset: asset.clone() },
            value: BigInt::from(value),
            available_value: BigInt::from(available_value),
            fee_asset: asset.id,
            fee_asset_balance: BigInt::from(fee_asset_balance),
            fee: BigInt::from(5_000),
            is_max_amount: false,
        }
    }

    #[test]
    fn test_calculate_transfer_amount() {
        assert_eq!(
            input(10_000_000, 100_000_000, 100_000_000).calculate().unwrap(),
            BsvHomeTransferAmount {
                value: BigInt::from(10_000_000),
                network_fee: BigInt::from(5_000),
                is_max_amount: false,
            }
        );

        assert_eq!(
            input(20_000_000, 10_000_000, 10_000_000).calculate().unwrap_err(),
            BsvHomeTransferAmountError::InsufficientBalance {
                asset_id: Asset::from_chain(Chain::Solana).id,
                required: BigInt::from(20_005_000u64),
                available: BigInt::from(10_000_000u64),
            }
        );
    }
}
