use super::BsvHomeBsvMultisigError as Error;
use super::model::*;
use bsv_home_bsv::BsvAddress;
use bsv_home_bsv::multisig::{Policy, assemble_proposal, member_address, policy_descriptor, proposal_transaction_id, review_proposal};
use primitives::{Address, Chain, Wallet, WalletType};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const INVITATION_PREFIX: &str = "bsvhome://multisig/join?token=";
const INVITATION_TOKEN_LENGTH: usize = 43;

#[uniffi::export]
pub fn parse_bsv_multisig_invitation(value: String) -> Result<String, Error> {
    let value = value.trim();
    let token = match value.strip_prefix(INVITATION_PREFIX) {
        Some(token) => token,
        None => value,
    };
    validate_invitation_token(token)?;
    Ok(token.to_string())
}

#[uniffi::export]
pub fn bsv_multisig_invitation_qr_payload(token: String) -> Result<String, Error> {
    validate_invitation_token(&token)?;
    Ok(format!("{INVITATION_PREFIX}{token}"))
}

pub(crate) fn validate_invitation_token(value: &str) -> Result<(), Error> {
    if value.len() != INVITATION_TOKEN_LENGTH || !value.bytes().all(|value| value.is_ascii_alphanumeric() || value == b'-' || value == b'_') {
        return Err(Error::invalid("INVALID_INVITE"));
    }
    Ok(())
}

#[uniffi::export]
pub fn bsv_multisig_signing_wallets(wallets: Vec<Wallet>) -> Result<Vec<Wallet>, Error> {
    let mut eligible = Vec::new();
    for wallet in wallets {
        match wallet.wallet_type {
            WalletType::View => continue,
            WalletType::Multicoin | WalletType::Single | WalletType::PrivateKey => {}
        }
        let accounts = wallet.accounts.iter().filter(|account| account.chain == Chain::BitcoinSv).collect::<Vec<_>>();
        if accounts.is_empty() {
            if wallet.id.chain() == Some(Chain::BitcoinSv) {
                return Err(Error::MissingSigner);
            }
            continue;
        }
        if accounts.len() != 1 || wallet.id.chain().is_some_and(|chain| chain != Chain::BitcoinSv) {
            return Err(Error::MissingSigner);
        }
        BsvAddress::from_str(&accounts[0].address).map_err(|_| Error::MissingSigner)?;
        eligible.push(wallet);
    }
    Ok(eligible)
}

#[uniffi::export]
pub fn merge_bsv_multisig_wallets(wallets: Vec<BsvHomeBsvMultisigWallet>, preferred_signer_wallet_id: Option<String>) -> Vec<BsvHomeBsvMultisigWallet> {
    let mut merged: Vec<BsvHomeBsvMultisigWallet> = Vec::new();
    let mut positions: BTreeMap<String, usize> = BTreeMap::new();
    for wallet in wallets {
        match positions.get(&wallet.id).copied() {
            Some(index) => {
                let previous = &merged[index];
                let preferred = preferred_signer_wallet_id
                    .as_ref()
                    .is_some_and(|id| wallet.local_signer_wallet_id.as_ref() == Some(id) && previous.local_signer_wallet_id.as_ref() != Some(id));
                if wallet.version > previous.version || (wallet.version == previous.version && preferred) {
                    merged[index] = wallet;
                }
            }
            None => {
                positions.insert(wallet.id.clone(), merged.len());
                merged.push(wallet);
            }
        }
    }
    merged
}

pub(crate) fn amount_bsv(value: &str) -> Result<u64, Error> {
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 2 || parts[0].is_empty() || !parts[0].bytes().all(|v| v.is_ascii_digit()) {
        return Err(Error::invalid("INVALID_AMOUNT"));
    }
    let whole = parts[0].parse::<u64>().map_err(|_| Error::invalid("INVALID_AMOUNT"))?;
    let fractional = if parts.len() == 2 {
        let digits = parts[1];
        if digits.is_empty() || digits.len() > 8 || !digits.bytes().all(|v| v.is_ascii_digit()) {
            return Err(Error::invalid("INVALID_AMOUNT"));
        }
        digits.parse::<u64>().map_err(|_| Error::invalid("INVALID_AMOUNT"))? * 10u64.pow(8 - digits.len() as u32)
    } else {
        0
    };
    whole
        .checked_mul(100_000_000)
        .and_then(|v| v.checked_add(fractional))
        .filter(|v| *v > 0 && *v <= 2_100_000_000_000_000)
        .ok_or_else(|| Error::invalid("INVALID_AMOUNT"))
}

