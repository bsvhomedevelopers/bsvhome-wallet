use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use uuid::Uuid;

use bsv_home_bsv::multisig::{Funding, FundingReview, funding_spend_metadata};
use bsv_home_hash::sha2::sha256;
use primitives::{BsvSpendMetadata, Chain, TransactionIdRequest, TransactionState, Wallet, WalletId, WalletType};
use serde::{Deserialize, Serialize};

use super::ledger::{PayerState, SingleAddressState, outpoint};
use super::state::{SpendReservation, pool_error};
use super::{BsvHomeBsvAddressPool, BsvSigningLease};
use crate::services::bsv_multisig::{BsvHomeBsvMultisigFundingStatus, PayerFundingBinding, PayerFundingCheckpoint};
use crate::services::error::BsvHomeServiceError;

const MAX_CANCELLED_OPERATIONS: usize = 64;

#[derive(Clone, PartialEq, Eq, Hash)]
enum SigningPurpose {
    Transfer { reservation_id: String },
    Funding { operation_id: String },
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct SigningKey {
    payer: WalletId,
    purpose: SigningPurpose,
}
struct SigningClaim {
    nonce: Uuid,
    binding_hash: String,
}
static ACTIVE_PAYER_SIGNING: LazyLock<Mutex<HashMap<SigningKey, SigningClaim>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) struct PayerSigningLease {
    key: SigningKey,
    nonce: Uuid,
}
impl PayerSigningLease {
    pub(super) fn transfer(wallet: &Wallet, reservation: &SpendReservation) -> Result<Self, BsvHomeServiceError> {
        if !reservation.purpose.is_transfer() || reservation.checkpoint.is_some() || reservation.confirmed {
            return Err(pool_error("BSV signing lease requires an unsigned transfer"));
        }
        let key = SigningKey {
            payer: wallet.id.clone(),
            purpose: SigningPurpose::Transfer {
                reservation_id: reservation.id.clone(),
            },
        };
        let binding_hash = hex::encode(sha256(&serde_json::to_vec(reservation).map_err(pool_error)?));
        Self::acquire(key, binding_hash, "BSV payment is already being signed")
    }

    fn funding(wallet: &Wallet, binding: &PayerFundingBinding) -> Result<Self, BsvHomeServiceError> {
        if binding.payer_wallet_id != wallet.id {
            return Err(pool_error("BSV funding signing payer changed"));
        }
        let key = SigningKey {
            payer: wallet.id.clone(),
            purpose: SigningPurpose::Funding {
                operation_id: binding.operation_id.clone(),
            },
        };
        Self::acquire(key, cancelled(binding)?.binding_hash, "BSV funding is still being signed")
    }

