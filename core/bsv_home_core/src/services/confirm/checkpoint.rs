use std::sync::Arc;

use async_trait::async_trait;
use bsv_home_bsv::signer::BsvChainSigner;
use bsv_home_tron::{validate_signed_native_transfer, validate_signed_token_transfer};
use primitives::{
    Asset, AssetId, BsvSpendMetadata, Chain, GasPriceType, SignerInput, TransactionFee, TransactionInputType, TransactionLoadInput, TransactionLoadMetadata, TransactionType,
    Wallet, WalletId,
};
use serde::{Deserialize, Serialize};

use super::{BsvHomeConfirmError, SendInput};
use crate::BsvHomeCoreError;
use crate::models::custom_types::{BsvHomeBigInt, BsvHomeBigUint};
use crate::models::transaction::BsvHomeSignedTransaction;
use crate::services::transfer::BsvHomeTransferData;
use crate::services::wallet::addresses::single::{single_checkpoint_input, spend_from_single_input, validate_single_checkpoint};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
pub struct BsvHomeTransactionCheckpointData {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub bsv_spend: Option<BsvSpendMetadata>,
    #[serde(default)]
    pub original_value: Option<BsvHomeBigInt>,
    #[serde(default)]
    pub use_max_amount: bool,
    #[serde(default)]
    pub gas_price: Option<GasPriceType>,
    pub wallet_id: WalletId,
    pub asset_id: AssetId,
    pub sender_address: String,
    pub recipient_address: String,
    pub value: BsvHomeBigInt,
    pub network_fee: BsvHomeBigUint,
    pub fee_asset_id: AssetId,
    pub transaction_id: String,
    pub signed_data: String,
}

