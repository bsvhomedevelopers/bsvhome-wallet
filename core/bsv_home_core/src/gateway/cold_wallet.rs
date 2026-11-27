use std::{
    collections::{HashMap, hash_map::Entry},
    error::Error as StdError,
    fmt::Display,
    future::Future,
};

use bigdecimal::num_traits::ToPrimitive;
use bsv_home_bsv::cold_wallet::{ColdAddress, MAX_PAYLOAD, Pairing, Request, Review, Signed, decode_json};
use primitives::{Account, Asset, BsvAddressBranch, BsvSpendMetadata, Chain, FeePriority, GasPriceType, TransactionInputType, UTXO};

use super::{BsvHomeGateway, GatewayError, map_network_error};
use crate::services::error::BsvHomeServiceError;

pub(crate) struct ColdPreparation {
    pub request: Request,
    pub spend: BsvSpendMetadata,
    pub gas_price: GasPriceType,
}

#[uniffi::export]
impl BsvHomeGateway {
    pub async fn bsv_cold_prepare(&self, account: Account, recipient: String, amount: u64, change_index: u32) -> Result<String, GatewayError> {
        self.require_chain(Chain::BitcoinSv).map_err(invalid)?;
        let pairing = Pairing::from_account(&account).map_err(invalid)?;
        let mut next_change_index = change_index;
        let mut addresses = Vec::new();
        for branch in 0..2 {
            let mut empty = 0;
            for index in 0..1000 {
                let child = pairing.address(branch, index).map_err(invalid)?;
                let (history, _) = self.bsv_client().address_activity(&child.address).await.map_err(map_network_error)?;
                empty = if history { 0 } else { empty + 1 };
                if branch == 1 && history && index >= next_change_index {
                    next_change_index = index + 1;
                }
                addresses.push(child);
                if empty == 20 {
                    break;
                }
                if index == 999 {
                    return Err(invalid("Cold wallet discovery incomplete"));
                }
            }
        }
        let mut change_available = false;
        for _ in 0..1000 {
            let change = pairing.address(1, next_change_index).map_err(invalid)?;
            let (history, _) = self.bsv_client().address_activity(&change.address).await.map_err(map_network_error)?;
            if !history {
                change_available = true;
                break;
            }
            next_change_index += 1;
        }
        if !change_available {
            return Err(invalid("Cold wallet change discovery incomplete"));
        }
        let names = addresses.iter().map(|child| child.address.clone()).collect::<Vec<_>>();
        let outputs = self.bsv_client().wallet_native_unspent(&names).await.map_err(map_network_error)?;
        let utxos = outputs
            .into_iter()
            .filter(|(_, output)| output.status == "confirmed")
            .map(|(address, output)| {
                Ok(UTXO {
                    transaction_id: output.tx_hash,
                    vout: i32::try_from(output.tx_pos).map_err(invalid)?,
                    value: output.value.into(),
                    address,
                })
            })
            .collect::<Result<Vec<_>, GatewayError>>()?;
        let gas_price = self.cold_fee_rate().await?;
        let rate = gas_price.gas_price().to_u64().ok_or_else(|| invalid("Invalid cold wallet sat/KB fee rate"))?;
        let request = Request::prepare(&account, recipient, amount, rate, utxos, addresses, next_change_index).map_err(invalid)?;
        serde_json::to_string(&self.cold_previous_transactions(request).await?).map_err(invalid)
    }

    pub async fn bsv_cold_broadcast(&self, request_json: String, review_json: String, signed_json: String) -> Result<String, GatewayError> {
        self.require_chain(Chain::BitcoinSv).map_err(invalid)?;
        let request: Request = decode_json(&request_json).map_err(invalid)?;
        let review: Review = decode_json(&review_json).map_err(invalid)?;
        let signed: Signed = decode_json(&signed_json).map_err(invalid)?;
        let expected = request.transaction_id(&review, &signed).map_err(invalid)?;
        let returned = self.bsv_client().broadcast_cold_transaction(&signed.raw_transaction).await.map_err(map_network_error)?;
        if returned != expected {
            return Err(invalid("Cold wallet broadcast transaction ID mismatch"));
        }
        Ok(returned)
    }
}

