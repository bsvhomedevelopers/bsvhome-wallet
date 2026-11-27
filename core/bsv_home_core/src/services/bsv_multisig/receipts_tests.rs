use std::sync::Arc;

use futures::executor::block_on;
use serde::Serialize;

use super::{BsvHomeBsvMultisigActivityDirection, RemoteReceipt};
use crate::services::bsv_multisig::testkit::{Device, fixture, member, remote_proposal};

fn receipt(id: &str, confirmed: bool, timestamp: i64) -> RemoteReceipt {
    RemoteReceipt {
        txid: id.into(),
        satoshis: 42_000,
        confirmed,
        timestamp,
        fee: Some(200),
        sender_addresses: vec![member(2, true).address],
        sender_script_hashes: Vec::new(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    items: Vec<RemoteReceipt>,
    next_cursor: String,
}

#[test]
fn test_activity_merges_both_directions_excludes_change_and_keeps_public_detail_fields() {
    block_on(async {
        let device = Device::new(1);
        let mut outgoing = remote_proposal("sent", "CONFIRMED");
        outgoing.created_at = 100;
        *device.provider.proposals.lock().unwrap() = vec![outgoing];
        device.service.load().await.unwrap();
        let received_id = "a".repeat(64);
        device.provider.override_data(
            "/auth/multisig/wallets/team-wallet/receipts",
            vec![receipt(&fixture().signed_transaction_id, true, 120), receipt(&received_id, true, 200)],
        );
        let snapshot = device.service.refresh_activity("team-wallet".into()).await.unwrap();
        assert_eq!(snapshot.items.len(), 2);
        let incoming = &snapshot.items[0];
        assert_eq!(incoming.transaction_id, received_id);
        assert_eq!(incoming.direction, BsvHomeBsvMultisigActivityDirection::Received);
        assert_eq!(incoming.timestamp, Some(200));
        assert_eq!(incoming.fee_satoshis, Some("200".into()));
        assert_eq!(incoming.sender_addresses, vec![member(2, true).address]);
        assert_eq!(incoming.recipient_address, None);
        assert_eq!(incoming.recipient_script_hash, device.service.remote_wallet("team-wallet").unwrap().script_hash);
        let sent = &snapshot.items[1];
        assert_eq!(sent.direction, BsvHomeBsvMultisigActivityDirection::Sent);
        assert_eq!(sent.proposal_id, Some("sent".into()));
        assert_eq!(sent.memo, "Team payment");
        assert_eq!(sent.recipient_address, Some(fixture().proposal.recipient));
        assert!(!snapshot.is_stale);
        assert_eq!(snapshot.next_cursor, Some(String::new()));
        assert_eq!(device.provider.posts().len(), 0);
    });
}

#[test]
fn test_unbroadcast_proposal_transaction_ids_do_not_hide_real_receipts() {
    block_on(async {
        for status in ["COLLECTING", "READY", "BROADCAST_FAILED"] {
            let device = Device::new(1);
            let mut proposal = remote_proposal("not-broadcast", status);
            let transaction_id = "c".repeat(64);
            proposal.txid = Some(transaction_id.clone());
            *device.provider.proposals.lock().unwrap() = vec![proposal];
            device.service.load().await.unwrap();
            device
                .provider
                .override_data("/auth/multisig/wallets/team-wallet/receipts", vec![receipt(&transaction_id, true, 100)]);
            let activity = device.service.refresh_activity("team-wallet".into()).await.unwrap();
            assert_eq!(activity.items.len(), 1);
            assert_eq!(activity.items[0].transaction_id, transaction_id);
            assert_eq!(activity.items[0].direction, BsvHomeBsvMultisigActivityDirection::Received);
        }
    });
}

#[test]
fn test_receipt_pagination_deduplicates_but_rejects_cycles_without_publishing_the_bad_page() {
    block_on(async {
        let device = Device::new(1);
        device.service.load().await.unwrap();
        let one = receipt(&"a".repeat(64), true, 100);
        let two = receipt(&"b".repeat(64), true, 200);
        device.provider.override_data(
            "/auth/multisig/wallets/team-wallet/receipt-page?cursor=",
            Page {
                items: vec![one.clone()],
                next_cursor: "A&B /?".into(),
            },
        );
        let first = device.service.load_more_activity("team-wallet".into()).await.unwrap();
        assert_eq!(first.items.len(), 1);
        device.provider.override_data(
            "/auth/multisig/wallets/team-wallet/receipt-page?cursor=A%26B+%2F%3F",
            Page {
                items: vec![two, one],
                next_cursor: "again".into(),
            },
        );
        let second = device.service.load_more_activity("team-wallet".into()).await.unwrap();
        assert_eq!(second.items.len(), 2);
        device.provider.override_data(
            "/auth/multisig/wallets/team-wallet/receipt-page?cursor=again",
            Page {
                items: vec![receipt(&"c".repeat(64), true, 300)],
                next_cursor: "A&B /?".into(),
            },
        );
        assert!(device.service.load_more_activity("team-wallet".into()).await.is_err());
        let preserved = device.service.get_activity("team-wallet".into()).unwrap();
        assert_eq!(preserved.items, second.items);
        assert_eq!(preserved.next_cursor, second.next_cursor);
        assert!(preserved.is_stale);
        device.provider.override_data("/auth/multisig/wallets/team-wallet/receipts", Vec::<RemoteReceipt>::new());
        assert!(device.service.refresh_activity("team-wallet".into()).await.unwrap().is_stale);
    });
}

#[test]
fn test_receipts_reject_inconsistent_duplicates_and_never_downgrade_confirmed_state() {
    block_on(async {
        let device = Device::new(1);
        device.service.load().await.unwrap();
        let id = "a".repeat(64);
        device.provider.override_data("/auth/multisig/wallets/team-wallet/receipts", vec![receipt(&id, true, 100)]);
        let confirmed = device.service.refresh_activity("team-wallet".into()).await.unwrap();
        device.provider.override_data("/auth/multisig/wallets/team-wallet/receipts", vec![receipt(&id, false, 0)]);
        assert_eq!(device.service.refresh_activity("team-wallet".into()).await.unwrap().items, confirmed.items);
        let mut inconsistent = receipt(&id, true, 100);
        inconsistent.satoshis += 1;
        device.provider.override_data("/auth/multisig/wallets/team-wallet/receipts", vec![inconsistent]);
        assert!(device.service.refresh_activity("team-wallet".into()).await.is_err());
        assert_eq!(device.service.get_activity("team-wallet".into()).unwrap().items, confirmed.items);
    });
}

#[test]
fn test_receipt_validation_preserves_unknown_chain_time_for_pending_and_confirmed_receipts() {
    block_on(async {
        let device = Device::new(1);
        device.service.load().await.unwrap();
        let id = "a".repeat(64);
        device.provider.override_data("/auth/multisig/wallets/team-wallet/receipts", vec![receipt(&id, false, 0)]);
        let pending = device.service.refresh_activity("team-wallet".into()).await.unwrap();
        assert_eq!(pending.items[0].timestamp, None);
        assert_eq!(device.service.refresh_activity("team-wallet".into()).await.unwrap().items, pending.items);
        device.provider.override_data("/auth/multisig/wallets/team-wallet/receipts", vec![receipt(&id, true, 0)]);
        let confirmed = device.service.refresh_activity("team-wallet".into()).await.unwrap();
        assert_eq!(confirmed.items[0].timestamp, None);
        for invalid in [
            receipt("invalid", true, 100),
            RemoteReceipt {
                satoshis: 0,
                ..receipt(&id, true, 100)
            },
            RemoteReceipt {
                fee: Some(-1),
                ..receipt(&id, true, 100)
            },
        ] {
            device.provider.override_data("/auth/multisig/wallets/team-wallet/receipts", vec![invalid]);
            assert!(device.service.refresh_activity("team-wallet".into()).await.is_err());
            assert_eq!(device.service.get_activity("team-wallet".into()).unwrap().items, confirmed.items);
        }
        device.service.cancel();
        let before = device.provider.paths().len();
        assert!(device.service.refresh_activity("team-wallet".into()).await.is_err());
        assert_eq!(device.provider.paths().len(), before);
    });
}

#[test]
fn test_unknown_receipt_times_remain_private_and_preserve_first_seen_order_across_refresh() {
    block_on(async {
        let device = Device::new(1);
        device.service.load().await.unwrap();
        let first = receipt(&"a".repeat(64), false, 0);
        let second = receipt(&"b".repeat(64), false, 0);
        let chain = receipt(&"c".repeat(64), true, 150);
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/receipts", vec![first.clone(), second.clone(), chain.clone()]);
        device.service.refresh_activity("team-wallet".into()).await.unwrap();
        {
            let mut cache = device.service.cache().unwrap();
            let activity = cache.activity.get_mut("team-wallet").unwrap();
            activity.first_seen.insert(first.txid.clone(), 100);
            activity.first_seen.insert(second.txid.clone(), 200);
        }
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/receipts", vec![second.clone(), chain.clone(), first.clone()]);
        let refreshed = device.service.refresh_activity("team-wallet".into()).await.unwrap();
        assert_eq!(
            refreshed.items.iter().map(|item| item.transaction_id.clone()).collect::<Vec<_>>(),
            vec![second.txid.clone(), chain.txid.clone(), first.txid.clone()]
        );
        assert_eq!(refreshed.items.iter().map(|item| item.timestamp).collect::<Vec<_>>(), vec![None, Some(150), None]);
        assert_eq!(device.service.cache().unwrap().activity["team-wallet"].first_seen[&first.txid], 100);
        assert_eq!(device.service.cache().unwrap().activity["team-wallet"].first_seen[&second.txid], 200);
        assert_eq!(device.service.refresh_activity("team-wallet".into()).await.unwrap(), refreshed);

        let confirmed_unknown = RemoteReceipt {
            confirmed: true,
            ..second.clone()
        };
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/receipts", vec![first.clone(), confirmed_unknown.clone(), chain.clone()]);
        let unknown = device.service.refresh_activity("team-wallet".into()).await.unwrap();
        assert_eq!(unknown.items.last().unwrap().transaction_id, second.txid);
        assert_eq!(unknown.items.last().unwrap().timestamp, None);
        let confirmed_known = RemoteReceipt {
            timestamp: 300,
            ..confirmed_unknown
        };
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/receipts", vec![first.clone(), confirmed_known, chain.clone()]);
        let known = device.service.refresh_activity("team-wallet".into()).await.unwrap();
        assert_eq!(known.items[0].timestamp, Some(300));
        device.provider.override_data("/auth/multisig/wallets/team-wallet/receipts", vec![first, second, chain]);
        assert_eq!(device.service.refresh_activity("team-wallet".into()).await.unwrap(), known);
    });
}

#[test]
fn test_receipt_response_after_scope_cancellation_never_replaces_the_cached_activity() {
    block_on(async {
        let device = Device::new(1);
        device.service.load().await.unwrap();
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/receipts", vec![receipt(&"a".repeat(64), true, 100)]);
        device.service.refresh_activity("team-wallet".into()).await.unwrap();
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/receipts", vec![receipt(&"b".repeat(64), true, 200)]);
        let auth = device.auth.clone();
        *device.provider.on_request.lock().unwrap() = Some(Arc::new(move |_| auth.cancel()));
        assert!(device.service.refresh_activity("team-wallet".into()).await.is_err());
        let cache = device.service.cache().unwrap();
        assert_eq!(cache.activity["team-wallet"].recent.keys().cloned().collect::<Vec<_>>(), vec!["a".repeat(64)]);
    });
}