    fn acquire(key: SigningKey, binding_hash: String, busy: &str) -> Result<Self, BsvHomeServiceError> {
        let mut active = ACTIVE_PAYER_SIGNING.lock().map_err(pool_error)?;
        if let Some(previous) = active.get(&key) {
            if previous.binding_hash != binding_hash {
                return Err(pool_error("BSV active signing operation changed its binding"));
            }
            return Err(pool_error(busy));
        }
        let nonce = Uuid::new_v4();
        active.insert(key.clone(), SigningClaim { nonce, binding_hash });
        Ok(Self { key, nonce })
    }
}
impl Drop for PayerSigningLease {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE_PAYER_SIGNING.lock()
            && active.get(&self.key).is_some_and(|claim| claim.nonce == self.nonce)
        {
            active.remove(&self.key);
        }
    }
}
fn funding_is_active(binding: &PayerFundingBinding) -> Result<bool, BsvHomeServiceError> {
    Ok(ACTIVE_PAYER_SIGNING.lock().map_err(pool_error)?.contains_key(&SigningKey {
        payer: binding.payer_wallet_id.clone(),
        purpose: SigningPurpose::Funding {
            operation_id: binding.operation_id.clone(),
        },
    }))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FundingSigned {
    raw: String,
    transaction_id: String,
    status: BsvHomeBsvMultisigFundingStatus,
    error_code: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FundingReservation {
    pub id: String,
    pub binding: PayerFundingBinding,
    pub spend: BsvSpendMetadata,
    signed: Option<FundingSigned>,
    pub confirmed: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CancelledFunding {
    operation_id: String,
    binding_hash: String,
}

impl FundingReservation {
    fn unsigned(binding: PayerFundingBinding, spend: BsvSpendMetadata) -> Self {
        Self {
            id: format!("funding:{}", binding.operation_id),
            binding,
            spend,
            signed: None,
            confirmed: false,
        }
    }

    fn adopted(checkpoint: PayerFundingCheckpoint, spend: BsvSpendMetadata) -> Self {
        let mut record = Self::unsigned(checkpoint.binding.clone(), spend);
        record.signed = Some(FundingSigned::from_checkpoint(&checkpoint));
        record
    }

    pub(super) fn checkpoint(&self) -> Option<PayerFundingCheckpoint> {
        self.signed.as_ref().map(|signed| PayerFundingCheckpoint {
            binding: self.binding.clone(),
            raw: signed.raw.clone(),
            transaction_id: signed.transaction_id.clone(),
            status: signed.status.clone(),
            error_code: signed.error_code.clone(),
        })
    }

    fn validate(&self, state: &PayerState, wallet: &Wallet) -> Result<(), BsvHomeServiceError> {
        if self.id != format!("funding:{}", self.binding.operation_id) || (self.confirmed && self.signed.is_none()) {
            return Err(pool_error("Invalid BSV funding reservation state"));
        }
        let verified = verify_binding(&self.binding, wallet, state, true)?;
        if !state.funding_change_recorded(&self.spend) {
            return Err(pool_error("BSV funding change is not recorded by the payer"));
        }
        if verified.spend != self.spend {
            return Err(pool_error("BSV funding reservation differs from its actual SDK plan"));
        }
        match &self.signed {
            Some(signed) => {
                let checkpoint = self.checkpoint().ok_or_else(|| pool_error("BSV signed funding missing"))?;
                checkpoint.validate().map_err(pool_error)?;
                validate_outcome(&signed.status, signed.error_code.as_deref())?;
            }
            None => validate_new_change(state, &verified)?,
        }
        Ok(())
    }
}

impl FundingSigned {
    fn from_checkpoint(checkpoint: &PayerFundingCheckpoint) -> Self {
        Self {
            raw: checkpoint.raw.clone(),
            transaction_id: checkpoint.transaction_id.clone(),
            status: checkpoint.status.clone(),
            error_code: checkpoint.error_code.clone(),
        }
    }
}

struct VerifiedFunding {
    funding: Funding,
    review: FundingReview,
    spend: BsvSpendMetadata,
}

fn verify_binding(binding: &PayerFundingBinding, wallet: &Wallet, state: &PayerState, require_records: bool) -> Result<VerifiedFunding, BsvHomeServiceError> {
    let (funding, review) = binding.validate().map_err(pool_error)?;
    let root = &wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?.address;
    if binding.payer_wallet_id != wallet.id || wallet.wallet_type == WalletType::View || funding.sender_address != *root {
        return Err(pool_error("BSV funding payer differs from its immutable binding"));
    }
    for owner in &funding.addresses {
        if state.funding_address(owner.locator, root)? != owner.address {
            return Err(pool_error("BSV funding source or change does not belong to payer"));
        }
    }
    let spend = funding_spend_metadata(&funding).map_err(pool_error)?;
    state.validate_funding_spend(&spend, root, require_records)?;
    Ok(VerifiedFunding { funding, review, spend })
}

fn validate_new_change(state: &PayerState, verified: &VerifiedFunding) -> Result<(), BsvHomeServiceError> {
    match state {
        PayerState::Hd(_) if verified.review.change > 0 && verified.funding.change.is_none() => Err(pool_error("New HD funding requires a real allocated change address")),
        PayerState::Hd(_) | PayerState::SingleAddress(_) => Ok(()),
    }
}

fn validate_outcome(status: &BsvHomeBsvMultisigFundingStatus, code: Option<&str>) -> Result<(), BsvHomeServiceError> {
    if code.is_some_and(|code| code.is_empty() || code.len() > 96 || !code.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')) {
        return Err(pool_error("Invalid BSV funding outcome code"));
    }
    match status {
        BsvHomeBsvMultisigFundingStatus::Pending | BsvHomeBsvMultisigFundingStatus::Broadcast | BsvHomeBsvMultisigFundingStatus::SyncFailed => Ok(()),
    }
}

fn accepted(status: &BsvHomeBsvMultisigFundingStatus) -> bool {
    match status {
        BsvHomeBsvMultisigFundingStatus::Pending => false,
        BsvHomeBsvMultisigFundingStatus::Broadcast | BsvHomeBsvMultisigFundingStatus::SyncFailed => true,
    }
}

fn merge_outcome(record: &mut FundingReservation, checkpoint: &PayerFundingCheckpoint) -> Result<bool, BsvHomeServiceError> {
    let current = record.checkpoint().ok_or_else(|| pool_error("BSV funding has not been durably signed"))?;
    if !current.same_signed_payment(checkpoint) {
        return Err(pool_error("BSV signed funding cannot be replaced"));
    }
    validate_outcome(&checkpoint.status, checkpoint.error_code.as_deref())?;
    if accepted(&current.status) && !accepted(&checkpoint.status) {
        return Ok(false);
    }
    if current.status == checkpoint.status && current.error_code == checkpoint.error_code {
        return Ok(false);
    }
    record.signed = Some(FundingSigned::from_checkpoint(checkpoint));
    Ok(true)
}

fn cancelled(binding: &PayerFundingBinding) -> Result<CancelledFunding, BsvHomeServiceError> {
    Ok(CancelledFunding {
        operation_id: binding.operation_id.clone(),
        binding_hash: hex::encode(sha256(&serde_json::to_vec(binding).map_err(pool_error)?)),
    })
}

fn check_cancelled(state: &PayerState, binding: &PayerFundingBinding) -> Result<(), BsvHomeServiceError> {
    if state.cancelled_funding().iter().any(|entry| entry.operation_id == binding.operation_id) {
        return Err(pool_error("BSV funding preparation was cancelled; review a new operation"));
    }
    Ok(())
}

pub(super) fn validate_funding_state(state: &PayerState, wallet: &Wallet) -> Result<(), BsvHomeServiceError> {
    let mut operations = HashSet::new();
    for record in state.funding() {
        if !operations.insert(&record.binding.operation_id) {
            return Err(pool_error("Duplicated BSV funding operation"));
        }
        record.validate(state, wallet)?;
    }
    let mut cancelled_operations = HashSet::new();
    if state.cancelled_funding().len() > MAX_CANCELLED_OPERATIONS {
        return Err(pool_error("Too many BSV funding cancellation markers"));
    }
    for marker in state.cancelled_funding() {
        if marker.operation_id.is_empty()
            || marker.binding_hash.len() != 64
            || !marker.binding_hash.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || !cancelled_operations.insert(&marker.operation_id)
        {
            return Err(pool_error("Invalid BSV funding cancellation marker"));
        }
        if state
            .funding()
            .iter()
            .any(|record| record.binding.operation_id == marker.operation_id && record.signed.is_none())
        {
            return Err(pool_error("Cancelled BSV funding still has an unsigned reservation"));
        }
    }
    let mut outpoints = HashSet::new();
    for input in state.unconfirmed_inputs() {
        if !outpoints.insert(outpoint(input)?) {
            return Err(pool_error("Conflicting BSV payer reservations"));
        }
    }
    Ok(())
}

fn funding_conflicts(state: &PayerState, spend: &BsvSpendMetadata) -> Result<(), BsvHomeServiceError> {
    let wanted = spend.inputs.iter().map(outpoint).collect::<Result<HashSet<_>, _>>()?;
    for input in state.unconfirmed_inputs() {
        if wanted.contains(&outpoint(input)?) {
            return Err(pool_error("BSV inputs are reserved by an unfinished payment"));
        }
    }
    Ok(())
}

impl BsvHomeBsvAddressPool {
    pub(crate) async fn reserve_funding(&self, wallet: &Wallet, binding: PayerFundingBinding) -> Result<BsvSigningLease, BsvHomeServiceError> {
        binding.validate().map_err(pool_error)?;
        let lease = PayerSigningLease::funding(wallet, &binding)?;
        self.reconcile_funding(wallet).await?;
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.funding_state(wallet).await?;
        check_cancelled(&state, &binding)?;
        let verified = verify_binding(&binding, wallet, &state, true)?;
        validate_new_change(&state, &verified)?;
        self.verify_payer_pending(wallet, &state).await?;
        self.check_revision(&wallet.id, revision).await?;
        self.check_payer_wallet(wallet).await?;
        if let Some(record) = state.funding().iter().find(|record| record.binding.operation_id == binding.operation_id) {
            if record.binding != binding || record.spend != verified.spend || record.signed.is_some() {
                return Err(pool_error("BSV funding operation already has a different or signed record"));
            }
            return Ok(BsvSigningLease::new(record.id.clone(), lease));
        }
        funding_conflicts(&state, &verified.spend)?;
        if let PayerState::Hd(hd) = &mut state
            && let Some(change) = &verified.spend.change
        {
            if Some(change.locator) != hd.next_change() {
                return Err(pool_error("BSV funding change allocation changed; review again"));
            }
            hd.add_address(change.locator)?;
        }
        let record = FundingReservation::unsigned(binding, verified.spend);
        let id = record.id.clone();
        state.funding_mut().push(record);
        self.check_payer_wallet(wallet).await?;
        self.save_payer_state(wallet, revision, &state).await?;
        Ok(BsvSigningLease::new(id, lease))
    }

    pub(crate) async fn save_funding_signed(&self, wallet: &Wallet, reservation_id: &str, checkpoint: PayerFundingCheckpoint) -> Result<(), BsvHomeServiceError> {
        checkpoint.validate().map_err(pool_error)?;
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.funding_state(wallet).await?;
        check_cancelled(&state, &checkpoint.binding)?;
        let verified = verify_binding(&checkpoint.binding, wallet, &state, true)?;
        let record = state
            .funding_mut()
            .iter_mut()
            .find(|record| record.id == reservation_id)
            .ok_or_else(|| pool_error("BSV funding reservation missing"))?;
        if record.binding != checkpoint.binding || record.spend != verified.spend {
            return Err(pool_error("BSV signed funding differs from its reservation"));
        }
        if let Some(previous) = record.checkpoint() {
            if !previous.same_signed_payment(&checkpoint) {
                return Err(pool_error("BSV signed funding cannot be replaced"));
            }
            return Ok(());
        }
        if checkpoint.status != BsvHomeBsvMultisigFundingStatus::Pending || checkpoint.error_code.is_some() {
            return Err(pool_error("BSV first signed funding cannot claim network acceptance"));
        }
        validate_outcome(&checkpoint.status, checkpoint.error_code.as_deref())?;
        record.signed = Some(FundingSigned::from_checkpoint(&checkpoint));
        state.mark_funding_change_used(&verified.spend)?;
        self.check_payer_wallet(wallet).await?;
        self.save_payer_state(wallet, revision, &state).await
    }

    pub(crate) async fn require_funding_checkpoint(&self, wallet: &Wallet, checkpoint: &PayerFundingCheckpoint) -> Result<(), BsvHomeServiceError> {
        self.reconcile_funding(wallet).await?;
        let (revision, state) = self.funding_state(wallet).await?;
        checkpoint.validate().map_err(pool_error)?;
        verify_binding(&checkpoint.binding, wallet, &state, true)?;
        let stored = state
            .funding()
            .iter()
            .find(|record| record.binding.operation_id == checkpoint.binding.operation_id)
            .and_then(FundingReservation::checkpoint)
            .ok_or_else(|| pool_error("BSV durable funding checkpoint missing"))?;
        if !stored.same_signed_payment(checkpoint) {
            return Err(pool_error("BSV funding checkpoint differs from payer authority"));
        }
        self.check_revision(&wallet.id, revision).await?;
        self.check_payer_wallet(wallet).await
    }

    pub(crate) async fn funding_checkpoints(&self, wallet: &Wallet) -> Result<Vec<PayerFundingCheckpoint>, BsvHomeServiceError> {
        self.reconcile_funding(wallet).await?;
        let Some((revision, state)) = self.payer_state(wallet).await? else {
            return Ok(Vec::new());
        };
        let result = state.funding().iter().filter_map(FundingReservation::checkpoint).collect();
        self.check_revision(&wallet.id, Some(revision)).await?;
        self.check_payer_wallet(wallet).await?;
        Ok(result)
    }

    pub(crate) async fn unsubmitted_funding(&self, wallet: &Wallet) -> Result<Vec<PayerFundingBinding>, BsvHomeServiceError> {
        self.reconcile_funding(wallet).await?;
        let Some((revision, state)) = self.payer_state(wallet).await? else {
            return Ok(Vec::new());
        };
        let result = state
            .funding()
            .iter()
            .filter(|record| record.signed.is_none())
            .map(|record| record.binding.clone())
            .collect();
        self.check_revision(&wallet.id, Some(revision)).await?;
        self.check_payer_wallet(wallet).await?;
        Ok(result)
    }

    pub(crate) async fn record_funding_outcome(&self, wallet: &Wallet, checkpoint: &PayerFundingCheckpoint) -> Result<PayerFundingCheckpoint, BsvHomeServiceError> {
        checkpoint.validate().map_err(pool_error)?;
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.funding_state(wallet).await?;
        verify_binding(&checkpoint.binding, wallet, &state, true)?;
        let record = state
            .funding_mut()
            .iter_mut()
            .find(|record| record.binding.operation_id == checkpoint.binding.operation_id)
            .ok_or_else(|| pool_error("BSV funding operation missing"))?;
        let changed = merge_outcome(record, checkpoint)?;
        let result = record.checkpoint().ok_or_else(|| pool_error("BSV signed funding checkpoint missing"))?;
        if changed {
            self.check_payer_wallet(wallet).await?;
            self.save_payer_state(wallet, revision, &state).await?;
        }
        Ok(result)
    }

    pub(crate) async fn release_funding_unsigned(&self, wallet: &Wallet, binding: &PayerFundingBinding) -> Result<(), BsvHomeServiceError> {
        binding.validate().map_err(pool_error)?;
        let marker = cancelled(binding)?;
        if funding_is_active(binding)? {
            return Err(pool_error("BSV funding is still being signed"));
        }
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.funding_state(wallet).await?;
        verify_binding(binding, wallet, &state, false)?;
        self.check_payer_cleanup(wallet).await?;
        if let Some(record) = state.funding().iter().find(|record| record.binding.operation_id == binding.operation_id) {
            if record.binding != *binding {
                return Err(pool_error("Cannot release a different BSV funding operation"));
            }
            if record.signed.is_some() {
                return Ok(());
            }
            if funding_is_active(binding)? {
                return Err(pool_error("BSV funding is still being signed"));
            }
        }
        if let Some(previous) = state.cancelled_funding().iter().find(|entry| entry.operation_id == binding.operation_id) {
            if previous != &marker {
                return Err(pool_error("BSV cancelled operation binding changed"));
            }
            return Ok(());
        }
        state.funding_mut().retain(|record| record.binding.operation_id != binding.operation_id);
        let markers = state.cancelled_funding_mut();
        if markers.len() == MAX_CANCELLED_OPERATIONS {
            markers.remove(0);
        }
        markers.push(marker);
        self.save_payer_state(wallet, revision, &state).await
    }

    pub(super) async fn funding_state(&self, wallet: &Wallet) -> Result<(Option<u64>, PayerState), BsvHomeServiceError> {
        match self.payer_state(wallet).await? {
            Some((revision, state)) => Ok((Some(revision), state)),
            None if wallet.wallet_type == WalletType::PrivateKey => Ok((None, PayerState::SingleAddress(SingleAddressState::new(wallet)?))),
            None => Err(pool_error("BSV HD payer ledger must be initialized before funding recovery")),
        }
    }

    pub(crate) async fn reconcile_funding(&self, wallet: &Wallet) -> Result<(), BsvHomeServiceError> {
        let archives = self.validated_funding_archives().await?;
        let root = &wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?.address;
        let _mutation = self.mutation.lock().await;
        let original = self.payer_state(wallet).await?;
        let mut candidates = Vec::new();
        for checkpoint in archives {
            let (funding, _) = checkpoint.validate().map_err(pool_error)?;
            let mut belongs = checkpoint.binding.payer_wallet_id == wallet.id || funding.sender_address == *root || funding.utxos.iter().any(|source| source.address == *root);
            if let Some((_, PayerState::Hd(state))) = &original {
                for owner in &funding.addresses {
                    belongs |= state.address(owner.locator)? == owner.address;
                }
            }
            if belongs {
                candidates.push(checkpoint);
            }
        }
        if candidates.is_empty() {
            return Ok(());
        }
        let (revision, mut state) = match original {
            Some((revision, state)) => (Some(revision), state),
            None if wallet.wallet_type == WalletType::PrivateKey => (None, PayerState::SingleAddress(SingleAddressState::new(wallet)?)),
            None => return Err(pool_error("BSV HD payer ledger must be initialized before legacy funding adoption")),
        };
        let mut changed = false;
        for checkpoint in candidates {
            changed |= adopt_into(&mut state, wallet, checkpoint)?;
        }
        validate_funding_state(&state, wallet)?;
        if changed {
            self.check_payer_wallet(wallet).await?;
            self.save_payer_state(wallet, revision, &state).await?;
        }
        Ok(())
    }

    pub(super) async fn refresh_funding_confirmations(&self, records: &mut [FundingReservation]) -> Result<bool, BsvHomeServiceError> {
        let mut changed = false;
        for record in records {
            if let Some(signed) = &mut record.signed {
                let transaction = self
                    .gateway
                    .transaction_by_hash(TransactionIdRequest::new(Chain::BitcoinSv, signed.transaction_id.clone(), None))
                    .await?;
                let confirmed = transaction.is_some_and(|transaction| transaction.hash() == signed.transaction_id && transaction.state == TransactionState::Confirmed);
                if record.confirmed != confirmed {
                    record.confirmed = confirmed;
                    changed = true;
                }
                if confirmed && !accepted(&signed.status) {
                    signed.status = BsvHomeBsvMultisigFundingStatus::Broadcast;
                    signed.error_code = None;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }
}

fn adopt_into(state: &mut PayerState, wallet: &Wallet, checkpoint: PayerFundingCheckpoint) -> Result<bool, BsvHomeServiceError> {
    let verified = verify_binding(&checkpoint.binding, wallet, state, false)?;
    checkpoint.validate().map_err(pool_error)?;
    validate_outcome(&checkpoint.status, checkpoint.error_code.as_deref())?;
    if let Some(index) = state.funding().iter().position(|record| record.binding.operation_id == checkpoint.binding.operation_id) {
        let existing = &state.funding()[index];
        if existing.binding != checkpoint.binding || existing.spend != verified.spend {
            return Err(pool_error("BSV recovered funding operation changed immutable data"));
        }
        if let Some(previous) = existing.checkpoint() {
            if !previous.same_signed_payment(&checkpoint) {
                return Err(pool_error("Conflicting signed BSV funding recovery"));
            }
            return Ok(false);
        }
        state.materialize_funding_owners(&verified.spend)?;
        state.funding_mut()[index].signed = Some(FundingSigned::from_checkpoint(&checkpoint));
        state.mark_funding_change_used(&verified.spend)?;
        return Ok(true);
    }
    funding_conflicts(state, &verified.spend)?;
    state.materialize_funding_owners(&verified.spend)?;
    state.mark_funding_change_used(&verified.spend)?;
    state.funding_mut().push(FundingReservation::adopted(checkpoint, verified.spend));
    Ok(true)
}
