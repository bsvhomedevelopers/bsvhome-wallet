use super::{BsvHomeBsvMultisigActionIntent as ActionIntent, BsvHomeBsvMultisigActionScope as ActionScope};
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use std::sync::Arc;

use super::model::RemoteWallet;
use super::service::{authentication, identity_address};
use super::{BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigService, rules};
use crate::services::portal::BsvMultisigTarget as Target;

#[cfg(test)]
#[path = "paymail_tests.rs"]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, uniffi::Record)]
pub struct BsvHomeBsvMultisigPaymailSettings {
    pub wallet_id: String,
    pub handle: String,
    pub alias: String,
    pub domain: String,
    pub active: bool,
    pub can_manage: bool,
    pub version: u64,
    pub configuration_hash: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemotePaymailSettings {
    handle: String,
    alias: String,
    domain: String,
    active: bool,
    can_manage: bool,
    version: i64,
    configuration_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyPaymail {
    handle: String,
    configuration_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePaymail<'a> {
    alias: &'a str,
    active: bool,
    expected_version: u64,
    configuration_hash: &'a str,
}

#[uniffi::export]
impl BsvHomeBsvMultisigService {
    pub async fn load_paymail_settings(&self, wallet_id: String) -> Result<BsvHomeBsvMultisigPaymailSettings, Error> {
        self.workflow_checkpoint()?;
        let _mutation = self.mutation.lock().await;
        let local = self.current_wallet().await?;
        self.check_workflow_wallet(&local).await?;
        let wallet = self.paymail_wallet(&wallet_id)?;
        let response = self
            .workflow_request::<RemotePaymailSettings>(&local, Target::PaymailSettings { wallet_id: wallet_id.clone() })
            .await;
        let remote = match response {
            Ok(settings) => settings,
            Err(Error::Api { code }) if code == "NOT_FOUND" || code == "HTTP_404" => {
                let legacy: LegacyPaymail = self.workflow_request(&local, Target::LegacyPaymail { wallet_id: wallet_id.clone() }).await?;
                let alias = format!("multisig-{wallet_id}");
                let prefix = format!("{alias}@");
                let domain = legacy.handle.strip_prefix(&prefix).ok_or(Error::ConfigurationConflict)?.to_string();
                RemotePaymailSettings {
                    handle: legacy.handle,
                    alias,
                    domain,
                    active: true,
                    can_manage: false,
                    version: 0,
                    configuration_hash: legacy.configuration_hash,
                }
            }
            Err(error) => return Err(error),
        };
        let settings = remote.validate(&wallet)?;
        self.check_workflow_wallet(&local).await?;
        self.cache()?.paymail_settings.insert(wallet_id, settings.clone());
        Ok(settings)
    }

    pub async fn update_paymail_settings(
        &self,
        settings: BsvHomeBsvMultisigPaymailSettings,
        alias: String,
        active: bool,
        action: Arc<ActionScope>,
    ) -> Result<BsvHomeBsvMultisigPaymailSettings, Error> {
        self.claim_action(
            &action,
            ActionIntent::UpdatePaymailSettings {
                wallet_id: settings.wallet_id.clone(),
                alias: alias.clone(),
                active,
                expected_version: settings.version,
                configuration_hash: settings.configuration_hash.clone(),
            },
        )?;
        action
            .run(async {
                action.check()?;
                let alias = bsv_multisig_paymail_alias(alias)?;
                let _mutation = self.mutation.lock().await;
                let local = self.action_current_wallet(&action).await?;
                let wallet = self.paymail_wallet(&settings.wallet_id)?;
                if !settings.can_manage {
                    return Err(Error::Api { code: "CREATOR_REQUIRED".into() });
                }
                if self.cache()?.paymail_settings.get(&settings.wallet_id) != Some(&settings) {
                    return Err(Error::ConcurrentUpdate);
                }
                if wallet.configuration_hash.as_ref() != Some(&settings.configuration_hash) {
                    return Err(Error::ConfigurationConflict);
                }
                self.signer.authenticate(local.clone(), Some(action.clone())).await.map_err(authentication)?;
                self.check_action(&local, &action).await?;
                let fresh: RemoteWallet = self
                    .action_request(
                        &local,
                        Target::Wallet {
                            wallet_id: settings.wallet_id.clone(),
                        },
                        &action,
                    )
                    .await?;
                rules::validate_wallet(&fresh)?;
                let address = identity_address(&local)?;
                if fresh.id != settings.wallet_id || fresh.members.iter().all(|member| member.address != address) {
                    return Err(Error::MissingSigner);
                }
                self.check_action(&local, &action).await?;
                if fresh.status != "ACTIVE" || fresh.configuration_hash.as_ref() != Some(&settings.configuration_hash) {
                    return Err(Error::ConfigurationConflict);
                }
                let body = to_value(UpdatePaymail {
                    alias: &alias,
                    active,
                    expected_version: settings.version,
                    configuration_hash: &settings.configuration_hash,
                })?;
                let result = async {
                    let remote: RemotePaymailSettings = self
                        .action_request(
                            &local,
                            Target::UpdatePaymailSettings {
                                wallet_id: settings.wallet_id.clone(),
                                body,
                            },
                            &action,
                        )
                        .await?;
                    let updated = remote.validate(&fresh)?;
                    if updated.alias != alias || updated.active != active || updated.domain != settings.domain || updated.version < settings.version || !updated.can_manage {
                        return Err(Error::ConfigurationConflict);
                    }
                    action.submitted(None, None)?;
                    action.finish()?;
                    Ok(updated)
                }
                .await;
                self.check_action(&local, &action).await?;
                match result {
                    Ok(updated) => {
                        self.cache()?.paymail_settings.insert(settings.wallet_id, updated.clone());
                        Ok(updated)
                    }
                    Err(error) => {
                        self.cache()?.paymail_settings.remove(&settings.wallet_id);
                        Err(error)
                    }
                }
            })
            .await
    }
}

impl BsvHomeBsvMultisigService {
    fn paymail_wallet(&self, wallet_id: &str) -> Result<RemoteWallet, Error> {
        if !self.get_wallet(wallet_id.into())?.can_transact {
            return Err(Error::ConfigurationNotConfirmed);
        }
        let wallet = self.remote_wallet(wallet_id)?;
        rules::wallet_policy(&wallet)?;
        Ok(wallet)
    }
}

impl RemotePaymailSettings {
    fn validate(self, wallet: &RemoteWallet) -> Result<BsvHomeBsvMultisigPaymailSettings, Error> {
        if !valid_alias(&self.alias)
            || !valid_domain(&self.domain)
            || self.handle != format!("{}@{}", self.alias, self.domain)
            || wallet.configuration_hash.as_ref() != Some(&self.configuration_hash)
        {
            return Err(Error::ConfigurationConflict);
        }
        Ok(BsvHomeBsvMultisigPaymailSettings {
            wallet_id: wallet.id.clone(),
            handle: self.handle,
            alias: self.alias,
            domain: self.domain,
            active: self.active,
            can_manage: self.can_manage,
            version: rules::nonnegative(self.version)?,
            configuration_hash: self.configuration_hash,
        })
    }
}

fn valid_alias(value: &str) -> bool {
    let alphanumeric = |value: u8| value.is_ascii_lowercase() || value.is_ascii_digit();
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().next().is_some_and(alphanumeric)
        && value.bytes().last().is_some_and(alphanumeric)
        && value.bytes().all(|byte| alphanumeric(byte) || byte == b'.' || byte == b'_' || byte == b'-')
}

fn valid_domain(value: &str) -> bool {
    value.len() <= 253
        && value.contains('.')
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.bytes().next().is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                && label.bytes().last().is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                && label.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

#[uniffi::export]
pub fn bsv_multisig_paymail_alias(value: String) -> Result<String, Error> {
    let alias = value.trim().to_ascii_lowercase();
    if !valid_alias(&alias) {
        return Err(Error::Api { code: "INVALID_ALIAS".into() });
    }
    Ok(alias)
}