pub(crate) fn text(value: &str, maximum: usize, allow_empty: bool) -> Result<String, Error> {
    let value = value.trim();
    if (!allow_empty && value.is_empty()) || value.encode_utf16().count() > maximum || value.chars().any(char::is_control) {
        return Err(Error::invalid("INVALID_REQUEST"));
    }
    Ok(value.to_string())
}

pub(crate) fn creation_text(value: &str, maximum: usize) -> Result<String, Error> {
    text(value, maximum, false)
}

pub(crate) fn nonnegative(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::invalid("INVALID_RESPONSE"))
}

pub(crate) fn validate_wallet(wallet: &RemoteWallet) -> Result<(), Error> {
    if wallet.id.is_empty()
        || wallet.name.trim().is_empty()
        || !(2..=5).contains(&wallet.member_count)
        || !(2..=wallet.member_count).contains(&wallet.threshold)
        || wallet.members.is_empty()
        || wallet.members.len() > wallet.member_count as usize
        || wallet.version < 0
    {
        return Err(Error::InvalidRules);
    }
    let mut keys = BTreeSet::new();
    let mut addresses = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for member in &wallet.members {
        if member.user_id < 0
            || member_address(&member.public_key)? != member.address
            || !keys.insert(member.public_key.clone())
            || !addresses.insert(member.address.clone())
            || !identities.insert(member.user_id)
        {
            return Err(Error::InvalidRules);
        }
    }
    if wallet.members.len() == wallet.member_count as usize {
        wallet_policy(wallet)?;
    }
    if wallet.status == "ACTIVE" && (wallet.members.len() != wallet.member_count as usize || wallet.members.iter().any(|m| !m.confirmed)) {
        return Err(Error::InvalidRules);
    }
    Ok(())
}

pub(crate) fn wallet_policy(wallet: &RemoteWallet) -> Result<Policy, Error> {
    if wallet.members.len() != wallet.member_count as usize {
        return Err(Error::ConfigurationNotConfirmed);
    }
    let descriptor = policy_descriptor(&Policy {
        threshold: wallet.threshold,
        public_keys: wallet.members.iter().map(|m| m.public_key.clone()).collect(),
    })?;
    if wallet.locking_script.as_ref() != Some(&descriptor.locking_script)
        || wallet.script_hash.as_ref() != Some(&descriptor.script_hash)
        || wallet.configuration_hash.as_ref() != Some(&descriptor.configuration_hash)
    {
        return Err(Error::ConfigurationConflict);
    }
    Ok(descriptor.policy)
}

pub(crate) fn configuration_status(wallet: &RemoteWallet, address: &str, trusted: Option<&Policy>) -> Result<BsvHomeBsvMultisigConfigurationStatus, Error> {
    let member = wallet.members.iter().find(|m| m.address == address);
    if wallet.members.len() != wallet.member_count as usize || member.is_none() || (wallet.status != "DRAFT" && wallet.status != "ACTIVE") {
        return Ok(BsvHomeBsvMultisigConfigurationStatus::Unavailable);
    }
    let policy = wallet_policy(wallet)?;
    if trusted.is_some_and(|v| v != &policy) {
        return Ok(BsvHomeBsvMultisigConfigurationStatus::Changed);
    }
    let member = member.ok_or(Error::MissingSigner)?;
    if !member.confirmed {
        return Ok(BsvHomeBsvMultisigConfigurationStatus::NeedsConfirmation);
    }
    if trusted.is_none() {
        return Ok(BsvHomeBsvMultisigConfigurationStatus::NeedsLocalReview);
    }
    if wallet.status == "DRAFT" {
        return Ok(BsvHomeBsvMultisigConfigurationStatus::WaitingForMembers);
    }
    Ok(BsvHomeBsvMultisigConfigurationStatus::Confirmed)
}

