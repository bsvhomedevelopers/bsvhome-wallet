use std::collections::{BTreeMap, HashMap};

use super::{BsvHomeBsvMultisigConfigurationStatus, BsvHomeBsvMultisigProposal, BsvHomeBsvMultisigWallet, BsvHomeBsvMultisigWalletStatus, merge_bsv_multisig_wallets, rules};

#[cfg(test)]
#[path = "overview_tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvMultisigIdentitySnapshot {
    pub signer_wallet_id: String,
    pub wallets: Vec<BsvHomeBsvMultisigWallet>,
    pub proposals: Vec<BsvHomeBsvMultisigProposal>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BsvHomeBsvMultisigOverview {
    pub wallets: Vec<BsvHomeBsvMultisigWallet>,
    pub pending_wallets: Vec<BsvHomeBsvMultisigWallet>,
    pub pending_proposals: Vec<BsvHomeBsvMultisigProposal>,
    pub pending_count: u64,
}

#[uniffi::export]
pub fn bsv_multisig_overview(
    identities: Vec<BsvHomeBsvMultisigIdentitySnapshot>,
    preferred_signers: HashMap<String, String>,
    fallback_signer: Option<String>,
) -> BsvHomeBsvMultisigOverview {
    overview_at(identities, preferred_signers, fallback_signer, super::service::now())
}

fn overview_at(
    identities: Vec<BsvHomeBsvMultisigIdentitySnapshot>,
    preferred_signers: HashMap<String, String>,
    fallback_signer: Option<String>,
    now: u64,
) -> BsvHomeBsvMultisigOverview {
    let candidates = identities.iter().flat_map(|identity| identity.wallets.clone()).fold(BTreeMap::new(), |mut groups, wallet| {
        groups.entry(wallet.id.clone()).or_insert_with(Vec::new).push(wallet);
        groups
    });
    let wallets = candidates
        .into_iter()
        .flat_map(|(id, wallets)| merge_bsv_multisig_wallets(wallets, preferred_signers.get(&id).cloned().or_else(|| fallback_signer.clone())))
        .collect::<Vec<_>>();
    let pending_wallets = wallets
        .iter()
        .filter(|wallet| {
            matches!(
                wallet.status,
                BsvHomeBsvMultisigWalletStatus::WaitingForMembers | BsvHomeBsvMultisigWalletStatus::WaitingForConfirmations
            ) || wallet.configuration_status == BsvHomeBsvMultisigConfigurationStatus::NeedsLocalReview
        })
        .cloned()
        .collect();
    let mut pending_proposals = Vec::new();
    for wallet in &wallets {
        let local_identities = identities
            .iter()
            .filter_map(|identity| {
                let candidate = identity
                    .wallets
                    .iter()
                    .find(|candidate| candidate.id == wallet.id && candidate.local_signer_wallet_id.as_ref() == Some(&identity.signer_wallet_id))?;
                let public_key = candidate.local_signer_public_key.as_deref()?;
                wallet
                    .members
                    .iter()
                    .any(|member| member.public_key.as_deref() == Some(public_key))
                    .then_some((identity, public_key))
            })
            .collect::<Vec<_>>();
        let candidates = local_identities
            .iter()
            .flat_map(|(identity, _)| {
                identity
                    .proposals
                    .iter()
                    .filter(|proposal| proposal.wallet_id == wallet.id)
                    .map(move |proposal| (&identity.signer_wallet_id, proposal))
            })
            .fold(BTreeMap::new(), |mut groups, proposal| {
                groups.entry(proposal.1.id.clone()).or_insert_with(Vec::new).push(proposal);
                groups
            });
        for proposals in candidates.into_values() {
            let Some(latest_version) = proposals.iter().map(|(_, proposal)| proposal.version).max() else {
                continue;
            };
            let latest = proposals.into_iter().filter(|(_, proposal)| proposal.version == latest_version).collect::<Vec<_>>();
            if let Some((_, proposal)) = latest
                .iter()
                .find(|(signer, _)| Some(*signer) == wallet.local_signer_wallet_id.as_ref())
                .or_else(|| latest.first())
                && local_identities.iter().any(|(_, public_key)| {
                    rules::proposal_pending(
                        &proposal.remote_status,
                        proposal.expires_at,
                        proposal.signed_members.iter().any(|signed| signed == public_key),
                        now,
                    )
                })
            {
                pending_proposals.push((*proposal).clone());
            }
        }
    }
    pending_proposals.sort_by(|left, right| right.created_at.cmp(&left.created_at).then_with(|| left.id.cmp(&right.id)));
    let pending_count = pending_proposals.len() as u64 + wallets.iter().filter(|wallet| rules::needs_configuration_review(&wallet.configuration_status)).count() as u64;
    BsvHomeBsvMultisigOverview {
        wallets,
        pending_wallets,
        pending_proposals,
        pending_count,
    }
}
