use std::sync::Arc;

use primitives::{Chain, Wallet};

use super::model::{BsvHomeRecipientError, BsvHomeRecipientNext, BsvHomeRecipientScan, BsvHomeRecipientType};
use super::rules::{next_step, scan_route};
use crate::BsvHomeCoreError;
use crate::models::payment::BsvHomePayment;
use crate::payment::{BsvHomePaymentDestination, BsvHomePaymentRecipient, BsvHomePaymentService, BsvHomePaymentWalletAsset};
use crate::services::name::{BsvHomeNameRecordState, BsvHomeNameService};
use crate::services::transfer::model::BsvHomeRecipient;
use crate::services::wallet_session::BsvHomeWalletSessionService;

#[derive(uniffi::Object)]
pub struct BsvHomeRecipientService {
    names: Arc<BsvHomeNameService>,
    payments: Arc<BsvHomePaymentService>,
    session: Arc<BsvHomeWalletSessionService>,
}

#[uniffi::export]
impl BsvHomeRecipientService {
    #[uniffi::constructor]
    pub fn new(names: Arc<BsvHomeNameService>, payments: Arc<BsvHomePaymentService>, session: Arc<BsvHomeWalletSessionService>) -> Self {
        Self { names, payments, session }
    }

    pub fn recipient(
        &self,
        chain: Chain,
        input: String,
        state: BsvHomeNameRecordState,
        memo: Option<String>,
        references: Vec<String>,
    ) -> Result<BsvHomeRecipient, BsvHomeRecipientError> {
        self.names.recipient(chain, input, state, memo, references)
    }

    pub fn recipient_wallets(&self, wallets: Vec<Wallet>) -> Vec<Wallet> {
        let current = self.session.get_current_wallet_id().unwrap_or_default();
        wallets.into_iter().filter(|wallet| Some(&wallet.id) != current.as_ref()).collect()
    }

    pub fn scan(&self, url: String, recipient_type: BsvHomeRecipientType) -> Result<BsvHomeRecipientScan, BsvHomeRecipientError> {
        let asset = recipient_type.asset();
        let destination = self
            .scan_destination(
                url,
                BsvHomePaymentWalletAsset {
                    asset_id: asset.id.clone(),
                    decimals: asset.decimals,
                },
            )
            .map_err(|_| BsvHomeRecipientError::InvalidAddress)?;
        scan_route(destination, &recipient_type, |transfer| self.payments.transfer_data(transfer, asset))
    }

    pub fn next(&self, recipient_type: BsvHomeRecipientType, payment: BsvHomePaymentRecipient) -> BsvHomeRecipientNext {
        next_step(recipient_type, payment)
    }
}

impl BsvHomeRecipientService {
    fn scan_destination(&self, url: String, asset: BsvHomePaymentWalletAsset) -> Result<BsvHomePaymentDestination, BsvHomeCoreError> {
        Ok(match self.payments.decode_url(url)? {
            BsvHomePayment::Request { request } => self.payments.transfer_destination(request, asset),
            BsvHomePayment::Link { link: _ } => BsvHomePaymentDestination::Unsupported,
        })
    }
}
