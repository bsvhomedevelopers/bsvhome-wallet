use primitives::WalletId;
use url::Url;

use super::BsvHomePreferencesService;
use crate::services::error::BsvHomeServiceError;

impl BsvHomePreferencesService {
    pub(crate) fn is_multisig_identity(&self, wallet_id: &WalletId) -> bool {
        self.store.get(identity_key(wallet_id)).as_deref() == Some("true")
    }

    pub(crate) fn mark_multisig_identity(&self, wallet_id: &WalletId) -> Result<(), BsvHomeServiceError> {
        self.store.set(identity_key(wallet_id), "true".into())
    }
}

#[uniffi::export]
impl BsvHomePreferencesService {
    pub fn multisig_signer(&self, origin: String, wallet_id: String) -> Result<Option<WalletId>, BsvHomeServiceError> {
        self.store
            .get(signer_key(&origin, &wallet_id)?)
            .map(|value| {
                WalletId::from_id(&value).ok_or_else(|| BsvHomeServiceError::Store {
                    msg: "invalid multisig signing identity preference".into(),
                })
            })
            .transpose()
    }

    pub fn set_multisig_signer(&self, origin: String, wallet_id: String, signer_wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        self.store.set(signer_key(&origin, &wallet_id)?, signer_wallet_id.id())
    }
}

fn identity_key(wallet_id: &WalletId) -> String {
    format!("multisigidentity.{}", wallet_id.id())
}

fn signer_key(origin: &str, wallet_id: &str) -> Result<String, BsvHomeServiceError> {
    let invalid = || BsvHomeServiceError::InvalidInput {
        msg: "invalid multisig preference scope".into(),
    };
    let origin = Url::parse(origin).map_err(|_| invalid())?;
    if origin.scheme() != "https"
        || origin.host_str().is_none()
        || !origin.username().is_empty()
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
        || wallet_id.is_empty()
    {
        return Err(invalid());
    }
    let scope = serde_json::to_string(&(origin.as_str().trim_end_matches('/'), wallet_id)).map_err(|_| invalid())?;
    Ok(format!("multisigsigner.{scope}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use primitives::{Chain, WalletId};

    use super::super::testkit::MemoryPreferencesStore;
    use super::BsvHomePreferencesService;

    #[test]
    fn test_multisig_selection_preserves_scope_and_dedicated_metadata() {
        let store = Arc::new(MemoryPreferencesStore::default());
        let preferences = BsvHomePreferencesService::new(store.clone());
        let signer = WalletId::PrivateKey(Chain::BitcoinSv, "public-identity".into());
        let other = WalletId::PrivateKey(Chain::BitcoinSv, "another-public-identity".into());
        let origin = "https://paymail.bsvhome.com/api/portal";
        let retained = "https://api-dev.hj0252.com/v2/api/portal";

        preferences.mark_multisig_identity(&signer).unwrap();
        preferences.set_multisig_signer(origin.into(), "collective-a".into(), signer.clone()).unwrap();
        preferences.set_multisig_signer(origin.into(), "collective-b".into(), other.clone()).unwrap();
        preferences.set_multisig_signer(retained.into(), "collective-a".into(), other.clone()).unwrap();

        let restored = BsvHomePreferencesService::new(store);
        assert_eq!(restored.multisig_signer(format!("{origin}/"), "collective-a".into()).unwrap(), Some(signer.clone()));
        assert_eq!(restored.multisig_signer(origin.into(), "collective-b".into()).unwrap(), Some(other.clone()));
        assert_eq!(restored.multisig_signer(retained.into(), "collective-a".into()).unwrap(), Some(other.clone()));
        assert_eq!(restored.multisig_signer(origin.into(), "unknown".into()).unwrap(), None);
        assert!(restored.is_multisig_identity(&signer));
        assert!(!restored.is_multisig_identity(&other));
    }
}
