use bsv_home_bitcoin::signer::account_path::AccountPath;
use bsv_home_bsv::cold_wallet::{Pairing, decode_json};
use bsv_home_derivation::{BSV_ACCOUNT_PATH, SIMPLY_CASH_ACCOUNT_PATH, bsv_address_for_path, derive_bsv_account_from_recovery, derive_wallet_id_from_account};
use bsv_home_keystore::{HdDerivationMode, HdRecovery};
use primitives::{Account, BsvAddressBranch, BsvAddressLocator, Chain, WalletType};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use super::{BsvHomeKeystore, BsvHomeStoredWallet};
use crate::BsvHomeCoreError;
use crate::keystore::keystore_id_for_wallet;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Backup {
    version: u32,
    kind: String,
    phrase: String,
    passphrase: String,
    account_path: String,
    mode: HdDerivationMode,
}

impl Drop for Backup {
    fn drop(&mut self) {
        self.phrase.zeroize();
        self.passphrase.zeroize();
    }
}

pub(super) fn parse_recovery(input: &str, account_path: Option<&str>, passphrase: Option<&str>, legacy_non_compliant: bool) -> Result<HdRecovery, BsvHomeCoreError> {
    if input.len() > 8192 {
        return Err(BsvHomeCoreError::from("HD recovery exceeds limit"));
    }
    if input.trim_start().starts_with("xprv") {
        return Err(BsvHomeCoreError::from("xprv import is not supported; use mnemonic recovery"));
    }
    let (phrase, embedded_path, embedded_passphrase, mode) = if input.trim_start().starts_with('{') {
        let backup: Backup = serde_json::from_str(input).map_err(|_| BsvHomeCoreError::from("Invalid HD recovery backup"))?;
        if backup.version != 1 || backup.kind != "wowpay-hd-recovery" {
            return Err(BsvHomeCoreError::from("Unsupported HD recovery backup"));
        }
        if legacy_non_compliant && backup.mode != HdDerivationMode::LegacyUnpadded {
            return Err(BsvHomeCoreError::from("HD derivation mode conflicts with backup"));
        }
        (
            Zeroizing::new(backup.phrase.clone()),
            Some(backup.account_path.clone()),
            Some(Zeroizing::new(backup.passphrase.clone())),
            backup.mode,
        )
    } else {
        let parts = input.splitn(3, ':').collect::<Vec<_>>();
        (
            Zeroizing::new(parts[0].to_string()),
            parts
                .get(1)
                .map(|path| if path.is_empty() { SIMPLY_CASH_ACCOUNT_PATH.to_string() } else { path.to_string() }),
            parts.get(2).map(|value| Zeroizing::new(value.to_string())),
            if legacy_non_compliant {
                HdDerivationMode::LegacyUnpadded
            } else {
                HdDerivationMode::Standard
            },
        )
    };
    let provided_path = account_path.map(|path| AccountPath::parse(path).map(|path| path.canonical())).transpose()?;
    let embedded_path = embedded_path.as_deref().map(|path| AccountPath::parse(path).map(|path| path.canonical())).transpose()?;
    if let (Some(provided), Some(embedded)) = (&provided_path, &embedded_path)
        && provided != embedded
    {
        return Err(BsvHomeCoreError::from("Provided account path conflicts with backup"));
    }
    let path = provided_path.or(embedded_path).unwrap_or_else(|| BSV_ACCOUNT_PATH.to_string());
    if let (Some(provided), Some(embedded)) = (passphrase, embedded_passphrase.as_ref())
        && provided != embedded.as_str()
    {
        return Err(BsvHomeCoreError::from("Provided BIP39 passphrase conflicts with backup"));
    }
    let passphrase = match passphrase {
        Some(value) => value,
        None => embedded_passphrase.as_ref().map(|value| value.as_str()).unwrap_or(""),
    };
    HdRecovery::new(&phrase, passphrase, &path, mode).map_err(BsvHomeCoreError::from)
}

