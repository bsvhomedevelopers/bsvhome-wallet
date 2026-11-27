use crate::services::bsv_multisig::testkit::ScopedTestActions;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bsv_home_bsv::multisig::policy_descriptor;
use futures::{channel::oneshot, executor::block_on, join};
use primitives::{Account, Chain, Wallet, WalletId, WalletType};
use serde_json::{Value, from_value, json, to_value};

use super::model::{RemoteProposal, State};
use super::rules;
use super::testkit::{Device, TEST_WALLET_ID, fixture, member, remote_proposal, remote_wallet};
use super::{
    BsvHomeBsvMultisigConfigurationStatus as Configuration, BsvHomeBsvMultisigError as Error, BsvHomeBsvMultisigFundingStatus as FundingStatus,
    BsvHomeBsvMultisigProposalState as ProposalState, BsvHomeBsvMultisigService, BsvHomeBsvMultisigWallet, bsv_multisig_invitation_qr_payload, bsv_multisig_signing_wallets,
    merge_bsv_multisig_wallets, parse_bsv_multisig_invitation,
};

#[path = "keystore_tests.rs"]
mod keystore;

fn team() -> String {
    TEST_WALLET_ID.into()
}

async fn create(device: &Device, name: String, threshold: u32, member_names: Vec<String>) -> Result<BsvHomeBsvMultisigWallet, Error> {
    let draft = device.service.begin_creation(name, threshold, member_names, device.wallet.id.id()).await?;
    device.service.submit_creation(draft.id).await
}

#[test]
fn test_fixed_signer_loads_collective_wallets_without_changing_the_selected_wallet() {
    block_on(async {
        let device = Device::with_fixed_wallet(1);
        device.session.set_current_wallet_id(Some(Device::wallet(2).id)).unwrap();
        device.service.load().await.unwrap();
        let wallets = device.service.get_wallets().unwrap();
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].local_signer_wallet_id, Some(device.wallet.id.id()));
        let identity = device.service.current_signing_identity().await.unwrap();
        assert_eq!(identity.wallet_id, device.wallet.id.id());
        assert_eq!(identity.address, member(1, true).address);
        assert_eq!(device.session.current_wallet_id().unwrap(), Device::wallet(2).id);
        device.trust().await;
        assert!(device.service.get_wallet(team()).unwrap().can_transact);
    });
}

#[test]
fn test_fixed_signer_mutations_reject_deleted_changed_watch_only_and_cancelled_wallets() {
    block_on(async {
        for invalidation in ["deleted", "address", "watch", "cancelled"] {
            let device = Device::with_fixed_wallet(1);
            device.service.load().await.unwrap();
            device.trust().await;
            let requests = device.provider.paths();
            match invalidation {
                "deleted" => device.wallets.wallets.lock().unwrap().retain(|wallet| wallet.id != device.wallet.id),
                "address" => {
                    device
                        .wallets
                        .wallets
                        .lock()
                        .unwrap()
                        .iter_mut()
                        .find(|wallet| wallet.id == device.wallet.id)
                        .unwrap()
                        .accounts[0]
                        .address = member(2, true).address
                }
                "watch" => {
                    device
                        .wallets
                        .wallets
                        .lock()
                        .unwrap()
                        .iter_mut()
                        .find(|wallet| wallet.id == device.wallet.id)
                        .unwrap()
                        .wallet_type = WalletType::View;
                }
                "cancelled" => device.service.cancel(),
                _ => unreachable!(),
            }
            let expected = match invalidation {
                "watch" => Error::WatchOnly,
                "cancelled" => Error::Cancelled,
                _ => Error::WalletChanged,
            };
            assert_eq!(device.service.current_signing_identity().await.unwrap_err(), expected);
            assert_eq!(device.service.refresh_wallet(team()).await.unwrap_err(), expected);
            assert_eq!(device.service.approve_configuration(team(), 7).await.unwrap_err(), expected);
            if invalidation == "cancelled" {
                assert_eq!(device.service.get_wallets().unwrap_err(), Error::Cancelled);
            }
            assert_eq!(device.provider.paths(), requests);
            assert_eq!(*device.signer.authentications.lock().unwrap(), 0);
        }
    });
}

#[test]
fn test_fixed_signer_service_rejects_a_different_local_scope_before_cache_or_network() {
    block_on(async {
        let device = Device::with_fixed_wallet(1);
        let wrong_scope = BsvHomeBsvMultisigService::new(
            device.store.clone(),
            device.session.clone(),
            device.gateway.clone(),
            device.signer.clone(),
            device.auth.clone(),
            device.addresses.clone(),
            Device::wallet(2).id.id(),
            device.cold_store.clone(),
        );
        assert_eq!(wrong_scope.load().await, Err(Error::WalletChanged));
        assert_eq!(wrong_scope.get_wallets(), Err(Error::WalletChanged));
        assert_eq!(device.provider.paths(), Vec::<String>::new());
    });
}

#[test]
fn test_current_signer_scope_still_rejects_switching_the_selected_wallet() {
    block_on(async {
        let device = Device::new(1);
        device.service.load().await.unwrap();
        device.session.set_current_wallet_id(Some(Device::wallet(2).id)).unwrap();
        assert_eq!(device.service.get_wallets().unwrap_err(), Error::WalletChanged);
        assert_eq!(device.service.current_signing_identity().await.unwrap_err(), Error::WalletChanged);
    });
}

#[test]
fn test_signing_wallets_preserve_original_accounts_and_reject_invalid_bsv_records() {
    let first = Device::wallet(1);
    let mixed = Wallet {
        id: WalletId::Multicoin("mixed".into()),
        wallet_type: WalletType::Multicoin,
        accounts: vec![Account::mock(Chain::Ethereum, "0x1"), Account::mock(Chain::BitcoinSv, &member(2, true).address)],
        ..first.clone()
    };
    let watch = Wallet {
        wallet_type: WalletType::View,
        ..Device::wallet(3)
    };
    let unrelated = Wallet::mock_with_accounts(vec![Account::mock(Chain::Ethereum, "0x2")]);
    let eligible = bsv_multisig_signing_wallets(vec![watch, first.clone(), unrelated, mixed.clone()]).unwrap();
    assert_eq!(serde_json::to_value(eligible).unwrap(), serde_json::to_value([first.clone(), mixed]).unwrap());
    let invalid_address = Wallet {
        accounts: vec![Account::mock(Chain::BitcoinSv, "invalid")],
        ..first.clone()
    };
    let missing_account = Wallet {
        accounts: Vec::new(),
        ..first.clone()
    };
    let duplicate = Wallet {
        accounts: vec![first.accounts[0].clone(), first.accounts[0].clone()],
        ..first.clone()
    };
    let mismatched = Wallet {
        id: WalletId::Single(Chain::Ethereum, "0x3".into()),
        ..first
    };
    for invalid in [invalid_address, missing_account, duplicate, mismatched] {
        assert_eq!(bsv_multisig_signing_wallets(vec![Device::wallet(2), invalid]).unwrap_err(), Error::MissingSigner);
    }
}

#[test]
fn test_merge_collective_wallets_selects_one_complete_projection_with_stable_ties() {
    block_on(async {
        let device = Device::new(1);
        device.service.load().await.unwrap();
        device.trust().await;
        let first = device.service.get_wallet(team()).unwrap();
        let mut preferred = first.clone();
        preferred.local_signer_wallet_id = Some("preferred".into());
        preferred.configuration_status = Configuration::NeedsLocalReview;
        preferred.can_transact = false;
        preferred.pending_count = 9;
        preferred.personal_configuration_pending = true;
        let mut newest = first.clone();
        newest.version += 1;
        newest.local_signer_wallet_id = Some("newest".into());
        newest.can_invite = false;
        newest.pending_count = 2;
        let other = super::BsvHomeBsvMultisigWallet {
            id: "other".into(),
            ..first.clone()
        };
        assert_eq!(
            merge_bsv_multisig_wallets(vec![first.clone(), other.clone(), preferred.clone(), newest.clone()], Some("preferred".into())),
            [newest, other.clone()]
        );
        assert_eq!(
            merge_bsv_multisig_wallets(vec![first.clone(), other.clone(), preferred.clone(), first.clone()], Some("preferred".into())),
            [preferred.clone(), other.clone()]
        );
        assert_eq!(merge_bsv_multisig_wallets(vec![first.clone(), preferred.clone()], None), [first]);
        assert_eq!(
            merge_bsv_multisig_wallets(vec![preferred.clone(), other.clone(), preferred.clone()], Some("missing".into())),
            [preferred, other]
        );
    });
}

