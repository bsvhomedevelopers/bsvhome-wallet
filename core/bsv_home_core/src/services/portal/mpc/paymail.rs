use std::collections::BTreeMap;

use base64::{Engine, engine::general_purpose::STANDARD};
use bsv_home_bsv::{BsvAddress, signer::BsvChainSigner};
use chrono::Utc;
use primitives::{Address, ValueAccess};
use serde_json::{Value, json};
use url::Url;
use uuid::Uuid;

use super::rules::{bsv_amount, hex_text, paymail_fields};
use super::{BsvHomeMpcError, BsvHomeMpcPaymailBinding, BsvHomeMpcPaymailSnapshot, BsvHomeMpcWallet, BsvHomeMpcWalletService};
use crate::services::portal::paymail_rules::handle as paymail_handle;
use crate::services::portal::platform::PlatformSessionLease;
use crate::services::portal::{BsvHomePaymailBindingAvailability, BsvHomePaymailDestination};

#[uniffi::export]
impl BsvHomeMpcWalletService {
    pub async fn paymail_snapshot(&self, wallet_id: String) -> Result<BsvHomeMpcPaymailSnapshot, BsvHomeMpcError> {
        let lease = self.account.session_lease()?;
        let state = self.state(&lease)?;
        let known = self.known_wallet(&state, &wallet_id)?;
        self.wallet_paymail(&lease, known).await
    }

    pub async fn resolve_recipient(&self, wallet_id: String, recipient: String, amount_bsv: String) -> Result<BsvHomePaymailDestination, BsvHomeMpcError> {
        let amount = bsv_amount(&amount_bsv)?;
        let lease = self.account.session_lease()?;
        let state = self.state(&lease)?;
        let known = self.known_wallet(&state, &wallet_id)?;
        self.resolved_recipient(&lease, known, &recipient, amount).await
    }