fn verify_address(account: &Account, expected: Option<&str>) -> Result<(), BsvHomeCoreError> {
    if let Some(expected) = expected.filter(|expected| !expected.trim().is_empty())
        && account.address != expected.trim()
    {
        return Err(BsvHomeCoreError::from("BSV HD first address does not match; check passphrase and derivation mode"));
    }
    Ok(())
}

#[uniffi::export]
impl BsvHomeKeystore {
    pub fn bsv_hd_preview(
        &self,
        recovery_string: String,
        account_path: Option<String>,
        passphrase: Option<String>,
        expected_address: Option<String>,
        legacy_non_compliant: bool,
    ) -> Result<String, BsvHomeCoreError> {
        let input = Zeroizing::new(recovery_string);
        let passphrase = passphrase.map(Zeroizing::new);
        let recovery = parse_recovery(&input, account_path.as_deref(), passphrase.as_ref().map(|value| value.as_str()), legacy_non_compliant)?;
        let account = derive_bsv_account_from_recovery(&recovery)?;
        verify_address(&account, expected_address.as_deref())?;
        serde_json::to_string(&Pairing::from_account(&account)?).map_err(BsvHomeCoreError::from)
    }

    pub fn bsv_hd_xpub_pairing(&self, xpub: String, account_path: String, expected_address: Option<String>) -> Result<String, BsvHomeCoreError> {
        let xpub = xpub.trim().to_string();
        let account_path = AccountPath::parse(&account_path)?;
        account_path.validate_xpub(&xpub)?;
        let account = Account {
            chain: Chain::BitcoinSv,
            address: bsv_address_for_path(
                &xpub,
                &account_path.canonical(),
                BsvAddressLocator {
                    branch: BsvAddressBranch::Receive,
                    index: 0,
                },
            )?,
            derivation_path: format!("{}/0/0", account_path.canonical()),
            extended_public_key: Some(xpub),
        };
        verify_address(&account, expected_address.as_deref())?;
        serde_json::to_string(&Pairing::from_account(&account)?).map_err(BsvHomeCoreError::from)
    }

    pub fn bsv_hd_public_preview(&self, input: String, account_path: Option<String>, expected_address: Option<String>) -> Result<String, BsvHomeCoreError> {
        if input.trim_start().starts_with('{') {
            let pairing: Pairing = decode_json(&input)?;
            pairing.validate()?;
            if let Some(path) = account_path
                && AccountPath::parse(&path)?.canonical() != pairing.account_path
            {
                return Err(BsvHomeCoreError::from("provided account path conflicts with pairing"));
            }
            verify_address(&pairing.account()?, expected_address.as_deref())?;
            return serde_json::to_string(&pairing).map_err(BsvHomeCoreError::from);
        }
        self.bsv_hd_xpub_pairing(input, account_path.unwrap_or_else(|| BSV_ACCOUNT_PATH.to_string()), expected_address)
    }

    pub fn export_hd_recovery(&self, keystore_id: String, password: Vec<u8>) -> Result<String, BsvHomeCoreError> {
        let password = Zeroizing::new(password);
        let recovery = self.decrypt_hd_recovery(&keystore_id, &password)?;
        let backup = Backup {
            version: 1,
            kind: "wowpay-hd-recovery".into(),
            phrase: recovery.phrase.clone(),
            passphrase: recovery.passphrase.clone(),
            account_path: recovery.account_path.clone(),
            mode: recovery.mode,
        };
        serde_json::to_string(&backup).map_err(BsvHomeCoreError::from)
    }
}

impl BsvHomeKeystore {
    pub(crate) fn import_bsv_hd(
        &self,
        input: &str,
        account_path: Option<&str>,
        passphrase: Option<&str>,
        expected_address: Option<&str>,
        legacy_non_compliant: bool,
        password: &[u8],
    ) -> Result<BsvHomeStoredWallet, BsvHomeCoreError> {
        let recovery = parse_recovery(input, account_path, passphrase, legacy_non_compliant)?;
        let account = derive_bsv_account_from_recovery(&recovery)?;
        verify_address(&account, expected_address)?;
        let wallet_id = derive_wallet_id_from_account(&account, WalletType::Single)?;
        let keystore_id = keystore_id_for_wallet(wallet_id.to_string());
        let meta = self.inner_import_hd_recovery(&recovery, password, &keystore_id)?;
        let existing = self.bsv_recovery(&keystore_id, password)?;
        if Pairing::from_account(&derive_bsv_account_from_recovery(&existing)?)? != Pairing::from_account(&account)? {
            return Err(BsvHomeCoreError::from("Existing HD recovery does not match imported wallet"));
        }
        Ok(BsvHomeStoredWallet::new(wallet_id, WalletType::Single, meta.keystore_id, vec![account]))
    }
}

