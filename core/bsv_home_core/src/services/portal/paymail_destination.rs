use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::services::bsv_multisig::bsv_multisig_payment_request_from_script;
use bsv_home_bsv::BsvAddress;
use bsv_home_hash::sha2::sha256;
use chrono::{SecondsFormat, Utc};
use futures::lock::Mutex;
use number_formatter::BigNumberFormatter;
use primitives::{Address, Chain, Wallet};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use url::Url;
use uuid::Uuid;

use crate::alien::{AlienHttpMethod, AlienProvider, AlienTarget};
use crate::services::error::BsvHomeServiceError;

use super::BsvHomePortalAuthError;
use super::paymail_error::BsvHomePaymailError;
use super::paymail_model::{BsvHomePaymailDestination, BsvHomePaymailOutput, BsvHomePaymailResolvedDestination};
use super::{BsvHomePaymailService, paymail_rules};

pub(crate) const MAX_SATOSHIS: u64 = 2_100_000_000_000_000;
const P2PKH_SCRIPT_LENGTH: usize = 25;

#[derive(uniffi::Object)]
pub struct BsvHomePaymailDestinationService {
    provider: Arc<dyn AlienProvider>,
    retry_after: Mutex<Option<Instant>>,
    sender: Option<Arc<BsvHomePaymailService>>,
    owner: Uuid,
}

enum Request {
    Discovery { domain: String },
    Destination { url: Url, body: DestinationRequest },
}

#[derive(Clone)]
struct Sender {
    wallet: Wallet,
    version: u64,
    handle: String,
}

pub(crate) struct BsvHomePaymailDestinationSnapshot {
    owner: Uuid,
    sender: Option<Sender>,
    alias: String,
    domain: String,
    pub(crate) result: BsvHomePaymailResolvedDestination,
}

#[derive(Deserialize)]
struct Discovery {
    bsvalias: String,
    capabilities: HashMap<String, Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DestinationRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    sender_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount: Option<u64>,
}

#[derive(Deserialize)]
struct Destination {
    output: String,
    reference: Option<String>,
}

#[derive(Deserialize)]
struct PublicError {
    success: Option<bool>,
    code: String,
    message: Option<String>,
}

#[uniffi::export]
impl BsvHomePaymailDestinationService {
    #[uniffi::constructor]
    pub fn new(provider: Arc<dyn AlienProvider>) -> Self {
        Self {
            provider,
            retry_after: Mutex::new(None),
            sender: None,
            owner: Uuid::new_v4(),
        }
    }

    #[uniffi::constructor]
    pub fn new_for_wallet(provider: Arc<dyn AlienProvider>, sender: Arc<BsvHomePaymailService>) -> Self {
        Self {
            sender: Some(sender),
            ..Self::new(provider)
        }
    }

    pub async fn destination(&self, handle: String, amount: Option<u64>) -> Result<BsvHomePaymailDestination, BsvHomePaymailError> {
        let destination = self.resolve_output(handle, amount).await?;
        match destination.output {
            BsvHomePaymailOutput::P2pkh { address, .. } => Ok(BsvHomePaymailDestination {
                handle: destination.handle,
                address,
                reference: destination.reference,
            }),
            BsvHomePaymailOutput::P2ms { .. } => Err(BsvHomePaymailError::UnsupportedOutput),
        }
    }

    pub async fn resolve_output(&self, handle: String, amount: Option<u64>) -> Result<BsvHomePaymailResolvedDestination, BsvHomePaymailError> {
        if amount.is_some_and(|amount| amount > MAX_SATOSHIS) {
            return Err(BsvHomePaymailError::InvalidAmount);
        }
        let (alias, domain) = paymail_rules::handle(&handle)?;
        let check = || Ok(());
        let sender = self.sender(&check).await?;
        Ok(self.resolve_with_sender(alias, domain, amount, sender, &check).await?.result)
    }
}

impl BsvHomePaymailDestinationService {
    pub(crate) async fn prepare_for_send(&self, handle: String) -> Result<BsvHomePaymailDestinationSnapshot, BsvHomePaymailError> {
        let (alias, domain) = paymail_rules::handle(&handle)?;
        let check = || Ok(());
        let sender = self.sender(&check).await?.ok_or(BsvHomePaymailError::SenderRequired)?;
        self.resolve_with_sender(alias, domain, None, Some(sender), &check).await
    }

    pub(crate) fn check_prepared_sender_scope(&self, snapshot: &BsvHomePaymailDestinationSnapshot) -> Result<(), BsvHomePaymailError> {
        if snapshot.owner != self.owner {
            return Err(BsvHomePaymailError::BindingChanged);
        }
        match (&self.sender, &snapshot.sender) {
            (Some(service), Some(sender)) => service.check_workflow_scope(&sender.wallet, sender.version),
            (Some(_), None) | (None, Some(_)) | (None, None) => Err(BsvHomePaymailError::SenderRequired),
        }
    }