impl BsvHomeGateway {
    pub(crate) async fn bsv_cold_broadcast_checked<F, Fut>(
        &self,
        request_json: String,
        review_json: String,
        signed_json: String,
        check: &(dyn Fn() -> Result<(), BsvHomeServiceError> + Sync),
        before_send: F,
        before_submission: &(dyn Fn() -> Result<(), BsvHomeServiceError> + Sync),
    ) -> Result<String, BsvHomeServiceError>
    where
        F: Fn() -> Fut + Sync,
        Fut: Future<Output = Result<(), BsvHomeServiceError>> + Send,
    {
        self.require_chain(Chain::BitcoinSv)?;
        let request: Request = decode_json(&request_json).map_err(invalid)?;
        let review: Review = decode_json(&review_json).map_err(invalid)?;
        let signed: Signed = decode_json(&signed_json).map_err(invalid)?;
        let expected = request.transaction_id(&review, &signed).map_err(invalid)?;
        let sdk_check = || check().map_err(|error| Box::new(error) as Box<dyn StdError + Send + Sync>);
        let sdk_before_send = || async { before_send().await.map_err(|error| Box::new(error) as Box<dyn StdError + Send + Sync>) };
        let sdk_submission = || before_submission().map_err(|error| Box::new(error) as Box<dyn StdError + Send + Sync>);
        match self
            .bsv_client()
            .broadcast_cold_transaction_checked(&signed.raw_transaction, &sdk_check, sdk_before_send, &sdk_submission)
            .await
        {
            Ok(returned) if returned == expected => Ok(returned),
            Ok(_) => Err(invalid("Cold wallet broadcast transaction ID mismatch").into()),
            Err(error) => match error.downcast_ref::<BsvHomeServiceError>() {
                Some(error) => Err(error.clone()),
                None => Err(map_network_error(error).into()),
            },
        }
    }

    pub(crate) async fn prepare_cold_spend(&self, account: Account, recipient: String, amount: u64, spend: BsvSpendMetadata) -> Result<ColdPreparation, GatewayError> {
        let change = spend.change.as_ref().ok_or_else(|| invalid("Cold wallet change address unavailable"))?;
        let addresses = spend
            .inputs
            .iter()
            .map(|input| ColdAddress {
                address: input.address.clone(),
                branch: match input.locator.branch {
                    BsvAddressBranch::Receive => 0,
                    BsvAddressBranch::Change => 1,
                },
                index: input.locator.index,
            })
            .collect();
        let utxos = spend
            .inputs
            .iter()
            .map(|input| {
                Ok(UTXO {
                    transaction_id: input.transaction_id.clone(),
                    vout: i32::try_from(input.vout).map_err(invalid)?,
                    value: input.value.into(),
                    address: input.address.clone(),
                })
            })
            .collect::<Result<Vec<_>, GatewayError>>()?;
        let gas_price = self.cold_fee_rate().await?;
        let rate = gas_price.gas_price().to_u64().ok_or_else(|| invalid("Invalid cold wallet sat/KB fee rate"))?;
        let request = Request::prepare(&account, recipient, amount, rate, utxos, addresses, change.locator.index).map_err(invalid)?;
        let transaction = request.outpoints().map_err(invalid)?;
        let inputs = transaction
            .into_iter()
            .map(|(hash, vout)| {
                spend
                    .inputs
                    .iter()
                    .find(|input| input.transaction_id == hash && input.vout == vout)
                    .cloned()
                    .ok_or_else(|| invalid("Cold wallet selected input missing"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let selected = BsvSpendMetadata {
            inputs,
            change: request.change_index.map(|_| change.clone()),
        };
        Ok(ColdPreparation {
            request: self.cold_previous_transactions(request).await?,
            spend: selected,
            gas_price,
        })
    }

    async fn cold_fee_rate(&self) -> Result<GasPriceType, GatewayError> {
        self.require_chain(Chain::BitcoinSv).map_err(invalid)?;
        self.get_fee_rates(
            Chain::BitcoinSv,
            TransactionInputType::Transfer {
                asset: Asset::from_chain(Chain::BitcoinSv),
            },
        )
        .await?
        .into_iter()
        .find(|rate| rate.priority == FeePriority::Normal)
        .map(|rate| rate.gas_price_type)
        .ok_or_else(|| invalid("Cold wallet normal fee rate unavailable"))
    }

    async fn cold_previous_transactions(&self, mut request: Request) -> Result<Request, GatewayError> {
        let mut previous = HashMap::new();
        for (index, hash) in request.previous_transaction_ids().map_err(invalid)?.into_iter().enumerate() {
            if let Entry::Vacant(entry) = previous.entry(hash.clone()) {
                let raw = self.bsv_client().raw_transaction(&hash).await.map_err(map_network_error)?;
                if raw.len() > MAX_PAYLOAD {
                    return Err(invalid("Cold wallet previous transaction exceeds limit"));
                }
                entry.insert(raw);
            }
            request.inputs[index].previous_transaction = previous.get(&hash).ok_or_else(|| invalid("Cold wallet previous transaction unavailable"))?.clone();
            if serde_json::to_vec(&request).map_err(invalid)?.len() > MAX_PAYLOAD {
                return Err(invalid("Cold wallet request exceeds QR payload limit"));
            }
        }
        request.review(&request.pairing).map_err(invalid)?;
        Ok(request)
    }
}

fn invalid(error: impl Display) -> GatewayError {
    GatewayError::PlatformError { msg: error.to_string() }
}