    pub async fn bind_paymail(&self, wallet_id: String, handle: String, active: bool) -> Result<String, BsvHomeMpcError> {
        let _operation = self.operation.lock().await;
        let lease = self.account.session_lease()?;
        let mut state = self.state(&lease)?;
        self.reconcile_task(&lease, &mut state).await?;
        let config = self.account.authenticated_get_json(&lease, "portal/auth/paymail/config").await?;
        if config.get_string("network")? != "BSV_MAINNET" || config.get_string("addressType")? != "P2PKH" {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        let domains = config.get_value("domains")?.as_array().ok_or(BsvHomeMpcError::InvalidResponse)?;
        let (alias, domain) = handle.split_once('@').ok_or(BsvHomeMpcError::InvalidInput)?;
        if !(2..=31).contains(&alias.len())
            || !alias
                .bytes()
                .enumerate()
                .all(|(index, byte)| byte.is_ascii_lowercase() || byte.is_ascii_digit() || (index > 0 && b"._-".contains(&byte)))
            || !domains.iter().any(|value| value.as_str() == Some(domain))
        {
            return Err(BsvHomeMpcError::InvalidInput);
        }
        let origin = config.get_string("publicBaseUrl")?;
        let origin_url = Url::parse(origin).map_err(|_| BsvHomeMpcError::InvalidResponse)?;
        if origin_url.scheme() != "https"
            || origin_url.host_str().is_none()
            || !origin_url.username().is_empty()
            || origin_url.password().is_some()
            || origin_url.query().is_some()
            || origin_url.fragment().is_some()
        {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        let known = self.public_wallet(&lease, self.known_wallet(&state, &wallet_id)?).await?;
        let result = async {
            self.local_identity(&lease, &known, true).await?;
            let identity = self.native(&lease, &wallet_id, json!({"op":"identity_public_key"})).await?;
            let identity_key = identity.get_string("public_key")?;
            if !hex_text(identity_key, 66) || identity_key == known.public_key {
                return Err(BsvHomeMpcError::IdentityMismatch);
            }
            let binding = json!({"alias":alias,"domain":domain,"address":known.address,"identityPublicKey":identity_key,"active":active});
            let challenge = self
                .request(&lease, "portal/mpc/paymail/challenge", json!({"walletId":wallet_id,"binding":binding}))
                .await?;
            let message = challenge.get_string("message")?;
            let challenge_id = challenge.get_string("challengeId")?;
            if message.len() > 8192 || Uuid::parse_str(challenge_id).is_err() {
                return Err(BsvHomeMpcError::InvalidResponse);
            }
            let expected = BTreeMap::from([
                ("Origin", origin.to_string()),
                ("User", lease.user_id.clone()),
                ("Handle", handle.clone()),
                ("Network", "BSV_MAINNET".into()),
                ("Address", known.address.clone()),
                ("Identity Key", identity_key.into()),
                ("Active", active.to_string()),
                ("MPC Wallet", wallet_id.clone()),
                ("MPC Epoch", known.epoch.to_string()),
                ("Nonce", challenge_id.into()),
            ]);
            paymail_fields(message, &expected, Utc::now())?;
            let identity_signature = self.native(&lease, &wallet_id, json!({"op":"identity_sign","message":message})).await?;
            let signature = identity_signature.get_string("signature")?;
            let digest = hex::encode(BsvChainSigner::message_hash(message));
            if identity_signature.get_string("digest")? != digest
                || hex::encode(BsvChainSigner::recover_message_public_key(
                    message,
                    &STANDARD.decode(signature).map_err(|_| BsvHomeMpcError::InvalidResponse)?,
                )?) != identity_key
            {
                return Err(BsvHomeMpcError::IdentityMismatch);
            }
            let task = self
                .prepare_task(
                    &lease,
                    &mut state,
                    json!({"kind":"PAYMAIL","walletId":wallet_id,"paymailChallengeId":challenge_id}),
                    Some(known.epoch),
                )
                .await?;
            if task.context.get_string("paymailMessage")? != message {
                return Err(BsvHomeMpcError::InvalidResponse);
            }
            self.signing(&lease, &task, &[digest]).await?;
            let complete = self.task_status(&lease, &task).await?;
            if complete.get_string("status")? != "COMPLETE" {
                return Err(BsvHomeMpcError::OperationPending);
            }
            let result = self
                .request(&lease, "portal/mpc/paymail/confirm", json!({"taskId":task.id,"identitySignature":signature}))
                .await?;
            if result.get_string("handle")? != handle
                || result.get_string("address")? != known.address
                || result.get_string("identityPublicKey")? != identity_key
                || result.get_value("active")? != &Value::Bool(active)
            {
                return Err(BsvHomeMpcError::InvalidResponse);
            }
            state.active_task = None;
            self.save(&lease, &state)?;
            Ok(handle.clone())
        }
        .await;
        self.participant.close(wallet_id).await?;
        result
    }
}

impl BsvHomeMpcWalletService {
    pub(super) async fn resolved_recipient(
        &self,
        lease: &PlatformSessionLease,
        known: &BsvHomeMpcWallet,
        recipient: &str,
        amount: u64,
    ) -> Result<BsvHomePaymailDestination, BsvHomeMpcError> {
        if !recipient.contains('@') {
            if !BsvAddress::is_valid(recipient) {
                return Err(BsvHomeMpcError::InvalidInput);
            }
            return Ok(BsvHomePaymailDestination {
                handle: String::new(),
                address: recipient.into(),
                reference: None,
            });
        }
        let snapshot = self.wallet_paymail(lease, known).await?;
        if snapshot
            .bindings
            .iter()
            .all(|binding| !binding.active || binding.availability != BsvHomePaymailBindingAvailability::Available)
        {
            return Err(BsvHomeMpcError::PaymailSenderRequired);
        }
        let destination = self
            .paymail
            .destination(recipient.into(), Some(amount))
            .await
            .map_err(|_| BsvHomeMpcError::PaymailUnavailable)?;
        self.account.require_lease(lease)?;
        Ok(destination)
    }
    async fn wallet_paymail(&self, lease: &PlatformSessionLease, known: &BsvHomeMpcWallet) -> Result<BsvHomeMpcPaymailSnapshot, BsvHomeMpcError> {
        let config = self.account.authenticated_get_json(lease, "portal/auth/paymail/config").await?;
        if config.get_string("network")? != "BSV_MAINNET" || config.get_string("addressType")? != "P2PKH" {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        let domains = config
            .get_value("domains")?
            .as_array()
            .ok_or(BsvHomeMpcError::InvalidResponse)?
            .iter()
            .map(|domain| Ok(domain.string()?.to_string()))
            .collect::<Result<Vec<_>, BsvHomeMpcError>>()?;
        for domain in &domains {
            paymail_handle(&format!("aa@{domain}")).map_err(|_| BsvHomeMpcError::InvalidResponse)?;
        }
        let public_base_url = config.get_string("publicBaseUrl")?.to_string();
        let origin = Url::parse(&public_base_url).map_err(|_| BsvHomeMpcError::InvalidResponse)?;
        if origin.scheme() != "https"
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            return Err(BsvHomeMpcError::InvalidResponse);
        }
        let value = self.account.authenticated_get_json(lease, "portal/auth/paymail/me").await?;
        let rows = value.as_array().ok_or(BsvHomeMpcError::InvalidResponse)?;
        let mut bindings = Vec::new();
        for row in rows {
            let handle = row.get_string("handle")?;
            let (_, domain) = paymail_handle(handle).map_err(|_| BsvHomeMpcError::InvalidResponse)?;
            let address = row.get_string("address")?;
            let identity_public_key = row.get_string("identityPublicKey")?;
            let active = row.get_value("active")?.as_bool().ok_or(BsvHomeMpcError::InvalidResponse)?;
            if !hex_text(identity_public_key, 66) || !BsvAddress::is_valid(address) {
                return Err(BsvHomeMpcError::InvalidResponse);
            }
            if address == known.address {
                bindings.push(BsvHomeMpcPaymailBinding {
                    handle: handle.into(),
                    address: address.into(),
                    identity_public_key: identity_public_key.into(),
                    active,
                    availability: if domains.contains(&domain) {
                        BsvHomePaymailBindingAvailability::Available
                    } else {
                        BsvHomePaymailBindingAvailability::DomainRemoved
                    },
                });
            }
        }
        Ok(BsvHomeMpcPaymailSnapshot {
            wallet_id: known.wallet_id.clone(),
            domains,
            public_base_url,
            bindings,
        })
    }
}
