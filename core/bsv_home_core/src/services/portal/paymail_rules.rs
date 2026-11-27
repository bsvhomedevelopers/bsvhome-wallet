use std::collections::HashSet;

use bsv_home_bsv::BsvAddress;
use chrono::{DateTime, Duration, Utc};
use primitives::{Address, Wallet};
use url::{Host, Url};
use uuid::Uuid;

use super::paymail_error::BsvHomePaymailError;
use super::paymail_model::{
    Binding, BindingChallenge, BsvHomePaymailBinding, BsvHomePaymailBindingAvailability, BsvHomePaymailBindingIntent, BsvHomePaymailSnapshot, BsvHomePaymailVerification,
    Configuration,
};
use super::rules;

const MAX_ALIAS_LENGTH: usize = 64;
const MAX_DOMAIN_LENGTH: usize = 253;
const MAX_CHALLENGE_SECONDS: i64 = 900;

pub(super) fn alias(value: &str) -> Result<String, BsvHomePaymailError> {
    let value = value.trim().to_ascii_lowercase();
    let edge = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    if value.is_empty()
        || value.len() > MAX_ALIAS_LENGTH
        || !value.bytes().all(|byte| edge(byte) || byte == b'.' || byte == b'_' || byte == b'-')
        || !value.bytes().next().is_some_and(edge)
        || !value.bytes().last().is_some_and(edge)
    {
        return Err(BsvHomePaymailError::InvalidAlias);
    }
    Ok(value)
}

pub(super) fn domain(value: &str) -> Result<String, BsvHomePaymailError> {
    let domain = match Host::parse(value.trim()) {
        Ok(Host::Domain(domain)) => domain,
        Ok(Host::Ipv4(_) | Host::Ipv6(_)) | Err(_) => return Err(BsvHomePaymailError::InvalidDomain),
    };
    if domain.len() > MAX_DOMAIN_LENGTH
        || !domain.contains('.')
        || domain.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || !label.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || label.starts_with('-')
                || label.ends_with('-')
        })
    {
        return Err(BsvHomePaymailError::InvalidDomain);
    }
    Ok(domain)
}

pub(super) fn handle(value: &str) -> Result<(String, String), BsvHomePaymailError> {
    let (name, host) = value.trim().split_once('@').ok_or(BsvHomePaymailError::InvalidHandle)?;
    let name = alias(name).map_err(|_| BsvHomePaymailError::InvalidHandle)?;
    let host = domain(host).map_err(|_| BsvHomePaymailError::InvalidHandle)?;
    Ok((name, host))
}

pub(super) fn origin(value: &str) -> Result<String, BsvHomePaymailError> {
    let url = Url::parse(value).map_err(|_| BsvHomePaymailError::InvalidConfiguration)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(BsvHomePaymailError::InvalidConfiguration);
    }
    Ok(url.origin().ascii_serialization())
}

pub(super) fn configuration(configuration: Configuration, trusted_origins: &[String]) -> Result<Configuration, BsvHomePaymailError> {
    if configuration.network != "BSV_MAINNET" || configuration.address_type != "P2PKH" || configuration.identity_key_rotation {
        return Err(BsvHomePaymailError::InvalidConfiguration);
    }
    let public_base_url = origin(&configuration.public_base_url)?;
    if !trusted_origins.contains(&public_base_url) {
        return Err(BsvHomePaymailError::InvalidConfiguration);
    }
    let domains = configuration.domains.iter().map(|value| domain(value)).collect::<Result<Vec<_>, _>>()?;
    let unique = domains.iter().collect::<HashSet<_>>();
    if unique.len() != domains.len() || domains.is_empty() {
        return Err(BsvHomePaymailError::InvalidConfiguration);
    }
    Ok(Configuration {
        domains,
        public_base_url: configuration.public_base_url.clone(),
        ..configuration
    })
}

pub(super) fn identity_public_key(value: &str) -> Result<(), BsvHomePaymailError> {
    if value.len() != 66 || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(BsvHomePaymailError::InvalidResponse);
    }
    let public_key = hex::decode(value).map_err(|_| BsvHomePaymailError::InvalidResponse)?;
    BsvAddress::from_public_key(&public_key).map_err(|_| BsvHomePaymailError::InvalidResponse)?;
    Ok(())
}

