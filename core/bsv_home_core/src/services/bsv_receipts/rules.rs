use std::collections::{BTreeMap, VecDeque};

use bsv_home_bsv::BsvAddress;
use bsv_home_hash::sha2::sha256;
use primitives::Address;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::services::error::BsvHomeServiceError;
use crate::services::portal::rules::base_url;

const MAX_EVENT_BYTES: usize = 150_000;
const MAX_QUEUED_RECEIPTS: usize = 32;
const MAX_COMPLETED_RECEIPTS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Receipt {
    pub hash: String,
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Event {
    Changed(Receipt),
    Resync,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Work {
    Receipt(Receipt),
    Resync,
}

#[derive(Default)]
pub(super) struct ReceiptQueue {
    receipts: VecDeque<Receipt>,
    completed: VecDeque<String>,
    in_flight: Option<String>,
    resync: bool,
}

impl ReceiptQueue {
    pub fn push(&mut self, event: Event) -> bool {
        match event {
            Event::Resync => self.resync = true,
            Event::Changed(receipt) => {
                if self.completed.contains(&receipt.hash) || self.in_flight.as_ref() == Some(&receipt.hash) || self.receipts.iter().any(|queued| queued.hash == receipt.hash) {
                    return false;
                }
                if self.receipts.len() == MAX_QUEUED_RECEIPTS {
                    self.resync = true;
                } else {
                    self.receipts.push_back(receipt);
                }
            }
        }
        true
    }

    pub fn next(&mut self) -> Option<Work> {
        if self.resync {
            self.resync = false;
            return Some(Work::Resync);
        }
        self.receipts.pop_front().map(|receipt| {
            self.in_flight = Some(receipt.hash.clone());
            Work::Receipt(receipt)
        })
    }

    pub fn finish(&mut self, receipt: &Receipt, verified: bool) {
        self.in_flight = None;
        if verified {
            if !self.completed.contains(&receipt.hash) {
                self.completed.push_back(receipt.hash.clone());
                if self.completed.len() > MAX_COMPLETED_RECEIPTS {
                    self.completed.pop_front();
                }
            }
        } else {
            self.resync = true;
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Subscription<'a> {
    version: u32,
    #[serde(rename = "type")]
    kind: &'static str,
    network: &'static str,
    request_id: &'a str,
    script_hashes: Vec<&'a String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    version: u32,
    #[serde(rename = "type")]
    kind: String,
    network: String,
    request_id: String,
    transaction_id: Option<String>,
    script_hashes: Option<Vec<String>>,
}

pub(super) fn invalid(message: impl ToString) -> BsvHomeServiceError {
    BsvHomeServiceError::InvalidInput { msg: message.to_string() }
}

pub(super) fn socket_url(portal_url: &str) -> Result<String, BsvHomeServiceError> {
    let base = base_url(portal_url).map_err(|_| invalid("invalid BSV receipt Portal origin"))?;
    let mut url = Url::parse(&base).map_err(invalid)?;
    if url.path() != "/api/portal" {
        return Err(invalid("BSV receipt events require the ordinary Portal environment"));
    }
    url.set_scheme("wss").map_err(|_| invalid("invalid BSV receipt scheme"))?;
    url.set_path("/api/v1/wallet-events");
    Ok(url.into())
}

pub(super) fn receive_scripts(addresses: Vec<String>) -> Result<BTreeMap<String, String>, BsvHomeServiceError> {
    addresses
        .into_iter()
        .map(|address| {
            let parsed = BsvAddress::from_str(&address).map_err(invalid)?;
            let mut hash = sha256(parsed.script_pubkey().as_bytes());
            hash.reverse();
            Ok((hex::encode(hash), address))
        })
        .collect()
}

pub(super) fn subscription(request_id: &str, scripts: &BTreeMap<String, String>) -> Result<String, BsvHomeServiceError> {
    serde_json::to_string(&Subscription {
        version: 1,
        kind: "subscribe",
        network: "bsv-mainnet",
        request_id,
        script_hashes: scripts.keys().collect(),
    })
    .map_err(invalid)
}

pub(super) fn parse_event(text: &str, request_id: &str, scripts: &BTreeMap<String, String>) -> Option<Event> {
    if text.len() > MAX_EVENT_BYTES {
        return None;
    }
    let event: Envelope = serde_json::from_str(text).ok()?;
    if event.version != 1 || event.network != "bsv-mainnet" || event.request_id != request_id {
        return None;
    }
    match event.kind.as_str() {
        "resync.required" => Some(Event::Resync),
        "transaction.changed" => {
            let hash = event.transaction_id?;
            if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
                return None;
            }
            let address = event.script_hashes?.into_iter().find_map(|hash| scripts.get(&hash).cloned())?;
            Some(Event::Changed(Receipt { hash, address }))
        }
        _ => None,
    }
}