#[test]
fn test_load_reads_all_proposal_pages_and_preserves_last_balance_on_provider_failure() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.proposals.lock().unwrap() = (0..201).map(|index| remote_proposal(&format!("proposal-{index:03}"), "COLLECTING")).collect();
        device.service.load().await.unwrap();
        let proposals = device.service.get_proposals(Some(team())).unwrap();
        assert_eq!(proposals.len(), 201);
        assert_eq!(proposals.first().unwrap().id, "proposal-000");
        assert_eq!(proposals.last().unwrap().id, "proposal-200");
        assert_eq!(
            device.provider.paths(),
            [
                "/auth/multisig/wallets",
                "/auth/multisig/wallets/team-wallet/proposal-page?offset=0&limit=200",
                "/auth/multisig/wallets/team-wallet/proposal-page?offset=200&limit=200",
                "/auth/multisig/wallets/team-wallet/utxos",
            ]
        );
        assert_eq!(device.service.get_wallet(team()).unwrap().balance_satoshis, Some("100000".into()));
        *device.provider.fail_balances.lock().unwrap() = true;
        assert!(device.service.load().await.is_err());
        let wallet = device.service.get_wallet(team()).unwrap();
        assert_eq!(wallet.balance_satoshis, Some("100000".into()));
        assert!(wallet.balance_is_stale);
    });
}

#[test]
fn test_wallet_refresh_preserves_last_verified_balance_and_marks_all_failures_stale() {
    block_on(async {
        let device = Device::new(1);
        device.service.load().await.unwrap();
        let initial = device.service.get_wallet(team()).unwrap();
        assert!(!initial.balance_is_stale);
        *device.provider.fail_balances.lock().unwrap() = true;
        assert!(device.service.refresh_wallet(team()).await.is_err());
        let offline = device.service.get_wallet(team()).unwrap();
        assert_eq!(offline.balance_satoshis, initial.balance_satoshis);
        assert_eq!(offline.balance_updated_at, initial.balance_updated_at);
        assert!(offline.balance_is_stale);
        *device.provider.fail_balances.lock().unwrap() = false;
        assert!(!device.service.refresh_wallet(team()).await.unwrap().balance_is_stale);
        let input = fixture().proposal.inputs[0].clone();
        device.provider.override_data(
            "/auth/multisig/wallets/team-wallet/utxos",
            serde_json::json!([{
                "txid": super::testkit::transaction_id(&input.transaction_hex), "outputIndex": input.output_index,
                "satoshis": 30001, "transactionHex": input.transaction_hex,
            }]),
        );
        assert!(device.service.refresh_wallet(team()).await.is_err());
        let invalid = device.service.get_wallet(team()).unwrap();
        assert_eq!(invalid.balance_satoshis, initial.balance_satoshis);
        assert!(invalid.balance_is_stale);
        device.provider.overrides.lock().unwrap().clear();
        assert!(!device.service.refresh_wallet(team()).await.unwrap().balance_is_stale);
    });
}

#[test]
fn test_duplicate_pages_reject_incomplete_snapshot_and_keep_previous_cache() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.proposals.lock().unwrap() = vec![remote_proposal("original", "COLLECTING")];
        device.service.load().await.unwrap();
        *device.provider.proposals.lock().unwrap() = (0..201).map(|index| remote_proposal(&format!("proposal-{index}"), "COLLECTING")).collect();
        *device.provider.duplicate_second_page.lock().unwrap() = true;
        assert_eq!(device.service.load().await, Err(Error::ConcurrentUpdate));
        assert_eq!(
            device.service.get_proposals(None).unwrap().into_iter().map(|proposal| proposal.id).collect::<Vec<_>>(),
            ["original"]
        );
    });
}

#[test]
fn test_configuration_requires_device_trust_and_preserves_changed_policy() {
    block_on(async {
        let device = Device::new(1);
        device.service.load().await.unwrap();
        assert_eq!(device.service.get_wallet(team()).unwrap().configuration_status, Configuration::NeedsLocalReview);
        assert_eq!(device.service.receive_request(team()).unwrap_err(), Error::ConfigurationNotConfirmed);
        device.trust().await;
        assert_eq!(device.service.get_wallet(team()).unwrap().configuration_status, Configuration::Confirmed);
        assert!(device.service.get_wallet(team()).unwrap().can_transact);
        let configuration: Value = serde_json::from_str(&device.service.receive_request(team()).unwrap()).unwrap();
        assert_eq!(configuration["policy"], serde_json::to_value(fixture().proposal.policy).unwrap());
        assert_eq!(configuration.as_object().unwrap().len(), 6);

        let mut changed = remote_wallet("DRAFT", true);
        changed.threshold = 3;
        let mut policy = fixture().proposal.policy;
        policy.threshold = 3;
        let descriptor = policy_descriptor(&policy).unwrap();
        changed.locking_script = Some(descriptor.locking_script);
        changed.script_hash = Some(descriptor.script_hash);
        changed.configuration_hash = Some(descriptor.configuration_hash);
        *device.provider.wallets.lock().unwrap() = vec![changed];
        device.service.load().await.unwrap();
        assert_eq!(device.service.get_wallet(team()).unwrap().configuration_status, Configuration::Changed);
        assert!(!device.service.get_wallet(team()).unwrap().can_transact);
        assert_eq!(device.service.approve_configuration(team(), 7).await.unwrap_err(), Error::ConfigurationConflict);
        assert_eq!(device.store.read().trusted[&device.service.scope(TEST_WALLET_ID)], fixture().proposal.policy);
    });
}

#[test]
fn test_incomplete_unconfirmed_waiting_and_unknown_configurations_gate_actions() {
    block_on(async {
        let device = Device::new(1);
        let mut draft = remote_wallet("DRAFT", false);
        draft.members.truncate(1);
        draft.locking_script = None;
        draft.configuration_hash = None;
        draft.script_hash = None;
        *device.provider.wallets.lock().unwrap() = vec![draft];
        device.service.load().await.unwrap();
        assert_eq!(device.service.get_wallet(team()).unwrap().configuration_status, Configuration::Unavailable);
        assert!(!device.service.get_wallet(team()).unwrap().personal_configuration_pending);

        *device.provider.wallets.lock().unwrap() = vec![remote_wallet("DRAFT", false)];
        device.service.load().await.unwrap();
        assert_eq!(device.service.get_wallet(team()).unwrap().configuration_status, Configuration::NeedsConfirmation);
        let reviewed = device.service.approve_configuration(team(), 7).await.unwrap();
        assert_eq!(reviewed.configuration_status, Configuration::WaitingForMembers);
        assert!(!reviewed.can_transact);
        assert!(!reviewed.personal_configuration_pending);
        *device.provider.wallets.lock().unwrap() = vec![remote_wallet("UNRECOGNIZED", true)];
        device.service.load().await.unwrap();
        assert_eq!(device.service.get_wallet(team()).unwrap().configuration_status, Configuration::Unavailable);
        assert!(!device.service.get_wallet(team()).unwrap().can_transact);
    });
}

#[test]
fn test_configuration_approval_binds_reviewed_version_identity_and_committed_local_trust() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.wallets.lock().unwrap() = vec![remote_wallet("DRAFT", false)];
        device.service.load().await.unwrap();
        device.provider.wallets.lock().unwrap()[0].version += 1;
        assert_eq!(device.service.approve_configuration(team(), 7).await.unwrap_err(), Error::ConcurrentUpdate);
        assert_eq!(device.provider.posts().len(), 0);
        assert_eq!(*device.signer.authentications.lock().unwrap(), 0);
        device.provider.wallets.lock().unwrap()[0].version = 7;
        *device.store.fail_next_cas.lock().unwrap() = true;
        assert_eq!(device.service.approve_configuration(team(), 7).await.unwrap_err(), Error::ConcurrentUpdate);
        assert_eq!(device.store.read().trusted.len(), 0);
        device.service.load().await.unwrap();
        assert_eq!(device.service.get_wallet(team()).unwrap().configuration_status, Configuration::NeedsLocalReview);
        let reviewed = device.service.approve_configuration(team(), 7).await.unwrap();
        assert_eq!(reviewed.configuration_status, Configuration::WaitingForMembers);
        assert_eq!(device.store.read().trusted.len(), 1);

        let session = device.session.clone();
        *device.signer.on_authenticate.lock().unwrap() = Some(Box::new(move || session.set_current_wallet_id(Some(Device::wallet(2).id)).unwrap()));
        let before = device.provider.posts().len();
        assert_eq!(device.service.approve_configuration(team(), 7).await.unwrap_err(), Error::WalletChanged);
        assert_eq!(device.provider.posts().len(), before);
    });
}

