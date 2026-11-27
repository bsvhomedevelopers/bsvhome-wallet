use std::sync::Arc;

use primitives::{Currency, TransactionExtended};

use super::model::BsvHomeTransactionDetailRows;
use super::rules;
use crate::services::explorer::BsvHomeExplorerService;
use crate::services::preferences::BsvHomePreferencesService;

#[derive(uniffi::Object)]
pub struct BsvHomeTransactionDetailsService {
    explorer: Arc<BsvHomeExplorerService>,
    preferences: Arc<BsvHomePreferencesService>,
}

#[uniffi::export]
impl BsvHomeTransactionDetailsService {
    #[uniffi::constructor]
    pub fn new(explorer: Arc<BsvHomeExplorerService>, preferences: Arc<BsvHomePreferencesService>) -> Self {
        Self { explorer, preferences }
    }

    pub fn get_currency(&self) -> Currency {
        self.preferences.get_currency()
    }

    pub fn detail_rows(&self, transaction: TransactionExtended) -> BsvHomeTransactionDetailRows {
        let chain = transaction.transaction.asset_id.chain;
        let participant = rules::participant(&transaction, |address| self.explorer.get_address_url(chain, address.to_string()));
        let explorer = self.explorer.get_transaction_link(
            chain,
            transaction.transaction.hash().to_string(),
            transaction.transaction.swap_metadata().and_then(|metadata| metadata.provider),
            Some(transaction.transaction.to.clone()),
            transaction.transaction.memo.clone(),
        );
        rules::detail_rows(&transaction, participant, explorer)
    }
}
