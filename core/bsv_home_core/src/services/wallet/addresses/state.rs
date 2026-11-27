use std::collections::HashSet;

use bsv_home_bsv::signer::BsvChainSigner;
use bsv_home_derivation::{bsv_address, bsv_address_for_path};
use primitives::{Account, BsvAddressBranch, BsvAddressLocator, BsvSpendMetadata, Chain, GasPriceType, WalletId};
use serde::{Deserialize, Serialize};

use super::funding::{CancelledFunding, FundingReservation};
use super::ledger::PayerReservationPurpose;
use super::model::{BsvHomeBsvAddressRecord, BsvHomeBsvAddressSnapshot, BsvHomeBsvAddressStatus, BsvHomeBsvDiscoveryState};
use crate::services::confirm::BsvHomeTransactionCheckpointData;
use crate::services::error::BsvHomeServiceError;

pub(super) const GAP_LIMIT: u32 = 20;
const MAX_MANUAL_RECEIVE_COUNT: u32 = 10_000 - GAP_LIMIT;
const RESERVE_COUNT: usize = 10;

pub(super) fn pool_error(message: impl ToString) -> BsvHomeServiceError {
    BsvHomeServiceError::Core { msg: message.to_string() }
}

pub(super) fn can_add_receive_address(receive_count: u32, used_count: u32) -> bool {
    receive_count < used_count.saturating_add(GAP_LIMIT).min(MAX_MANUAL_RECEIVE_COUNT)
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct PoolAddress {
    pub locator: BsvAddressLocator,
    pub address: String,
    pub status: BsvHomeBsvAddressStatus,
    pub confirmed_history: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct ScanProgress {
    pub next_index: u32,
    pub empty_count: u32,
    pub stopped: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct SpendReservation {
    pub id: String,
    #[serde(default, skip_serializing_if = "PayerReservationPurpose::is_transfer")]
    pub purpose: PayerReservationPurpose,
    pub recipient: String,
    pub original_value: String,
    pub is_max: bool,
    pub value: u64,
    pub fee: u64,
    pub gas_price: GasPriceType,
    pub spend: BsvSpendMetadata,
    pub checkpoint: Option<BsvHomeTransactionCheckpointData>,
    pub confirmed: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct PoolState {
    pub version: u32,
    pub wallet_id: WalletId,
    pub account_xpub: String,
    pub account_path: Option<String>,
    pub cold_receive_count: Option<u32>,
    pub cold_selected_index: Option<u32>,
    pub receive_count: Option<u32>,
    pub selected_index: Option<u32>,
    pub records: Vec<PoolAddress>,
    pub receive: ScanProgress,
    pub change: ScanProgress,
    pub discovery: BsvHomeBsvDiscoveryState,
    pub reservations: Vec<SpendReservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub funding_reservations: Vec<FundingReservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cancelled_funding: Vec<CancelledFunding>,
}

impl PoolState {
    pub fn new(wallet_id: WalletId, account_xpub: String) -> Result<Self, BsvHomeServiceError> {
        let mut state = Self {
            version: 1,
            wallet_id,
            account_xpub,
            account_path: None,
            cold_receive_count: None,
            cold_selected_index: None,
            receive_count: Some(RESERVE_COUNT as u32),
            selected_index: Some(0),
            records: Vec::new(),
            receive: ScanProgress::default(),
            change: ScanProgress::default(),
            discovery: BsvHomeBsvDiscoveryState::NotStarted,
            reservations: Vec::new(),
            funding_reservations: Vec::new(),
            cancelled_funding: Vec::new(),
        };
        state.refill()?;
        Ok(state)
    }

    pub fn from_account(wallet_id: WalletId, account: &Account) -> Result<Self, BsvHomeServiceError> {
        let path = account
            .derivation_path
            .strip_suffix("/0/0")
            .ok_or_else(|| pool_error("BSV account must identify its first receiving address"))?;
        let xpub = account.extended_public_key.clone().ok_or_else(|| pool_error("BSV account xPub is missing"))?;
        let mut state = Self {
            version: 1,
            wallet_id,
            account_xpub: xpub,
            account_path: Some(path.to_owned()),
            cold_receive_count: None,
            cold_selected_index: None,
            receive_count: Some(RESERVE_COUNT as u32),
            selected_index: Some(0),
            records: Vec::new(),
            receive: ScanProgress::default(),
            change: ScanProgress::default(),
            discovery: BsvHomeBsvDiscoveryState::NotStarted,
            reservations: Vec::new(),
            funding_reservations: Vec::new(),
            cancelled_funding: Vec::new(),
        };
        state.refill()?;
        state.validate(&state.wallet_id, &account.address)?;
        Ok(state)
    }

    pub fn public_account(&self, account: &Account) -> Account {
        Account {
            chain: account.chain,
            address: account.address.clone(),
            derivation_path: format!("{}/0/0", self.account_path.as_deref().unwrap_or("m/44'/236'/0'")),
            extended_public_key: Some(self.account_xpub.clone()),
        }
    }

    pub fn validate(&self, wallet_id: &WalletId, main_address: &str) -> Result<(), BsvHomeServiceError> {
        if self.version != 1
            || &self.wallet_id != wallet_id
            || self.address(BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index: 0,
            })? != main_address
        {
            return Err(pool_error("BSV address pool does not match wallet"));
        }
        let mut locators = HashSet::new();
        let mut addresses = HashSet::new();
        for record in &self.records {
            if !locators.insert(record.locator) || !addresses.insert(&record.address) || self.address(record.locator)? != record.address {
                return Err(pool_error("invalid or duplicated BSV address ownership"));
            }
        }
        if !locators.contains(&BsvAddressLocator {
            branch: BsvAddressBranch::Receive,
            index: 0,
        }) {
            return Err(pool_error("BSV stable account is missing from address records"));
        }
        match (self.cold_receive_count, self.cold_selected_index) {
            (Some(count), Some(selected)) if (10..=9980).contains(&count) && selected < count => {}
            (None, None) => {}
            _ => return Err(pool_error("Invalid cold wallet receiving address selection")),
        }
        match (self.receive_count, self.selected_index) {
            (Some(count), Some(selected))
                if count > 0
                    && count <= (1 << 31)
                    && selected < count
                    && count as usize <= self.records.len()
                    && (0..count).all(|index| {
                        locators.contains(&BsvAddressLocator {
                            branch: BsvAddressBranch::Receive,
                            index,
                        })
                    }) => {}
            (None, None) => {}
            _ => return Err(pool_error("Invalid BSV receiving address selection")),
        }
        for progress in [&self.receive, &self.change] {
            if progress.next_index >= (1 << 31)
                || progress.empty_count > progress.next_index
                || progress.empty_count > GAP_LIMIT
                || progress.stopped != (progress.empty_count == GAP_LIMIT)
            {
                return Err(pool_error("invalid BSV discovery progress"));
            }
        }
        if (self.discovery == BsvHomeBsvDiscoveryState::Complete) != (self.receive.stopped && self.change.stopped) {
            return Err(pool_error("inconsistent BSV discovery completion"));
        }
        let mut reservation_ids = HashSet::new();
        for reservation in &self.reservations {
            if !reservation.purpose.is_transfer() {
                return Err(pool_error("BSV funding cannot use an ordinary transfer reservation"));
            }
            if !reservation_ids.insert(&reservation.id) || (reservation.confirmed && reservation.checkpoint.is_none()) {
                return Err(pool_error("invalid BSV spend reservation state"));
            }
            self.validate_spend(&reservation.spend)?;
            if let Some(saved) = &reservation.checkpoint {
                self.validate_checkpoint(saved)?;
                validate_reserved_checkpoint(reservation, saved)?;
            }
        }
        Ok(())
    }

    pub fn address(&self, locator: BsvAddressLocator) -> Result<String, BsvHomeServiceError> {
        match &self.account_path {
            Some(path) => bsv_address_for_path(&self.account_xpub, path, locator).map_err(pool_error),
            None => bsv_address(&self.account_xpub, locator).map_err(pool_error),
        }
    }

    pub fn validate_spend(&self, spend: &BsvSpendMetadata) -> Result<(), BsvHomeServiceError> {
        let mut outpoints = HashSet::new();
        for input in &spend.inputs {
            if !outpoints.insert((&input.transaction_id, input.vout))
                || self.address(input.locator)? != input.address
                || !self.records.iter().any(|record| record.locator == input.locator && record.address == input.address)
            {
                return Err(pool_error("BSV input does not belong to wallet"));
            }
        }
        if let Some(change) = &spend.change
            && (change.locator.branch != BsvAddressBranch::Change || self.address(change.locator)? != change.address)
        {
            return Err(pool_error("BSV change does not belong to wallet"));
        }
        Ok(())
    }

    pub fn validate_checkpoint(&self, saved: &BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeServiceError> {
        let spend = saved.bsv_spend.as_ref().ok_or_else(|| pool_error("BSV checkpoint spend missing"))?;
        self.validate_spend(spend)?;
        if saved.wallet_id != self.wallet_id
            || saved.asset_id != Chain::BitcoinSv.as_asset_id()
            || saved.fee_asset_id != saved.asset_id
            || saved.sender_address
                != self.address(BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index: 0,
                })?
        {
            return Err(pool_error("BSV checkpoint wallet identity mismatch"));
        }
        let input = saved.signer_input().map_err(pool_error)?;
        let transaction_id = match saved.version {
            1 => BsvChainSigner::validate_signed_spend(&saved.signed_data, &input),
            2 => BsvChainSigner::validate_cold_signed_spend(&saved.signed_data, &input),
            _ => return Err(pool_error("Unsupported BSV checkpoint version")),
        }
        .map_err(pool_error)?;
        if transaction_id != saved.transaction_id {
            return Err(pool_error("BSV saved transaction id mismatch"));
        }
        Ok(())
    }

    pub fn allocation_limit(&self, branch: BsvAddressBranch) -> u32 {
        self.records
            .iter()
            .filter(|record| record.locator.branch == branch && record.confirmed_history)
            .map(|record| record.locator.index)
            .max()
            .map(|index| index.saturating_add(GAP_LIMIT).min((1 << 31) - 1))
            .unwrap_or(GAP_LIMIT - 1)
    }

    pub fn add_address(&mut self, locator: BsvAddressLocator) -> Result<(), BsvHomeServiceError> {
        if self.records.iter().any(|record| record.locator == locator) {
            return Ok(());
        }
        self.records.push(PoolAddress {
            locator,
            address: self.address(locator)?,
            status: BsvHomeBsvAddressStatus::Unused,
            confirmed_history: false,
        });
        self.records.sort_by_key(|record| (record.locator.branch.index(), record.locator.index));
        Ok(())
    }

    pub fn refill(&mut self) -> Result<(), BsvHomeServiceError> {
        let mut unused = self
            .records
            .iter()
            .filter(|record| record.locator.branch == BsvAddressBranch::Receive && record.status == BsvHomeBsvAddressStatus::Unused)
            .count();
        let limit = self.allocation_limit(BsvAddressBranch::Receive);
        for index in 0..=limit {
            if unused >= RESERVE_COUNT {
                break;
            }
            let locator = BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index,
            };
            if !self.records.iter().any(|record| record.locator == locator) {
                self.add_address(locator)?;
                unused += 1;
            }
        }
        Ok(())
    }

    pub fn observe(&mut self, locator: BsvAddressLocator, has_history: bool, has_confirmed_history: bool) -> Result<(), BsvHomeServiceError> {
        if has_history {
            self.add_address(locator)?;
            if locator.branch == BsvAddressBranch::Receive
                && let Some(count) = self.receive_count
                && locator.index >= count
            {
                self.issue_receive_addresses(locator.index + 1)?;
            }
        }
        if let Some(record) = self.records.iter_mut().find(|record| record.locator == locator) {
            if has_history {
                record.status = BsvHomeBsvAddressStatus::Used;
            }
            record.confirmed_history = has_confirmed_history;
        }
        Ok(())
    }

    pub fn next_change(&self) -> Option<BsvAddressLocator> {
        let limit = self.allocation_limit(BsvAddressBranch::Change);
        (0..=limit)
            .map(|index| BsvAddressLocator {
                branch: BsvAddressBranch::Change,
                index,
            })
            .find(|locator| {
                !self
                    .records
                    .iter()
                    .any(|record| record.locator == *locator && record.status == BsvHomeBsvAddressStatus::Used)
                    && !self
                        .reservations
                        .iter()
                        .any(|reservation| !reservation.confirmed && reservation.spend.change.as_ref().is_some_and(|change| change.locator == *locator))
                    && !self
                        .funding_reservations
                        .iter()
                        .any(|reservation| !reservation.confirmed && reservation.spend.change.as_ref().is_some_and(|change| change.locator == *locator))
            })
    }

    pub fn snapshot(&self) -> BsvHomeBsvAddressSnapshot {
        let current = self.current_receive();
        let receive_count = self.issued_receive_count();
        let used_count = self
            .records
            .iter()
            .filter(|record| record.locator.branch == BsvAddressBranch::Receive && record.status == BsvHomeBsvAddressStatus::Used)
            .map(|record| record.locator.index + 1)
            .max()
            .unwrap_or(0);
        BsvHomeBsvAddressSnapshot {
            records: self
                .records
                .iter()
                .map(|record| BsvHomeBsvAddressRecord {
                    branch: record.locator.branch,
                    index: record.locator.index,
                    address: record.address.clone(),
                    status: record.status,
                    is_current: current.is_some_and(|current| current.locator == record.locator),
                    is_main: record.locator.branch == BsvAddressBranch::Receive && record.locator.index == 0,
                })
                .collect(),
            current_address: current.map(|record| record.address.clone()),
            receive_count,
            selected_index: current.map(|record| record.locator.index),
            can_add: can_add_receive_address(receive_count, used_count),
            receive_scanned: self.receive.next_index,
            change_scanned: self.change.next_index,
            discovery: self.discovery,
            allocation_paused: self
                .records
                .iter()
                .filter(|record| record.locator.branch == BsvAddressBranch::Receive && record.status == BsvHomeBsvAddressStatus::Unused)
                .count()
                < RESERVE_COUNT,
        }
    }

    pub fn issued_receive_count(&self) -> u32 {
        self.receive_count
            .unwrap_or_else(|| {
                self.records
                    .iter()
                    .filter(|record| record.locator.branch == BsvAddressBranch::Receive && record.status == BsvHomeBsvAddressStatus::Used)
                    .map(|record| record.locator.index + 1)
                    .chain(self.current_receive().map(|record| record.locator.index + 1))
                    .max()
                    .unwrap_or(0)
                    .max(RESERVE_COUNT as u32)
            })
            .max(self.cold_receive_count.unwrap_or(0))
    }

    pub fn issue_receive_addresses(&mut self, count: u32) -> Result<(), BsvHomeServiceError> {
        for index in 0..count {
            self.add_address(BsvAddressLocator {
                branch: BsvAddressBranch::Receive,
                index,
            })?;
        }
        self.receive_count = Some(count);
        Ok(())
    }

    fn current_receive(&self) -> Option<&PoolAddress> {
        self.records.iter().find(|record| {
            record.locator.branch == BsvAddressBranch::Receive
                && match self.selected_index {
                    Some(index) => record.locator.index == index,
                    None => record.status == BsvHomeBsvAddressStatus::Unused,
                }
        })
    }
}

pub(super) fn validate_reserved_checkpoint(reservation: &SpendReservation, saved: &BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeServiceError> {
    if saved.bsv_spend.as_ref() != Some(&reservation.spend)
        || saved.recipient_address != reservation.recipient
        || saved.original_value.as_ref().map(ToString::to_string).as_deref() != Some(reservation.original_value.as_str())
        || saved.use_max_amount != reservation.is_max
        || saved.value != reservation.value.into()
        || saved.network_fee != reservation.fee.into()
        || saved.gas_price.as_ref() != Some(&reservation.gas_price)
    {
        return Err(pool_error("BSV signed spend differs from reservation"));
    }
    Ok(())
}
