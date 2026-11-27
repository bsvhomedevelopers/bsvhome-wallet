use primitives::{BsvAddressBranch, WalletId};

use super::state::pool_error;
use super::{BsvHomeBsvAddressPool, BsvHomeBsvAddressSnapshot, has_hd_account, single_address_snapshot};
use crate::services::error::BsvHomeServiceError;

#[uniffi::export]
impl BsvHomeBsvAddressPool {
    pub async fn add_address(&self, wallet_id: WalletId) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.prepare(wallet_id.clone()).await?;
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) {
            return Err(pool_error("This BSV wallet has only one receiving address"));
        }
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.required_state(&wallet).await?;
        let snapshot = state.snapshot();
        if !snapshot.can_add {
            return Err(pool_error("BSV receiving address gap limit reached"));
        }
        let index = snapshot.receive_count;
        state.issue_receive_addresses(index + 1)?;
        state.selected_index = Some(index);
        self.store_state(&wallet, Some(revision), &state).await?;
        Ok(state.snapshot())
    }

    pub async fn select_address(&self, wallet_id: WalletId, index: u32) -> Result<BsvHomeBsvAddressSnapshot, BsvHomeServiceError> {
        self.prepare(wallet_id.clone()).await?;
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) {
            return if index == 0 {
                single_address_snapshot(&wallet)
            } else {
                Err(pool_error("BSV receiving address is outside the issued range"))
            };
        }
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.required_state(&wallet).await?;
        let count = state.issued_receive_count();
        if index >= count {
            return Err(pool_error("BSV receiving address is outside the issued range"));
        }
        state.issue_receive_addresses(count)?;
        state.selected_index = Some(index);
        self.store_state(&wallet, Some(revision), &state).await?;
        Ok(state.snapshot())
    }
}

impl BsvHomeBsvAddressPool {
    pub(crate) async fn receipt_addresses(&self, wallet_id: WalletId) -> Result<Option<Vec<String>>, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if !has_hd_account(&wallet) {
            return Ok(single_address_snapshot(&wallet)?.current_address.map(|address| vec![address]));
        }
        let Some((_, state)) = self.load(&wallet).await? else {
            return Ok(None);
        };
        let count = state.issued_receive_count();
        Ok(Some(
            state
                .records
                .iter()
                .filter(|record| record.locator.branch == BsvAddressBranch::Receive && record.locator.index < count)
                .map(|record| record.address.clone())
                .collect(),
        ))
    }
}
