use std::collections::HashSet;
use std::str::FromStr;

use bitcoin::Txid;

use super::funding::{CancelledFunding, FundingReservation, PayerSigningLease, validate_funding_state};
use bsv_home_bsv::BsvAddress;
use primitives::{
    Address, BsvAddressBranch, BsvAddressLocator, BsvSpendInput, BsvSpendMetadata, Chain, GasPriceType, SignerInput, TransactionLoadMetadata, Wallet, WalletId, WalletType,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{from_str, to_string};

use super::single::{spend_from_single_input, validate_single_checkpoint};
use super::state::{PoolState, SpendReservation, pool_error, validate_reserved_checkpoint};
use super::{BsvHomeBsvAddressPool, BsvHomeBsvAddressStoredState, BsvSigningLease, has_hd_account};
use crate::services::error::BsvHomeServiceError;

pub(crate) use crate::services::bsv_multisig::PayerFundingBinding;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum PayerReservationPurpose {
    #[default]
    Transfer,
    Funding {
        binding: Box<PayerFundingBinding>,
    },
}

impl PayerReservationPurpose {
    pub fn is_transfer(&self) -> bool {
        match self {
            Self::Transfer => true,
            Self::Funding { .. } => false,
        }
    }
}

pub(crate) struct PayerReservationRequest {
    pub purpose: PayerReservationPurpose,
    pub recipient: String,
    pub original_value: String,
    pub is_max: bool,
    pub value: u64,
    pub fee: u64,
    pub gas_price: GasPriceType,
    pub spend: BsvSpendMetadata,
}

impl PayerReservationRequest {
    pub(super) fn transfer(wallet: &Wallet, input: &SignerInput, original_value: String, is_max: bool) -> Result<Self, BsvHomeServiceError> {
        let account = wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?;
        if input.sender_address != account.address {
            return Err(pool_error("BSV spend wallet identity mismatch"));
        }
        let spend = if has_hd_account(wallet) {
            let TransactionLoadMetadata::BitcoinSv { spend } = &input.metadata else {
                return Err(pool_error("BSV derived spend metadata missing"));
            };
            spend.clone()
        } else if wallet.wallet_type == WalletType::PrivateKey {
            spend_from_single_input(wallet, input)?
        } else {
            return Err(pool_error("BSV single-address reservation requires a private-key payer"));
        };
        Ok(Self {
            purpose: PayerReservationPurpose::Transfer,
            recipient: input.destination_address.clone(),
            original_value,
            is_max,
            value: input.value_as_u64().map_err(pool_error)?,
            fee: input.fee.fee().map_err(pool_error)?,
            gas_price: input.gas_price.clone(),
            spend,
        })
    }

    fn matches(&self, reservation: &SpendReservation) -> bool {
        !reservation.confirmed
            && reservation.checkpoint.is_none()
            && reservation.purpose == self.purpose
            && reservation.recipient == self.recipient
            && reservation.original_value == self.original_value
            && reservation.is_max == self.is_max
            && reservation.spend == self.spend
            && reservation.value == self.value
            && reservation.fee == self.fee
            && reservation.gas_price == self.gas_price
    }

    fn into_reservation(self, id: String) -> SpendReservation {
        SpendReservation {
            id,
            purpose: self.purpose,
            recipient: self.recipient,
            original_value: self.original_value,
            is_max: self.is_max,
            value: self.value,
            fee: self.fee,
            gas_price: self.gas_price,
            spend: self.spend,
            checkpoint: None,
            confirmed: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SingleAddressState {
    kind: String,
    version: u32,
    wallet_id: WalletId,
    root_address: String,
    reservations: Vec<SpendReservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    funding_reservations: Vec<FundingReservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cancelled_funding: Vec<CancelledFunding>,
}

impl SingleAddressState {
    pub(super) fn new(wallet: &Wallet) -> Result<Self, BsvHomeServiceError> {
        let state = Self {
            kind: "singleAddress".into(),
            version: 1,
            wallet_id: wallet.id.clone(),
            root_address: wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?.address.clone(),
            reservations: Vec::new(),
            funding_reservations: Vec::new(),
            cancelled_funding: Vec::new(),
        };
        state.validate(wallet)?;
        Ok(state)
    }

    fn validate(&self, wallet: &Wallet) -> Result<(), BsvHomeServiceError> {
        let accounts = wallet.accounts.iter().filter(|account| account.chain == Chain::BitcoinSv).collect::<Vec<_>>();
        if self.kind != "singleAddress"
            || self.version != 1
            || self.wallet_id != wallet.id
            || wallet.wallet_type != WalletType::PrivateKey
            || wallet.id.wallet_type() != WalletType::PrivateKey
            || wallet.id.chain() != Some(Chain::BitcoinSv)
            || accounts.len() != 1
            || accounts[0].address != self.root_address
            || wallet.id.address() != self.root_address
        {
            return Err(pool_error("BSV single-address ledger does not match its private-key wallet"));
        }
        BsvAddress::from_str(&self.root_address).map_err(pool_error)?;
        let mut ids = HashSet::new();
        let mut outpoints = HashSet::new();
        for reservation in &self.reservations {
            if !reservation.purpose.is_transfer() {
                return Err(pool_error("BSV funding cannot use an ordinary transfer reservation"));
            }
            if !ids.insert(&reservation.id) || (reservation.confirmed && reservation.checkpoint.is_none()) {
                return Err(pool_error("Invalid BSV single-address reservation state"));
            }
            self.validate_spend(&reservation.spend)?;
            if let Some(saved) = &reservation.checkpoint {
                if !reservation.purpose.is_transfer() {
                    return Err(pool_error("BSV funding requires its own typed signed checkpoint"));
                }
                validate_single_checkpoint(wallet, saved)?;
                validate_reserved_checkpoint(reservation, saved)?;
            }
            if !reservation.confirmed {
                for input in &reservation.spend.inputs {
                    if !outpoints.insert(outpoint(input)?) {
                        return Err(pool_error("Conflicting BSV single-address reservations"));
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_spend(&self, spend: &BsvSpendMetadata) -> Result<(), BsvHomeServiceError> {
        if spend.inputs.is_empty() || spend.change.is_some() {
            return Err(pool_error("BSV single-address spend must retain real root change semantics"));
        }
        let script = BsvAddress::from_str(&self.root_address).map_err(pool_error)?.script_pubkey();
        let mut outpoints = HashSet::new();
        for input in &spend.inputs {
            if input.address != self.root_address
                || input.locator
                    != (BsvAddressLocator {
                        branch: BsvAddressBranch::Receive,
                        index: 0,
                    })
                || input.value <= 1
                || hex::decode(&input.script_pubkey).map_err(pool_error)?.as_slice() != script.as_bytes()
                || !outpoints.insert(outpoint(input)?)
            {
                return Err(pool_error("Invalid BSV single-address input ownership"));
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct OwnershipHeader {
    #[serde(default, deserialize_with = "ownership_kind")]
    kind: Option<String>,
}
fn ownership_kind<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

pub(super) enum PayerState {
    Hd(PoolState),
    SingleAddress(SingleAddressState),
}

impl PayerState {
    pub(super) fn decode(value: &str, wallet: &Wallet) -> Result<Self, BsvHomeServiceError> {
        let header: OwnershipHeader = from_str(value).map_err(pool_error)?;
        let state = if let Some(kind) = header.kind {
            if kind != "singleAddress" {
                return Err(pool_error("Unsupported BSV payer ledger kind"));
            }
            let state: SingleAddressState = from_str(value).map_err(pool_error)?;
            state.validate(wallet)?;
            Self::SingleAddress(state)
        } else {
            if !has_hd_account(wallet) {
                return Err(pool_error("BSV single-address ledger must declare its ownership kind"));
            }
            let state: PoolState = from_str(value).map_err(pool_error)?;
            let account = wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?;
            state.validate(&wallet.id, &account.address)?;
            if account.extended_public_key.as_ref().is_some_and(|key| key.starts_with("xpub")) && state.public_account(account) != *account {
                return Err(pool_error("BSV public account does not match the address pool"));
            }
            Self::Hd(state)
        };
        validate_funding_state(&state, wallet)?;
        Ok(state)
    }

    fn encode(&self) -> Result<String, BsvHomeServiceError> {
        match self {
            Self::Hd(state) => to_string(state).map_err(pool_error),
            Self::SingleAddress(state) => to_string(state).map_err(pool_error),
        }
    }

    pub(super) fn reservations(&self) -> &[SpendReservation] {
        match self {
            Self::Hd(state) => &state.reservations,
            Self::SingleAddress(state) => &state.reservations,
        }
    }

    pub(super) fn reservations_mut(&mut self) -> &mut Vec<SpendReservation> {
        match self {
            Self::Hd(state) => &mut state.reservations,
            Self::SingleAddress(state) => &mut state.reservations,
        }
    }

    pub(super) fn funding(&self) -> &[FundingReservation] {
        match self {
            Self::Hd(state) => &state.funding_reservations,
            Self::SingleAddress(state) => &state.funding_reservations,
        }
    }
    pub(super) fn funding_mut(&mut self) -> &mut Vec<FundingReservation> {
        match self {
            Self::Hd(state) => &mut state.funding_reservations,
            Self::SingleAddress(state) => &mut state.funding_reservations,
        }
    }
    pub(super) fn cancelled_funding(&self) -> &[CancelledFunding] {
        match self {
            Self::Hd(state) => &state.cancelled_funding,
            Self::SingleAddress(state) => &state.cancelled_funding,
        }
    }
    pub(super) fn cancelled_funding_mut(&mut self) -> &mut Vec<CancelledFunding> {
        match self {
            Self::Hd(state) => &mut state.cancelled_funding,
            Self::SingleAddress(state) => &mut state.cancelled_funding,
        }
    }
    pub(super) fn unconfirmed_inputs(&self) -> impl Iterator<Item = &BsvSpendInput> {
        self.reservations()
            .iter()
            .filter(|record| !record.confirmed)
            .flat_map(|record| &record.spend.inputs)
            .chain(self.funding().iter().filter(|record| !record.confirmed).flat_map(|record| &record.spend.inputs))
    }
    pub(super) fn funding_address(&self, locator: BsvAddressLocator, root: &str) -> Result<String, BsvHomeServiceError> {
        locator.validate().map_err(pool_error)?;
        match self {
            Self::Hd(state) => state.address(locator),
            Self::SingleAddress(_)
                if locator
                    == (BsvAddressLocator {
                        branch: BsvAddressBranch::Receive,
                        index: 0,
                    }) =>
            {
                Ok(root.to_string())
            }
            Self::SingleAddress(_) => Err(pool_error("BSV single-address funding cannot claim a derived owner")),
        }
    }
    pub(super) fn validate_funding_spend(&self, spend: &BsvSpendMetadata, root: &str, require_records: bool) -> Result<(), BsvHomeServiceError> {
        if spend.inputs.is_empty() {
            return Err(pool_error("BSV funding inputs missing"));
        }
        let mut outpoints = HashSet::new();
        for input in &spend.inputs {
            let script = BsvAddress::from_str(&input.address).map_err(pool_error)?.script_pubkey();
            if input.value <= 1
                || self.funding_address(input.locator, root)? != input.address
                || hex::decode(&input.script_pubkey).map_err(pool_error)?.as_slice() != script.as_bytes()
                || !outpoints.insert(outpoint(input)?)
            {
                return Err(pool_error("Invalid BSV funding input ownership"));
            }
        }
        match self {
            Self::Hd(state) => {
                if require_records
                    && spend
                        .inputs
                        .iter()
                        .any(|input| !state.records.iter().any(|record| record.locator == input.locator && record.address == input.address))
                {
                    return Err(pool_error("BSV funding input is outside recorded payer owners"));
                }
                if let Some(change) = &spend.change {
                    let script = BsvAddress::from_str(&change.address).map_err(pool_error)?.script_pubkey();
                    if change.locator.branch != BsvAddressBranch::Change
                        || self.funding_address(change.locator, root)? != change.address
                        || hex::decode(&change.script_pubkey).map_err(pool_error)?.as_slice() != script.as_bytes()
                    {
                        return Err(pool_error("Invalid BSV funding change ownership"));
                    }
                }
                Ok(())
            }
            Self::SingleAddress(state) => state.validate_spend(spend),
        }
    }
    pub(super) fn materialize_funding_owners(&mut self, spend: &BsvSpendMetadata) -> Result<(), BsvHomeServiceError> {
        match self {
            Self::Hd(state) => {
                for input in &spend.inputs {
                    state.add_address(input.locator)?;
                    let record = state
                        .records
                        .iter_mut()
                        .find(|record| record.locator == input.locator)
                        .ok_or_else(|| pool_error("BSV recovered funding owner missing"))?;
                    record.status = super::BsvHomeBsvAddressStatus::Used;
                }
                if let Some(change) = &spend.change {
                    state.add_address(change.locator)?;
                }
                Ok(())
            }
            Self::SingleAddress(state) => state.validate_spend(spend),
        }
    }
    pub(super) fn mark_funding_change_used(&mut self, spend: &BsvSpendMetadata) -> Result<(), BsvHomeServiceError> {
        if let Self::Hd(state) = self
            && let Some(change) = &spend.change
        {
            let record = state
                .records
                .iter_mut()
                .find(|record| record.locator == change.locator && record.address == change.address)
                .ok_or_else(|| pool_error("BSV signed funding change record missing"))?;
            record.status = super::BsvHomeBsvAddressStatus::Used;
        }
        Ok(())
    }

    pub(super) fn funding_change_recorded(&self, spend: &BsvSpendMetadata) -> bool {
        match self {
            Self::Hd(state) => spend
                .change
                .as_ref()
                .is_none_or(|change| state.records.iter().any(|record| record.locator == change.locator && record.address == change.address)),
            Self::SingleAddress(_) => spend.change.is_none(),
        }
    }

    pub(super) fn validate_spend(&self, spend: &BsvSpendMetadata) -> Result<(), BsvHomeServiceError> {
        if spend.inputs.is_empty() {
            return Err(pool_error("BSV reservation inputs are missing"));
        }
        let mut outpoints = HashSet::new();
        for input in &spend.inputs {
            input.locator.validate().map_err(pool_error)?;
            let script = BsvAddress::from_str(&input.address).map_err(pool_error)?.script_pubkey();
            if input.value <= 1 || hex::decode(&input.script_pubkey).map_err(pool_error)?.as_slice() != script.as_bytes() || !outpoints.insert(outpoint(input)?) {
                return Err(pool_error("Invalid or duplicated BSV reservation input"));
            }
        }
        if let Some(change) = &spend.change {
            change.locator.validate().map_err(pool_error)?;
            let script = BsvAddress::from_str(&change.address).map_err(pool_error)?.script_pubkey();
            if hex::decode(&change.script_pubkey).map_err(pool_error)?.as_slice() != script.as_bytes() {
                return Err(pool_error("BSV change script differs from its owner"));
            }
        }
        match self {
            Self::Hd(state) => state.validate_spend(spend),
            Self::SingleAddress(state) => state.validate_spend(spend),
        }
    }
}

impl BsvHomeBsvAddressPool {
    pub(crate) async fn import_record(
        &self,
        wallet: &Wallet,
        create_initial: impl FnOnce() -> Result<Option<BsvHomeBsvAddressStoredState>, BsvHomeServiceError> + Send,
    ) -> Result<Option<BsvHomeBsvAddressStoredState>, BsvHomeServiceError> {
        if wallet.account(Chain::BitcoinSv).is_none() || self.payer_state(wallet).await?.is_some() {
            return Ok(None);
        }
        create_initial()
    }

    pub(super) async fn payer_state(&self, wallet: &Wallet) -> Result<Option<(u64, PayerState)>, BsvHomeServiceError> {
        self.store
            .get(wallet.id.clone())
            .await?
            .map(|record| Ok((record.revision, PayerState::decode(&record.value, wallet)?)))
            .transpose()
    }

    pub(super) async fn save_payer_state(&self, wallet: &Wallet, revision: Option<u64>, state: &PayerState) -> Result<(), BsvHomeServiceError> {
        let revision_next = revision
            .map(|revision| revision.checked_add(1).ok_or_else(|| pool_error("BSV payer revision exhausted")))
            .transpose()?
            .unwrap_or(0);
        if !self
            .store
            .save(
                wallet.id.clone(),
                revision,
                BsvHomeBsvAddressStoredState {
                    revision: revision_next,
                    value: state.encode()?,
                },
            )
            .await?
        {
            return Err(pool_error("BSV payer ledger changed concurrently; retry"));
        }
        Ok(())
    }

    pub(crate) async fn reserve_payer_spend(&self, wallet: &Wallet, request: PayerReservationRequest) -> Result<BsvSigningLease, BsvHomeServiceError> {
        if !request.purpose.is_transfer() {
            return Err(pool_error("Use the typed BSV funding reservation owner"));
        }
        self.reconcile_funding(wallet).await?;
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = match self.payer_state(wallet).await? {
            Some((revision, state)) => (Some(revision), state),
            None if wallet.wallet_type == WalletType::PrivateKey => (None, PayerState::SingleAddress(SingleAddressState::new(wallet)?)),
            None => return Err(pool_error("BSV HD payer ledger is not initialized")),
        };
        state.validate_spend(&request.spend)?;
        self.verify_payer_pending(wallet, &state).await?;
        self.check_revision(&wallet.id, revision).await?;
        self.check_payer_wallet(wallet).await?;
        if let Some(reservation) = state.reservations().iter().find(|reservation| request.matches(reservation)) {
            let lease = PayerSigningLease::transfer(wallet, reservation)?;
            return Ok(BsvSigningLease::new(reservation.id.clone(), lease));
        }
        let outpoints = request.spend.inputs.iter().map(outpoint).collect::<Result<HashSet<_>, _>>()?;
        for input in state.unconfirmed_inputs() {
            if outpoints.contains(&outpoint(input)?) {
                return Err(pool_error("BSV inputs are reserved by an unfinished payment"));
            }
        }
        if let PayerState::Hd(state) = &mut state
            && let Some(change) = &request.spend.change
        {
            if Some(change.locator) != state.next_change() {
                return Err(pool_error("BSV change allocation changed; reload confirmation"));
            }
            state.add_address(change.locator)?;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let reservation = request.into_reservation(id.clone());
        let lease = PayerSigningLease::transfer(wallet, &reservation)?;
        state.reservations_mut().push(reservation);
        self.check_payer_wallet(wallet).await?;
        self.save_payer_state(wallet, revision, &state).await?;
        Ok(BsvSigningLease::new(id, lease))
    }

    pub(super) async fn release_payer_unsigned(&self, wallet: &Wallet, id: &str) -> Result<(), BsvHomeServiceError> {
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.payer_state(wallet).await?.ok_or_else(|| pool_error("BSV payer ledger is missing"))?;
        state.reservations_mut().retain(|reservation| reservation.id != id || reservation.checkpoint.is_some());
        self.save_payer_state(wallet, Some(revision), &state).await
    }
}

pub(super) fn outpoint(input: &BsvSpendInput) -> Result<(Txid, u32), BsvHomeServiceError> {
    Ok((Txid::from_str(&input.transaction_id).map_err(pool_error)?, input.vout))
}