#[uniffi::export(rust, foreign)]
#[async_trait]
pub trait BsvHomeTransactionCheckpoint: Send + Sync {
    async fn load(&self) -> Result<Option<BsvHomeTransactionCheckpointData>, BsvHomeCoreError>;
    async fn save(&self, data: BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeCoreError>;
    async fn authorize_broadcast(&self, data: BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeCoreError>;
}

fn record_error(message: impl ToString) -> BsvHomeConfirmError {
    BsvHomeConfirmError::Record { msg: message.to_string() }
}

pub(super) fn checkpoint_error(error: BsvHomeCoreError) -> BsvHomeConfirmError {
    match error {
        BsvHomeCoreError::Cancelled => BsvHomeConfirmError::Cancelled,
        error => record_error(error),
    }
}

impl BsvHomeTransactionCheckpointData {
    pub(super) fn from_signed(input: &SendInput, transaction: &BsvHomeSignedTransaction) -> Result<Self, BsvHomeConfirmError> {
        if let TransactionLoadMetadata::BitcoinSv { spend } = &input.confirm.metadata {
            return Self::from_bsv_signed(input, transaction, spend);
        }
        if input.wallet.wallet_type == primitives::WalletType::PrivateKey && input.confirm.input.transfer.input_type.get_asset().chain() == Chain::BitcoinSv {
            return Self::from_single_bsv_signed(input, transaction);
        }
        let transfer = &input.confirm.input.transfer;
        if input.value != transfer.value {
            return Err(record_error("checkpoint payment value differs from confirmed value"));
        }
        let asset = fixed_transfer_asset(transfer)?;
        if input.confirm.fee.fee_asset != asset.chain.as_asset_id() {
            return Err(record_error("checkpoint fee asset differs from source network"));
        }
        let network_fee = input.network_fee.to_biguint().ok_or_else(|| record_error("negative checkpoint network fee"))?;
        let sender = input.wallet.account(asset.chain).ok_or(BsvHomeConfirmError::AccountMissing { chain: asset.chain })?;
        let transaction_id = payment_transaction_id(asset, &sender.address, &transfer.recipient.address, &input.value.to_string(), &transaction.data)?;
        Ok(Self {
            version: 0,
            bsv_spend: None,
            original_value: None,
            use_max_amount: false,
            gas_price: None,
            wallet_id: input.wallet.id.clone(),
            asset_id: asset.clone(),
            sender_address: sender.address.clone(),
            recipient_address: transfer.recipient.address.clone(),
            value: input.value.clone(),
            network_fee,
            fee_asset_id: input.confirm.fee.fee_asset.clone(),
            transaction_id,
            signed_data: transaction.data.clone(),
        })
    }

    fn from_bsv_signed(input: &SendInput, transaction: &BsvHomeSignedTransaction, spend: &BsvSpendMetadata) -> Result<Self, BsvHomeConfirmError> {
        let transfer = &input.confirm.input.transfer;
        let asset = bsv_transfer_asset(transfer)?;
        if input.value <= 0.into() || (!transfer.use_max_amount && input.value != transfer.value) {
            return Err(record_error("checkpoint payment value differs from confirmed value"));
        }
        if input.confirm.fee.fee_asset != asset.chain.as_asset_id() {
            return Err(record_error("checkpoint fee asset differs from source network"));
        }
        let signer_input: SignerInput = input.signer_input()?.into();
        let transaction_id = BsvChainSigner::validate_signed_spend(&transaction.data, &signer_input).map_err(record_error)?;
        let sender = input.wallet.account(asset.chain).ok_or(BsvHomeConfirmError::AccountMissing { chain: asset.chain })?;
        Ok(Self {
            version: 1,
            bsv_spend: Some(spend.clone()),
            original_value: Some(transfer.value.clone()),
            use_max_amount: transfer.use_max_amount,
            gas_price: Some(input.confirm.fee.gas_price_type.clone()),
            wallet_id: input.wallet.id.clone(),
            asset_id: asset.clone(),
            sender_address: sender.address.clone(),
            recipient_address: transfer.recipient.address.clone(),
            value: input.value.clone(),
            network_fee: input.network_fee.to_biguint().ok_or_else(|| record_error("negative checkpoint network fee"))?,
            fee_asset_id: input.confirm.fee.fee_asset.clone(),
            transaction_id,
            signed_data: transaction.data.clone(),
        })
    }

    fn from_single_bsv_signed(input: &SendInput, transaction: &BsvHomeSignedTransaction) -> Result<Self, BsvHomeConfirmError> {
        let transfer = &input.confirm.input.transfer;
        let asset = bsv_transfer_asset(transfer)?;
        if input.value <= 0.into() || (!transfer.use_max_amount && input.value != transfer.value) {
            return Err(record_error("single-address payment value differs from confirmation"));
        }
        if input.confirm.fee.fee_asset != *asset {
            return Err(record_error("single-address fee asset differs from native BSV"));
        }
        let signer_input: SignerInput = input.signer_input()?.into();
        let spend = spend_from_single_input(&input.wallet, &signer_input).map_err(record_error)?;
        let plan = bsv_home_bsv::signer::plan_transfer(&signer_input.input).map_err(record_error)?;
        let transaction_id = bsv_home_bitcoin::signer::verify_fork_id_plan(Chain::BitcoinSv, &plan, &transaction.data).map_err(record_error)?;
        Ok(Self {
            version: 3,
            bsv_spend: Some(spend),
            original_value: Some(transfer.value.clone()),
            use_max_amount: transfer.use_max_amount,
            gas_price: Some(input.confirm.fee.gas_price_type.clone()),
            wallet_id: input.wallet.id.clone(),
            asset_id: asset.clone(),
            sender_address: input.confirm.input.from.address.clone(),
            recipient_address: transfer.recipient.address.clone(),
            value: input.value.clone(),
            network_fee: input.network_fee.to_biguint().ok_or_else(|| record_error("negative single-address fee"))?,
            fee_asset_id: input.confirm.fee.fee_asset.clone(),
            transaction_id,
            signed_data: transaction.data.clone(),
        })
    }

    pub(crate) fn validate_transfer(&self, wallet: &Wallet, transfer: &BsvHomeTransferData) -> Result<(), BsvHomeConfirmError> {
        match self.version {
            0 => {
                if self.bsv_spend.is_some() || self.original_value.is_some() || self.use_max_amount || self.gas_price.is_some() {
                    return Err(record_error("legacy checkpoint contains unsupported spend fields"));
                }
            }
            1..=3 => return self.validate_bsv_transfer(wallet, transfer),
            _ => return Err(record_error("unsupported checkpoint version")),
        }
        let asset = fixed_transfer_asset(transfer)?;
        let sender = wallet.account(asset.chain).ok_or(BsvHomeConfirmError::AccountMissing { chain: asset.chain })?;
        if self.wallet_id != wallet.id
            || self.asset_id != *asset
            || self.sender_address != sender.address
            || self.recipient_address != transfer.recipient.address
            || self.value != transfer.value
            || self.fee_asset_id != asset.chain.as_asset_id()
        {
            return Err(record_error("saved payment does not match wallet or confirmed transfer"));
        }
        let transaction_id = payment_transaction_id(asset, &sender.address, &transfer.recipient.address, &self.value.to_string(), &self.signed_data)?;
        if transaction_id != self.transaction_id {
            return Err(record_error("saved payment transaction id mismatch"));
        }
        Ok(())
    }

    pub(super) fn validate_confirmation(&self, input: &SendInput) -> Result<(), BsvHomeConfirmError> {
        self.validate_transfer(&input.wallet, &input.confirm.input.transfer)?;
        if input.confirm.input.from.address != self.sender_address {
            return Err(BsvHomeConfirmError::SenderMismatch {
                from: input.confirm.input.from.address.clone(),
                signer: self.sender_address.clone(),
            });
        }
        if input.value != input.confirm.input.transfer.value && !input.confirm.input.transfer.use_max_amount {
            return Err(record_error("checkpoint payment value differs from confirmed value"));
        }
        if input.network_fee.to_biguint().as_ref() != Some(&self.network_fee) || input.confirm.fee.fee_asset != self.fee_asset_id {
            return Err(record_error("saved payment fee differs from confirmation"));
        }
        if matches!(self.version, 1 | 2) {
            if self.value != input.value || self.gas_price.as_ref() != Some(&input.confirm.fee.gas_price_type) {
                return Err(record_error("saved BSV spend amount or fee rate differs from confirmation"));
            }
            let TransactionLoadMetadata::BitcoinSv { spend } = &input.confirm.metadata else {
                return Err(record_error("saved BSV spend description missing from confirmation"));
            };
            if self.bsv_spend.as_ref() != Some(spend) {
                return Err(record_error("saved BSV spend differs from confirmation"));
            }
        }
        if self.version == 3 {
            let signer_input: SignerInput = input.signer_input()?.into();
            let spend = spend_from_single_input(&input.wallet, &signer_input).map_err(record_error)?;
            if self.value != input.value || self.gas_price.as_ref() != Some(&input.confirm.fee.gas_price_type) || self.bsv_spend.as_ref() != Some(&spend) {
                return Err(record_error("saved single-address plan differs from confirmation"));
            }
        }
        Ok(())
    }

    fn validate_bsv_transfer(&self, wallet: &Wallet, transfer: &BsvHomeTransferData) -> Result<(), BsvHomeConfirmError> {
        let asset = bsv_transfer_asset(transfer)?;
        let sender = wallet.account(asset.chain).ok_or(BsvHomeConfirmError::AccountMissing { chain: asset.chain })?;
        if self.wallet_id != wallet.id
            || self.asset_id != *asset
            || self.sender_address != sender.address
            || self.recipient_address != transfer.recipient.address
            || self.original_value.as_ref() != Some(&transfer.value)
            || self.use_max_amount != transfer.use_max_amount
            || (!self.use_max_amount && self.value != transfer.value)
            || self.value <= 0.into()
            || self.fee_asset_id != asset.chain.as_asset_id()
        {
            return Err(record_error("saved payment does not match wallet or confirmed transfer"));
        }
        let transaction_id = match self.version {
            1 => BsvChainSigner::validate_signed_spend(&self.signed_data, &self.signer_input()?),
            2 => BsvChainSigner::validate_cold_signed_spend(&self.signed_data, &self.signer_input()?),
            3 => {
                validate_single_checkpoint(wallet, self).map_err(record_error)?;
                Ok(self.transaction_id.clone())
            }
            _ => return Err(record_error("unsupported BSV checkpoint version")),
        }
        .map_err(record_error)?;
        if transaction_id != self.transaction_id {
            return Err(record_error("saved payment transaction id mismatch"));
        }
        Ok(())
    }

    pub(crate) fn signer_input(&self) -> Result<SignerInput, BsvHomeConfirmError> {
        if self.version == 3 {
            return single_checkpoint_input(self).map_err(record_error);
        }
        if !matches!(self.version, 1 | 2) || self.asset_id != Chain::BitcoinSv.as_asset_id() || self.fee_asset_id != self.asset_id {
            return Err(record_error("checkpoint has no BSV derived spend plan"));
        }
        let spend = self.bsv_spend.clone().ok_or_else(|| record_error("checkpoint BSV spend plan missing"))?;
        let gas_price = self.gas_price.clone().ok_or_else(|| record_error("checkpoint BSV fee rate missing"))?;
        let value = self.value.to_biguint().ok_or_else(|| record_error("negative checkpoint value"))?;
        Ok(SignerInput::new(
            TransactionLoadInput {
                input_type: TransactionInputType::Transfer {
                    asset: Asset::from_chain(Chain::BitcoinSv),
                },
                sender_address: self.sender_address.clone(),
                destination_address: self.recipient_address.clone(),
                value,
                gas_price: gas_price.clone(),
                memo: None,
                is_max_value: self.use_max_amount,
                metadata: TransactionLoadMetadata::BitcoinSv { spend },
            },
            TransactionFee {
                fee: self.network_fee.clone().into(),
                gas_price_type: gas_price,
                gas_limit: 1.into(),
                options: Default::default(),
                fee_asset: self.fee_asset_id.clone(),
            },
        ))
    }

    pub(super) fn signed_transaction(&self) -> BsvHomeSignedTransaction {
        BsvHomeSignedTransaction {
            data: self.signed_data.clone(),
            transaction_type: TransactionType::Transfer,
        }
    }
}

pub(super) fn bsv_transfer_asset(transfer: &BsvHomeTransferData) -> Result<&AssetId, BsvHomeConfirmError> {
    if transfer.recipient.memo.is_some() || !transfer.recipient.references.is_empty() || transfer.value < 0.into() || (!transfer.use_max_amount && transfer.value == 0.into()) {
        return Err(record_error("BSV checkpoint requires a positive native transfer without memo"));
    }
    match &transfer.input_type {
        TransactionInputType::Transfer { asset } if asset.id == Chain::BitcoinSv.as_asset_id() => Ok(&asset.id),
        _ => Err(record_error("unsupported BSV checkpoint asset or operation")),
    }
}

fn fixed_transfer_asset(transfer: &BsvHomeTransferData) -> Result<&AssetId, BsvHomeConfirmError> {
    if transfer.use_max_amount || transfer.value <= 0.into() || transfer.recipient.memo.is_some() || !transfer.recipient.references.is_empty() {
        return Err(record_error("checkpoint payments require a fixed positive amount without memo"));
    }
    match &transfer.input_type {
        TransactionInputType::Transfer { asset }
            if asset.id == Chain::BitcoinSv.as_asset_id()
                || asset.id == Chain::Tron.as_asset_id()
                || asset.id == AssetId::from(Chain::Tron, Some("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".to_string())) =>
        {
            Ok(&asset.id)
        }
        _ => Err(record_error("unsupported checkpoint payment asset or operation")),
    }
}

fn payment_transaction_id(asset: &AssetId, sender: &str, recipient: &str, value: &str, data: &str) -> Result<String, BsvHomeConfirmError> {
    if *asset == Chain::BitcoinSv.as_asset_id() {
        return BsvChainSigner::validate_signed_transfer(data, sender, recipient, value.parse::<u64>().map_err(record_error)?).map_err(record_error);
    }
    match &asset.token_id {
        Some(token) => validate_signed_token_transfer(data, sender, recipient, token, value).map_err(record_error),
        None => validate_signed_native_transfer(data, sender, recipient, value).map_err(record_error),
    }
}

pub(super) async fn saved_transaction(
    input: &SendInput,
    checkpoint: &Arc<dyn BsvHomeTransactionCheckpoint>,
) -> Result<Option<BsvHomeTransactionCheckpointData>, BsvHomeConfirmError> {
    let transfer = &input.confirm.input.transfer;
    if transfer.input_type.get_asset().chain() == Chain::BitcoinSv {
        bsv_transfer_asset(transfer)?;
    } else {
        fixed_transfer_asset(transfer)?;
    }
    if input.value != transfer.value && !transfer.use_max_amount {
        return Err(record_error("checkpoint payment value differs from confirmed value"));
    }
    let saved = checkpoint.load().await.map_err(checkpoint_error)?;
    if let Some(saved) = &saved {
        saved.validate_confirmation(input)?;
    }
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::Ordering};

    use bsv_home_bsv::BsvAddress;
    use bsv_home_tron::address::TronAddress;
    use futures::executor::block_on;
    use primitives::testkit::{ABANDON_PHRASE, signer_mock::TEST_PRIVATE_KEY_ETHEREUM_ADDRESS};
    use primitives::{
        Account, Address, Asset, BsvAddressBranch, BsvAddressLocator, BsvSpendInput, Chain, GasPriceType, TransactionInputType, TransactionLoadMetadata, TronStakeData, Wallet,
    };
    use zeroize::Zeroizing;

    use super::*;
    use crate::services::confirm::testkit::{
        BsvTransactionSigner, ConfirmTestkit, DerivedBsvTransactionSigner, DerivedSendContext, MemoryTransactionCheckpoint, RecordingBroadcastProvider, TronTransactionSigner,
        bsv_send_input as send_input, confirm_data,
    };
    use crate::services::confirm::{BsvHomeExecuteResult, BsvHomeTransactionSigner};
    use crate::services::transfer::BsvHomeRecipient;
    use crate::services::wallet::addresses::testkit::MemoryBsvAddressStore;

    fn tron_send_input() -> SendInput {
        let chain = Chain::Tron;
        let address = TronAddress::from_hex(&format!("41{}", TEST_PRIVATE_KEY_ETHEREUM_ADDRESS.strip_prefix("0x").unwrap()))
            .unwrap()
            .encode();
        let wallet = Wallet::mock_with_accounts(vec![Account::mock(chain, &address)]);
        let input_type = TransactionInputType::Transfer { asset: Asset::from_chain(chain) };
        let mut confirm = confirm_data(chain, input_type, &address);
        confirm.input.transfer.recipient = BsvHomeRecipient::address("THTR75o8xXAgCTQqpiot2AFRAjvW1tSbVV".into());
        confirm.input.transfer.value = 2_000_000.into();
        confirm.input.transfer.use_max_amount = false;
        confirm.fee.fee = 23.into();
        confirm.fee.fee_asset = chain.as_asset_id();
        confirm.metadata = TransactionLoadMetadata::Tron {
            block_number: 3_111_739,
            block_version: 3,
            block_timestamp: 1_539_295_479_000,
            transaction_tree_root: "64288c2db0641316762a99dbb02ef7c90f968b60f9f2e410835980614332f86d".into(),
            parent_hash: "00000000002f7b3af4f5f8b9e23a30c530f719f165b742e7358536b280eead2d".into(),
            witness_address: "415863f6091b8e71766da808b1dd3159790f61de7d".into(),
            stake_data: TronStakeData::Votes { votes: vec![] },
        };
        SendInput {
            wallet,
            confirm,
            value: 2_000_000.into(),
            network_fee: 23.into(),
            simulation: None,
        }
    }

    fn derived_max_input() -> SendInput {
        let mut input = send_input();
        let addresses = ["1C6Rc3w25VHud3dLDamutaqfKWqhrLRTaD", "1NVYv5jmr9JRF3usPZJQmJFJhbQhrPESTP"];
        let spend = BsvSpendMetadata {
            inputs: addresses
                .iter()
                .enumerate()
                .map(|(index, address)| BsvSpendInput {
                    transaction_id: format!("{:064}", index + 1),
                    vout: 0,
                    value: [50000, 30000][index],
                    address: address.to_string(),
                    script_pubkey: hex::encode(BsvAddress::from_str(address).unwrap().script_pubkey().as_bytes()),
                    locator: BsvAddressLocator {
                        branch: BsvAddressBranch::Receive,
                        index: index as u32,
                    },
                })
                .collect(),
            change: None,
        };
        input.confirm.metadata = TransactionLoadMetadata::BitcoinSv { spend };
        input.confirm.input.transfer.recipient = BsvHomeRecipient::address("16yH2E12NYA5pg1d4BB7wtXXnBTZ8Lws7L".to_string());
        input.confirm.input.transfer.value = 0.into();
        input.confirm.input.transfer.use_max_amount = true;
        input.confirm.fee.fee = 34.into();
        input.value = 79966.into();
        input.network_fee = 34.into();
        input
    }

    fn signed_derived(input: &SendInput) -> BsvHomeSignedTransaction {
        let signer_input = input.signer_input().unwrap().into();
        let data = BsvChainSigner
            .sign_transfer_with_keys(&signer_input, |locator| Ok(Zeroizing::new(vec![locator.index as u8 + 1; 32])))
            .unwrap();
        BsvHomeSignedTransaction {
            data,
            transaction_type: TransactionType::Transfer,
        }
    }

    #[test]
    fn test_hd_send_stops_before_broadcast_when_signed_pool_checkpoint_cannot_be_saved() {
        block_on(async {
            let context = DerivedSendContext::new();
            let provider = Arc::new(RecordingBroadcastProvider::default());
            let store = Arc::new(MemoryBsvAddressStore::default());
            let testkit = context.testkit(provider.clone(), store.clone());
            testkit.bsv_addresses.initialize_import(&context.input.wallet, ABANDON_PHRASE).await.unwrap();
            let signer = Arc::new(DerivedBsvTransactionSigner {
                fail_signed_save: Some(store.clone()),
                ..Default::default()
            });
            let result = testkit.confirm.execute(context.input.clone(), signer.clone()).await;
            assert_eq!(result.unwrap_err().to_string(), "HD signed checkpoint storage failed");
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert!(provider.requests.lock().unwrap().is_empty());
            assert!(testkit.transaction_status.tracked.lock().unwrap().is_empty());
            let transfer = &context.input.confirm.input.transfer;
            assert_eq!(
                testkit
                    .bsv_addresses
                    .saved_for_transfer(&context.input.wallet, &transfer.recipient.address, &transfer.value.to_string(), false)
                    .await
                    .unwrap(),
                None
            );
            let pending = testkit
                .bsv_addresses
                .pending_input(&context.input.wallet, &transfer.recipient.address, &transfer.value.to_string(), false)
                .await
                .unwrap()
                .unwrap();
            let TransactionLoadMetadata::BitcoinSv { spend: pending_spend } = &pending.metadata else {
                panic!("pending send must retain BSV metadata");
            };
            let TransactionLoadMetadata::BitcoinSv { spend: confirmed_spend } = &context.input.confirm.metadata else {
                panic!("derived send must retain BSV metadata");
            };
            assert_eq!(pending_spend, confirmed_spend);
            assert_eq!(pending.value, 60000u64.into());
            assert_eq!(pending.fee.fee, 38u64.into());
            assert_eq!(pending.gas_price, GasPriceType::regular(100));
        });
    }

    #[test]
    fn test_hd_send_stops_before_broadcast_when_external_checkpoint_cannot_be_saved() {
        block_on(async {
            let context = DerivedSendContext::new();
            let provider = Arc::new(RecordingBroadcastProvider::default());
            let testkit = context.testkit(provider.clone(), Arc::new(MemoryBsvAddressStore::default()));
            testkit.bsv_addresses.initialize_import(&context.input.wallet, ABANDON_PHRASE).await.unwrap();
            let signer = Arc::new(DerivedBsvTransactionSigner::default());
            let checkpoint = Arc::new(MemoryTransactionCheckpoint {
                fail_writes: true,
                ..Default::default()
            });
            let result = testkit
                .confirm
                .execute_with_checkpoint(context.input.clone(), signer.clone(), Some(checkpoint.clone()))
                .await;
            assert_eq!(result.unwrap_err().to_string(), "checkpoint storage failed");
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert!(provider.requests.lock().unwrap().is_empty());
            assert_eq!(checkpoint.authorizations.load(Ordering::SeqCst), 0);
            assert_eq!(*checkpoint.saved.lock().unwrap(), None);
            let transfer = &context.input.confirm.input.transfer;
            let saved = testkit
                .bsv_addresses
                .saved_for_transfer(&context.input.wallet, &transfer.recipient.address, &transfer.value.to_string(), false)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(saved.version, 1);
            assert_eq!(saved.value, 60000.into());
            assert_eq!(saved.network_fee, 38u32.into());
            saved.validate_transfer(&context.input.wallet, transfer).unwrap();
        });
    }

    #[test]
    fn test_hd_send_timeout_restarts_and_rebroadcasts_only_the_original_signed_bytes() {
        block_on(async {
            let context = DerivedSendContext::new();
            let provider = Arc::new(RecordingBroadcastProvider::default());
            let store = Arc::new(MemoryBsvAddressStore::default());
            let signer = Arc::new(DerivedBsvTransactionSigner::default());
            let testkit = context.testkit(provider.clone(), store.clone());
            testkit.bsv_addresses.initialize_import(&context.input.wallet, ABANDON_PHRASE).await.unwrap();
            assert!(testkit.confirm.execute(context.input.clone(), signer.clone()).await.is_err());
            let transfer = &context.input.confirm.input.transfer;
            let saved = testkit
                .bsv_addresses
                .saved_for_transfer(&context.input.wallet, &transfer.recipient.address, &transfer.value.to_string(), false)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(saved.version, 1);
            assert_eq!(saved.original_value, Some(60000.into()));
            assert!(!saved.use_max_amount);
            assert_eq!(saved.value, 60000.into());
            assert_eq!(saved.network_fee, 38u32.into());
            let TransactionLoadMetadata::BitcoinSv { spend } = &context.input.confirm.metadata else {
                panic!("derived send must retain BSV metadata");
            };
            assert_eq!(saved.bsv_spend.as_ref(), Some(spend));
            assert!(testkit.transaction_status.tracked.lock().unwrap().is_empty());
            drop(testkit);

            let restarted = context.testkit(provider.clone(), store);
            assert!(restarted.confirm.execute(context.input.clone(), signer.clone()).await.is_err());
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert!(restarted.transaction_status.tracked.lock().unwrap().is_empty());
            let request_count = provider.requests.lock().unwrap().len();
            for alteration in 0..4 {
                let mut changed = context.input.clone();
                match alteration {
                    0 => {
                        let TransactionLoadMetadata::BitcoinSv { spend } = &mut changed.confirm.metadata else {
                            panic!("derived send must retain BSV metadata");
                        };
                        spend.inputs[0].locator.index += 1;
                    }
                    1 => changed.confirm.fee.gas_price_type = GasPriceType::regular(101),
                    2 => changed.confirm.fee.fee_asset = Chain::Tron.as_asset_id(),
                    3 => changed.confirm.input.from.address = saved.recipient_address.clone(),
                    _ => panic!("unexpected alteration"),
                }
                assert!(restarted.confirm.execute(changed, signer.clone()).await.is_err(), "alteration {alteration}");
                assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
                assert_eq!(provider.requests.lock().unwrap().len(), request_count);
            }
            assert_eq!(
                restarted
                    .bsv_addresses
                    .saved_for_transfer(&context.input.wallet, &transfer.recipient.address, &transfer.value.to_string(), false)
                    .await
                    .unwrap(),
                Some(saved.clone())
            );
            let broadcasts = provider
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|request| request.url.ends_with("/tx/raw"))
                .map(|request| request.body.clone())
                .collect::<Vec<_>>();
            assert_eq!(broadcasts.len(), 2);
            assert_eq!(broadcasts[0], broadcasts[1]);
            let body: serde_json::Value = serde_json::from_slice(broadcasts[0].as_ref().unwrap()).unwrap();
            assert_eq!(body["txhex"].as_str(), Some(saved.signed_data.as_str()));
            BsvChainSigner::validate_signed_spend(&saved.signed_data, &saved.signer_input().unwrap()).unwrap();
        });
    }