pub(super) fn snapshot(wallet: &Wallet, configuration: &Configuration, bindings: Vec<Binding>) -> Result<BsvHomePaymailSnapshot, BsvHomePaymailError> {
    let identity = rules::identity(wallet)?;
    let mut domains = HashSet::new();
    let bindings = bindings
        .into_iter()
        .map(|binding| {
            let (alias, domain) = handle(&binding.handle).map_err(|_| BsvHomePaymailError::InvalidResponse)?;
            if binding.handle != format!("{alias}@{domain}") || binding.version == 0 || binding.version >= i64::MAX as u64 || !domains.insert(domain.clone()) {
                return Err(BsvHomePaymailError::InvalidResponse);
            }
            BsvAddress::from_str(&binding.address).map_err(|_| BsvHomePaymailError::InvalidResponse)?;
            identity_public_key(&binding.identity_public_key)?;
            let availability = if !configuration.domains.contains(&domain) {
                BsvHomePaymailBindingAvailability::DomainRemoved
            } else if binding.address != identity.address {
                BsvHomePaymailBindingAvailability::AddressMismatch
            } else {
                BsvHomePaymailBindingAvailability::Available
            };
            Ok(BsvHomePaymailBinding {
                handle: binding.handle,
                address: binding.address,
                identity_public_key: binding.identity_public_key,
                active: binding.active,
                version: binding.version,
                availability,
            })
        })
        .collect::<Result<Vec<_>, BsvHomePaymailError>>()?;
    let active_handle = configuration.domains.iter().find_map(|domain| {
        bindings
            .iter()
            .find(|binding| binding.active && binding.availability == BsvHomePaymailBindingAvailability::Available && binding.handle.ends_with(&format!("@{domain}")))
            .map(|binding| binding.handle.clone())
    });
    let registration_domains = configuration
        .domains
        .iter()
        .filter(|domain| !bindings.iter().any(|binding| binding.handle.ends_with(&format!("@{domain}"))))
        .cloned()
        .collect();
    Ok(BsvHomePaymailSnapshot {
        wallet_id: identity.wallet_id,
        address: identity.address,
        public_origin: configuration.public_base_url.clone(),
        domains: configuration.domains.clone(),
        registration_domains,
        bindings,
        active_handle,
        verification: BsvHomePaymailVerification::Clear,
    })
}

pub(super) fn previous_binding<'a>(
    snapshot: &'a BsvHomePaymailSnapshot,
    alias: &str,
    domain: &str,
    active: bool,
) -> Result<Option<&'a BsvHomePaymailBinding>, BsvHomePaymailError> {
    if !snapshot.domains.iter().any(|configured| configured == domain) {
        return Err(BsvHomePaymailError::InvalidDomain);
    }
    let handle = format!("{alias}@{domain}");
    let previous = snapshot.bindings.iter().find(|binding| binding.handle.ends_with(&format!("@{domain}")));
    if let Some(previous) = previous {
        if previous.handle != handle || previous.availability != BsvHomePaymailBindingAvailability::Available {
            return Err(BsvHomePaymailError::BindingChanged);
        }
    } else if !active {
        return Err(BsvHomePaymailError::BindingChanged);
    }
    Ok(previous)
}

pub(super) fn validate_challenge(
    challenge: &BindingChallenge,
    intent: &BsvHomePaymailBindingIntent,
    identity_public_key: &str,
    now: DateTime<Utc>,
) -> Result<(), BsvHomePaymailError> {
    let id = Uuid::parse_str(&challenge.challenge_id).map_err(|_| BsvHomePaymailError::InvalidChallenge)?;
    let expiry = DateTime::parse_from_rfc3339(&challenge.expires_at).map_err(|_| BsvHomePaymailError::InvalidChallenge)?;
    if id.to_string() != challenge.challenge_id || expiry <= now || expiry > now + Duration::seconds(MAX_CHALLENGE_SECONDS) {
        return Err(BsvHomePaymailError::InvalidChallenge);
    }
    let lines = challenge.message.split('\n').collect::<Vec<_>>();
    let user = lines.get(2).and_then(|line| line.strip_prefix("User: ")).ok_or(BsvHomePaymailError::InvalidChallenge)?;
    if !user.as_bytes().first().is_some_and(|byte| (b'1'..=b'9').contains(byte)) || !user.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(BsvHomePaymailError::InvalidChallenge);
    }
    let expected = format!(
        "WowPay Paymail Binding\nOrigin: {}\nUser: {user}\nHandle: {}\nNetwork: BSV_MAINNET\nAddress: {}\nIdentity Key: {}\nActive: {}\nVersion: {}\nNonce: {}\nExpires: {}",
        intent.public_origin, intent.handle, intent.address, identity_public_key, intent.active, intent.previous_version, challenge.challenge_id, challenge.expires_at
    );
    if challenge.message != expected {
        return Err(BsvHomePaymailError::InvalidChallenge);
    }
    Ok(())
}

pub(super) fn binding_matches(binding: &BsvHomePaymailBinding, intent: &BsvHomePaymailBindingIntent, identity_public_key: &str) -> bool {
    binding.handle == intent.handle
        && binding.address == intent.address
        && binding.identity_public_key == identity_public_key
        && binding.active == intent.active
        && binding.version == intent.previous_version + 1
}

pub(super) fn binding_rejection(code: &str) -> bool {
    match code {
        "PAYMAIL_DISABLED"
        | "DOMAIN_NOT_HOSTED"
        | "INVALID_ALIAS"
        | "INVALID_DOMAIN"
        | "INVALID_HANDLE"
        | "ALIAS_RESERVED"
        | "ALIAS_UNAVAILABLE"
        | "DOMAIN_ALREADY_REGISTERED"
        | "ALIAS_OR_DOMAIN_UNAVAILABLE"
        | "INVALID_BSV_ADDRESS"
        | "INVALID_IDENTITY_KEY"
        | "SEPARATE_IDENTITY_KEY_REQUIRED"
        | "IDENTITY_KEY_IMMUTABLE"
        | "BINDING_NOT_REGISTERED"
        | "INVALID_SIGNATURE"
        | "CHALLENGE_EXPIRED_OR_USED"
        | "BINDING_CHANGED"
        | "INVALID_REQUEST"
        | "RATE_LIMITED"
        | "HTTP_401" => true,
        _ => rules::requires_authentication(code),
    }
}