#[cfg(test)]
mod tests {
    use primitives::testkit::ABANDON_PHRASE;

    use super::*;

    #[test]
    fn test_recovery_defaults_embedded_metadata_and_conflicts() {
        let default = parse_recovery(ABANDON_PHRASE, None, None, false).unwrap();
        assert_eq!(default.account_path, BSV_ACCOUNT_PATH);
        let embedded = parse_recovery(&format!("{ABANDON_PHRASE}:: public:Pass "), None, None, true).unwrap();
        assert_eq!(embedded.account_path, SIMPLY_CASH_ACCOUNT_PATH);
        assert_eq!(embedded.passphrase, " public:Pass ");
        assert_eq!(embedded.mode, HdDerivationMode::LegacyUnpadded);
        assert!(parse_recovery(&format!("{ABANDON_PHRASE}::"), Some(BSV_ACCOUNT_PATH), None, false).is_err());
        assert!(parse_recovery(&format!("{ABANDON_PHRASE}:m/44h/145H/02':pass"), Some("m/44'/145'/2'"), Some("different"), false).is_err());
        let canonical = parse_recovery(&format!("{ABANDON_PHRASE}:m/44h/145H/02':pass"), Some("m/44'/145'/2'"), None, false).unwrap();
        assert_eq!(canonical.account_path, "m/44'/145'/2'");
        let backup = Backup {
            version: 1,
            kind: "wowpay-hd-recovery".into(),
            phrase: ABANDON_PHRASE.into(),
            passphrase: " public:Pass ".into(),
            account_path: "m/0'".into(),
            mode: HdDerivationMode::LegacyUnpadded,
        };
        let json = serde_json::to_string(&backup).unwrap();
        let restored = parse_recovery(&json, None, None, false).unwrap();
        assert_eq!(restored.mode, HdDerivationMode::LegacyUnpadded);
        assert_eq!(restored.passphrase, " public:Pass ");
        assert_eq!(restored.account_path, "m/0'");
        assert!(parse_recovery(&json, Some(BSV_ACCOUNT_PATH), None, false).is_err());
        assert!(parse_recovery(&json, None, Some("public:Pass"), false).is_err());
        assert!(parse_recovery("xprv-invalid", None, None, false).is_err());
        assert!(parse_recovery(&"x".repeat(8193), None, None, false).is_err());
    }
    #[test]
    fn test_public_preview_preserves_pairing_configuration_and_rejects_conflicts() {
        let directory = tempfile::TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let pairing_json = keystore.bsv_hd_preview(ABANDON_PHRASE.into(), Some("m/44'/145'/2'".into()), None, None, false).unwrap();
        let pairing: Pairing = decode_json(&pairing_json).unwrap();
        assert_eq!(
            keystore.bsv_hd_public_preview(pairing_json.clone(), None, Some(pairing.first_address.clone())).unwrap(),
            pairing_json
        );
        assert_eq!(
            keystore.bsv_hd_public_preview(pairing_json.clone(), Some("m/044h/145H/002'".into()), None).unwrap(),
            pairing_json
        );
        assert!(keystore.bsv_hd_public_preview(pairing_json.clone(), Some(BSV_ACCOUNT_PATH.into()), None).is_err());
        assert!(
            keystore
                .bsv_hd_public_preview(pairing_json.clone(), None, Some("1BoatSLRHtKNngkdXEeobR76b53LETtpyT".into()))
                .is_err()
        );
        assert_eq!(
            keystore
                .bsv_hd_public_preview(pairing.xpub, Some(pairing.account_path), Some(pairing.first_address))
                .unwrap(),
            pairing_json
        );
    }
}
