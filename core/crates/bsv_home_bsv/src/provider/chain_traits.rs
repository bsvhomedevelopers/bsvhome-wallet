use crate::rpc::{
    BsvClient,
    client::{ProviderError, rate_limited},
    models::*,
    target::BsvTarget,
};
use crate::{
    BsvAddress,
    signer::{finalize_spend_metadata, plan_transfer},
};
use async_trait::async_trait;
use bitcoin::{Transaction as BitcoinTransaction, consensus::deserialize};
use bsv_home_client::Target;
use bsv_home_client::{Client, ClientError};
use chain_traits::*;
use chrono::{TimeZone, Utc};
use num_bigint::{BigInt, BigUint};
use primitives::{
    Address, AssetBalance, Balance, BroadcastOptions, Chain, FeePriority, FeeRate, GasPriceType, NodeSyncStatus, TransactionFee, TransactionIdRequest, TransactionInputType,
    TransactionLoadData, TransactionLoadInput, TransactionLoadMetadata, TransactionPreloadInput, TransactionState, TransactionStateRequest, TransactionType, TransactionUpdate,
    TransactionUtxoInput, UTXO,
};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    time::Duration,
};

impl<C: Client> ChainProvider for BsvClient<C> {
    fn get_chain(&self) -> Chain {
        Chain::BitcoinSv
    }
}
#[async_trait]
impl<C: Client> ChainTraits for BsvClient<C> {}
#[async_trait]
impl<C: Client> ChainAccount for BsvClient<C> {}
#[async_trait]
impl<C: Client> ChainStaking for BsvClient<C> {}
#[async_trait]
impl<C: Client> ChainPerpetual for BsvClient<C> {}
#[async_trait]
impl<C: Client> ChainToken for BsvClient<C> {}
#[async_trait]
impl<C: Client> ChainAddressStatus for BsvClient<C> {}
#[async_trait]
impl<C: Client> ChainSimulation for BsvClient<C> {}
#[async_trait]
impl<C: Client> ChainBlockTransactions for BsvClient<C> {}

#[async_trait]
impl<C: Client> ChainState for BsvClient<C> {
    async fn get_chain_id(&self) -> Result<Option<String>, ProviderError> {
        let info: ChainInfo = self.get(BsvTarget::ChainInfo).await?;
        let checkpoint: Block = self.get(BsvTarget::Block { height: 556767 }).await?;
        if info.chain != "main" || checkpoint.hash != "000000000000000001d956714215d96ffc00e0afda4cd0a96c96f8d802b1662b" {
            return Err("node is not BSV mainnet".into());
        }
        Ok(Some(self.get_chain().network_id().to_string()))
    }
    async fn get_block_latest_number(&self) -> Result<u64, ProviderError> {
        Ok(self.get::<ChainInfo>(BsvTarget::ChainInfo).await?.blocks)
    }
    async fn get_node_status(&self) -> Result<NodeSyncStatus, ProviderError> {
        let info: ChainInfo = self.get(BsvTarget::ChainInfo).await?;
        Ok(NodeSyncStatus::new(info.blocks >= info.headers, Some(info.headers), Some(info.blocks)))
    }
}
#[async_trait]
impl<C: Client> ChainBalances for BsvClient<C> {
    async fn get_balance_staking(&self, _address: String) -> Result<Option<AssetBalance>, ProviderError> {
        Ok(None)
    }