    #[test]
    fn test_legacy_checkpoint_fixture_restores_without_derived_fields() {
        let saved: BsvHomeTransactionCheckpointData = serde_json::from_str(include_str!("../../../testdata/bsv_legacy_checkpoint.json")).unwrap();
        let input = send_input();
        saved.validate_transfer(&input.wallet, &input.confirm.input.transfer).unwrap();
        assert_eq!(saved.version, 0);
        assert_eq!(saved.bsv_spend, None);
        assert_eq!(saved.original_value, None);
        assert!(!saved.use_max_amount);
        assert_eq!(saved.gas_price, None);
        let expected = block_on(BsvTransactionSigner::default().sign(input.wallet.clone(), input.signer_input().unwrap()))
            .unwrap()
            .remove(0);
        assert_eq!(BsvHomeTransactionCheckpointData::from_signed(&input, &expected).unwrap(), saved);
    }

    #[test]
    fn test_derived_max_checkpoint_restores_frozen_amount_plan_and_bytes() {
        block_on(async {
            let input = derived_max_input();
            let signed = signed_derived(&input);
            let saved = BsvHomeTransactionCheckpointData::from_signed(&input, &signed).unwrap();
            assert_eq!(saved.version, 1);
            assert_eq!(saved.original_value, Some(0.into()));
            assert!(saved.use_max_amount);
            assert_eq!(saved.value, 79966.into());
            assert_eq!(saved.network_fee, 34u32.into());
            assert_eq!(saved.gas_price, Some(GasPriceType::regular(100)));
            assert_eq!(saved.bsv_spend.as_ref().unwrap().change, None);
            saved.validate_transfer(&input.wallet, &input.confirm.input.transfer).unwrap();
            let serialized = serde_json::to_string(&saved).unwrap();
            let restored: BsvHomeTransactionCheckpointData = serde_json::from_str(&serialized).unwrap();
            assert_eq!(restored, saved);
            assert_eq!(signed_derived(&input).data, restored.signed_transaction().data);
            let checkpoint = Arc::new(MemoryTransactionCheckpoint::default());
            checkpoint.save(restored.clone()).await.unwrap();
            let checkpoint: Arc<dyn BsvHomeTransactionCheckpoint> = checkpoint;
            for _ in 0..2 {
                assert_eq!(saved_transaction(&input, &checkpoint).await.unwrap(), Some(restored.clone()));
            }
            let mut changed = input.clone();
            changed.value += 1;
            assert!(saved_transaction(&changed, &checkpoint).await.is_err());
            changed = input.clone();
            changed.confirm.fee.gas_price_type = GasPriceType::regular(101);
            assert!(saved_transaction(&changed, &checkpoint).await.is_err());
            changed = input.clone();
            if let TransactionLoadMetadata::BitcoinSv { spend } = &mut changed.confirm.metadata {
                spend.inputs[0].locator.index += 1;
            }
            assert!(saved_transaction(&changed, &checkpoint).await.is_err());
        });
    }