pub(crate) fn validate_proposal(proposal: &RemoteProposal, wallet: &RemoteWallet) -> Result<(), Error> {
    if proposal.id.is_empty()
        || proposal.wallet_id != wallet.id
        || proposal.version < 0
        || proposal.created_at < 0
        || proposal.expires_at <= proposal.created_at
        || proposal.creator_id.is_some_and(|id| id < 0)
        || proposal.confirmations.is_some_and(|confirmations| confirmations < 0)
    {
        return Err(Error::invalid("INVALID_RESPONSE"));
    }
    let policy = wallet_policy(wallet)?;
    review_proposal(&policy, &proposal.proposal)?;
    let mut keys = BTreeSet::new();
    for signatures in &proposal.signatures {
        if !policy.public_keys.contains(&signatures.public_key) || !keys.insert(signatures.public_key.clone()) || signatures.signatures.len() != proposal.proposal.inputs.len() {
            return Err(Error::invalid("INVALID_MEMBER_SIGNATURE"));
        }
        bsv_home_bsv::multisig::validate_signature_set(&policy, &proposal.proposal, signatures)?;
    }
    if matches!(proposal.status.as_str(), "READY" | "BROADCAST" | "CONFIRMED") && proposal.signatures.len() < policy.threshold as usize {
        return Err(Error::invalid("INVALID_MEMBER_SIGNATURE"));
    }
    if matches!(proposal.status.as_str(), "BROADCAST" | "CONFIRMED") {
        let raw = assemble_proposal(&policy, &proposal.proposal, &proposal.signatures)?;
        let txid = proposal_transaction_id(&policy, &proposal.proposal, &raw)?;
        if proposal.txid.as_deref() != Some(&txid) {
            return Err(Error::invalid("INVALID_BSV_TRANSACTION"));
        }
    }
    proposal_creator(proposal, wallet)?;
    Ok(())
}

pub(crate) fn proposal_creator<'a>(proposal: &RemoteProposal, wallet: &'a RemoteWallet) -> Result<Option<&'a RemoteMember>, Error> {
    let by_id = proposal
        .creator_id
        .map(|id| {
            wallet
                .members
                .iter()
                .find(|member| id >= 0 && member.user_id == id)
                .ok_or_else(|| Error::invalid("INVALID_RESPONSE"))
        })
        .transpose()?;
    let by_address = proposal
        .creator_address
        .as_ref()
        .map(|address| {
            wallet
                .members
                .iter()
                .find(|member| &member.address == address)
                .ok_or_else(|| Error::invalid("INVALID_RESPONSE"))
        })
        .transpose()?;
    match (by_id, by_address) {
        (Some(by_id), Some(by_address)) if by_id.user_id != by_address.user_id || by_id.address != by_address.address => Err(Error::invalid("INVALID_RESPONSE")),
        (Some(member), _) | (_, Some(member)) => Ok(Some(member)),
        (None, None) => Ok(None),
    }
}

pub(crate) fn needs_configuration_review(status: &BsvHomeBsvMultisigConfigurationStatus) -> bool {
    matches!(
        status,
        BsvHomeBsvMultisigConfigurationStatus::NeedsConfirmation | BsvHomeBsvMultisigConfigurationStatus::NeedsLocalReview
    )
}

pub(crate) fn personal_pending(wallet: &RemoteWallet, proposals: &[RemoteProposal], address: &str, configuration: &BsvHomeBsvMultisigConfigurationStatus, now: u64) -> u32 {
    let member = wallet.members.iter().find(|m| m.address == address);
    let wallet_pending = u32::from(needs_configuration_review(configuration));
    wallet_pending
        + proposals
            .iter()
            .filter(|p| {
                p.wallet_id == wallet.id
                    && p.expires_at > 0
                    && proposal_pending(
                        &p.status,
                        p.expires_at as u64,
                        member.is_none_or(|m| p.signatures.iter().any(|s| s.public_key == m.public_key)),
                        now,
                    )
            })
            .count() as u32
}

pub(crate) fn proposal_pending(status: &str, expires_at: u64, signed: bool, now: u64) -> bool {
    expires_at > now
        && match status {
            "COLLECTING" => !signed,
            "READY" | "BROADCAST_FAILED" => true,
            _ => false,
        }
}

pub(crate) fn public_configuration(wallet: &RemoteWallet) -> Result<String, Error> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Backup {
        network: &'static str,
        r#type: &'static str,
        policy: Policy,
        locking_script: String,
        script_hash: String,
        configuration_hash: String,
    }
    let descriptor = policy_descriptor(&wallet_policy(wallet)?)?;
    Ok(serde_json::to_string(&Backup {
        network: "BSV_MAINNET",
        r#type: "P2MS",
        policy: descriptor.policy,
        locking_script: descriptor.locking_script,
        script_hash: descriptor.script_hash,
        configuration_hash: descriptor.configuration_hash,
    })?)
}
