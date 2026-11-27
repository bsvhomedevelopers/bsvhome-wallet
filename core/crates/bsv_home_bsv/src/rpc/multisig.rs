use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use bitcoin::{
    Transaction, Txid,
    consensus::{deserialize, serialize},
};
use bsv_home_client::{Client, ClientError, Target};
use serde::{Deserialize, de::DeserializeOwned};

use super::{
    BsvClient, History, ProviderError, Unspent,
    target::{BsvTarget, Confirmation},
};
use crate::multisig::{MultisigConfig, PreviousOutput, TransactionPlan, parse_locking_script, validate_plan};

const MAX_SCRIPT_PAGES: usize = 100;
const MAX_UNCONFIRMED_RESULTS: usize = 100_000;
const MAX_RAW_TRANSACTION_HEX: usize = 4_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionStatus {
    pub transaction_id: String,
    pub confirmations: u64,
    pub block_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutpointSpend {
    pub transaction_id: String,
    pub input_index: u32,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptTransactionHistory {
    pub transaction_id: String,
    pub confirmations: u64,
}

#[derive(Deserialize)]
struct ScriptResult<T> {
    script: String,
    result: Vec<T>,
    error: String,
    #[serde(rename = "nextPageToken", alias = "next-page", alias = "next_page")]
    next_page: Option<String>,
}

#[derive(Deserialize)]
struct SpentResponse {
    txid: String,
    vin: u32,
    status: String,
}

enum ScriptOutputState {
    Spendable,
    Unconfirmed,
    ImmatureCoinbase,
}

impl<C: Client> BsvClient<C> {
    async fn script_pages<T: DeserializeOwned + Send>(&self, script_hash: &str, target: impl Fn(Option<String>) -> BsvTarget) -> Result<Vec<T>, ProviderError> {
        let mut results = Vec::new();
        let mut tokens = HashSet::new();
        let mut token = None;
        for _ in 0..MAX_SCRIPT_PAGES {
            let page_target = target(token.clone());
            let confirmed_history_first_page = match &page_target {
                BsvTarget::ScriptHistory {
                    confirmation: Confirmation::Confirmed,
                    token,
                    ..
                } => token.is_none(),
                _ => false,
            };
            let page: ScriptResult<T> = match self.get(page_target).await {
                Ok(page) => page,
                Err(ClientError::Http { status: 404, body }) if confirmed_history_first_page && body.as_slice() == b"Not Found" => return Ok(results),
                Err(error) => return Err(error.into()),
            };
            if page.script != script_hash || !page.error.is_empty() {
                return Err("invalid WhatsOnChain script response".into());
            }
            results.extend(page.result);
            match page.next_page.filter(|token| !token.is_empty()) {
                Some(next) if tokens.insert(next.clone()) => token = Some(next),
                Some(_) => return Err("BSV script pagination token repeated".into()),
                None => return Ok(results),
            }
        }
        Err("BSV script pagination exceeded 100 pages".into())
    }

    pub async fn script_unspent(&self, script: &str) -> Result<Vec<Unspent>, ProviderError> {
        let script_hash = parse_locking_script(script)?.script_hash()?;
        let outputs: Vec<Unspent> = self
            .script_pages(&script_hash, |token| BsvTarget::ScriptUnspent {
                script_hash: script_hash.clone(),
                token,
            })
            .await?;
        let mut outpoints = HashSet::new();
        for output in &outputs {
            Txid::from_str(&output.tx_hash)?;
            if !outpoints.insert((&output.tx_hash, output.tx_pos)) || output.value == 0 || output.value > 2_100_000_000_000_000 {
                return Err("invalid or duplicate BSV script UTXO".into());
            }
            match output.status.as_str() {
                "confirmed" if output.height.is_some_and(|height| height > 0) => {}
                "unconfirmed" if output.height.is_none_or(|height| height == 0) => {}
                _ => return Err("invalid BSV script confirmation status".into()),
            }
        }
        if outputs.iter().filter(|output| output.status == "unconfirmed").count() >= MAX_UNCONFIRMED_RESULTS {
            return Err("BSV unconfirmed script output response reached supplier limit".into());
        }
        Ok(outputs)
    }

    pub async fn script_history(&self, script: &str) -> Result<Vec<History>, ProviderError> {
        let script_hash = parse_locking_script(script)?.script_hash()?;
        let mut history = Vec::new();
        let mut ids = HashSet::new();
        for confirmation in [Confirmation::Confirmed, Confirmation::Unconfirmed] {
            let entries: Vec<History> = self
                .script_pages(&script_hash, |token| BsvTarget::ScriptHistory {
                    script_hash: script_hash.clone(),
                    confirmation,
                    token,
                })
                .await?;
            let confirmed = match confirmation {
                Confirmation::Confirmed => true,
                Confirmation::Unconfirmed => false,
            };
            if !confirmed && entries.len() >= MAX_UNCONFIRMED_RESULTS {
                return Err("BSV unconfirmed script history response reached supplier limit".into());
            }
            for entry in entries {
                Txid::from_str(&entry.tx_hash)?;
                if confirmed != entry.height.is_some_and(|height| height > 0) {
                    return Err("BSV script history confirmation mismatch".into());
                }
                if ids.insert(entry.tx_hash.clone()) {
                    history.push(entry);
                }
            }
        }
        Ok(history)
    }

    pub async fn raw_transaction(&self, transaction_id: &str) -> Result<String, ProviderError> {
        self.raw_transaction_checked(transaction_id, &|| Ok(())).await
    }

    pub async fn raw_transaction_checked(&self, transaction_id: &str, check: &(dyn Fn() -> Result<(), ProviderError> + Sync)) -> Result<String, ProviderError> {
        let expected = Txid::from_str(transaction_id)?;
        let path = BsvTarget::RawTransaction { txid: transaction_id.to_string() }.path();
        let read_check = || check().map_err(|_| bsv_home_client::ClientError::Network("BSV raw transaction scope cancelled".into()));
        let raw = self.read_with_checkpoint("raw_transaction", || self.client.get_text(&path), &read_check).await?;
        let raw = raw.trim();
        if raw.len() > MAX_RAW_TRANSACTION_HEX {
            return Err("BSV previous transaction exceeds supported size".into());
        }
        let bytes = hex::decode(raw)?;
        let transaction: Transaction = deserialize(&bytes)?;
        if serialize(&transaction) != bytes || transaction.compute_txid() != expected || transaction.input.iter().any(|input| !input.witness.is_empty()) {
            return Err("BSV raw transaction identity or serialization mismatch".into());
        }
        Ok(hex::encode(bytes))
    }

    pub async fn script_transaction_history(&self, configuration: &MultisigConfig) -> Result<Vec<ScriptTransactionHistory>, ProviderError> {
        let script = configuration.locking_script()?;
        let history = self.script_history(&hex::encode(script.as_bytes())).await?;
        let mut previous_transactions = HashMap::new();
        let mut transactions = Vec::with_capacity(history.len());
        for entry in history {
            let raw = self.raw_transaction(&entry.tx_hash).await?;
            let transaction: Transaction = deserialize(&hex::decode(raw)?)?;
            let mut owned = transaction.output.iter().any(|output| output.script_pubkey == script);
            if !owned && !transaction.is_coinbase() {
                for input in &transaction.input {
                    let transaction_id = input.previous_output.txid.to_string();
                    if !previous_transactions.contains_key(&transaction_id) {
                        let raw = self.raw_transaction(&transaction_id).await?;
                        previous_transactions.insert(transaction_id.clone(), deserialize::<Transaction>(&hex::decode(raw)?)?);
                    }
                    let previous = previous_transactions
                        .get(&transaction_id)
                        .ok_or("BSV history previous transaction missing")?
                        .output
                        .get(input.previous_output.vout as usize)
                        .ok_or("BSV history previous output missing")?;
                    if previous.script_pubkey == script {
                        owned = true;
                        break;
                    }
                }
            }
            if !owned {
                return Err("BSV history transaction does not belong to the script".into());
            }
            let status = self.transaction_status(&entry.tx_hash).await?.ok_or("BSV history transaction disappeared")?;
            transactions.push(ScriptTransactionHistory {
                transaction_id: entry.tx_hash,
                confirmations: status.confirmations,
            });
        }
        Ok(transactions)
    }

    pub async fn transaction_status(&self, transaction_id: &str) -> Result<Option<TransactionStatus>, ProviderError> {
        let info = match self.transaction_info(transaction_id).await {
            Ok(info) => info,
            Err(error) => match error.downcast_ref::<ClientError>() {
                Some(ClientError::Http { status: 404, .. }) => return Ok(None),
                _ => return Err(error),
            },
        };
        let confirmations = info.confirmations.unwrap_or(0);
        if (confirmations > 0) != info.blockheight.is_some_and(|height| height > 0) {
            return Err("BSV transaction confirmation and block height disagree".into());
        }
        Ok(Some(TransactionStatus {
            transaction_id: info.txid,
            confirmations,
            block_height: info.blockheight,
        }))
    }

    pub async fn outpoint_spend(&self, transaction_id: &str, vout: u32) -> Result<Option<OutpointSpend>, ProviderError> {
        Txid::from_str(transaction_id)?;
        let target = BsvTarget::OutpointSpend {
            txid: transaction_id.to_string(),
            vout,
        };
        let response: SpentResponse = match self.get(target).await {
            Ok(value) => value,
            Err(ClientError::Http { status: 404, .. }) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let confirmed = match response.status.as_str() {
            "confirmed" => true,
            "unconfirmed" => false,
            _ => return Err("invalid BSV outpoint spending status".into()),
        };
        let raw = self.raw_transaction(&response.txid).await?;
        let transaction: Transaction = deserialize(&hex::decode(raw)?)?;
        let input = transaction.input.get(response.vin as usize).ok_or("BSV spending input missing")?;
        if input.previous_output.txid.to_string() != transaction_id || input.previous_output.vout != vout {
            return Err("BSV outpoint spending transaction mismatch".into());
        }
        Ok(Some(OutpointSpend {
            transaction_id: response.txid,
            input_index: response.vin,
            confirmed,
        }))
    }

    pub async fn script_balance(&self, configuration: &MultisigConfig) -> Result<(u64, u64), ProviderError> {
        let outputs = self.script_unspent(&hex::encode(configuration.locking_script()?.as_bytes())).await?;
        let mut confirmed = 0u64;
        let mut pending = 0u64;
        for (source, state) in self.script_output_evidence(configuration, outputs).await? {
            let total = match state {
                ScriptOutputState::Spendable => &mut confirmed,
                ScriptOutputState::Unconfirmed | ScriptOutputState::ImmatureCoinbase => &mut pending,
            };
            *total = total.checked_add(source.value).ok_or("BSV script balance overflow")?;
        }
        if confirmed as u128 + pending as u128 > 2_100_000_000_000_000 {
            return Err("BSV script balance exceeds monetary supply".into());
        }
        Ok((confirmed, pending))
    }

    pub async fn script_previous_outputs(&self, configuration: &MultisigConfig) -> Result<Vec<PreviousOutput>, ProviderError> {
        let outputs = self
            .script_unspent(&hex::encode(configuration.locking_script()?.as_bytes()))
            .await?
            .into_iter()
            .filter(|output| output.status == "confirmed")
            .collect();
        Ok(self
            .script_output_evidence(configuration, outputs)
            .await?
            .into_iter()
            .filter_map(|(source, state)| match state {
                ScriptOutputState::Spendable => Some(source),
                ScriptOutputState::Unconfirmed | ScriptOutputState::ImmatureCoinbase => None,
            })
            .collect())
    }

    async fn script_output_evidence(&self, configuration: &MultisigConfig, outputs: Vec<Unspent>) -> Result<Vec<(PreviousOutput, ScriptOutputState)>, ProviderError> {
        let script = configuration.locking_script()?;
        let mut transactions = HashMap::new();
        let mut sources = Vec::new();
        for output in outputs.into_iter().filter(|output| !output.spent_in_mempool) {
            if !transactions.contains_key(&output.tx_hash) {
                transactions.insert(
                    output.tx_hash.clone(),
                    (self.raw_transaction(&output.tx_hash).await?, self.transaction_info(&output.tx_hash).await?),
                );
            }
            let (raw_transaction, info) = transactions.get(&output.tx_hash).ok_or("BSV previous transaction missing")?;
            let confirmed = output.status == "confirmed";
            if confirmed != info.confirmations.is_some_and(|confirmations| confirmations > 0) || (confirmed && info.blockheight != output.height) {
                return Err("BSV multisig previous transaction confirmation changed".into());
            }
            let transaction: Transaction = deserialize(&hex::decode(raw_transaction)?)?;
            let state = if confirmed {
                if transaction.is_coinbase() && !info.confirmations.is_some_and(|confirmations| confirmations >= 100) {
                    ScriptOutputState::ImmatureCoinbase
                } else {
                    ScriptOutputState::Spendable
                }
            } else {
                ScriptOutputState::Unconfirmed
            };
            let previous = PreviousOutput {
                transaction_id: output.tx_hash,
                vout: output.tx_pos,
                value: output.value,
                raw_transaction: raw_transaction.clone(),
            };
            previous.validate(&script)?;
            sources.push((previous, state));
        }
        Ok(sources)
    }

    pub async fn validate_plan_sources(&self, configuration: &MultisigConfig, plan: &TransactionPlan) -> Result<(), ProviderError> {
        validate_plan(configuration, plan)?;
        let available = self.script_previous_outputs(configuration).await?;
        for previous in &plan.previous_outputs {
            if !available.iter().any(|source| source == previous) {
                return Err("BSV multisig source is spent, unconfirmed or missing".into());
            }
            if self.outpoint_spend(&previous.transaction_id, previous.vout).await?.is_some() {
                return Err("BSV multisig source was consumed".into());
            }
        }
        Ok(())
    }
}
