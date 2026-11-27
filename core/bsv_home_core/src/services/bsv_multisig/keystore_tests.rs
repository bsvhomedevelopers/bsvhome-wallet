use crate::services::bsv_multisig::testkit::ScopedTestActions;
use std::{slice::from_ref, sync::Arc};

use futures::executor::block_on;

use super::super::testkit::{KeystoreMultisigDevice, MemoryMultisigProvider, TEST_V2_BASE_URL, fixture, member};
use super::super::{BsvHomeBsvMultisigConfigurationStatus, BsvHomeBsvMultisigError, BsvHomeBsvMultisigProposalState, BsvHomeBsvMultisigSigner};

#[test]
fn test_independent_keystores_create_join_confirm_sign_all_inputs_and_broadcast() {
    block_on(async {
        let provider = Arc::new(MemoryMultisigProvider::new(1, TEST_V2_BASE_URL));
        provider.wallets.lock().unwrap().clear();
        let creator = KeystoreMultisigDevice::new(1, provider.clone());
        let second = KeystoreMultisigDevice::new(2, Arc::new(provider.for_member(2)));
        let third = KeystoreMultisigDevice::new(3, Arc::new(provider.for_member(3)));
        assert!(second.signer.public_key(creator.wallet.clone()).await.is_err());
        assert!(third.signer.public_key(second.wallet.clone()).await.is_err());

        let identity = creator.service.current_signing_identity().await.unwrap();
        let draft = creator
            .service
            .begin_creation("Keystore team".into(), 2, vec!["Creator".into(), "Second".into(), "Third".into()], identity.wallet_id)
            .await
            .unwrap();
        let created = creator.service.submit_creation(draft.id).await.unwrap();
        let wallet_id = created.id;
        assert!(!created.can_transact);
        for joining in [&second, &third] {
            let invitation = creator.service.invitation(wallet_id.clone()).await.unwrap();
            let preview = joining.service.preview_invitation(invitation.token).await.unwrap();
            assert_eq!(preview.wallet.id, wallet_id);
            assert_eq!(preview.signing_identity.wallet_id, joining.wallet.id.id());
            let joined = joining.service.join_invitation(preview).await.unwrap();
            assert!(!joined.can_transact);
        }

        for device in [&creator, &second, &third] {
            device.service.load().await.unwrap();
            let wallet = device.service.get_wallet(wallet_id.clone()).unwrap();
            assert_eq!(wallet.configuration_status, BsvHomeBsvMultisigConfigurationStatus::NeedsConfirmation);
            let approved = device.service.approve_configuration(wallet_id.clone(), wallet.version).await.unwrap();
            assert!(!approved.personal_configuration_pending);
        }
        for device in [&creator, &second, &third] {
            device.service.load().await.unwrap();
            let wallet = device.service.get_wallet(wallet_id.clone()).unwrap();
            assert!(wallet.can_transact);
            assert_eq!(wallet.configuration_status, BsvHomeBsvMultisigConfigurationStatus::Confirmed);
        }

        let expected = fixture();
        let prepared = creator
            .service
            .scoped_prepare_proposal(wallet_id.clone(), expected.proposal.recipient.clone(), "0.00042".into(), "Keystore payment".into())
            .await
            .unwrap();
        let proposal_id = prepared.id;
        assert_eq!(prepared.input_count, 2);
        assert_eq!(prepared.signed_members, Vec::<String>::new());
        assert_eq!(prepared.state, BsvHomeBsvMultisigProposalState::CollectingSignatures);
        assert_eq!(creator.signer.signed_wallets.lock().unwrap().len(), 0);

        second.service.load().await.unwrap();
        let collecting = second.service.get_proposal(proposal_id.clone()).unwrap();
        let first_signature = second.service.scoped_sign_proposal(proposal_id.clone(), collecting.version).await.unwrap();
        assert_eq!(first_signature.signed_members, [member(2, true).public_key]);
        assert!(!first_signature.can_broadcast);
        assert_eq!(second.signer.signed_wallets.lock().unwrap().as_slice(), from_ref(&second.wallet.id));
        assert_eq!(
            second.service.scoped_sign_proposal(proposal_id.clone(), first_signature.version).await.unwrap_err(),
            BsvHomeBsvMultisigError::invalid("PROPOSAL_CLOSED"),
        );
        let insufficient = creator.service.refresh_proposal(proposal_id.clone()).await.unwrap();
        assert_eq!(
            creator.service.scoped_broadcast_proposal(proposal_id.clone(), insufficient.version).await.unwrap_err(),
            BsvHomeBsvMultisigError::invalid("PROPOSAL_NOT_READY"),
        );
        assert!(provider.broadcasts.lock().unwrap().is_empty());

        let reopened = third.reopened_service();
        reopened.load().await.unwrap();
        assert_eq!(
            reopened.get_wallet(wallet_id.clone()).unwrap().configuration_status,
            BsvHomeBsvMultisigConfigurationStatus::Confirmed,
        );
        let collecting = reopened.get_proposal(proposal_id.clone()).unwrap();
        let ready = reopened.scoped_sign_proposal(proposal_id.clone(), collecting.version).await.unwrap();
        assert_eq!(ready.state, BsvHomeBsvMultisigProposalState::ReadyToBroadcast);
        assert_eq!(third.signer.signed_wallets.lock().unwrap().as_slice(), from_ref(&third.wallet.id));
        let actual_sets = provider.proposals.lock().unwrap()[0].signatures.clone();
        assert_eq!(actual_sets, expected.signature_sets.into_iter().take(2).collect::<Vec<_>>());
        assert!(actual_sets.iter().all(|set| set.signatures.len() == expected.proposal.inputs.len()));

        let reopened_creator = creator.reopened_service();
        reopened_creator.load().await.unwrap();
        let ready = reopened_creator.get_proposal(proposal_id.clone()).unwrap();
        assert!(ready.can_broadcast);
        assert_eq!(ready.signed_members.len(), 2);
        let broadcast = reopened_creator.scoped_broadcast_proposal(proposal_id, ready.version).await.unwrap();
        assert_eq!(broadcast.state, BsvHomeBsvMultisigProposalState::PendingConfirmation);
        assert_eq!(broadcast.transaction_id, Some(expected.signed_transaction_id));
        assert_eq!(provider.broadcasts.lock().unwrap().as_slice(), [expected.signed_transaction]);
        assert_eq!(creator.signer.signed_wallets.lock().unwrap().len(), 0);
        assert_eq!(creator.provider.posts().iter().filter(|request| request.url.ends_with("/wallets")).count(), 1);
        for (device, index) in [(&creator, 1), (&second, 2), (&third, 3)] {
            for request in device.provider.requests.lock().unwrap().iter() {
                assert!(request.url.starts_with(TEST_V2_BASE_URL));
                assert_eq!(
                    request.headers.as_ref().unwrap().get("Authorization"),
                    Some(&format!("public-multisig-member-{index}-test-token")),
                );
            }
        }
    });
}