    async fn get_balance_coin(&self, address: String) -> Result<AssetBalance, ProviderError> {
        let mut available = BigUint::ZERO;
        let mut pending = BigUint::ZERO;
        for output in self.native_unspent(&address).await? {
            match output.status.as_str() {
                "confirmed" if output.height.is_some_and(|height| height > 0) => available += output.value,
                "unconfirmed" => pending += output.value,
                _ => return Err("invalid BSV UTXO confirmation status".into()),
            }
        }
        Ok(AssetBalance::new_balance(
            self.get_chain().as_asset_id(),
            Balance::with_pending_unconfirmed(available, pending),
        ))
    }
}
#[async_trait]
impl<C: Client> ChainTransactionLoad for BsvClient<C> {
    async fn get_utxos(&self, address: String) -> Result<Vec<UTXO>, ProviderError> {
        let mut outputs = Vec::new();
        for output in self.native_unspent(&address).await? {
            if output.status == "unconfirmed" {
                continue;
            }
            outputs.push(UTXO {
                transaction_id: output.tx_hash,
                vout: i32::try_from(output.tx_pos)?,
                value: BigUint::from(output.value),
                address: address.clone(),
            });
        }
        Ok(outputs)
    }
    async fn get_transaction_preload(&self, input: TransactionPreloadInput) -> Result<TransactionLoadMetadata, ProviderError> {
        Ok(TransactionLoadMetadata::Bitcoin {
            utxos: self.get_utxos(input.sender_address).await?,
        })
    }
    async fn get_transaction_load(&self, input: TransactionLoadInput) -> Result<TransactionLoadData, ProviderError> {
        let plan = plan_transfer(&input)?;
        let fee = TransactionFee {
            fee: BigInt::from(plan.fee),
            gas_price_type: input.gas_price.clone(),
            gas_limit: BigInt::from(1),
            options: HashMap::new(),
            fee_asset: self.get_chain().as_asset_id(),
        };
        Ok(TransactionLoadData {
            fee,
            metadata: finalize_spend_metadata(&input, &plan)?,
        })
    }
    async fn get_transaction_fee_rates(&self, input_type: TransactionInputType) -> Result<Vec<FeeRate>, ProviderError> {
        if input_type.get_asset().id != self.get_chain().as_asset_id() {
            return Err("BSV token fees are unsupported".into());
        }
        let quote: FeeRecommendation = self.get(BsvTarget::FeeRecommendation).await?;
        if quote.fee_unit != "sat/KB" || quote.fee == 0 || quote.mempool_min_fee == 0 {
            return Err("unsupported BSV fee quotation".into());
        }
        Ok(vec![FeeRate::new(
            FeePriority::Normal,
            GasPriceType::regular(BigInt::from(quote.fee.max(quote.mempool_min_fee))),
        )])
    }
}
#[async_trait]
impl<C: Client> ChainTransactionBroadcast for BsvClient<C> {
    async fn transaction_broadcast(&self, data: String, _options: BroadcastOptions) -> Result<String, ProviderError> {
        self.broadcast_checked(data, &|| Ok(()), || async { Ok(()) }, &|| Ok(())).await
    }
}

