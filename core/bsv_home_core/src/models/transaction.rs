use crate::address::checksum_address;
use crate::models::custom_types::{BsvHomeBigInt, BsvHomeBigUint};
use crate::models::*;
use primitives::contract_call_data::ContractCallData;
use primitives::solana_nft::SolanaNftStandard;
use primitives::solana_token_program::SolanaTokenProgramId;
use primitives::{
    AssetId, EarnType, FeeOption, GasPriceType, HyperliquidOrder, SignerInput, TransactionFee, TransactionInputType, TransactionLoadInput, TransactionLoadMetadata,
    TransactionType, TronStakeData,
};
use std::collections::HashMap;

pub type BsvHomeFeeOption = FeeOption;
pub type BsvHomeTronStakeData = TronStakeData;

#[uniffi::remote(Enum)]
pub enum FeeOption {
    TokenAccountCreation,
}

pub type BsvHomeHyperliquidOrder = HyperliquidOrder;

#[uniffi::remote(Record)]
pub struct BsvHomeHyperliquidOrder {
    pub approve_agent_required: bool,
    pub approve_referral_required: bool,
    pub approve_builder_required: bool,
    pub builder_fee_bps: u32,
    pub agent_name: String,
    pub agent_address: String,
    pub agent_private_key: String,
}

pub type BsvHomeContractCallData = ContractCallData;