    #[test]
    fn test_derived_checkpoint_rejects_altered_intent_amount_fee_sources_and_signatures() {
        let input = derived_max_input();
        let saved = BsvHomeTransactionCheckpointData::from_signed(&input, &signed_derived(&input)).unwrap();
        let mut corrupted = saved.clone();
        corrupted.original_value = Some(1.into());
        assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
        corrupted = saved.clone();
        corrupted.use_max_amount = false;
        assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
        corrupted = saved.clone();
        corrupted.value += 1;
        assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
        corrupted = saved.clone();
        corrupted.network_fee += 1u32;
        assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
        corrupted = saved.clone();
        corrupted.gas_price = Some(GasPriceType::regular(200));
        assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
        corrupted = saved.clone();
        let spend = corrupted.bsv_spend.as_mut().unwrap();
        spend.inputs[0].value += 1;
        spend.inputs[1].value -= 1;
        assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
        corrupted = saved.clone();
        corrupted.transaction_id = format!("{:064}", 99);
        assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
        corrupted = saved.clone();
        corrupted.signed_data.replace_range(0..2, "03");
        assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
        corrupted = saved.clone();
        corrupted.version = 2;
        assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
        corrupted.version = 0;
        assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
        let mut changed = input.confirm.input.transfer.clone();
        changed.recipient.address = saved.sender_address.clone();
        assert!(saved.validate_transfer(&input.wallet, &changed).is_err());
        let mut wrong_wallet = input.wallet.clone();
        wrong_wallet.accounts[0].address = saved.recipient_address.clone();
        assert!(saved.validate_transfer(&wrong_wallet, &input.confirm.input.transfer).is_err());
    }

