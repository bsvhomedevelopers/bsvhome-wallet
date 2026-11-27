mod action;
#[cfg(test)]
mod action_tests;
#[cfg(test)]
mod import_reservation_tests;
mod model;
#[cfg(test)]
mod pairing_export_tests;
pub(crate) mod password;
#[cfg(test)]
mod role_tests;
pub(crate) mod rules;
mod service;
mod state;
mod store;
#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod unified_tests;

use bsv_home_bsv::cold_wallet::Pairing;
use primitives::{Chain, Wallet, WalletType};

pub use action::{BsvHomeBsvColdWalletActionIntent, BsvHomeBsvColdWalletActionOutcome, BsvHomeBsvColdWalletActionScope, BsvHomeBsvColdWalletSubmission};
pub use model::{
    BsvHomeBsvColdWalletCandidate, BsvHomeBsvColdWalletPending, BsvHomeBsvColdWalletReview, BsvHomeBsvColdWalletSnapshot, BsvHomeBsvColdWalletStateStamp,
    BsvHomeBsvColdWalletStoredState, BsvHomeOfflineSigningPasswordDomain,
};
pub use password::BsvHomeOfflineSigningPasswordService;
pub use rules::{BsvHomeBsvColdWalletRole, bsv_cold_wallet_require_hot_signing, bsv_cold_wallet_role};
pub use service::BsvHomeBsvColdWalletService;
pub use store::{BsvHomeBsvColdWalletSigner, BsvHomeBsvColdWalletStore};

#[uniffi::export]
pub fn bsv_cold_wallet_can_send(wallet: Wallet) -> bool {
    wallet.wallet_type == WalletType::View && wallet.account(Chain::BitcoinSv).is_some_and(|account| Pairing::from_account(account).is_ok())
}
