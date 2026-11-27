use bsv_home_bitcoin::signer::verify_fork_id_plan;
use bsv_home_bsv::{BsvAddress, signer::plan_transfer};
use primitives::{
    Address, Asset, BsvAddressBranch, BsvAddressLocator, BsvSpendInput, BsvSpendMetadata, Chain, SignerInput, TransactionFee, TransactionInputType, TransactionLoadInput,
    TransactionLoadMetadata, UTXO, Wallet, WalletType,
};

use super::state::pool_error;
use crate::services::confirm::BsvHomeTransactionCheckpointData;
use crate::services::error::BsvHomeServiceError;

pub(crate) fn spend_from_single_input(wallet: &Wallet, input: &SignerInput) -> Result<BsvSpendMetadata, BsvHomeServiceError> {
    let account = wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?;
    if wallet.wallet_type != WalletType::PrivateKey
        || wallet.id.wallet_type() != WalletType::PrivateKey
        || wallet.id.chain() != Some(Chain::BitcoinSv)
        || wallet.id.address() != account.address
        || input.sender_address != account.address
    {
        return Err(pool_error("BSV single-address wallet identity mismatch"));
    }
    let TransactionLoadMetadata::Bitcoin { .. } = &input.metadata else {
        return Err(pool_error("BSV single-address send requires its original UTXO plan"));
    };
    let plan = plan_transfer(&input.input).map_err(pool_error)?;
    if input.fee.fee_asset != Chain::BitcoinSv.as_asset_id()
        || input.gas_price != input.fee.gas_price_type
        || plan.fee != input.fee.fee().map_err(pool_error)?
        || plan.outputs.first().map(|output| output.value.to_sat()) != Some(input.value_as_u64().map_err(pool_error)?)
    {
        return Err(pool_error("BSV single-address plan differs from confirmation"));
    }
    let script = BsvAddress::from_str(&account.address).map_err(pool_error)?.script_pubkey();
    if plan.inputs.iter().any(|source| source.script_pubkey != script) {
        return Err(pool_error("BSV single-address input owner differs from wallet"));
    }
    Ok(BsvSpendMetadata {
        inputs: plan
            .inputs
            .iter()
            .map(|source| BsvSpendInput {
                transaction_id: source.previous_output.txid.to_string(),
                vout: source.previous_output.vout,
                value: source.value.to_sat(),
                address: account.address.clone(),
                script_pubkey: hex::encode(script.as_bytes()),
                locator: BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index: 0,
                },
            })
            .collect(),
        change: None,
    })
}

pub(crate) fn single_checkpoint_input(saved: &BsvHomeTransactionCheckpointData) -> Result<SignerInput, BsvHomeServiceError> {
    if saved.version != 3 || saved.asset_id != Chain::BitcoinSv.as_asset_id() || saved.fee_asset_id != saved.asset_id {
        return Err(pool_error("checkpoint has no single-address BSV plan"));
    }
    let spend = saved.bsv_spend.as_ref().ok_or_else(|| pool_error("single-address checkpoint inputs missing"))?;
    if spend.inputs.is_empty() || spend.change.is_some() {
        return Err(pool_error("invalid single-address checkpoint ownership"));
    }
    let gas_price = saved.gas_price.clone().ok_or_else(|| pool_error("single-address checkpoint fee rate missing"))?;
    let value = saved.value.to_biguint().ok_or_else(|| pool_error("negative single-address checkpoint value"))?;
    Ok(SignerInput::new(
        TransactionLoadInput {
            input_type: TransactionInputType::Transfer {
                asset: Asset::from_chain(Chain::BitcoinSv),
            },
            sender_address: saved.sender_address.clone(),
            destination_address: saved.recipient_address.clone(),
            value,
            gas_price: gas_price.clone(),
            memo: None,
            is_max_value: saved.use_max_amount,
            metadata: TransactionLoadMetadata::Bitcoin {
                utxos: spend
                    .inputs
                    .iter()
                    .map(|source| {
                        Ok(UTXO {
                            transaction_id: source.transaction_id.clone(),
                            vout: i32::try_from(source.vout).map_err(pool_error)?,
                            value: source.value.into(),
                            address: source.address.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, BsvHomeServiceError>>()?,
            },
        },
        TransactionFee {
            fee: saved.network_fee.clone().into(),
            gas_price_type: gas_price,
            gas_limit: 1.into(),
            options: Default::default(),
            fee_asset: saved.fee_asset_id.clone(),
        },
    ))
}

pub(crate) fn validate_single_checkpoint(wallet: &Wallet, saved: &BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeServiceError> {
    if saved.wallet_id != wallet.id
        || saved.original_value.as_ref().is_none_or(|value| *value < 0.into())
        || (!saved.use_max_amount && saved.original_value.as_ref() != Some(&saved.value))
        || saved.value <= 0.into()
    {
        return Err(pool_error("single-address checkpoint wallet or original intent missing"));
    }
    let input = single_checkpoint_input(saved)?;
    let spend = spend_from_single_input(wallet, &input)?;
    if saved.bsv_spend.as_ref() != Some(&spend) {
        return Err(pool_error("single-address checkpoint selected inputs differ from its actual plan"));
    }
    let plan = plan_transfer(&input.input).map_err(pool_error)?;
    let transaction_id = verify_fork_id_plan(Chain::BitcoinSv, &plan, &saved.signed_data).map_err(pool_error)?;
    if transaction_id != saved.transaction_id {
        return Err(pool_error("single-address checkpoint transaction id mismatch"));
    }
    Ok(())
}
