use futures::executor::block_on;

use super::*;
use crate::services::bsv_multisig::testkit::{Device, fixture, member, remote_proposal, remote_wallet};

fn snapshot(device: &Device) -> BsvHomeBsvMultisigIdentitySnapshot {
    BsvHomeBsvMultisigIdentitySnapshot {
        signer_wallet_id: device.wallet.id.id(),
        wallets: device.service.get_wallets().unwrap(),
        proposals: device.service.get_proposals(None).unwrap(),
    }
}

#[test]
fn global_pending_includes_another_unsigned_identity_without_switching_the_selected_signer() {
    block_on(async {
        let first = Device::new(1);
        let second = Device::new(2);
        let mut proposal = remote_proposal("pending", "COLLECTING");
        proposal.signatures = fixture()
            .signature_sets
            .into_iter()
            .filter(|signature| signature.public_key == member(1, true).public_key)
            .collect();
        for device in [&first, &second] {
            *device.provider.proposals.lock().unwrap() = vec![proposal.clone()];
            device.trust().await;
            device.service.load().await.unwrap();
        }
        let first = snapshot(&first);
        let second = snapshot(&second);
        assert!(!first.proposals[0].personal_pending);
        assert!(second.proposals[0].personal_pending);
        let selected = first.signer_wallet_id.clone();
        let overview = bsv_multisig_overview(vec![first.clone(), second], HashMap::new(), Some(selected.clone()));
        assert_eq!(overview.wallets[0].local_signer_wallet_id, Some(selected));
        assert_eq!(overview.pending_proposals, first.proposals);
        assert_eq!(overview.pending_count, 1);
        assert!(overview.pending_wallets.is_empty());
    });
}

#[test]
fn latest_proposal_uses_every_local_member_even_when_their_proposal_cache_is_stale_or_empty() {
    block_on(async {
        for has_stale_proposal in [false, true] {
            let first = Device::new(1);
            let second = Device::new(2);
            let stale = remote_proposal("pending", "COLLECTING");
            let mut latest = stale.clone();
            latest.version += 1;
            latest.signatures = fixture()
                .signature_sets
                .into_iter()
                .filter(|signature| signature.public_key == member(1, true).public_key)
                .collect();
            *first.provider.proposals.lock().unwrap() = vec![latest.clone()];
            *second.provider.proposals.lock().unwrap() = if has_stale_proposal { vec![stale] } else { Vec::new() };
            for device in [&first, &second] {
                device.trust().await;
                device.service.load().await.unwrap();
            }
            let selected = first.wallet.id.id();
            let first_snapshot = snapshot(&first);
            let second_snapshot = snapshot(&second);
            assert!(!first_snapshot.proposals[0].personal_pending);
            let overview = overview_at(vec![first_snapshot.clone(), second_snapshot.clone()], HashMap::new(), Some(selected.clone()), 1);
            assert_eq!(overview.wallets[0].local_signer_wallet_id, Some(selected.clone()));
            assert_eq!(overview.pending_proposals, first_snapshot.proposals);
            assert_eq!(overview.pending_count, 1);

            latest.version += 1;
            latest.signatures = fixture()
                .signature_sets
                .into_iter()
                .filter(|signature| signature.public_key == member(1, true).public_key || signature.public_key == member(2, true).public_key)
                .collect();
            *first.provider.proposals.lock().unwrap() = vec![latest];
            first.service.load().await.unwrap();
            let overview = overview_at(vec![snapshot(&first), second_snapshot], HashMap::new(), Some(selected), 1);
            assert!(overview.pending_proposals.is_empty());
            assert_eq!(overview.pending_count, 0);
        }
    });
}

#[test]
fn newer_closed_proposal_deduplicates_stale_pending_and_expired_or_unknown_records_do_not_add_tasks() {
    block_on(async {
        let device = Device::new(1);
        let mut expired = remote_proposal("expired", "COLLECTING");
        expired.expires_at = chrono::Utc::now().timestamp() - 1;
        let unknown = remote_proposal("unknown", "UNRECOGNIZED");
        *device.provider.proposals.lock().unwrap() = vec![remote_proposal("pending", "COLLECTING"), expired.clone(), unknown.clone()];
        device.trust().await;
        device.service.load().await.unwrap();
        let previous = snapshot(&device);
        let mut closed = remote_proposal("pending", "WITHDRAWN");
        closed.version += 1;
        *device.provider.proposals.lock().unwrap() = vec![closed, expired, unknown];
        device.service.load().await.unwrap();
        let current = snapshot(&device);
        let overview = bsv_multisig_overview(vec![previous, current.clone(), current], HashMap::new(), None);
        assert!(overview.pending_proposals.is_empty());
        assert_eq!(overview.pending_count, 0);
    });
}

#[test]
fn cached_pending_stops_at_the_actual_expiry_without_another_network_load() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.proposals.lock().unwrap() = vec![remote_proposal("pending", "COLLECTING")];
        device.trust().await;
        device.service.load().await.unwrap();
        let cached = snapshot(&device);
        assert!(cached.proposals[0].personal_pending);
        let expires_at = cached.proposals[0].expires_at;
        let before = overview_at(vec![cached.clone()], HashMap::new(), None, expires_at - 1);
        assert_eq!(before.pending_count, 1);
        let expired = overview_at(vec![cached], HashMap::new(), None, expires_at);
        assert!(expired.pending_proposals.is_empty());
        assert_eq!(expired.pending_count, 0);
    });
}

#[test]
fn incomplete_wallet_stays_in_pending_list_without_badge_and_local_configuration_review_counts_once() {
    block_on(async {
        let device = Device::new(1);
        let mut incomplete = remote_wallet("DRAFT", false);
        incomplete.members.truncate(1);
        incomplete.locking_script = None;
        incomplete.configuration_hash = None;
        incomplete.script_hash = None;
        *device.provider.wallets.lock().unwrap() = vec![incomplete];
        device.provider.proposals.lock().unwrap().clear();
        device.service.load().await.unwrap();
        let incomplete = snapshot(&device);
        assert!(!incomplete.wallets[0].personal_configuration_pending);
        assert_eq!(incomplete.wallets[0].pending_count, 0);
        let overview = bsv_multisig_overview(vec![incomplete.clone(), incomplete], HashMap::new(), None);
        assert_eq!(overview.pending_wallets.len(), 1);
        assert_eq!(overview.pending_count, 0);

        *device.provider.wallets.lock().unwrap() = vec![remote_wallet("ACTIVE", true)];
        device.service.load().await.unwrap();
        let review = snapshot(&device);
        assert!(review.wallets[0].personal_configuration_pending);
        assert_eq!(review.wallets[0].pending_count, 1);
        let overview = bsv_multisig_overview(vec![review.clone(), review], HashMap::new(), None);
        assert_eq!(overview.pending_wallets.len(), 1);
        assert_eq!(overview.pending_count, 1);
    });
}
