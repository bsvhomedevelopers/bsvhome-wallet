use bsv_home_bsv::BsvAddress;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use primitives::{Address, Chain, Wallet, WalletType};
use url::Url;

use super::error::BsvHomePortalAuthError;
use super::model::{Challenge, Identity, IdentityRequest, LoginResponse, Session};

pub(super) const AUTH_CHAIN: &str = "BSV";
const LEGACY_MAX_CHALLENGE_SECONDS: i64 = 900;
const LEGACY_MAX_CLOCK_SKEW_SECONDS: i64 = 5;
const DEVICE_CHALLENGE_SECONDS: i64 = 300;
const DEVICE_MAX_CLOCK_SKEW_SECONDS: i64 = 60;

pub(crate) fn base_url(value: &str) -> Result<String, BsvHomePortalAuthError> {
    let url = Url::parse(value).map_err(|_| BsvHomePortalAuthError::InvalidSession)?;
    if !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
        return Err(BsvHomePortalAuthError::InvalidSession);
    }
    if url.scheme() != "https" || url.host_str().is_none() || !matches!(url.path().trim_end_matches('/'), "/api/portal" | "/v2/api/portal") {
        return Err(BsvHomePortalAuthError::InvalidSession);
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}

pub(super) fn identity(wallet: &Wallet) -> Result<Identity, BsvHomePortalAuthError> {
    if wallet.wallet_type == WalletType::View {
        return Err(BsvHomePortalAuthError::WatchOnly);
    }
    let account = wallet.account(Chain::BitcoinSv).ok_or(BsvHomePortalAuthError::MissingBsvAccount)?;
    BsvAddress::from_str(&account.address).map_err(|_| BsvHomePortalAuthError::MissingBsvAccount)?;
    Ok(Identity {
        wallet_id: wallet.id.id(),
        address: account.address.clone(),
    })
}

pub(super) fn validate_challenge(challenge: &Challenge, identity: &IdentityRequest<'_>, base_url: &str, now: DateTime<Utc>) -> Result<(), BsvHomePortalAuthError> {
    let fields = challenge.message.split('\n').collect::<Vec<_>>();
    let nonce = fields
        .get(2)
        .and_then(|field| field.strip_prefix("Nonce: "))
        .ok_or(BsvHomePortalAuthError::InvalidChallenge)?;
    let issued_at = fields
        .get(3)
        .and_then(|field| field.strip_prefix("Issued At: "))
        .ok_or(BsvHomePortalAuthError::InvalidChallenge)?;
    let issued = DateTime::parse_from_rfc3339(issued_at).map_err(|_| BsvHomePortalAuthError::InvalidChallenge)?;
    let expiry = DateTime::parse_from_rfc3339(&challenge.expires_at).map_err(|_| BsvHomePortalAuthError::InvalidChallenge)?;
    let (expected, valid_protocol) = if base_url.ends_with("/v2/api/portal") {
        (
            format!(
                "WowPay Wallet Auth\nAddress: {}\nNonce: {nonce}\nIssued At: {issued_at}\nExpiration Time: {}",
                identity.address, challenge.expires_at
            ),
            challenge.challenge_id.starts_with('C')
                && challenge.challenge_id.len() >= 2
                && challenge.challenge_id.is_ascii()
                && !challenge.challenge_id.bytes().any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
                && nonce.len() == 32
                && nonce.bytes().all(|byte| byte.is_ascii_hexdigit())
                && expiry <= now + Duration::seconds(LEGACY_MAX_CHALLENGE_SECONDS)
                && issued <= now + Duration::seconds(LEGACY_MAX_CLOCK_SKEW_SECONDS),
        )
    } else {
        let lower_hex = |value: &str| value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        (
            format!(
                "WowPay Wallet Auth\nAddress: {}\nNonce: {nonce}\nIssued At: {}\nExpiration Time: {}\nDevice: {}\nDevice Type: {}",
                identity.address,
                issued.to_rfc3339_opts(SecondsFormat::AutoSi, true),
                expiry.to_rfc3339_opts(SecondsFormat::AutoSi, true),
                identity.device_id,
                identity.device_type
            ),
            challenge.challenge_id.strip_prefix('C').is_some_and(lower_hex)
                && lower_hex(nonce)
                && issued <= now + Duration::seconds(DEVICE_MAX_CLOCK_SKEW_SECONDS)
                && expiry == issued + Duration::seconds(DEVICE_CHALLENGE_SECONDS),
        )
    };
    if !valid_protocol
        || challenge.chain != AUTH_CHAIN
        || challenge.address != identity.address
        || expiry <= now
        || issued >= expiry
        || issued.offset().local_minus_utc() != 0
        || expiry.offset().local_minus_utc() != 0
        || challenge.message != expected
    {
        return Err(BsvHomePortalAuthError::InvalidChallenge);
    }
    Ok(())
}

pub(super) fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.is_ascii() && !token.bytes().any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
}

pub(super) fn validate_login(login: &LoginResponse, identity: &Identity, now: DateTime<Utc>) -> Result<(), BsvHomePortalAuthError> {
    if !valid_token(&login.token)
        || login.address.as_deref() != Some(identity.address.as_str())
        || login.chain.as_deref() != Some(AUTH_CHAIN)
        || login.expires_at.is_some_and(|expires_at| expires_at <= now)
    {
        return Err(BsvHomePortalAuthError::InvalidSession);
    }
    Ok(())
}

pub(super) fn session_matches(session: &Session, identity: &Identity, base_url: &str, now: DateTime<Utc>) -> bool {
    session.base_url == base_url
        && session.wallet_id == identity.wallet_id
        && session.address == identity.address
        && valid_token(&session.token)
        && session.expires_at.is_none_or(|expires_at| expires_at > now)
}

pub(super) fn requires_authentication(code: &str) -> bool {
    matches!(
        code,
        "USER_SESSION_PLEASE_LOGIN_FIRST" | "USER_SESSION_EXPIRED" | "USER_SESSION_INVALID" | "LOGIN_REQUIRED" | "HTTP_401"
    )
}