    pub(crate) fn prepared_sender_matches_payer(&self, snapshot: &BsvHomePaymailDestinationSnapshot, payer: &Wallet) -> Result<bool, BsvHomePaymailError> {
        self.check_prepared_sender_scope(snapshot)?;
        let sender = snapshot.sender.as_ref().ok_or(BsvHomePaymailError::SenderRequired)?;
        Ok(sender.wallet.id == payer.id && sender.wallet.wallet_type == payer.wallet_type && sender.wallet.account(Chain::BitcoinSv) == payer.account(Chain::BitcoinSv))
    }

    pub(crate) async fn check_prepared_sender(
        &self,
        snapshot: &BsvHomePaymailDestinationSnapshot,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<(), BsvHomePaymailError> {
        check()?;
        self.check_prepared_sender_scope(snapshot)?;
        self.check_sender(snapshot.sender.as_ref(), check).await?;
        self.check_prepared_sender_scope(snapshot)?;
        check()?;
        Ok(())
    }

    pub(crate) async fn confirm_for_send(
        &self,
        snapshot: &BsvHomePaymailDestinationSnapshot,
        amount: u64,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<BsvHomePaymailResolvedDestination, BsvHomePaymailError> {
        check()?;
        if snapshot.owner != self.owner {
            return Err(BsvHomePaymailError::BindingChanged);
        }
        if amount > MAX_SATOSHIS {
            return Err(BsvHomePaymailError::InvalidAmount);
        }
        self.check_sender(snapshot.sender.as_ref(), check).await?;
        let fresh = self.sender(check).await?;
        let original = snapshot.sender.as_ref().ok_or(BsvHomePaymailError::SenderRequired)?;
        let current = fresh.as_ref().ok_or(BsvHomePaymailError::SenderRequired)?;
        if current.wallet.id != original.wallet.id
            || current.wallet.wallet_type != original.wallet.wallet_type
            || current.wallet.account(Chain::BitcoinSv) != original.wallet.account(Chain::BitcoinSv)
            || current.version != original.version
            || current.handle != original.handle
        {
            return Err(BsvHomePaymailError::BindingChanged);
        }
        Ok(self
            .resolve_with_sender(snapshot.alias.clone(), snapshot.domain.clone(), Some(amount), fresh, check)
            .await?
            .result)
    }

    async fn resolve_with_sender(
        &self,
        alias: String,
        domain: String,
        amount: Option<u64>,
        sender: Option<Sender>,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<BsvHomePaymailDestinationSnapshot, BsvHomePaymailError> {
        check()?;
        let discovery = self.request::<Discovery>(Request::Discovery { domain: domain.clone() }, sender.as_ref(), check).await?;
        if discovery.bsvalias != "1.0" {
            return Err(BsvHomePaymailError::UnsupportedCapability);
        }
        let template = discovery
            .capabilities
            .get("paymentDestination")
            .and_then(Value::as_str)
            .ok_or(BsvHomePaymailError::UnsupportedCapability)?;
        let url = destination_url(template, &alias, &domain)?;
        let body = DestinationRequest {
            sender_handle: sender.as_ref().map(|sender| sender.handle.clone()),
            dt: sender.as_ref().map(|_| Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)),
            amount,
        };
        let destination = self.request::<Destination>(Request::Destination { url, body }, sender.as_ref(), check).await?;
        let handle = format!("{alias}@{domain}");
        let output = destination_output(&handle, &destination.output, amount)?;
        Ok(BsvHomePaymailDestinationSnapshot {
            owner: self.owner,
            sender,
            alias,
            domain,
            result: BsvHomePaymailResolvedDestination {
                handle,
                output,
                reference: destination.reference,
            },
        })
    }

    async fn sender(&self, check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync)) -> Result<Option<Sender>, BsvHomePaymailError> {
        check()?;
        let Some(service) = &self.sender else { return Ok(None) };
        let version = service.workflow_version();
        let wallet = service.current_wallet().await?;
        check()?;
        service.check_workflow(&wallet, version).await?;
        check()?;
        let snapshot = service.load_with_checkpoint(check).await?;
        service.check_workflow(&wallet, version).await?;
        check()?;
        service.require_verified(&snapshot)?;
        let handle = snapshot.active_handle.ok_or(BsvHomePaymailError::SenderRequired)?;
        Ok(Some(Sender { wallet, version, handle }))
    }

    async fn check_sender(&self, sender: Option<&Sender>, check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync)) -> Result<(), BsvHomePaymailError> {
        check()?;
        if let (Some(service), Some(sender)) = (&self.sender, sender) {
            service.check_workflow(&sender.wallet, sender.version).await?;
        }
        check()?;
        Ok(())
    }

