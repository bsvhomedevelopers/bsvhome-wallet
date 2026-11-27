use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use bitcoin::{Address, Network};
use bsv_home_bsv::multisig::policy_descriptor;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::model::{RemoteProposal, RemoteWallet};
use super::service::Cache;
use super::{BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigService, rules};
use crate::services::portal::BsvMultisigTarget as Target;

#[cfg(test)]
#[path = "receipts_tests.rs"]
mod tests;

const MAX_RECEIPTS: usize = 10_000;
const MAX_CURSOR_BYTES: usize = 2048;
const MAX_SATOSHIS: i64 = 2_100_000_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvMultisigActivityDirection {
    Sent,
    Received,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvMultisigActivityItem {
    pub transaction_id: String,
    pub direction: BsvHomeBsvMultisigActivityDirection,
    pub proposal_id: Option<String>,
    pub value_satoshis: String,
    pub fee_satoshis: Option<String>,
    pub memo: String,
    pub confirmed: bool,
    pub timestamp: Option<u64>,
    pub sender_addresses: Vec<String>,
    pub sender_script_hashes: Vec<String>,
    pub recipient_address: Option<String>,
    pub recipient_script_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvMultisigActivity {
    pub wallet_id: String,
    pub items: Vec<BsvHomeBsvMultisigActivityItem>,
    pub next_cursor: Option<String>,
    pub is_stale: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteReceipt {
    pub txid: String,
    pub satoshis: i64,
    pub confirmed: bool,
    pub timestamp: i64,
    pub fee: Option<i64>,
    #[serde(default)]
    pub sender_addresses: Vec<String>,
    #[serde(default)]
    pub sender_script_hashes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReceiptPage {
    items: Vec<RemoteReceipt>,
    next_cursor: String,
}

#[derive(Clone)]
pub(super) struct ActivityCache {
    configuration_hash: String,
    recent: BTreeMap<String, BsvHomeBsvMultisigActivityItem>,
    history: BTreeMap<String, BsvHomeBsvMultisigActivityItem>,
    first_seen: BTreeMap<String, u64>,
    next_cursor: Option<String>,
    cursors: BTreeSet<String>,
    failed_pages: BTreeSet<ActivityPage>,
}

impl ActivityCache {
    fn new(configuration_hash: String) -> Self {
        Self {
            configuration_hash,
            recent: BTreeMap::new(),
            history: BTreeMap::new(),
            first_seen: BTreeMap::new(),
            next_cursor: Some(String::new()),
            cursors: BTreeSet::new(),
            failed_pages: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ActivityPage {
    Recent,
    Older,
}

#[uniffi::export]
impl BsvHomeBsvMultisigService {
    pub fn get_activity(&self, wallet_id: String) -> Result<BsvHomeBsvMultisigActivity, Error> {
        self.workflow_checkpoint()?;
        self.check_identity()?;
        let wallet = self.remote_wallet(&wallet_id)?;
        let cache = self.cache()?;
        activity_snapshot(&wallet, &cache)
    }

    pub async fn refresh_activity(&self, wallet_id: String) -> Result<BsvHomeBsvMultisigActivity, Error> {
        self.load_activity(wallet_id, ActivityPage::Recent).await
    }

    pub async fn load_more_activity(&self, wallet_id: String) -> Result<BsvHomeBsvMultisigActivity, Error> {
        self.load_activity(wallet_id, ActivityPage::Older).await
    }
}

impl BsvHomeBsvMultisigService {
    async fn load_activity(&self, wallet_id: String, page: ActivityPage) -> Result<BsvHomeBsvMultisigActivity, Error> {
        self.workflow_checkpoint()?;
        let _mutation = self.mutation.lock().await;
        let local = self.current_wallet().await?;
        self.check_workflow_wallet(&local).await?;
        let wallet = self.remote_wallet(&wallet_id)?;
        if wallet.status != "ACTIVE" {
            return Err(Error::NotActivated);
        }
        let descriptor = policy_descriptor(&rules::wallet_policy(&wallet)?)?.configuration_hash;
        let mut updated = self
            .cache()?
            .activity
            .get(&wallet_id)
            .filter(|entry| entry.configuration_hash == descriptor)
            .cloned()
            .unwrap_or_else(|| ActivityCache::new(descriptor));
        let result = async {
            let (rows, next) = match page {
                ActivityPage::Recent => (
                    self.workflow_request::<Vec<RemoteReceipt>>(&local, Target::Receipts { wallet_id: wallet_id.clone() })
                        .await?,
                    None,
                ),
                ActivityPage::Older => {
                    let Some(cursor) = updated.next_cursor.clone() else {
                        return self.get_activity(wallet_id.clone());
                    };
                    if updated.cursors.len() >= MAX_RECEIPTS {
                        return Err(Error::invalid("RECEIPT_HISTORY_LIMIT"));
                    }
                    let page: ReceiptPage = self
                        .workflow_request(
                            &local,
                            Target::ReceiptPage {
                                wallet_id: wallet_id.clone(),
                                cursor: cursor.clone(),
                            },
                        )
                        .await?;
                    let next = if page.next_cursor.is_empty() { None } else { Some(page.next_cursor) };
                    if next.is_some() && page.items.is_empty()
                        || next
                            .as_ref()
                            .is_some_and(|value| value.len() > MAX_CURSOR_BYTES || value == &cursor || updated.cursors.contains(value))
                    {
                        return Err(Error::invalid("RECEIPT_HISTORY_CURSOR"));
                    }
                    updated.cursors.insert(cursor);
                    (page.items, Some(next))
                }
            };
            if rows.len() > MAX_RECEIPTS {
                return Err(Error::invalid("RECEIPT_HISTORY_LIMIT"));
            }
            let observed_at = rules::nonnegative(Utc::now().timestamp())?;
            let mut received = BTreeMap::new();
            for receipt in rows {
                let previous = received
                    .get(&receipt.txid)
                    .or_else(|| updated.recent.get(&receipt.txid))
                    .or_else(|| updated.history.get(&receipt.txid));
                let item = receipt_item(receipt, &wallet, previous)?;
                if received.get(&item.transaction_id).is_some_and(|previous| previous != &item) {
                    return Err(Error::invalid("INCONSISTENT_RECEIPT"));
                }
                if item.confirmed {
                    updated.first_seen.remove(&item.transaction_id);
                } else {
                    updated.first_seen.entry(item.transaction_id.clone()).or_insert(observed_at);
                }
                if let Some(previous) = updated.recent.get_mut(&item.transaction_id) {
                    *previous = item.clone();
                }
                if let Some(previous) = updated.history.get_mut(&item.transaction_id) {
                    *previous = item.clone();
                }
                received.insert(item.transaction_id.clone(), item);
            }
            match page {
                ActivityPage::Recent => updated.recent = received,
                ActivityPage::Older => updated.history.extend(received),
            }
            if updated.recent.keys().chain(updated.history.keys()).collect::<BTreeSet<_>>().len() > MAX_RECEIPTS {
                return Err(Error::invalid("RECEIPT_HISTORY_LIMIT"));
            }
            let (recent, history) = (&updated.recent, &updated.history);
            updated.first_seen.retain(|id, _| recent.contains_key(id) || history.contains_key(id));
            if let Some(next) = next {
                updated.next_cursor = next;
            }
            updated.failed_pages.remove(&page);
            self.check_workflow_wallet(&local).await?;
            self.cache()?.activity.insert(wallet_id.clone(), updated);
            self.get_activity(wallet_id.clone())
        }
        .await;
        if result.is_err()
            && self.workflow_checkpoint().is_ok()
            && let Some(cache) = self.cache()?.activity.get_mut(&wallet_id)
        {
            cache.failed_pages.insert(page);
        }
        result
    }
}

fn activity_snapshot(wallet: &RemoteWallet, cache: &Cache) -> Result<BsvHomeBsvMultisigActivity, Error> {
    let descriptor = policy_descriptor(&rules::wallet_policy(wallet)?)?.configuration_hash;
    let receipt_cache = cache.activity.get(&wallet.id);
    if receipt_cache.is_some_and(|value| value.configuration_hash != descriptor) {
        return Err(Error::ConfigurationConflict);
    }
    let proposals = cache.proposals.values().filter(|proposal| proposal.wallet_id == wallet.id).collect::<Vec<_>>();
    let mut rows = BTreeMap::new();
    for proposal in proposals {
        if proposal.status == "BROADCAST" || proposal.status == "CONFIRMED" {
            let item = proposal_item(proposal, wallet)?;
            if let Some(previous) = rows.insert(item.transaction_id.clone(), item.clone())
                && previous != item
            {
                return Err(Error::invalid("INCONSISTENT_TRANSACTION"));
            }
        }
    }
    let outgoing_ids = rows.keys().cloned().collect::<BTreeSet<_>>();
    if let Some(receipts) = receipt_cache {
        for (id, receipt) in receipts.history.iter().chain(receipts.recent.iter()) {
            if !outgoing_ids.contains(id) {
                rows.insert(id.clone(), receipt.clone());
            }
        }
    }
    let mut items = rows.into_values().collect::<Vec<_>>();
    let sort_time = |item: &BsvHomeBsvMultisigActivityItem| {
        item.timestamp.or_else(|| match &item.direction {
            BsvHomeBsvMultisigActivityDirection::Sent => None,
            BsvHomeBsvMultisigActivityDirection::Received => {
                if item.confirmed {
                    None
                } else {
                    receipt_cache.and_then(|cache| cache.first_seen.get(&item.transaction_id).copied())
                }
            }
        })
    };
    items.sort_by(|first, second| sort_time(second).cmp(&sort_time(first)).then_with(|| first.transaction_id.cmp(&second.transaction_id)));
    Ok(BsvHomeBsvMultisigActivity {
        wallet_id: wallet.id.clone(),
        items,
        next_cursor: receipt_cache.map(|value| value.next_cursor.clone()).unwrap_or(Some(String::new())),
        is_stale: receipt_cache.is_none_or(|value| !value.failed_pages.is_empty()),
    })
}

fn receipt_item(receipt: RemoteReceipt, wallet: &RemoteWallet, previous: Option<&BsvHomeBsvMultisigActivityItem>) -> Result<BsvHomeBsvMultisigActivityItem, Error> {
    if !hash(&receipt.txid)
        || !(1..=MAX_SATOSHIS).contains(&receipt.satoshis)
        || receipt.timestamp < 0
        || receipt.fee.is_some_and(|value| !(0..=MAX_SATOSHIS).contains(&value))
        || receipt.sender_addresses.iter().any(|address| !sender_address(address))
        || receipt.sender_script_hashes.iter().any(|value| !hash(value))
    {
        return Err(Error::invalid("INVALID_RECEIPT"));
    }
    let timestamp = if receipt.timestamp > 0 {
        Some(receipt.timestamp as u64)
    } else if receipt.confirmed {
        previous.filter(|value| value.confirmed).and_then(|value| value.timestamp)
    } else {
        None
    };
    let mut item = BsvHomeBsvMultisigActivityItem {
        transaction_id: receipt.txid,
        direction: BsvHomeBsvMultisigActivityDirection::Received,
        proposal_id: None,
        value_satoshis: receipt.satoshis.to_string(),
        fee_satoshis: receipt.fee.map(|value| value.to_string()),
        memo: String::new(),
        confirmed: receipt.confirmed,
        timestamp,
        sender_addresses: receipt.sender_addresses.into_iter().collect::<BTreeSet<_>>().into_iter().collect(),
        sender_script_hashes: receipt.sender_script_hashes.into_iter().collect::<BTreeSet<_>>().into_iter().collect(),
        recipient_address: None,
        recipient_script_hash: wallet.script_hash.clone(),
    };
    if let Some(previous) = previous {
        if previous.value_satoshis != item.value_satoshis
            || previous.fee_satoshis.is_some() && item.fee_satoshis.is_some() && previous.fee_satoshis != item.fee_satoshis
            || !previous.sender_addresses.is_empty() && !item.sender_addresses.is_empty() && previous.sender_addresses != item.sender_addresses
            || !previous.sender_script_hashes.is_empty() && !item.sender_script_hashes.is_empty() && previous.sender_script_hashes != item.sender_script_hashes
        {
            return Err(Error::invalid("INCONSISTENT_RECEIPT"));
        }
        if previous.confirmed && !item.confirmed {
            return Ok(previous.clone());
        }
        if item.fee_satoshis.is_none() {
            item.fee_satoshis.clone_from(&previous.fee_satoshis);
        }
        if item.sender_addresses.is_empty() {
            item.sender_addresses.clone_from(&previous.sender_addresses);
        }
        if item.sender_script_hashes.is_empty() {
            item.sender_script_hashes.clone_from(&previous.sender_script_hashes);
        }
    }
    Ok(item)
}

fn proposal_item(proposal: &RemoteProposal, wallet: &RemoteWallet) -> Result<BsvHomeBsvMultisigActivityItem, Error> {
    rules::validate_proposal(proposal, wallet)?;
    let transaction_id = proposal.txid.clone().ok_or_else(|| Error::invalid("INVALID_TRANSACTION"))?;
    if !hash(&transaction_id) {
        return Err(Error::invalid("INVALID_TRANSACTION"));
    }
    Ok(BsvHomeBsvMultisigActivityItem {
        transaction_id,
        direction: BsvHomeBsvMultisigActivityDirection::Sent,
        proposal_id: Some(proposal.id.clone()),
        value_satoshis: proposal.proposal.amount.to_string(),
        fee_satoshis: Some(proposal.proposal.fee.to_string()),
        memo: proposal.note.clone().unwrap_or_default(),
        confirmed: proposal.status == "CONFIRMED",
        timestamp: (proposal.created_at > 0).then_some(rules::nonnegative(proposal.created_at)?),
        sender_addresses: Vec::new(),
        sender_script_hashes: wallet.script_hash.clone().into_iter().collect(),
        recipient_address: Some(proposal.proposal.recipient.clone()),
        recipient_script_hash: None,
    })
}

fn hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sender_address(value: &str) -> bool {
    Address::from_str(value)
        .ok()
        .and_then(|address| address.require_network(Network::Bitcoin).ok())
        .is_some_and(|address| address.script_pubkey().is_p2pkh() || address.script_pubkey().is_p2sh())
}