#[test]
fn test_personal_pending_counts_unsigned_and_ready_without_broadcast_or_signed_items() {
    block_on(async {
        let device = Device::new(1);
        let unsigned = remote_proposal("unsigned", "COLLECTING");
        let mut signed = remote_proposal("signed", "COLLECTING");
        signed.signatures = fixture()
            .signature_sets
            .into_iter()
            .filter(|signature| signature.public_key == member(1, true).public_key)
            .collect();
        let mut expired = remote_proposal("expired", "COLLECTING");
        expired.expires_at = chrono::Utc::now().timestamp() - 1;
        *device.provider.proposals.lock().unwrap() = vec![
            unsigned,
            signed,
            remote_proposal("ready", "READY"),
            remote_proposal("broadcast", "BROADCAST"),
            expired,
            remote_proposal("unknown", "NEW_STATE"),
        ];
        device.trust().await;
        device.service.load().await.unwrap();
        assert_eq!(device.service.pending_count().unwrap(), 2);
        assert!(device.service.get_proposal("unsigned".into()).unwrap().can_sign);
        assert!(!device.service.get_proposal("signed".into()).unwrap().personal_pending);
        assert!(!device.service.get_proposal("signed".into()).unwrap().can_sign);
        let ready = device.service.get_proposal("ready".into()).unwrap();
        assert!(!ready.can_sign);
        assert!(ready.can_broadcast);
        assert!(!ready.can_withdraw);
        assert!(!device.service.get_proposal("broadcast".into()).unwrap().personal_pending);
        assert_eq!(device.service.get_proposal("expired".into()).unwrap().state, ProposalState::Expired);
        let unknown = device.service.get_proposal("unknown".into()).unwrap();
        assert_eq!(unknown.remote_status, "NEW_STATE");
        assert_eq!((unknown.can_sign, unknown.can_broadcast, unknown.can_withdraw), (false, false, false));
    });
}

#[test]
fn test_any_trusted_member_can_broadcast_fully_verified_fixture_and_confirmation_can_reorg() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.proposals.lock().unwrap() = vec![remote_proposal("ready", "READY")];
        device.trust().await;
        device.service.load().await.unwrap();
        let ready = device.service.get_proposal("ready".into()).unwrap();
        assert!(!ready.signed_members.iter().any(|key| key == &member(1, true).public_key));
        let result = device.service.scoped_broadcast_proposal("ready".into(), 3).await.unwrap();
        assert_eq!(result.state, ProposalState::PendingConfirmation);
        assert_eq!(result.transaction_id, Some(fixture().signed_transaction_id));
        assert_eq!(*device.provider.broadcasts.lock().unwrap(), [fixture().signed_transaction]);
        assert_eq!(device.service.pending_count().unwrap(), 0);
        {
            let mut proposals = device.provider.proposals.lock().unwrap();
            proposals[0].status = "CONFIRMED".into();
            proposals[0].confirmations = Some(2);
        }
        device.service.load().await.unwrap();
        assert_eq!(device.service.get_proposal("ready".into()).unwrap().state, ProposalState::Confirmed);
        let refreshed = device.service.refresh_proposal("ready".into()).await.unwrap();
        assert_eq!(refreshed.state, ProposalState::PendingConfirmation);
        assert_eq!(refreshed.confirmations, Some(0));
    });
}

#[test]
fn test_broadcast_and_confirmed_snapshots_reject_wrong_txid_or_insufficient_signatures() {
    block_on(async {
        for status in ["BROADCAST", "CONFIRMED"] {
            let device = Device::new(1);
            let original = remote_proposal("history", status);
            *device.provider.proposals.lock().unwrap() = vec![original.clone()];
            device.trust().await;
            device.service.load().await.unwrap();
            let before = device.service.get_proposal("history".into()).unwrap();
            for insufficient in [false, true] {
                let mut invalid = original.clone();
                invalid.version += 1;
                let expected = if insufficient {
                    invalid.signatures.truncate(1);
                    Error::invalid("INVALID_MEMBER_SIGNATURE")
                } else {
                    invalid.txid = Some("11".repeat(32));
                    Error::invalid("INVALID_BSV_TRANSACTION")
                };
                device.provider.override_data("/auth/multisig/proposals/history/refresh", invalid.clone());
                assert_eq!(device.service.refresh_proposal("history".into()).await.unwrap_err(), expected);
                let preserved = device.service.get_proposal("history".into()).unwrap();
                assert_eq!(preserved.transaction_id, before.transaction_id);
                assert_eq!(preserved.signed_members, before.signed_members);
                assert_eq!(preserved.version, before.version);
                assert_eq!(preserved.state, before.state);
                device.provider.overrides.lock().unwrap().clear();
                *device.provider.proposals.lock().unwrap() = vec![invalid];
                assert_eq!(device.service.load().await, Err(expected));
                assert_eq!(device.service.get_proposal("history".into()).unwrap().transaction_id, before.transaction_id);
            }
        }
    });
}

#[test]
fn test_only_proposal_creator_can_withdraw_collecting_and_ready_cannot_withdraw() {
    block_on(async {
        let other = Device::new(2);
        *other.provider.proposals.lock().unwrap() = vec![remote_proposal("other", "COLLECTING")];
        other.trust().await;
        other.service.load().await.unwrap();
        assert_eq!(
            other.service.scoped_withdraw_proposal("other".into(), 3).await.unwrap_err(),
            Error::invalid("CREATOR_REQUIRED")
        );
        assert_eq!(other.provider.posts().len(), 0);
        let creator = Device::new(1);
        *creator.provider.proposals.lock().unwrap() = vec![remote_proposal("collecting", "COLLECTING"), remote_proposal("ready", "READY")];
        creator.trust().await;
        creator.service.load().await.unwrap();
        assert_eq!(
            creator.service.scoped_withdraw_proposal("ready".into(), 3).await.unwrap_err(),
            Error::invalid("CREATOR_REQUIRED")
        );
        let withdrawn = creator.service.scoped_withdraw_proposal("collecting".into(), 3).await.unwrap();
        assert_eq!(withdrawn.state, ProposalState::Stopped);
        assert_eq!((withdrawn.can_sign, withdrawn.can_broadcast, withdrawn.can_withdraw), (false, false, false));
    });
}