impl<C: Client> BsvClient<C> {
    pub async fn broadcast_checked<F, Fut>(
        &self,
        data: String,
        check: &(dyn Fn() -> Result<(), ProviderError> + Sync),
        before_send: F,
        before_submission: &(dyn Fn() -> Result<(), ProviderError> + Sync),
    ) -> Result<String, ProviderError>
    where
        F: Fn() -> Fut + Sync,
        Fut: Future<Output = Result<(), ProviderError>> + Send,
    {
        let transaction: BitcoinTransaction = deserialize(&hex::decode(&data)?)?;
        let txid = transaction.compute_txid().to_string();
        check()?;
        self.scheduler.acquire().await;
        check()?;
        before_send().await?;
        check()?;
        before_submission()?;
        let started = std::time::Instant::now();
        let response = self.client.post_text(&BsvTarget::Broadcast.path(), &serde_json::json!({"txhex":data})).await;
        bsv_home_tracing::info_with_fields!(
            "BSV provider request",
            operation = "broadcast",
            elapsed_ms = started.elapsed().as_millis(),
            success = response.is_ok(),
            limited = rate_limited(&response)
        );
        match response {
            Ok(hash) if hash.trim() == txid => Ok(txid),
            Ok(_) => Err("BSV broadcast returned an unexpected transaction id".into()),
            Err(error) => {
                for attempt in 0..3 {
                    check()?;
                    let path = BsvTarget::Transaction { txid: txid.clone() }.path();
                    let read_check = || check().map_err(|_| ClientError::Network("BSV broadcast scope cancelled".into()));
                    let info = self
                        .read_with_checkpoint(
                            "broadcast_acceptance",
                            || async {
                                let body = self.client.get_text(&path).await?;
                                serde_json::from_str::<TransactionInfo>(&body).map_err(ClientError::from)
                            },
                            &read_check,
                        )
                        .await;
                    if info.is_ok_and(|info| info.txid == txid) {
                        return Ok(txid);
                    }
                    check()?;
                    if attempt < 2 {
                        bsv_home_client::sleep(Duration::from_secs(1 << attempt)).await;
                    }
                }
                Err(error.into())
            }
        }
    }
}
#[async_trait]
impl<C: Client> ChainTransactionState for BsvClient<C> {
    async fn get_transaction_status(&self, request: TransactionStateRequest) -> Result<TransactionUpdate, ProviderError> {
        let info = self.transaction_info(&request.id).await?;
        Ok(TransactionUpdate::new_state(if info.confirmations.unwrap_or(0) > 0 {
            TransactionState::Confirmed
        } else {
            TransactionState::Pending
        }))
    }
}
#[async_trait]
impl<C: Client> ChainTransactions for BsvClient<C> {
    async fn get_transactions_by_address(&self, request: TransactionsRequest) -> Result<TransactionsResult, ProviderError> {
        BsvAddress::from_str(&request.address)?;
        if request.asset_id.as_ref().is_some_and(|asset| *asset != self.get_chain().as_asset_id()) {
            return Err("BSV token history is unsupported".into());
        }
        let mut ids = HashSet::new();
        let mut transactions = Vec::new();
        for item in self.history(&request.address).await? {
            if ids.insert(item.tx_hash.clone()) {
                transactions.push(TransactionIdRequest::new(self.get_chain(), item.tx_hash, item.height.filter(|height| *height > 0)));
            }
        }
        let mut mapped = Vec::with_capacity(transactions.len());
        for transaction in transactions {
            let transaction = self.get_transaction_by_hash(transaction).await?.ok_or("BSV history transaction missing")?;
            mapped.push(super::transactions_mapper::finalize(transaction, &request.address)?);
        }
        Ok(TransactionsResult::Transactions(mapped))
    }
}
#[async_trait]
impl<C: Client> ChainTransaction for BsvClient<C> {
    async fn get_transaction_by_hash(&self, request: TransactionIdRequest) -> Result<Option<primitives::Transaction>, ProviderError> {
        let info = match self.transaction_info(&request.hash).await {
            Ok(info) => info,
            Err(error) => match error.downcast_ref::<ClientError>() {
                Some(ClientError::Http { status: 404, .. }) => return Ok(None),
                _ => return Err(error),
            },
        };
        let mut previous_transactions = HashMap::new();
        let mut inputs = Vec::new();
        let mut input_total = 0u128;
        if !info.is_coinbase() {
            for input in &info.vin {
                let txid = input.txid.clone().ok_or("BSV transaction input txid missing")?;
                let index = input.vout.ok_or("BSV transaction input index missing")?;
                if !previous_transactions.contains_key(&txid) {
                    previous_transactions.insert(txid.clone(), self.transaction_info(&txid).await?);
                }
                let output = previous_transactions.get(&txid).ok_or("BSV previous transaction missing")?.output(index)?;
                input_total += output.satoshis()? as u128;
                if let Some(address) = output.address()? {
                    inputs.push(TransactionUtxoInput::new(address.to_string(), output.satoshis()?.into()));
                }
            }
        }
        let mut output_total = 0u128;
        let mut outputs = Vec::new();
        for output in &info.vout {
            output_total += output.satoshis()? as u128;
            if let Some(address) = output.address()? {
                outputs.push(TransactionUtxoInput::new(address.to_string(), output.satoshis()?.into()));
            }
        }
        let fee = if info.is_coinbase() {
            0
        } else {
            input_total.checked_sub(output_total).ok_or("BSV transaction outputs exceed inputs")?
        };
        let time = info.blocktime.or(info.time).ok_or("BSV transaction timestamp missing")?;
        let created_at = Utc.timestamp_opt(time, 0).single().ok_or("invalid BSV timestamp")?;
        let mut mapped = primitives::Transaction::new_with_utxo(
            info.txid.clone(),
            self.get_chain().as_asset_id(),
            TransactionType::Transfer,
            if info.confirmations.unwrap_or(0) > 0 {
                TransactionState::Confirmed
            } else {
                TransactionState::Pending
            },
            BigUint::from(fee),
            self.get_chain().as_asset_id(),
            BigUint::from(output_total),
            None,
            Some(inputs),
            Some(outputs),
            None,
            created_at,
        );
        if info.is_coinbase() {
            mapped.from = "Coinbase".to_string();
        }
        mapped.block_number = info.blockheight.map(|height| height.to_string());
        Ok(Some(mapped))
    }
}
