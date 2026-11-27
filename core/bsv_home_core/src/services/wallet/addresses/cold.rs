use primitives::{Account, BsvAddressBranch, BsvAddressLocator, Chain, Wallet, WalletId};

use super::state::{GAP_LIMIT, PoolState, pool_error};
use super::{BsvHomeBsvAddressPool, BsvHomeBsvAddressStatus, BsvHomeBsvAddressStoredState, BsvHomeBsvColdAddressState};
use crate::services::error::BsvHomeServiceError;

const MAX_COLD_RECEIVE_COUNT: u32 = 10_000 - GAP_LIMIT;

#[uniffi::export]
impl BsvHomeBsvAddressPool {
    pub async fn cold_addresses(&self, wallet_id: WalletId) -> Result<BsvHomeBsvColdAddressState, BsvHomeServiceError> {
        self.prepare(wallet_id.clone()).await?;
        let wallet = self.wallet(&wallet_id).await?;
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.required_state(&wallet).await?;
        if state.cold_receive_count.is_none() {
            let used_count = state
                .records
                .iter()
                .filter(|record| record.locator.branch == BsvAddressBranch::Receive && record.status == BsvHomeBsvAddressStatus::Used)
                .map(|record| record.locator.index + 1)
                .max()
                .unwrap_or(0);
            let count = used_count.max(10);
            if count > MAX_COLD_RECEIVE_COUNT {
                return Err(pool_error("BSV cold wallet address discovery exceeds its index limit"));
            }
            state.cold_receive_count = Some(count);
            state.cold_selected_index = Some(0);
            for index in 0..count {
                state.add_address(BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index,
                })?;
            }
            self.store_state(&wallet, Some(revision), &state).await?;
        }
        cold_snapshot(&state)
    }

    pub async fn cold_add_address(&self, wallet_id: WalletId) -> Result<BsvHomeBsvColdAddressState, BsvHomeServiceError> {
        self.cold_addresses(wallet_id.clone()).await?;
        let wallet = self.wallet(&wallet_id).await?;
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.required_state(&wallet).await?;
        let snapshot = cold_snapshot(&state)?;
        if !snapshot.can_add {
            return Err(pool_error("BSV receiving address gap limit reached"));
        }
        let index = state.cold_receive_count.ok_or_else(|| pool_error("Cold wallet addresses are not initialized"))?;
        state.add_address(BsvAddressLocator {
            branch: BsvAddressBranch::Receive,
            index,
        })?;
        state.cold_receive_count = Some(index + 1);
        state.cold_selected_index = Some(index);
        self.store_state(&wallet, Some(revision), &state).await?;
        cold_snapshot(&state)
    }

    pub async fn cold_select_address(&self, wallet_id: WalletId, index: u32) -> Result<BsvHomeBsvColdAddressState, BsvHomeServiceError> {
        self.cold_addresses(wallet_id.clone()).await?;
        let wallet = self.wallet(&wallet_id).await?;
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.required_state(&wallet).await?;
        if index >= state.cold_receive_count.ok_or_else(|| pool_error("Cold wallet addresses are not initialized"))? {
            return Err(pool_error("BSV receiving address is outside the issued range"));
        }
        state.cold_selected_index = Some(index);
        self.store_state(&wallet, Some(revision), &state).await?;
        cold_snapshot(&state)
    }
}

impl BsvHomeBsvAddressPool {
    pub(crate) async fn cold_account(&self, wallet_id: WalletId) -> Result<Account, BsvHomeServiceError> {
        self.prepare(wallet_id.clone()).await?;
        let wallet = self.wallet(&wallet_id).await?;
        let (_, state) = self.required_state(&wallet).await?;
        let account = wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("Wallet has no BSV account"))?;
        Ok(state.public_account(account))
    }

    pub(crate) fn initial_public_record(wallet: &Wallet, account: &Account) -> Result<Option<BsvHomeBsvAddressStoredState>, BsvHomeServiceError> {
        let stable = wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("Wallet has no BSV account"))?;
        if account.chain != Chain::BitcoinSv || stable.address != account.address {
            return Err(pool_error("BSV public profile does not match the wallet"));
        }
        let state = PoolState::from_account(wallet.id.clone(), account)?;
        Ok(Some(BsvHomeBsvAddressStoredState {
            revision: 0,
            value: serde_json::to_string(&state).map_err(pool_error)?,
        }))
    }

    pub(crate) async fn initialize_public_import(&self, wallet: &Wallet, account: &Account) -> Result<(), BsvHomeServiceError> {
        let _mutation = self.mutation.lock().await;
        if let Some((_, state)) = self.load(wallet).await? {
            if state.public_account(account) != *account {
                return Err(pool_error("Existing BSV address pool has a different account profile"));
            }
            return Ok(());
        }
        if let Some(record) = Self::initial_public_record(wallet, account)?
            && !self.store.save(wallet.id.clone(), None, record).await?
        {
            return Err(pool_error("BSV public address initialization changed concurrently"));
        }
        Ok(())
    }
}

fn cold_snapshot(state: &PoolState) -> Result<BsvHomeBsvColdAddressState, BsvHomeServiceError> {
    let count = state.cold_receive_count.ok_or_else(|| pool_error("Cold wallet addresses are not initialized"))?;
    let selected = state.cold_selected_index.ok_or_else(|| pool_error("Cold wallet address selection is missing"))?;
    let used_count = state
        .records
        .iter()
        .filter(|record| record.locator.branch == BsvAddressBranch::Receive && record.status == BsvHomeBsvAddressStatus::Used)
        .map(|record| record.locator.index + 1)
        .max()
        .unwrap_or(0);
    let addresses = state
        .snapshot()
        .records
        .into_iter()
        .filter(|record| record.branch == BsvAddressBranch::Receive && record.index < count)
        .map(|mut record| {
            record.is_current = record.index == selected;
            record
        })
        .collect();
    Ok(BsvHomeBsvColdAddressState {
        addresses,
        selected_index: Some(selected),
        can_add: count < (used_count + GAP_LIMIT).min(MAX_COLD_RECEIVE_COUNT),
    })
}
