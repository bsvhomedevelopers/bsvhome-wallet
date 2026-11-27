use std::sync::Arc;

use primitives::{Chain, Wallet, WalletId, WalletType};

use super::{BsvHomeBsvColdWalletStore, bsv_cold_wallet_can_send};
use crate::services::error::BsvHomeServiceError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum BsvHomeBsvColdWalletRole {
    None,
    Signer,
    Observer,
}

#[uniffi::export]
pub fn bsv_cold_wallet_role(wallet: Wallet, store: Arc<dyn BsvHomeBsvColdWalletStore>) -> Result<BsvHomeBsvColdWalletRole, BsvHomeServiceError> {
    if wallet.account(Chain::BitcoinSv).is_none() {
        return Ok(BsvHomeBsvColdWalletRole::None);
    }
    let present = store.has_state(wallet.id.clone())?;
    Ok(match wallet.wallet_type {
        WalletType::View => {
            if present || bsv_cold_wallet_can_send(wallet) {
                BsvHomeBsvColdWalletRole::Observer
            } else {
                BsvHomeBsvColdWalletRole::None
            }
        }
        WalletType::Single | WalletType::Multicoin | WalletType::PrivateKey => {
            if present {
                BsvHomeBsvColdWalletRole::Signer
            } else {
                BsvHomeBsvColdWalletRole::None
            }
        }
    })
}

#[uniffi::export]
pub fn bsv_cold_wallet_require_hot_signing(wallet: Wallet, chain: Chain, store: Arc<dyn BsvHomeBsvColdWalletStore>) -> Result<(), BsvHomeServiceError> {
    require_hot_wallet_id(wallet.id.clone(), chain, store.as_ref())?;
    if chain == Chain::BitcoinSv && wallet.wallet_type == WalletType::View {
        return Err(BsvHomeServiceError::Unsupported {
            msg: "BSV watch-only wallets cannot use online signing".into(),
        });
    }
    Ok(())
}

pub(crate) fn require_hot_wallet_id(wallet_id: WalletId, chain: Chain, store: &dyn BsvHomeBsvColdWalletStore) -> Result<(), BsvHomeServiceError> {
    if chain == Chain::BitcoinSv && store.has_state(wallet_id)? {
        return Err(BsvHomeServiceError::Unsupported {
            msg: "BSV cold wallets require the cold signing flow".into(),
        });
    }
    Ok(())
}