    #[test]
    fn test_trx_payment_reaches_signing_but_requires_durable_checkpoint_before_broadcast() {
        block_on(async {
            let input = tron_send_input();
            let provider = Arc::new(RecordingBroadcastProvider::default());
            let testkit = ConfirmTestkit::with_provider(input.wallet.clone(), input.wallet.clone(), provider.clone());
            let signer = Arc::new(TronTransactionSigner::default());
            let checkpoint = Arc::new(MemoryTransactionCheckpoint {
                fail_writes: true,
                ..Default::default()
            });
            let result = testkit.confirm.execute_with_checkpoint(input, signer.clone(), Some(checkpoint.clone())).await;
            assert_eq!(result.unwrap_err().to_string(), "checkpoint storage failed");
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert_eq!(provider.requests.lock().unwrap().len(), 0);
            assert_eq!(checkpoint.authorizations.load(Ordering::SeqCst), 0);
            assert_eq!(*checkpoint.saved.lock().unwrap(), None);
        });
    }

    #[test]
    fn test_trx_saved_payment_binds_signed_amount_addresses_hash_and_original_fee() {
        block_on(async {
            let input = tron_send_input();
            let signer = TronTransactionSigner::default();
            let signed = signer.sign(input.wallet.clone(), input.signer_input().unwrap()).await.unwrap().remove(0);
            let saved = BsvHomeTransactionCheckpointData::from_signed(&input, &signed).unwrap();
            saved.validate_transfer(&input.wallet, &input.confirm.input.transfer).unwrap();
            assert_eq!(saved.asset_id, Chain::Tron.as_asset_id());
            assert_eq!(saved.value, 2_000_000.into());
            assert_eq!(saved.network_fee, 23u32.into());
            assert_eq!(saved.fee_asset_id, Chain::Tron.as_asset_id());
            let checkpoint = Arc::new(MemoryTransactionCheckpoint::default());
            checkpoint.save(saved.clone()).await.unwrap();
            let checkpoint: Arc<dyn BsvHomeTransactionCheckpoint> = checkpoint;
            assert_eq!(saved_transaction(&input, &checkpoint).await.unwrap(), Some(saved.clone()));

            let mut changed = input.clone();
            changed.confirm.input.transfer.value += 1;
            assert!(saved.validate_transfer(&changed.wallet, &changed.confirm.input.transfer).is_err());
            changed = input.clone();
            changed.confirm.input.transfer.recipient.address = saved.sender_address.clone();
            assert!(saved.validate_transfer(&changed.wallet, &changed.confirm.input.transfer).is_err());
            changed = input.clone();
            changed.wallet.accounts[0].address = saved.recipient_address.clone();
            assert!(saved.validate_transfer(&changed.wallet, &changed.confirm.input.transfer).is_err());
            changed = input.clone();
            changed.network_fee += 1;
            assert_eq!(
                saved_transaction(&changed, &checkpoint).await.unwrap_err().to_string(),
                "saved payment fee differs from confirmation"
            );
            let mut corrupted = saved.clone();
            corrupted.transaction_id = format!("{:064}", 1);
            assert_eq!(
                corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).unwrap_err().to_string(),
                "saved payment transaction id mismatch"
            );
            assert_eq!(
                serde_json::from_str::<BsvHomeTransactionCheckpointData>(&serde_json::to_string(&saved).unwrap()).unwrap(),
                saved
            );
        });
    }

    #[test]
    fn test_trx_cancellation_after_signing_retains_payment_and_blocks_broadcast_and_replay() {
        block_on(async {
            let input = tron_send_input();
            let provider = Arc::new(RecordingBroadcastProvider::default());
            let testkit = ConfirmTestkit::with_provider(input.wallet.clone(), input.wallet.clone(), provider.clone());
            let signer = Arc::new(TronTransactionSigner::default());
            let checkpoint = Arc::new(MemoryTransactionCheckpoint {
                cancel_after_save: true,
                ..Default::default()
            });
            let result = testkit.confirm.execute_with_checkpoint(input.clone(), signer.clone(), Some(checkpoint.clone())).await;
            assert!(matches!(result.unwrap_err(), BsvHomeConfirmError::Cancelled));
            let saved = checkpoint.saved.lock().unwrap().clone().unwrap();
            let result = testkit
                .service
                .broadcast_saved_transaction(input.wallet.clone(), input.confirm.input.transfer.clone(), saved.clone(), checkpoint.clone())
                .await;
            assert!(matches!(result.unwrap_err(), BsvHomeConfirmError::Cancelled));
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert_eq!(provider.requests.lock().unwrap().len(), 0);
            assert_eq!(checkpoint.authorizations.load(Ordering::SeqCst), 2);
            assert_eq!(*checkpoint.saved.lock().unwrap(), Some(saved));
        });
    }

    #[test]
    fn test_trx_payment_broadcast_and_replay_use_one_signed_transaction() {
        block_on(async {
            let input = tron_send_input();
            let expected = TronTransactionSigner::default()
                .sign(input.wallet.clone(), input.signer_input().unwrap())
                .await
                .unwrap()
                .remove(0);
            let expected_hash = payment_transaction_id(
                &Chain::Tron.as_asset_id(),
                &input.wallet.accounts[0].address,
                &input.confirm.input.transfer.recipient.address,
                "2000000",
                &expected.data,
            )
            .unwrap();
            let provider = Arc::new(RecordingBroadcastProvider {
                tron_transaction_id: Some(expected_hash.clone()),
                ..Default::default()
            });
            let testkit = ConfirmTestkit::with_provider(input.wallet.clone(), input.wallet.clone(), provider.clone());
            let signer = Arc::new(TronTransactionSigner::default());
            let checkpoint = Arc::new(MemoryTransactionCheckpoint::default());

            for _ in 0..2 {
                let result = testkit
                    .confirm
                    .execute_with_checkpoint(input.clone(), signer.clone(), Some(checkpoint.clone()))
                    .await
                    .unwrap();
                match result {
                    BsvHomeExecuteResult::Sent { hashes, .. } => assert_eq!(hashes, vec![expected_hash.clone()]),
                    BsvHomeExecuteResult::Signed { .. } => panic!("checkpoint payment must broadcast"),
                }
            }
            let saved = checkpoint.saved.lock().unwrap().clone().unwrap();
            assert_eq!(saved.signed_data, expected.data);
            assert_eq!(saved.transaction_id, expected_hash);
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert_eq!(checkpoint.authorizations.load(Ordering::SeqCst), 2);
            let tracked = testkit.transaction_status.tracked.lock().unwrap().clone();
            assert_eq!(tracked.len(), 2);
            for (wallet_id, transactions) in tracked.iter() {
                assert_eq!(*wallet_id, input.wallet.id);
                assert_eq!(transactions.len(), 1);
                assert_eq!(transactions[0].id.hash, saved.transaction_id);
                assert_eq!(transactions[0].from, saved.sender_address);
                assert_eq!(transactions[0].to, saved.recipient_address);
                assert_eq!(transactions[0].value, saved.value.to_biguint().unwrap());
                assert_eq!(transactions[0].fee, saved.network_fee);
            }
            let broadcasts = provider.requests.lock().unwrap().iter().map(|request| request.body.clone()).collect::<Vec<_>>();
            assert_eq!(broadcasts.len(), 2);
            assert_eq!(broadcasts[0], broadcasts[1]);
            let mut changed = input.clone();
            changed.value += 1;
            changed.confirm.input.transfer.value += 1;
            assert!(testkit.confirm.execute_with_checkpoint(changed, signer.clone(), Some(checkpoint)).await.is_err());
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert_eq!(provider.requests.lock().unwrap().len(), 2);
        });
    }

    #[test]
    fn test_trx_broadcast_timeout_preserves_signed_payment_for_identical_replay() {
        block_on(async {
            let input = tron_send_input();
            let provider = Arc::new(RecordingBroadcastProvider::default());
            let testkit = ConfirmTestkit::with_provider(input.wallet.clone(), input.wallet.clone(), provider.clone());
            let signer = Arc::new(TronTransactionSigner::default());
            let checkpoint = Arc::new(MemoryTransactionCheckpoint::default());
            for _ in 0..2 {
                let result = testkit.confirm.execute_with_checkpoint(input.clone(), signer.clone(), Some(checkpoint.clone())).await;
                let error = result.unwrap_err();
                assert!(matches!(&error, BsvHomeConfirmError::Network { msg } if msg.contains("broadcast timed out")), "{error:?}");
            }
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert!(testkit.transaction_status.tracked.lock().unwrap().is_empty());
            let saved = checkpoint.saved.lock().unwrap().clone().unwrap();
            saved.validate_transfer(&input.wallet, &input.confirm.input.transfer).unwrap();
            let broadcasts = provider.requests.lock().unwrap().iter().map(|request| request.body.clone()).collect::<Vec<_>>();
            assert_eq!(broadcasts.len(), 2);
            assert_eq!(broadcasts[0], broadcasts[1]);
            assert_eq!(*checkpoint.saved.lock().unwrap(), Some(saved));
        });
    }

    #[test]
    fn test_broadcast_is_blocked_until_checkpoint_is_durable() {
        block_on(async {
            let input = send_input();
            let provider = Arc::new(RecordingBroadcastProvider::default());
            let testkit = ConfirmTestkit::with_provider(input.wallet.clone(), input.wallet.clone(), provider.clone());
            let signer = Arc::new(BsvTransactionSigner::default());
            let checkpoint = Arc::new(MemoryTransactionCheckpoint {
                fail_writes: true,
                ..Default::default()
            });
            let result = testkit.confirm.execute_with_checkpoint(input, signer.clone(), Some(checkpoint.clone())).await;
            assert_eq!(result.unwrap_err().to_string(), "checkpoint storage failed");
            assert_eq!(provider.requests.lock().unwrap().len(), 0);
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert_eq!(checkpoint.authorizations.load(Ordering::SeqCst), 0);
            assert_eq!(*checkpoint.saved.lock().unwrap(), None);
        });
    }

    #[test]
    fn test_cancellation_after_checkpoint_blocks_new_payment_and_saved_replay() {
        block_on(async {
            let input = send_input();
            let provider = Arc::new(RecordingBroadcastProvider::default());
            let testkit = ConfirmTestkit::with_provider(input.wallet.clone(), input.wallet.clone(), provider.clone());
            let signer = Arc::new(BsvTransactionSigner::default());
            let checkpoint = Arc::new(MemoryTransactionCheckpoint {
                cancel_after_save: true,
                ..Default::default()
            });
            let result = testkit.confirm.execute_with_checkpoint(input.clone(), signer.clone(), Some(checkpoint.clone())).await;
            assert!(matches!(result.unwrap_err(), BsvHomeConfirmError::Cancelled));
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert_eq!(checkpoint.authorizations.load(Ordering::SeqCst), 1);
            assert!(provider.requests.lock().unwrap().is_empty());
            let saved = checkpoint.saved.lock().unwrap().clone().unwrap();
            let result = testkit
                .service
                .broadcast_saved_transaction(input.wallet.clone(), input.confirm.input.transfer.clone(), saved.clone(), checkpoint.clone())
                .await;
            assert!(matches!(result.unwrap_err(), BsvHomeConfirmError::Cancelled));
            assert_eq!(checkpoint.authorizations.load(Ordering::SeqCst), 2);
            assert!(provider.requests.lock().unwrap().is_empty());
            assert_eq!(*checkpoint.saved.lock().unwrap(), Some(saved));
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn test_broadcast_timeout_retries_only_the_original_signed_payment() {
        block_on(async {
            let input = send_input();
            let provider = Arc::new(RecordingBroadcastProvider::default());
            let testkit = ConfirmTestkit::with_provider(input.wallet.clone(), input.wallet.clone(), provider.clone());
            let signer = Arc::new(BsvTransactionSigner::default());
            let checkpoint = Arc::new(MemoryTransactionCheckpoint::default());
            assert!(
                testkit
                    .confirm
                    .execute_with_checkpoint(input.clone(), signer.clone(), Some(checkpoint.clone()))
                    .await
                    .is_err()
            );
            let saved = checkpoint.saved.lock().unwrap().clone().unwrap();
            assert_eq!(saved.network_fee, 23u32.into());
            assert_eq!(saved.fee_asset_id, Chain::BitcoinSv.as_asset_id());
            assert!(
                testkit
                    .confirm
                    .execute_with_checkpoint(input.clone(), signer.clone(), Some(checkpoint.clone()))
                    .await
                    .is_err()
            );
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            assert_eq!(checkpoint.authorizations.load(Ordering::SeqCst), 2);
            assert_eq!(*checkpoint.saved.lock().unwrap(), Some(saved.clone()));
            let broadcasts = {
                let requests = provider.requests.lock().unwrap();
                requests
                    .iter()
                    .filter(|request| request.url.ends_with("/tx/raw"))
                    .map(|request| request.body.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(broadcasts.len(), 2);
            assert_eq!(broadcasts[0], broadcasts[1]);
            let mut changed = input.confirm.input.transfer.clone();
            changed.value += 1;
            assert!(saved.validate_transfer(&input.wallet, &changed).is_err());
            let mut corrupted = saved.clone();
            corrupted.transaction_id = format!("{:064}", 2);
            assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
            let mut corrupted = saved.clone();
            corrupted.fee_asset_id = Chain::Tron.as_asset_id();
            assert!(corrupted.validate_transfer(&input.wallet, &input.confirm.input.transfer).is_err());
            let mut changed_fee = input.clone();
            changed_fee.network_fee += 1;
            assert_eq!(
                testkit
                    .confirm
                    .execute_with_checkpoint(changed_fee, signer.clone(), Some(checkpoint.clone()))
                    .await
                    .unwrap_err()
                    .to_string(),
                "saved payment fee differs from confirmation"
            );
            assert_eq!(signer.calls.load(Ordering::SeqCst), 1);
            let serialized = serde_json::to_string(&saved).unwrap();
            assert_eq!(serde_json::from_str::<BsvHomeTransactionCheckpointData>(&serialized).unwrap(), saved);
        });
    }
}