    async fn request<R: DeserializeOwned>(
        &self,
        request: Request,
        sender: Option<&Sender>,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<R, BsvHomePaymailError> {
        check()?;
        {
            let retry_after = self.retry_after.lock().await;
            check()?;
            if retry_after.is_some_and(|deadline| deadline > Instant::now()) {
                return Err(BsvHomePaymailError::Api {
                    code: "RATE_LIMITED".into(),
                    msg: "RATE_LIMITED".into(),
                });
            }
        }
        let (url, method, body) = match request {
            Request::Discovery { domain } => (format!("https://{domain}/.well-known/bsvalias"), AlienHttpMethod::Get, None),
            Request::Destination { url, body } => (
                url.to_string(),
                AlienHttpMethod::Post,
                Some(serde_json::to_vec(&body).map_err(|_| BsvHomePaymailError::InvalidAmount)?),
            ),
        };
        let target = AlienTarget {
            url,
            method,
            headers: Some(HashMap::from([
                ("Content-Type".into(), "application/json".into()),
                ("Cache-Control".into(), "no-store".into()),
            ])),
            body,
        };
        self.check_sender(sender, check).await?;
        let response = self.provider.request(target).await;
        self.check_sender(sender, check).await?;
        let response = response
            .map_err(|error| BsvHomePaymailError::from(BsvHomeServiceError::Api { msg: error.to_string() }))?
            .to_rpc_response();
        let status = response.status.ok_or(BsvHomePaymailError::InvalidResponse)?;
        if !(200..300).contains(&status) {
            if status == 429 {
                *self.retry_after.lock().await = Some(Instant::now() + Duration::from_secs(1));
            }
            let error = serde_json::from_slice::<PublicError>(&response.data).map_err(|_| BsvHomePaymailError::Api {
                code: format!("HTTP_{status}"),
                msg: format!("HTTP {status}"),
            })?;
            if error.code == "RATE_LIMITED" {
                *self.retry_after.lock().await = Some(Instant::now() + Duration::from_secs(1));
            }
            return Err(BsvHomePaymailError::Api {
                code: error.code,
                msg: error.message.unwrap_or_else(|| format!("HTTP {status}")),
            });
        }
        if let Ok(error) = serde_json::from_slice::<PublicError>(&response.data)
            && error.success == Some(false)
        {
            if error.code == "RATE_LIMITED" {
                *self.retry_after.lock().await = Some(Instant::now() + Duration::from_secs(1));
            }
            return Err(BsvHomePaymailError::Api {
                code: error.code,
                msg: error.message.unwrap_or_else(|| "Paymail request failed".into()),
            });
        }
        serde_json::from_slice(&response.data).map_err(|_| BsvHomePaymailError::InvalidResponse)
    }
}

pub(super) fn destination_url(template: &str, alias: &str, domain: &str) -> Result<Url, BsvHomePaymailError> {
    let endpoint = template.replace("{alias}", alias).replace("{domain.tld}", domain).replace("{domain}", domain);
    if endpoint.contains('{') || endpoint.contains('}') {
        return Err(BsvHomePaymailError::UnsupportedCapability);
    }
    let url = Url::parse(&endpoint).map_err(|_| BsvHomePaymailError::UnsupportedCapability)?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err(BsvHomePaymailError::UnsupportedCapability);
    }
    Ok(url)
}

pub(super) fn destination_address(output: &str) -> Result<String, BsvHomePaymailError> {
    if output.len() != P2PKH_SCRIPT_LENGTH * 2 {
        return Err(BsvHomePaymailError::UnsupportedOutput);
    }
    let script = hex::decode(output).map_err(|_| BsvHomePaymailError::UnsupportedOutput)?;
    if script[..3] != [0x76, 0xa9, 0x14] || script[23..] != [0x88, 0xac] {
        return Err(BsvHomePaymailError::UnsupportedOutput);
    }
    let mut payload = Vec::with_capacity(P2PKH_SCRIPT_LENGTH);
    payload.push(0);
    payload.extend_from_slice(&script[3..23]);
    let checksum = sha256(&sha256(&payload));
    payload.extend_from_slice(&checksum[..4]);
    Ok(bs58::encode(payload).into_string())
}

pub(super) fn destination_output(handle: &str, output: &str, amount: Option<u64>) -> Result<BsvHomePaymailOutput, BsvHomePaymailError> {
    let script = hex::decode(output).map_err(|_| BsvHomePaymailError::UnsupportedOutput)?;
    let output_script = hex::encode(&script);
    if let Ok(address) = destination_address(&output_script) {
        let validated = BsvAddress::from_str(&address).map_err(|_| BsvHomePaymailError::UnsupportedOutput)?;
        if validated.script_pubkey().as_bytes() != script {
            return Err(BsvHomePaymailError::UnsupportedOutput);
        }
        return Ok(BsvHomePaymailOutput::P2pkh { address, output_script });
    }
    let amount = match amount {
        Some(amount) => BigNumberFormatter::big_decimal_value(&amount.to_string(), 8)
            .map_err(|_| BsvHomePaymailError::InvalidAmount)?
            .normalized()
            .to_plain_string(),
        None => String::new(),
    };
    let request = bsv_multisig_payment_request_from_script(handle.to_string(), output_script.clone(), amount).map_err(|_| BsvHomePaymailError::UnsupportedOutput)?;
    Ok(BsvHomePaymailOutput::P2ms { request, output_script })
}

#[cfg(test)]
#[path = "paymail_output_tests.rs"]
mod tests;