#[test]
fn test_prepare_persists_original_intent_and_retries_same_key_after_unknown_commit() {
    block_on(async {
        let device = Device::new(1);
        device.trust().await;
        device.service.load().await.unwrap();
        *device.provider.fail_prepare_after_commit.lock().unwrap() = true;
        let recipient = fixture().proposal.recipient;
        assert!(
            device
                .service
                .scoped_prepare_proposal(team(), recipient.clone(), "0.00042".into(), "Team payment".into())
                .await
                .is_err()
        );
        let intent = device.store.read().intentions[&device.service.scope(TEST_WALLET_ID)].clone();
        assert_eq!(intent.recipient, recipient);
        assert_eq!(intent.amount, 42000);
        assert_eq!(device.provider.proposals.lock().unwrap().len(), 1);
        let before = device.provider.posts().len();
        assert_eq!(
            device
                .service
                .scoped_prepare_proposal(team(), recipient.clone(), "0.00043".into(), "Team payment".into())
                .await
                .unwrap_err(),
            Error::invalid("IDEMPOTENCY_CONFLICT")
        );
        assert_eq!(device.provider.posts().len(), before);
        let proposal = device
            .service
            .scoped_prepare_proposal(team(), recipient, "0.00042".into(), "Team payment".into())
            .await
            .unwrap();
        assert_eq!(proposal.id, "prepared-proposal");
        assert_eq!(proposal.signed_members.len(), 0);
        assert_eq!(*device.signer.proposal_signatures.lock().unwrap(), 0);
        assert_eq!(device.provider.proposals.lock().unwrap().len(), 1);
        let bodies = device
            .provider
            .posts()
            .into_iter()
            .map(|request| serde_json::from_slice::<Value>(&request.body.unwrap()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(bodies.len(), 2);
        assert_eq!(bodies[0]["idempotencyKey"], bodies[1]["idempotencyKey"]);
        assert_eq!(bodies[1]["idempotencyKey"], intent.key);
        assert_eq!(device.store.read().intentions.len(), 0);
    });
}

#[test]
fn test_prepare_definite_first_rejection_releases_intent_for_corrected_amount() {
    block_on(async {
        let device = Device::new(1);
        device.trust().await;
        device.service.load().await.unwrap();
        device.provider.override_error("/auth/multisig/wallets/team-wallet/proposals/prepare", "INSUFFICIENT_FUNDS");
        assert_eq!(
            device
                .service
                .scoped_prepare_proposal(team(), fixture().proposal.recipient, "0.00100".into(), "Team payment".into())
                .await
                .unwrap_err(),
            Error::Api {
                code: "INSUFFICIENT_FUNDS".into()
            }
        );
        assert!(device.store.read().intentions.is_empty());
        assert!(device.provider.proposals.lock().unwrap().is_empty());
        device.provider.overrides.lock().unwrap().clear();
        let corrected = device
            .service
            .scoped_prepare_proposal(team(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into())
            .await
            .unwrap();
        assert_eq!(corrected.value_satoshis, "42000");
        let bodies = device
            .provider
            .posts()
            .into_iter()
            .map(|request| serde_json::from_slice::<Value>(&request.body.unwrap()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(bodies.len(), 2);
        assert_eq!(bodies[0]["amount"], 100000);
        assert_eq!(bodies[1]["amount"], 42000);
        assert_ne!(bodies[0]["idempotencyKey"], bodies[1]["idempotencyKey"]);
        assert!(device.store.read().intentions.is_empty());
    });
}

#[test]
fn test_prepare_prior_unknown_commit_survives_later_business_rejection_with_same_key() {
    block_on(async {
        let device = Device::new(1);
        device.trust().await;
        device.service.load().await.unwrap();
        *device.provider.fail_prepare_after_commit.lock().unwrap() = true;
        assert!(
            device
                .service
                .scoped_prepare_proposal(team(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into())
                .await
                .is_err()
        );
        let scope = device.service.scope(TEST_WALLET_ID);
        let original = device.store.read().intentions[&scope].clone();
        device.provider.override_error("/auth/multisig/wallets/team-wallet/proposals/prepare", "INSUFFICIENT_FUNDS");
        assert_eq!(
            device
                .service
                .scoped_prepare_proposal(team(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into())
                .await
                .unwrap_err(),
            Error::Api {
                code: "INSUFFICIENT_FUNDS".into()
            }
        );
        assert_eq!(device.store.read().intentions[&scope].key, original.key);
        let count = device.provider.posts().len();
        assert_eq!(
            device
                .service
                .scoped_prepare_proposal(team(), fixture().proposal.recipient, "0.00043".into(), "Team payment".into())
                .await
                .unwrap_err(),
            Error::invalid("IDEMPOTENCY_CONFLICT")
        );
        assert_eq!(device.provider.posts().len(), count);
        device.provider.overrides.lock().unwrap().clear();
        device
            .service
            .scoped_prepare_proposal(team(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into())
            .await
            .unwrap();
        let bodies = device
            .provider
            .posts()
            .into_iter()
            .map(|request| serde_json::from_slice::<Value>(&request.body.unwrap()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(bodies.len(), 3);
        assert!(bodies.iter().all(|body| body["idempotencyKey"] == original.key));
        assert_eq!(device.provider.proposals.lock().unwrap().len(), 1);
        assert!(device.store.read().intentions.is_empty());
    });
}

#[test]
fn test_prepare_storage_failure_prevents_server_mutation() {
    block_on(async {
        let device = Device::new(1);
        device.trust().await;
        device.service.load().await.unwrap();
        *device.store.fail_next_cas.lock().unwrap() = true;
        assert_eq!(
            device
                .service
                .scoped_prepare_proposal(team(), fixture().proposal.recipient, "0.00042".into(), "Team payment".into())
                .await
                .unwrap_err(),
            Error::ConcurrentUpdate
        );
        assert_eq!(device.provider.posts().len(), 0);
        assert_eq!(device.store.read().intentions.len(), 0);
    });
}

#[test]
fn test_sign_proposal_rechecks_version_before_key_access_and_never_uploads_stale_signature() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.proposals.lock().unwrap() = vec![remote_proposal("collecting", "COLLECTING")];
        device.trust().await;
        device.service.load().await.unwrap();
        device.provider.proposals.lock().unwrap()[0].version += 1;
        assert_eq!(device.service.scoped_sign_proposal("collecting".into(), 3).await.unwrap_err(), Error::ConcurrentUpdate);
        assert_eq!(*device.signer.proposal_signatures.lock().unwrap(), 0);
        assert_eq!(device.provider.posts().len(), 0);
        device.provider.proposals.lock().unwrap()[0].version = 3;
        let provider = device.provider.clone();
        *device.signer.on_sign.lock().unwrap() = Some(Box::new(move || provider.proposals.lock().unwrap()[0].version += 1));
        assert_eq!(
            device.service.scoped_sign_proposal("collecting".into(), 3).await.unwrap_err(),
            Error::Api { code: "STALE_VERSION".into() }
        );
        assert_eq!(device.provider.proposals.lock().unwrap()[0].signatures.len(), 0);
        assert_eq!(device.provider.posts().len(), 1);
    });
}

#[test]
fn test_wallet_switch_during_signing_discards_signature_before_upload() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.proposals.lock().unwrap() = vec![remote_proposal("collecting", "COLLECTING")];
        device.trust().await;
        device.service.load().await.unwrap();
        let (started, observed) = oneshot::channel();
        let (release, waiting) = oneshot::channel();
        *device.signer.sign_pause.lock().unwrap() = Some((started, waiting));
        let operation = device.service.scoped_sign_proposal("collecting".into(), 3);
        let switch = async {
            observed.await.unwrap();
            device.session.set_current_wallet_id(Some(Device::wallet(2).id)).unwrap();
            release.send(()).unwrap();
        };
        let (result, ()) = join!(operation, switch);
        assert_eq!(result.unwrap_err(), Error::Cancelled);
        assert_eq!(device.provider.posts().len(), 0);
        assert_eq!(*device.signer.proposal_signatures.lock().unwrap(), 0);
    });
}

#[test]
fn test_successful_signing_and_mutated_response_rejection_preserve_transaction_intent() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.proposals.lock().unwrap() = vec![remote_proposal("collecting", "COLLECTING")];
        device.trust().await;
        device.service.load().await.unwrap();
        let signed = device.service.scoped_sign_proposal("collecting".into(), 3).await.unwrap();
        assert_eq!(signed.signed_members, [member(1, true).public_key]);
        assert!(!signed.can_sign);
        let mut response = device.provider.proposals.lock().unwrap()[0].clone();
        response.proposal.amount += 1;
        device.provider.override_data("/auth/multisig/proposals/collecting/refresh", response);
        assert!(device.service.refresh_proposal("collecting".into()).await.is_err());
        assert_eq!(device.service.get_proposal("collecting".into()).unwrap().value_satoshis, "42000");
    });
}

#[test]
fn test_create_five_members_keeps_names_local_and_creator_unconfirmed() {
    block_on(async {
        let device = Device::new(1);
        let names = ["Me", "Alice", "Bob", "Carol", "Dave"].map(String::from).to_vec();
        let created = create(&device, " Five members ".into(), 3, names.clone()).await.unwrap();
        assert_eq!(created.member_count, 5);
        assert_eq!(created.threshold, 3);
        assert!(!created.personal_configuration_pending);
        assert_eq!(created.configuration_status, Configuration::Unavailable);
        assert!(created.can_invite);
        assert_eq!(device.store.read().member_names[&device.service.scope(&created.id)], names);
        let posts = device.provider.posts();
        assert_eq!(posts.len(), 1);
        let request: Value = serde_json::from_slice(posts[0].body.as_ref().unwrap()).unwrap();
        assert_eq!(request["memberCount"], 5);
        assert_eq!(request["threshold"], 3);
        assert_eq!(request["name"], "Five members");
        assert_eq!(request["publicKey"], member(1, false).public_key);
        assert_eq!(request.as_object().unwrap().len(), 4);
        assert_eq!(created.members[0].status, super::BsvHomeBsvMultisigMemberStatus::AwaitingApproval);
        let invalid = Device::new(1);
        assert_eq!(
            invalid
                .service
                .begin_creation("Too many".into(), 2, vec!["Member".into(); 6], invalid.wallet.id.id())
                .await
                .unwrap_err(),
            Error::InvalidRules
        );
        assert_eq!(invalid.provider.paths().len(), 0);
    });
}

#[test]
fn test_create_server_success_then_local_cas_failure_recovers_member_notes_without_second_post() {
    block_on(async {
        let device = Device::new(1);
        let names = vec!["Creator".into(), "Design".into(), "Finance".into(), "Operations".into(), "Recovery".into()];
        let draft = device
            .service
            .begin_creation("Recovery wallet".into(), 3, names.clone(), device.wallet.id.id())
            .await
            .unwrap();
        let store = device.store.clone();
        *device.provider.on_create.lock().unwrap() = Some(Box::new(move || *store.fail_next_cas.lock().unwrap() = true));
        assert_eq!(device.service.submit_creation(draft.id.clone()).await.unwrap_err(), Error::ConcurrentUpdate);
        assert!(device.store.read().member_names.is_empty());
        assert_eq!(device.provider.posts().len(), 1);
        let candidates = device.service.reconcile_creation(draft.id.clone()).await.unwrap();
        assert_eq!(candidates.len(), 1);
        let recovered = device.service.accept_creation(draft.id, candidates[0].id.clone()).await.unwrap();
        assert_eq!(recovered.members.iter().map(|member| member.name.clone()).collect::<Vec<_>>(), names);
        assert_eq!(device.store.read().member_names[&device.service.scope(&recovered.id)], names);
        assert_eq!(device.provider.posts().len(), 1);
        assert_eq!(device.provider.wallets.lock().unwrap().iter().filter(|wallet| wallet.name == "Recovery wallet").count(), 1);
    });
}

#[test]
fn test_only_draft_creator_with_available_slots_can_regenerate_invitation() {
    block_on(async {
        let creator = Device::new(1);
        let mut draft = remote_wallet("DRAFT", false);
        draft.members.truncate(2);
        draft.locking_script = None;
        draft.script_hash = None;
        draft.configuration_hash = None;
        *creator.provider.wallets.lock().unwrap() = vec![draft.clone()];
        let first = creator.service.invitation(team()).await.unwrap();
        let second = creator.service.invitation(team()).await.unwrap();
        assert_ne!(first.token, second.token);
        assert_eq!(first.token.len(), 43);
        assert!(second.expires_at > chrono::Utc::now().timestamp() as u64);
        let bodies = creator
            .provider
            .posts()
            .into_iter()
            .map(|target| serde_json::from_slice::<Value>(&target.body.unwrap()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(bodies[0]["expectedVersion"], 7);
        assert_eq!(bodies[1]["expectedVersion"], 8);
        let member_device = Device::new(2);
        *member_device.provider.wallets.lock().unwrap() = vec![draft];
        assert_eq!(member_device.service.invitation(team()).await.unwrap_err(), Error::invalid("CREATOR_REQUIRED"));
        assert_eq!(member_device.provider.posts().len(), 0);
        assert_eq!(*member_device.signer.authentications.lock().unwrap(), 0);
        *creator.provider.wallets.lock().unwrap() = vec![remote_wallet("DRAFT", false)];
        let before = creator.provider.posts().len();
        assert_eq!(creator.service.invitation(team()).await.unwrap_err(), Error::invalid("CREATOR_REQUIRED"));
        assert_eq!(creator.provider.posts().len(), before);
    });
}

fn incoming_wallet(device: &Device) {
    let mut incoming = remote_wallet("DRAFT", false);
    incoming.id = "invited-wallet".into();
    incoming.members = vec![member(2, false)];
    incoming.locking_script = None;
    incoming.script_hash = None;
    incoming.configuration_hash = None;
    device.provider.wallets.lock().unwrap().push(incoming);
}

#[test]
fn test_phase5_create_rejects_unreviewed_additional_members() {
    block_on(async {
        let device = Device::new(1);
        let mut forged = remote_wallet("DRAFT", false);
        forged.id = "forged-created-wallet".into();
        forged.name = "New collective".into();
        forged.members.truncate(2);
        forged.locking_script = None;
        forged.script_hash = None;
        forged.configuration_hash = None;
        device.provider.override_post_data("/auth/multisig/wallets", forged);
        assert_eq!(
            create(&device, "New collective".into(), 2, vec!["Me".into(), "Alice".into(), "Bob".into()])
                .await
                .unwrap_err(),
            Error::MissingSigner
        );
        assert_eq!(device.store.read().member_names.len(), 0);
        assert_eq!(device.service.get_wallet("forged-created-wallet".into()).unwrap_err(), Error::MissingRecord);
    });
}

#[test]
fn test_phase5_join_rejects_unreviewed_additional_members() {
    block_on(async {
        let device = Device::new(1);
        incoming_wallet(&device);
        let preview = device.service.preview_invitation(device.provider.invitation_token()).await.unwrap();
        let mut forged = remote_wallet("DRAFT", false);
        forged.id = "invited-wallet".into();
        forged.version = 8;
        device.provider.override_post_data("/auth/multisig/join", forged);
        assert_eq!(device.service.join_invitation(preview).await.unwrap_err(), Error::MissingSigner);
        assert_eq!(device.service.get_wallet("invited-wallet".into()).unwrap_err(), Error::MissingRecord);
    });
}

#[test]
fn test_phase5_join_preserves_reviewed_member_confirmation() {
    block_on(async {
        let device = Device::new(1);
        incoming_wallet(&device);
        let preview = device.service.preview_invitation(device.provider.invitation_token()).await.unwrap();
        let mut forged = device.provider.wallets.lock().unwrap().last().unwrap().clone();
        forged.members[0].confirmed = true;
        forged.members.push(member(1, false));
        forged.version += 1;
        device.provider.override_post_data("/auth/multisig/join", forged);
        assert_eq!(device.service.join_invitation(preview).await.unwrap_err(), Error::MissingSigner);
    });
}

#[test]
fn test_phase5_configuration_approval_reads_fresh_wallet_without_config_endpoint() {
    block_on(async {
        let device = Device::new(1);
        *device.provider.wallets.lock().unwrap() = vec![remote_wallet("DRAFT", false)];
        device.service.load().await.unwrap();
        device.provider.override_error("/auth/multisig/wallets/team-wallet/config", "UNSUPPORTED_ENDPOINT");
        let before = device.provider.paths().len();
        let reviewed = device.service.approve_configuration(team(), 7).await.unwrap();
        assert_eq!(reviewed.configuration_status, Configuration::WaitingForMembers);
        assert_eq!(
            &device.provider.paths()[before..],
            ["/auth/multisig/wallets/team-wallet", "/auth/multisig/wallets/team-wallet/confirm"]
        );
        assert_eq!(*device.signer.authentications.lock().unwrap(), 1);
        assert_eq!(device.store.read().trusted.len(), 1);
    });
}

#[test]
fn test_phase5_invitation_uri_previews_the_same_token_without_joining() {
    block_on(async {
        let device = Device::new(1);
        incoming_wallet(&device);
        let token = device.provider.invitation_token();
        let preview = device.service.preview_invitation(format!("bsvhome://multisig/join?token={token}")).await.unwrap();
        assert_eq!(preview.token, token);
        assert_eq!(preview.wallet.id, "invited-wallet");
        assert_eq!(device.provider.posts().len(), 1);
        assert_eq!(device.provider.wallets.lock().unwrap().last().unwrap().members.len(), 1);
    });
}

#[test]
fn test_phase5_invitation_payload_roundtrips_only_the_exact_public_protocol() {
    let token = format!("{}_-Z", "a".repeat(40));
    let payload = bsv_multisig_invitation_qr_payload(token.clone()).unwrap();
    assert_eq!(payload, format!("bsvhome://multisig/join?token={token}"));
    assert_eq!(parse_bsv_multisig_invitation(format!("  {token}\n")).unwrap(), token);
    assert_eq!(parse_bsv_multisig_invitation(format!("\n{payload}  ")).unwrap(), token);
    for invalid in [
        String::new(),
        "A".repeat(42),
        "A".repeat(44),
        "é".repeat(43),
        format!("https://multisig/join?token={token}"),
        format!("BSVHOME://multisig/join?token={token}"),
        format!("bsvhome://foreign/join?token={token}"),
        format!("bsvhome://user@multisig/join?token={token}"),
        format!("bsvhome://multisig:443/join?token={token}"),
        format!("bsvhome://multisig/join/?token={token}"),
        format!("bsvhome://multisig/a/../join?token={token}"),
        format!("bsvhome://multisig/join?token={token}#fragment"),
        format!("bsvhome://multisig/join?token={token}&extra=1"),
        format!("bsvhome://multisig/join?extra=1&token={token}"),
        format!("bsvhome://multisig/join?token={token}&token={token}"),
        format!("bsvhome://multisig/join?token=%61{}", "a".repeat(42)),
    ] {
        assert_eq!(parse_bsv_multisig_invitation(invalid), Err(Error::invalid("INVALID_INVITE")));
    }
    for invalid in [payload, format!(" {token}"), "A".repeat(42)] {
        assert_eq!(bsv_multisig_invitation_qr_payload(invalid), Err(Error::invalid("INVALID_INVITE")));
    }
}

#[test]
fn test_phase5_creation_limits_count_utf16_units() {
    block_on(async {
        for (name, label, allowed) in [
            ("😀".repeat(15), "Me".into(), true),
            ("😀".repeat(16), "Me".into(), false),
            ("Collective".into(), "😀".repeat(10), true),
            ("Collective".into(), "😀".repeat(11), false),
        ] {
            let device = Device::new(1);
            let result = create(&device, name.clone(), 2, vec![label.clone(), "Alice".into()]).await;
            if allowed {
                let wallet = result.unwrap();
                assert_eq!(wallet.name, name);
                assert_eq!(wallet.members[0].name, label);
            } else {
                assert_eq!(result.unwrap_err(), Error::invalid("INVALID_REQUEST"));
                assert_eq!(device.provider.paths(), Vec::<String>::new());
            }
        }
    });
}

#[test]
fn test_invitation_preview_does_not_join_and_join_sends_reviewed_wallet_and_version() {
    block_on(async {
        let device = Device::new(1);
        incoming_wallet(&device);
        let token = device.provider.invitation_token();
        let preview = device.service.preview_invitation(token.clone()).await.unwrap();
        assert_eq!(preview.wallet.id, "invited-wallet");
        assert_eq!(preview.signing_identity.public_key, member(1, false).public_key);
        assert_eq!(device.provider.wallets.lock().unwrap().last().unwrap().members.len(), 1);
        let joined = device.service.join_invitation(preview).await.unwrap();
        assert_eq!(joined.id, "invited-wallet");
        assert!(!joined.personal_configuration_pending);
        let posts = device.provider.posts();
        assert_eq!(posts.len(), 2);
        let body: Value = serde_json::from_slice(posts[1].body.as_ref().unwrap()).unwrap();
        assert_eq!(body["token"], token);
        assert_eq!(body["walletId"], "invited-wallet");
        assert_eq!(body["expectedVersion"], 7);
        assert_eq!(body["publicKey"], member(1, false).public_key);
    });
}

#[test]
fn test_stale_invitation_review_cannot_join_again_without_fresh_preview() {
    block_on(async {
        let device = Device::new(1);
        incoming_wallet(&device);
        for invalid in [String::new(), "A".repeat(42), format!("{}?", "A".repeat(42))] {
            assert_eq!(device.service.preview_invitation(invalid).await.unwrap_err(), Error::invalid("INVALID_INVITE"));
        }
        assert_eq!(device.provider.paths().len(), 0);
        let preview = device.service.preview_invitation(device.provider.invitation_token()).await.unwrap();
        device.provider.wallets.lock().unwrap().last_mut().unwrap().version += 1;
        assert_eq!(
            device.service.join_invitation(preview.clone()).await.unwrap_err(),
            Error::Api { code: "STALE_VERSION".into() }
        );
        let before = device.provider.posts().len();
        assert_eq!(device.service.join_invitation(preview).await.unwrap_err(), Error::invalid("INVALID_INVITE_REVIEW"));
        assert_eq!(device.provider.posts().len(), before);
    });
}

async fn funding_device() -> Device {
    let device = Device::new(1);
    device.provider.seed_funding(2);
    device.trust().await;
    device.service.load().await.unwrap();
    device
}

fn restarted_service(device: &Device) -> Arc<BsvHomeBsvMultisigService> {
    Arc::new(BsvHomeBsvMultisigService::new(
        device.store.clone(),
        device.session.clone(),
        device.gateway.clone(),
        device.signer.clone(),
        device.auth.clone(),
        device.addresses.clone(),
        device.wallet.id.id(),
        device.cold_store.clone(),
    ))
}

#[test]
fn test_funding_selects_signing_payer_without_changing_member_identity_and_excludes_watch_wallets() {
    block_on(async {
        let device = funding_device().await;
        device.wallets.wallets.lock().unwrap()[1].wallet_type = WalletType::View;
        assert_eq!(
            device.service.funding_payers().await.unwrap().into_iter().map(|payer| payer.wallet_id).collect::<Vec<_>>(),
            [Device::wallet(1).id.id(), Device::wallet(3).id.id()]
        );
        assert_eq!(
            device.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap_err(),
            Error::WatchOnly
        );
        device.wallets.wallets.lock().unwrap()[1].wallet_type = WalletType::PrivateKey;
        let prepared = device.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        assert_eq!(prepared.payer_wallet_id, Device::wallet(2).id.id());
        assert_eq!(prepared.sender_address, member(2, true).address);
        assert_eq!(prepared.amount_satoshis, "100000");
        assert_eq!(
            match &prepared.target {
                super::BsvHomeBsvMultisigFundingPreparationTarget::Member { members, .. } => members.len(),
                super::BsvHomeBsvMultisigFundingPreparationTarget::Public { .. } => panic!("member preparation must retain its actual members"),
            },
            3
        );
        assert_eq!(device.session.current_wallet_id().unwrap().id(), device.wallet.id.id());
        let result = device.service.scoped_submit_funding(prepared.id).await.unwrap();
        assert_eq!(result.status, FundingStatus::Broadcast);
        assert_eq!(result.payer_wallet_id, Device::wallet(2).id.id());
        assert_eq!(device.session.current_wallet_id().unwrap().id(), device.wallet.id.id());
        assert_eq!(device.service.get_wallet(team()).unwrap().balance_satoshis, Some("200000".into()));
        assert_eq!(device.service.get_proposals(None).unwrap().len(), 0);
    });
}

#[test]
fn test_funding_review_expiry_and_storage_failure_prevent_broadcast() {
    block_on(async {
        let device = funding_device().await;
        let prepared = device.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        device.service.cache().unwrap().preparations.get_mut(&prepared.id).unwrap().created = Instant::now() - Duration::from_secs(301);
        assert_eq!(device.service.scoped_submit_funding(prepared.id).await.unwrap_err(), Error::FundingExpired);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
        let prepared = device.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        *device.store.fail_next_cas.lock().unwrap() = true;
        assert_eq!(device.service.scoped_submit_funding(prepared.id).await.unwrap_err(), Error::ConcurrentUpdate);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
        assert_eq!(device.store.read().funding.len(), 0);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_funding_payer_removed_during_authentication_blocks_signed_transaction_before_broadcast() {
    block_on(async {
        let device = funding_device().await;
        let prepared = device.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        let wallets = device.wallets.clone();
        *device.signer.on_sign.lock().unwrap() = Some(Box::new(move || wallets.wallets.lock().unwrap().retain(|wallet| wallet.id != Device::wallet(2).id)));
        assert_eq!(
            device.service.scoped_submit_funding(prepared.id).await.unwrap_err(),
            Error::Authentication {
                reason: "BSV wallet not found".into()
            }
        );
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 0);
    });
}

#[test]
fn test_funding_configuration_change_during_signing_blocks_gateway_and_preserves_original_checkpoint() {
    block_on(async {
        for replace_member in [false, true] {
            let device = funding_device().await;
            let prepared = device.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
            let mut changed = remote_wallet("ACTIVE", true);
            if replace_member {
                changed.members[2] = member(4, true);
            } else {
                changed.threshold = 3;
            }
            let descriptor = policy_descriptor(&bsv_home_bsv::multisig::Policy {
                threshold: changed.threshold,
                public_keys: changed.members.iter().map(|member| member.public_key.clone()).collect(),
            })
            .unwrap();
            changed.locking_script = Some(descriptor.locking_script);
            changed.script_hash = Some(descriptor.script_hash);
            changed.configuration_hash = Some(descriptor.configuration_hash);
            changed.version += 1;
            let provider = device.provider.clone();
            *device.signer.on_sign.lock().unwrap() = Some(Box::new(move || provider.wallets.lock().unwrap()[0] = changed.clone()));
            assert_eq!(device.service.scoped_submit_funding(prepared.id).await.unwrap_err(), Error::ConfigurationConflict);
            assert!(device.provider.broadcasts.lock().unwrap().is_empty());
            assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
            let original = device.store.read().funding[&device.service.scope(TEST_WALLET_ID)].validated_checkpoint().unwrap();
            assert_eq!(original.status, FundingStatus::Pending);
            assert_eq!(original.transaction_id, super::testkit::transaction_id(&original.raw));
            let funding: bsv_home_bsv::multisig::Funding = serde_json::from_str(&original.binding.funding_json).unwrap();
            assert_eq!(funding.policy, fixture().proposal.policy);
            let restart = restarted_service(&device);
            assert_eq!(restart.scoped_retry_funding(team()).await.unwrap_err(), Error::ConfigurationConflict);
            assert!(device.provider.broadcasts.lock().unwrap().is_empty());
            assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
            assert_eq!(
                device.store.read().funding[&device.service.scope(TEST_WALLET_ID)].validated_checkpoint().unwrap().raw,
                original.raw
            );
        }
    });
}

#[test]
fn test_funding_unknown_result_survives_restart_and_retries_identical_raw_without_resigning() {
    block_on(async {
        let device = funding_device().await;
        *device.provider.fail_broadcast.lock().unwrap() = true;
        let prepared = device.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        let result = device.service.scoped_submit_funding(prepared.id).await.unwrap();
        assert_eq!(result.status, FundingStatus::Pending);
        assert_eq!(result.error_code, Some("BSV_BROADCAST_UNCONFIRMED".into()));
        let original = device.store.read().funding[&device.service.scope(TEST_WALLET_ID)].validated_checkpoint().unwrap();
        assert_eq!(
            device.service.prepare_funding(team(), Device::wallet(1).id.id(), "0.001".into()).await.unwrap_err(),
            Error::FundingPending
        );
        let restarted = restarted_service(&device);
        let recovered = restarted.pending_funding(team()).await.unwrap().unwrap();
        assert_eq!(recovered.transaction_id, original.transaction_id);
        assert_eq!(recovered.payer_wallet_id, original.binding.payer_wallet_id.id());
        *device.provider.fail_broadcast.lock().unwrap() = false;
        let accepted = restarted.scoped_retry_funding(team()).await.unwrap();
        assert_eq!(accepted.status, FundingStatus::Broadcast);
        assert_eq!(accepted.transaction_id, original.transaction_id);
        assert_eq!(*device.provider.broadcasts.lock().unwrap(), [original.raw.clone(), original.raw]);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
        assert!(restarted.pending_funding(team()).await.unwrap().is_none());
    });
}

#[test]
fn test_accepted_funding_index_cas_failure_recovers_authoritative_broadcast_without_resigning_or_rebroadcasting() {
    block_on(async {
        let device = funding_device().await;
        let prepared = device.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        let store = device.store.clone();
        *device.provider.on_accepted_funding.lock().unwrap() = Some(Box::new(move || *store.fail_next_cas.lock().unwrap() = true));
        assert_eq!(device.service.scoped_submit_funding(prepared.id.clone()).await.unwrap_err(), Error::ConcurrentUpdate);
        let original = device.store.read().funding[&device.service.scope(TEST_WALLET_ID)].validated_checkpoint().unwrap();
        assert_eq!(original.status, FundingStatus::Pending);
        assert_eq!(original.transaction_id, super::testkit::transaction_id(&original.raw));
        assert_eq!(device.provider.broadcasts.lock().unwrap().as_slice(), std::slice::from_ref(&original.raw));
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
        let restart = restarted_service(&device);
        restart.load().await.unwrap();
        assert!(restart.pending_funding(team()).await.unwrap().is_none());
        let recovered = restart.funding_result(team()).await.unwrap().unwrap();
        assert_eq!(recovered.status, FundingStatus::Broadcast);
        assert_eq!(recovered.transaction_id, original.transaction_id);
        assert_eq!(recovered.payer_wallet_id, original.binding.payer_wallet_id.id());
        assert_eq!(restart.scoped_submit_funding(prepared.id).await.unwrap_err(), Error::FundingExpired);
        assert_eq!(device.provider.broadcasts.lock().unwrap().as_slice(), std::slice::from_ref(&original.raw));
        let accepted = restart.scoped_retry_funding(team()).await.unwrap();
        assert_eq!(accepted.status, FundingStatus::Broadcast);
        assert_eq!(accepted.transaction_id, original.transaction_id);
        assert_eq!(accepted.amount_satoshis, "100000");
        assert!(device.provider.broadcasts.lock().unwrap().as_slice() == std::slice::from_ref(&original.raw));
        assert_eq!(*device.signer.authentications.lock().unwrap(), 0);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
        assert!(device.provider.posts().iter().all(|request| request.url.ends_with("/tx/raw")));
        assert!(device.store.read().intentions.is_empty());
        let saved = device.store.read().funding[&device.service.scope(TEST_WALLET_ID)].validated_checkpoint().unwrap();
        assert_eq!(saved.status, FundingStatus::Broadcast);
        assert_eq!(saved.raw, original.raw);
        assert_eq!(saved.transaction_id, original.transaction_id);
        assert!(restart.pending_funding(team()).await.unwrap().is_none());
        let synced = restart.get_wallet(team()).unwrap();
        assert_eq!(synced.balance_satoshis, Some("200000".into()));
        assert!(!synced.balance_is_stale);
    });
}

#[test]
fn test_shared_store_does_not_reuse_configuration_trust_or_funding_across_origin_or_local_wallet() {
    block_on(async {
        let original = funding_device().await;
        *original.provider.fail_broadcast.lock().unwrap() = true;
        let prepared = original.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        let pending = original.service.scoped_submit_funding(prepared.id).await.unwrap();
        assert_eq!(pending.status, FundingStatus::Pending);
        assert!(original.service.get_wallet(team()).unwrap().can_transact);
        for other in [Device::with_origin(1, "https://another-portal.test/api/portal"), Device::new(2)] {
            let isolated = BsvHomeBsvMultisigService::new(
                original.store.clone(),
                other.session.clone(),
                other.gateway.clone(),
                other.signer.clone(),
                other.auth.clone(),
                other.addresses.clone(),
                other.wallet.id.id(),
                other.cold_store.clone(),
            );
            isolated.load().await.unwrap();
            let wallet = isolated.get_wallet(team()).unwrap();
            assert_eq!(wallet.configuration_status, Configuration::NeedsLocalReview);
            assert!(!wallet.can_transact);
            assert_eq!(isolated.receive_request(team()).unwrap_err(), Error::ConfigurationNotConfirmed);
            assert!(isolated.pending_funding(team()).await.unwrap().is_none());
            assert!(isolated.funding_result(team()).await.unwrap().is_none());
            assert_eq!(isolated.scoped_retry_funding(team()).await.unwrap_err(), Error::MissingRecord);
            assert_eq!(
                isolated.prepare_funding(team(), Device::wallet(1).id.id(), "0.001".into()).await.unwrap_err(),
                Error::ConfigurationNotConfirmed
            );
            assert!(other.provider.broadcasts.lock().unwrap().is_empty());
            assert!(other.provider.posts().is_empty());
        }
        assert_eq!(original.store.read().trusted.len(), 1);
        assert_eq!(original.store.read().funding.len(), 1);
        assert_eq!(original.service.pending_funding(team()).await.unwrap().unwrap().transaction_id, pending.transaction_id);
        assert!(original.service.get_wallet(team()).unwrap().can_transact);
    });
}

#[test]
fn test_funding_balance_sync_failure_remains_broadcast_and_retries_only_reads() {
    block_on(async {
        let device = funding_device().await;
        let prepared = device.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        *device.provider.fail_balances.lock().unwrap() = true;
        let result = device.service.scoped_submit_funding(prepared.id).await.unwrap();
        assert_eq!(result.status, FundingStatus::SyncFailed);
        assert_eq!(result.error_code, Some("MULTISIG_BALANCE_SYNC_FAILED".into()));
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 1);
        let stale = device.service.get_wallet(team()).unwrap();
        assert_eq!(stale.balance_satoshis, Some("100000".into()));
        assert!(stale.balance_is_stale);
        *device.provider.fail_balances.lock().unwrap() = false;
        let refreshed = device.service.retry_funding_sync(team()).await.unwrap();
        assert_eq!(refreshed.status, FundingStatus::Broadcast);
        assert_eq!(refreshed.transaction_id, result.transaction_id);
        let fresh = device.service.get_wallet(team()).unwrap();
        assert_eq!(fresh.balance_satoshis, Some("200000".into()));
        assert!(!fresh.balance_is_stale);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 1);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 1);
    });
}

#[test]
fn test_funding_recovery_blocks_missing_payer_and_corrupt_checkpoint_without_new_payment() {
    block_on(async {
        let device = funding_device().await;
        *device.provider.fail_broadcast.lock().unwrap() = true;
        let prepared = device.service.prepare_funding(team(), Device::wallet(2).id.id(), "0.001".into()).await.unwrap();
        device.service.scoped_submit_funding(prepared.id).await.unwrap();
        device.wallets.wallets.lock().unwrap().retain(|wallet| wallet.id != Device::wallet(2).id);
        let restarted = restarted_service(&device);
        assert_eq!(restarted.pending_funding(team()).await.unwrap_err(), Error::FundingPayerMissing);
        assert_eq!(restarted.scoped_retry_funding(team()).await.unwrap_err(), Error::FundingPayerMissing);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 1);
        device.wallets.wallets.lock().unwrap().push(Device::wallet(2));
        let (revision, mut state): (u64, State) = device.service.read().unwrap();
        match state.funding.get_mut(&device.service.scope(TEST_WALLET_ID)).unwrap() {
            super::FundingCheckpoint::Legacy(checkpoint) => checkpoint.transaction_id = "ff".repeat(32),
            super::FundingCheckpoint::Current(checkpoint) => checkpoint.transaction_id = "ff".repeat(32),
        }
        device.service.save(revision, state).await.unwrap();
        assert_eq!(restarted.pending_funding(team()).await.unwrap_err(), Error::FundingChanged);
        assert_eq!(restarted.scoped_retry_funding(team()).await.unwrap_err(), Error::FundingChanged);
        assert_eq!(device.provider.broadcasts.lock().unwrap().len(), 1);
    });
}

fn proposal_wire_without_metadata(fields: &[&str]) -> Value {
    let mut value = to_value(remote_proposal("metadata", "COLLECTING")).unwrap();
    for field in fields {
        assert!(value.as_object_mut().unwrap().remove(*field).is_some());
    }
    value
}

fn proposal_without_metadata(fields: &[&str]) -> RemoteProposal {
    from_value(proposal_wire_without_metadata(fields)).unwrap()
}

#[test]
fn test_proposal_wire_allows_missing_creator_and_confirmation_metadata() {
    let wire = proposal_wire_without_metadata(&["creatorId", "creatorAddress", "confirmations"]);
    assert!(from_value::<RemoteProposal>(wire).is_ok());
}

#[test]
fn test_missing_proposal_metadata_loads_unknown_confirmations_and_cannot_withdraw() {
    block_on(async {
        let device = Device::new(1);
        let wire = proposal_wire_without_metadata(&["creatorId", "creatorAddress", "confirmations"]);
        let proposal: RemoteProposal = from_value(wire.clone()).unwrap();
        assert_eq!(proposal.creator_id, None);
        assert_eq!(proposal.creator_address, None);
        assert_eq!(proposal.confirmations, None);
        *device.provider.proposals.lock().unwrap() = vec![proposal];
        device.provider.override_data(
            &format!("/auth/multisig/wallets/{TEST_WALLET_ID}/proposal-page?offset=0&limit=200"),
            json!({"items": [wire], "total": 1, "offset": 0, "limit": 200, "hasMore": false}),
        );
        device.trust().await;
        device.service.load().await.unwrap();
        let view = device.service.get_proposal("metadata".into()).unwrap();
        assert_eq!(view.confirmations, None);
        assert!(!view.can_withdraw);
        let writes = device.provider.posts().len();
        assert_eq!(
            device.service.scoped_withdraw_proposal("metadata".into(), 3).await.unwrap_err(),
            Error::invalid("CREATOR_REQUIRED")
        );
        assert_eq!(device.provider.posts().len(), writes);
    });
}

#[test]
fn test_proposal_creator_address_or_id_resolves_only_the_matching_wallet_member() {
    block_on(async {
        for removed in [&["creatorId"][..], &["creatorAddress"][..], &[][..]] {
            for index in [1, 2] {
                let device = Device::new(index);
                *device.provider.proposals.lock().unwrap() = vec![proposal_without_metadata(removed)];
                device.trust().await;
                device.service.load().await.unwrap();
                let view = device.service.get_proposal("metadata".into()).unwrap();
                assert_eq!(view.can_withdraw, index == 1);
                assert_eq!(view.confirmations, Some(0));
            }
        }
    });
}

#[test]
fn test_proposal_creator_metadata_rejects_conflicts_unknown_members_and_negative_id() {
    let wallet = remote_wallet("ACTIVE", true);
    let original = remote_proposal("metadata", "COLLECTING");
    let mut mutations = Vec::new();
    let mut changed = original.clone();
    changed.creator_id = Some(2);
    mutations.push(changed);
    let mut changed = original.clone();
    changed.creator_id = Some(99);
    changed.creator_address = None;
    mutations.push(changed);
    let mut changed = original.clone();
    changed.creator_id = None;
    changed.creator_address = Some("1BitcoinEaterAddressDontSendf59kuE".into());
    mutations.push(changed);
    let mut changed = original.clone();
    changed.creator_id = Some(-1);
    changed.creator_address = None;
    mutations.push(changed);
    let mut changed = original;
    changed.creator_id = None;
    changed.creator_address = Some(String::new());
    mutations.push(changed);
    for changed in mutations {
        assert_eq!(rules::validate_proposal(&changed, &wallet).unwrap_err(), Error::invalid("INVALID_RESPONSE"));
    }
}

#[test]
fn test_optional_proposal_confirmations_reject_negative_and_wrong_wire_types() {
    let wallet = remote_wallet("ACTIVE", true);
    let mut proposal = proposal_without_metadata(&["creatorId", "creatorAddress", "confirmations"]);
    rules::validate_proposal(&proposal, &wallet).unwrap();
    proposal.confirmations = Some(-1);
    assert_eq!(rules::validate_proposal(&proposal, &wallet).unwrap_err(), Error::invalid("INVALID_RESPONSE"));
    let original = to_value(remote_proposal("metadata", "COLLECTING")).unwrap();
    for (field, value) in [
        ("creatorId", json!("1")),
        ("creatorId", json!(false)),
        ("confirmations", json!("0")),
        ("confirmations", json!(0.5)),
        ("confirmations", json!(false)),
    ] {
        let mut changed = original.clone();
        changed[field] = value;
        assert!(from_value::<RemoteProposal>(changed).is_err());
    }
    let mut nulls = original;
    nulls["creatorId"] = Value::Null;
    nulls["creatorAddress"] = Value::Null;
    nulls["confirmations"] = Value::Null;
    let nulls: RemoteProposal = from_value(nulls).unwrap();
    assert_eq!(rules::proposal_creator(&nulls, &wallet).unwrap().map(|member| member.user_id), None);
    assert_eq!(nulls.confirmations, None);
    let mut zero_member = wallet;
    zero_member.members[0].user_id = 0;
    rules::validate_wallet(&zero_member).unwrap();
    let mut explicit_zero = remote_proposal("zero", "COLLECTING");
    explicit_zero.creator_id = Some(0);
    explicit_zero.confirmations = Some(0);
    rules::validate_proposal(&explicit_zero, &zero_member).unwrap();
    assert_eq!(rules::proposal_creator(&explicit_zero, &zero_member).unwrap().map(|member| member.user_id), Some(0));
}

#[test]
fn test_unknown_proposal_metadata_does_not_weaken_amount_policy_signature_or_txid_validation() {
    let wallet = remote_wallet("ACTIVE", true);
    let mut proposal = remote_proposal("metadata", "CONFIRMED");
    proposal.creator_id = None;
    proposal.creator_address = None;
    proposal.confirmations = None;
    rules::validate_proposal(&proposal, &wallet).unwrap();
    let mut mutations = Vec::new();
    let mut changed = proposal.clone();
    changed.proposal.amount += 1;
    mutations.push(changed);
    let mut changed = proposal.clone();
    changed.proposal.policy.threshold = 3;
    mutations.push(changed);
    let mut changed = proposal.clone();
    changed.signatures[0].signatures[0] = "00".into();
    mutations.push(changed);
    let mut changed = proposal;
    changed.txid = Some("00".repeat(32));
    mutations.push(changed);
    for changed in mutations {
        assert!(rules::validate_proposal(&changed, &wallet).is_err());
    }
}

#[test]
fn test_proposal_memo_limit_counts_utf16_units_like_android() {
    assert_eq!(rules::text(&"😀".repeat(50), 100, true).unwrap(), "😀".repeat(50));
    assert_eq!(rules::text(&"😀".repeat(51), 100, true), Err(Error::invalid("INVALID_REQUEST")));
    assert_eq!(rules::text(&"a".repeat(100), 100, true).unwrap(), "a".repeat(100));
    assert_eq!(rules::text(&"a".repeat(101), 100, true), Err(Error::invalid("INVALID_REQUEST")));
}
