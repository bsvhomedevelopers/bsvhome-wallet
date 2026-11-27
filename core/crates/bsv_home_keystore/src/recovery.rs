use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::{KeystoreError, Mnemonic};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HdDerivationMode {
    Standard,
    LegacyUnpadded,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HdRecovery {
    pub phrase: String,
    pub passphrase: String,
    pub account_path: String,
    pub mode: HdDerivationMode,
}

impl fmt::Debug for HdRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdRecovery")
            .field("phrase", &"<redacted>")
            .field("passphrase", &"<redacted>")
            .field("account_path", &self.account_path)
            .field("mode", &self.mode)
            .finish()
    }
}

impl Drop for HdRecovery {
    fn drop(&mut self) {
        self.phrase.zeroize();
        self.passphrase.zeroize();
    }
}

impl HdRecovery {
    pub fn new(phrase: &str, passphrase: &str, account_path: &str, mode: HdDerivationMode) -> Result<Self, KeystoreError> {
        if passphrase.len() > 1024 || account_path.is_empty() || account_path.len() > 256 {
            return Err(KeystoreError::invalid_input("HD recovery metadata"));
        }
        Ok(Self {
            phrase: Mnemonic::clean(phrase)?.to_string(),
            passphrase: passphrase.to_string(),
            account_path: account_path.to_string(),
            mode,
        })
    }
}