pub type BsvHomeEarnType = EarnType;

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeTransactionLoadInput {
    pub input_type: TransactionInputType,
    pub sender_address: String,
    pub destination_address: String,
    pub value: BsvHomeBigUint,
    pub gas_price: GasPriceType,
    pub memo: Option<String>,
    pub is_max_value: bool,
    pub metadata: BsvHomeTransactionLoadMetadata,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeSignerInput {
    pub input: BsvHomeTransactionLoadInput,
    pub fee: BsvHomeTransactionLoadFee,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSignedTransaction {
    pub data: String,
    pub transaction_type: TransactionType,
}

#[derive(Debug, Default, Clone, uniffi::Record)]
pub struct BsvHomeFeeOptions {
    pub options: HashMap<BsvHomeFeeOption, BsvHomeBigInt>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeTransactionLoadFee {
    pub fee: BsvHomeBigInt,
    pub gas_price_type: GasPriceType,
    pub gas_limit: BsvHomeBigInt,
    pub options: BsvHomeFeeOptions,
    pub fee_asset: AssetId,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BsvHomeTransactionData {
    pub fee: BsvHomeTransactionLoadFee,
    pub metadata: BsvHomeTransactionLoadMetadata,
}

pub type BsvHomeTransactionLoadMetadata = TransactionLoadMetadata;

#[uniffi::remote(Enum)]
pub enum BsvHomeTransactionLoadMetadata {
    None,
    Solana {
        sender_token_address: Option<String>,
        recipient_token_address: Option<String>,
        token_program: Option<SolanaTokenProgramId>,
        nft: Option<SolanaNftStandard>,
        block_hash: String,
        references: Vec<String>,
    },
    Ton {
        sender_token_address: Option<String>,
        recipient_token_address: Option<String>,
        sequence: u64,
    },
    Cosmos {
        account_number: u64,
        sequence: u64,
        chain_id: String,
    },
    Bitcoin {
        utxos: Vec<BsvHomeUTXO>,
    },
    BitcoinSv {
        spend: primitives::BsvSpendMetadata,
    },
    Zcash {
        utxos: Vec<BsvHomeUTXO>,
        branch_id: String,
    },
    Cardano {
        utxos: Vec<BsvHomeUTXO>,
        block_number: u64,
    },
    Evm {
        nonce: u64,
        chain_id: u64,
        contract_call: Option<BsvHomeContractCallData>,
    },
    Near {
        sequence: u64,
        block_hash: String,
    },
    Stellar {
        sequence: u64,
        is_destination_address_exist: bool,
    },
    Xrp {
        sequence: u64,
        block_number: u64,
    },
    Algorand {
        sequence: u64,
        block_hash: String,
        chain_id: String,
    },
    Aptos {
        sequence: u64,
        data: Option<String>,
    },
    Polkadot {
        sequence: u64,
        genesis_hash: String,
        block_hash: String,
        block_number: u64,
        spec_version: u64,
        transaction_version: u64,
        period: u64,
    },
    Tron {
        block_number: u64,
        block_version: u64,
        block_timestamp: u64,
        transaction_tree_root: String,
        parent_hash: String,
        witness_address: String,
        stake_data: BsvHomeTronStakeData,
    },
    Sui {
        message_bytes: String,
    },
    Hyperliquid {
        order: Option<BsvHomeHyperliquidOrder>,
    },
}

impl From<BsvHomeTransactionLoadInput> for TransactionLoadInput {
    fn from(value: BsvHomeTransactionLoadInput) -> Self {
        let input_type: TransactionInputType = value.input_type;
        let destination_address = checksum_address(&value.destination_address, input_type.get_asset().chain());
        TransactionLoadInput {
            input_type,
            sender_address: value.sender_address,
            destination_address,
            value: value.value,
            gas_price: value.gas_price,
            memo: value.memo,
            is_max_value: value.is_max_value,
            metadata: value.metadata,
        }
    }
}

impl From<BsvHomeSignerInput> for SignerInput {
    fn from(value: BsvHomeSignerInput) -> Self {
        SignerInput::new(value.input.into(), value.fee.into())
    }
}

impl From<TransactionLoadInput> for BsvHomeTransactionLoadInput {
    fn from(value: TransactionLoadInput) -> Self {
        BsvHomeTransactionLoadInput {
            input_type: value.input_type,
            sender_address: value.sender_address,
            destination_address: value.destination_address,
            value: value.value,
            gas_price: value.gas_price,
            memo: value.memo,
            is_max_value: value.is_max_value,
            metadata: value.metadata,
        }
    }
}

impl From<SignerInput> for BsvHomeSignerInput {
    fn from(value: SignerInput) -> Self {
        BsvHomeSignerInput {
            input: value.input.into(),
            fee: value.fee.into(),
        }
    }
}

pub fn transaction_metadata_block_number(metadata: &BsvHomeTransactionLoadMetadata) -> String {
    match metadata {
        BsvHomeTransactionLoadMetadata::Polkadot { block_number, .. }
        | BsvHomeTransactionLoadMetadata::Tron { block_number, .. }
        | BsvHomeTransactionLoadMetadata::Xrp { block_number, .. }
        | BsvHomeTransactionLoadMetadata::Cardano { block_number, .. } => block_number.to_string(),
        _ => "0".to_string(),
    }
}

pub fn transaction_metadata_sequence(metadata: &BsvHomeTransactionLoadMetadata) -> String {
    match metadata {
        BsvHomeTransactionLoadMetadata::Ton { sequence, .. }
        | BsvHomeTransactionLoadMetadata::Cosmos { sequence, .. }
        | BsvHomeTransactionLoadMetadata::Near { sequence, .. }
        | BsvHomeTransactionLoadMetadata::Stellar { sequence, .. }
        | BsvHomeTransactionLoadMetadata::Xrp { sequence, .. }
        | BsvHomeTransactionLoadMetadata::Algorand { sequence, .. }
        | BsvHomeTransactionLoadMetadata::Aptos { sequence, .. }
        | BsvHomeTransactionLoadMetadata::Polkadot { sequence, .. } => sequence.to_string(),
        BsvHomeTransactionLoadMetadata::Evm { nonce, .. } => nonce.to_string(),
        _ => "0".to_string(),
    }
}

impl BsvHomeFeeOptions {
    pub fn get(&self, option: &BsvHomeFeeOption) -> Option<&BsvHomeBigInt> {
        self.options.get(option)
    }

    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    pub fn total(&self) -> BsvHomeBigInt {
        self.options.values().sum()
    }
}

impl From<BsvHomeTransactionLoadFee> for TransactionFee {
    fn from(value: BsvHomeTransactionLoadFee) -> Self {
        TransactionFee {
            fee: value.fee,
            gas_price_type: value.gas_price_type,
            gas_limit: value.gas_limit,
            options: value.options.options,
            fee_asset: value.fee_asset,
        }
    }
}

impl From<TransactionFee> for BsvHomeTransactionLoadFee {
    fn from(value: TransactionFee) -> Self {
        BsvHomeTransactionLoadFee {
            fee: value.fee,
            gas_price_type: value.gas_price_type,
            gas_limit: value.gas_limit,
            options: BsvHomeFeeOptions { options: value.options },
            fee_asset: value.fee_asset,
        }
    }
}
