use async_trait::async_trait;
use std::mem::discriminant;
use std::sync::Arc;

use bsv_home_bsv::multisig::{Policy, policy_descriptor};
use futures::channel::oneshot;
use futures::executor::block_on;
use futures::{FutureExt, join, pin_mut, poll};
use primitives::{Account, Asset, Chain};
use serde_json::Value;
use std::task::Poll;

use super::*;
use crate::services::bsv_cold_wallet::testkit::MemoryColdWalletStore;
use crate::services::bsv_multisig::testkit::{Device, member};
use crate::services::bsv_multisig::{
    BsvHomeBsvMultisigActionScope, BsvHomeBsvMultisigAuthorization, BsvHomeBsvMultisigAuthorizationRequest, BsvHomeBsvMultisigError, BsvHomeBsvMultisigPublicFundingService,
    BsvHomeBsvMultisigPublicFundingSource, bsv_multisig_public_funding_payers,
};
use crate::services::bsv_multisig::{bsv_multisig_payment_request_qr_payload, parse_bsv_multisig_payment_request};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::portal::testkit::PublicPaymailContext;
use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};

fn service(context: &PublicPaymailContext) -> BsvHomeSendService {
    let resolver = Arc::new(BsvHomePaymailDestinationService::new_for_wallet(context.provider.clone(), context.sender.clone()));
    super::tests::service_with_resolver(context.provider.clone(), BsvHomeRuntimeEnvironment::WowPay, resolver)
}

fn script(count: u8) -> String {
    policy_descriptor(&Policy {
        threshold: 2,
        public_keys: (1..=count).map(|index| member(index, true).public_key).collect(),
    })
    .unwrap()
    .locking_script
}

async fn prepare(service: &BsvHomeSendService, context: &PublicPaymailContext, value: u64, use_max: bool) -> Arc<BsvHomeSendPublicFundingPreparation> {
    context.replies(&script(2), Some("initial-public-reference"));
    match service
        .resolve_destination(
            Asset::from_chain(Chain::BitcoinSv),
            " Team@Example.com ".into(),
            BsvHomeNameRecordState::None,
            None,
            value.into(),
            use_max,
        )
        .await
        .unwrap()
    {
        BsvHomeSendDestination::PublicFunding { preparation } => preparation,
        BsvHomeSendDestination::Transfer { .. } => panic!("P2MS must not become ordinary transfer data"),
    }
}

#[test]
fn test_public_paymail_send_keeps_initial_no_amount_and_revalidates_the_final_edited_exact_amount_without_reference_or_net_max() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let service = service(&context);
        let preparation = prepare(&service, &context, 42_000, true).await;
        let initial = preparation.payment_request();
        assert_eq!(initial.name, "team@example.com");
        assert_eq!(initial.amount, "0.00042");
        let initial_body: Value = serde_json::from_slice(context.provider.requests.lock().unwrap()[1].body.as_ref().unwrap()).unwrap();
        assert_eq!(initial_body.get("amount"), None);
        assert_eq!(initial_body["senderHandle"], "sender@api-dev.hj0252.com");
        context.replies(&script(2), Some("different-final-public-reference"));
        let confirmed = service.confirm_public_funding(preparation.clone(), 84_000.into()).await.unwrap();
        assert_eq!(confirmed.amount, "0.00084");
        assert_eq!(confirmed.fingerprint, initial.fingerprint);
        assert_eq!(preparation.payment_request(), initial);
        let final_body: Value = serde_json::from_slice(context.provider.requests.lock().unwrap()[3].body.as_ref().unwrap()).unwrap();
        assert_eq!(final_body["amount"], 84_000);
        let code = bsv_multisig_payment_request_qr_payload(confirmed.clone()).unwrap();
        assert_eq!(parse_bsv_multisig_payment_request(code).unwrap(), confirmed);
        assert_eq!(context.auth.multisig_version(), 0);
    });
}

#[test]
fn test_final_public_paymail_output_changes_require_new_review_instead_of_adopting_the_new_script() {
    block_on(async {
        for output in [script(3), "76a914000000000000000000000000000000000000000088ac".into()] {
            let context = PublicPaymailContext::new();
            let service = service(&context);
            let preparation = prepare(&service, &context, 42_000, false).await;
            let original = preparation.payment_request();
            context.replies(&output, None);
            assert_eq!(
                service.confirm_public_funding(preparation.clone(), 84_000.into()).await,
                Err(BsvHomePaymailError::BindingChanged)
            );
            assert_eq!(preparation.payment_request(), original);
        }
    });
}

#[test]
fn test_public_paymail_preparation_is_owner_scoped_and_cancel_does_not_cancel_shared_sender_auth() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let resolver = Arc::new(BsvHomePaymailDestinationService::new_for_wallet(context.provider.clone(), context.sender.clone()));
        let first = super::tests::service_with_resolver(context.provider.clone(), BsvHomeRuntimeEnvironment::WowPay, resolver.clone());
        let second = super::tests::service_with_resolver(context.provider.clone(), BsvHomeRuntimeEnvironment::WowPay, resolver);
        let preparation = prepare(&first, &context, 42_000, false).await;
        let requests = (context.management_request_count(), context.provider.requests.lock().unwrap().len());
        assert_eq!(
            second.confirm_public_funding(preparation.clone(), 42_000.into()).await,
            Err(BsvHomePaymailError::BindingChanged)
        );
        preparation.cancel();
        assert_eq!(first.confirm_public_funding(preparation, 42_000.into()).await, Err(BsvHomeServiceError::Cancelled.into()));
        assert_eq!((context.management_request_count(), context.provider.requests.lock().unwrap().len()), requests);
        assert_eq!(context.auth.multisig_version(), 0);
        let fresh = prepare(&first, &context, 42_000, false).await;
        context.replies(&script(2), None);
        assert_eq!(first.confirm_public_funding(fresh, 42_000.into()).await.unwrap().amount, "0.00042");
    });
}

#[test]
fn test_public_paymail_finalization_rechecks_original_sender_wallet_account_and_active_handle() {
    block_on(async {
        for change in ["wallet", "deleted", "account", "handle"] {
            let context = PublicPaymailContext::new();
            let service = service(&context);
            let preparation = prepare(&service, &context, 42_000, false).await;
            match change {
                "wallet" => context.switch_wallet(),
                "deleted" => context.replace_wallet(None),
                "account" => {
                    let mut wallet = context.wallet.clone();
                    wallet.accounts = vec![Account::mock(Chain::BitcoinSv, "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5")];
                    context.replace_wallet(Some(wallet));
                }
                "handle" => context.set_sender_handle("changed@api-dev.hj0252.com".into()),
                _ => unreachable!(),
            }
            assert!(service.confirm_public_funding(preparation, 42_000.into()).await.is_err());
            assert_eq!(context.provider.requests.lock().unwrap().len(), 2);
        }
    });
}

#[test]
fn test_cancelling_final_public_paymail_discovery_or_destination_discards_late_results_and_keeps_shared_auth() {
    block_on(async {
        for pause_at in [3, 4] {
            let context = PublicPaymailContext::new();
            let service = service(&context);
            let preparation = prepare(&service, &context, 42_000, false).await;
            context.replies(&script(2), None);
            let (reached, waiting) = oneshot::channel();
            let (resume, resumed) = oneshot::channel();
            *context.provider.request_pause.lock().unwrap() = Some((pause_at, reached, resumed));
            let (result, ()) = join!(service.confirm_public_funding(preparation.clone(), 84_000.into()), async {
                waiting.await.unwrap();
                preparation.cancel();
                let _ = resume.send(());
            });
            assert_eq!(result, Err(BsvHomeServiceError::Cancelled.into()));
            assert_eq!(context.provider.requests.lock().unwrap().len(), pause_at);
            assert_eq!(context.auth.multisig_version(), 0);
        }
    });
}

#[test]
fn test_public_paymail_rejects_invalid_final_amount_before_any_lookup_and_form_scan_stays_ordinary() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let service = service(&context);
        let preparation = prepare(&service, &context, 42_000, false).await;
        let requests = (context.management_request_count(), context.provider.requests.lock().unwrap().len());
        for value in [BsvHomeBigInt::from(0), BsvHomeBigInt::from(-1), BsvHomeBigInt::from(2_100_000_000_000_001u64)] {
            assert_eq!(service.confirm_public_funding(preparation.clone(), value).await, Err(BsvHomePaymailError::InvalidAmount));
        }
        assert_eq!((context.management_request_count(), context.provider.requests.lock().unwrap().len()), requests);
        let code = bsv_multisig_payment_request_qr_payload(preparation.payment_request()).unwrap();
        assert!(service.scan(Asset::from_chain(Chain::BitcoinSv), code).is_err());
        assert_eq!(context.provider.requests.lock().unwrap().len(), 2);
    });
}

#[test]
fn test_typed_p2pkh_send_preserves_the_existing_ordinary_transfer_and_max_contract() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let service = service(&context);
        let output = "76a914000000000000000000000000000000000000000088ac";
        context.replies(output, Some("public-reference"));
        let typed = match service
            .resolve_destination(
                Asset::from_chain(Chain::BitcoinSv),
                "alice@example.com".into(),
                BsvHomeNameRecordState::None,
                None,
                42_000.into(),
                true,
            )
            .await
            .unwrap()
        {
            BsvHomeSendDestination::Transfer { transfer } => transfer,
            BsvHomeSendDestination::PublicFunding { .. } => panic!("P2PKH must remain ordinary transfer data"),
        };
        context.replies(output, Some("public-reference"));
        let legacy = service
            .transfer_data(
                Asset::from_chain(Chain::BitcoinSv),
                "alice@example.com".into(),
                BsvHomeNameRecordState::None,
                None,
                42_000.into(),
                true,
            )
            .await
            .unwrap();
        assert_eq!(
            (typed.input_type.get_asset(), &typed.recipient, &typed.value, typed.use_max_amount),
            (legacy.input_type.get_asset(), &legacy.recipient, &legacy.value, legacy.use_max_amount)
        );
        assert_eq!(
            discriminant(&typed.input_type),
            discriminant(&TransactionInputType::Transfer {
                asset: Asset::from_chain(Chain::BitcoinSv)
            })
        );
        assert!(typed.use_max_amount);
        assert_eq!(typed.value, 42_000.into());
        assert_eq!(typed.recipient.name, Some("alice@example.com".into()));
    });
}

#[test]
fn test_public_paymail_cancel_aborts_a_blocked_final_read_without_releasing_the_response_gate() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let service = service(&context);
        let preparation = prepare(&service, &context, 42_000, false).await;
        context.replies(&script(2), None);
        let (reached, waiting) = oneshot::channel();
        let (resume, resumed) = oneshot::channel();
        *context.provider.request_pause.lock().unwrap() = Some((3, reached, resumed));
        let operation = service.confirm_public_funding(preparation.clone(), 84_000.into());
        pin_mut!(operation);
        assert_eq!(poll!(operation.as_mut()), Poll::Pending);
        assert_eq!(waiting.now_or_never(), Some(Ok(())));
        preparation.cancel();
        assert_eq!(operation.now_or_never(), Some(Err(BsvHomeServiceError::Cancelled.into())));
        assert_eq!(context.provider.requests.lock().unwrap().len(), 3);
        assert_eq!(context.auth.multisig_version(), 0);
        drop(resume);
    });
}

#[test]
fn test_dropping_an_old_amount_read_allows_new_amount_verification_with_the_same_original_paymail_source() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let service = service(&context);
        let preparation = prepare(&service, &context, 42_000, false).await;
        context.replies(&script(2), None);
        let (reached, waiting) = oneshot::channel();
        let (resume, resumed) = oneshot::channel();
        *context.provider.request_pause.lock().unwrap() = Some((3, reached, resumed));
        let mut old = Box::pin(service.confirm_public_funding(preparation.clone(), 84_000.into()));
        assert_eq!(poll!(old.as_mut()), Poll::Pending);
        assert_eq!(waiting.now_or_never(), Some(Ok(())));
        drop(old);
        context.provider.replies.lock().unwrap().clear();
        context.replies(&script(2), Some("latest-public-reference"));
        let updated = service.confirm_public_funding(preparation.clone(), 126_000.into()).await.unwrap();
        assert_eq!(updated.amount, "0.00126");
        assert_eq!(updated.fingerprint, preparation.payment_request().fingerprint);
        assert_eq!(preparation.payment_request().amount, "0.00042");
        assert!(resume.send(()).is_err());
        assert_eq!(context.provider.requests.lock().unwrap().len(), 5);
        assert_eq!(context.auth.multisig_version(), 0);
    });
}

#[test]
fn test_borrowed_public_funding_source_scope_rejects_original_sender_switch_without_cancelling_shared_auth() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let service = service(&context);
        let preparation = prepare(&service, &context, 42_000, false).await;
        assert_eq!(service.check_public_funding_scope(&preparation), Ok(()));
        let counts = (context.management_request_count(), context.provider.requests.lock().unwrap().len());
        context.switch_wallet();
        assert_eq!(
            service.check_public_funding_scope(&preparation),
            Err(BsvHomePaymailError::Auth {
                error: BsvHomePortalAuthError::WalletChanged
            })
        );
        assert_eq!(
            service.confirm_public_funding_with_checkpoint(preparation, 42_000.into(), &|| Ok(())).await.err(),
            Some(BsvHomePaymailError::Auth {
                error: BsvHomePortalAuthError::WalletChanged
            })
        );
        assert_eq!((context.management_request_count(), context.provider.requests.lock().unwrap().len()), counts);
        assert_eq!(context.auth.multisig_version(), 0);
    });
}

#[test]
fn test_borrowed_public_funding_source_rechecks_the_full_original_account_before_public_resolve() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let service = service(&context);
        let preparation = prepare(&service, &context, 42_000, false).await;
        let mut changed = context.wallet.clone();
        changed.accounts[0].derivation_path = "changed-account".into();
        context.replace_wallet(Some(changed));
        let counts = (context.management_request_count(), context.provider.requests.lock().unwrap().len());
        assert_eq!(
            service.check_public_funding_source(&preparation, &|| Ok(())).await,
            Err(BsvHomePaymailError::BindingChanged)
        );
        assert_eq!((context.management_request_count(), context.provider.requests.lock().unwrap().len()), counts);
        assert_eq!(context.auth.multisig_version(), 0);
    });
}

#[test]
fn test_public_paymail_send_preserves_plain_satoshi_boundaries_in_initial_and_final_requests() {
    block_on(async {
        for (satoshis, amount) in [(1, "0.00000001"), (10, "0.0000001"), (100, "0.000001"), (1000, "0.00001")] {
            let context = PublicPaymailContext::new();
            let service = service(&context);
            let preparation = prepare(&service, &context, satoshis, false).await;
            assert_eq!(preparation.payment_request().amount, amount);
            context.replies(&script(2), None);
            let confirmed = service.confirm_public_funding(preparation, satoshis.into()).await.unwrap();
            assert_eq!(confirmed.amount, amount);
            let code = bsv_multisig_payment_request_qr_payload(confirmed.clone()).unwrap();
            assert_eq!(parse_bsv_multisig_payment_request(code).unwrap(), confirmed);
        }
    });
}

struct RejectedPublicFundingAuthorization;

#[async_trait]
impl BsvHomeBsvMultisigAuthorization for RejectedPublicFundingAuthorization {
    async fn authorize(&self, _wallet: Wallet, _request: BsvHomeBsvMultisigAuthorizationRequest, _scope: Arc<BsvHomeBsvMultisigActionScope>) -> Result<(), BsvHomeServiceError> {
        panic!("a different Paymail payer must never reach online password authorization")
    }
}

#[test]
fn test_public_paymail_funding_chooser_and_final_owner_require_the_original_complete_sender_wallet() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let sender = Arc::new(service(&context));
        let preparation = prepare(&sender, &context, 42_000, false).await;
        let original = context.wallet.clone();
        let other = Device::wallet(2);
        assert_ne!(original.id, other.id);
        assert_eq!(sender.public_funding_payer_matches(&preparation, &original), Ok(true));
        assert_eq!(sender.public_funding_payer_matches(&preparation, &other), Ok(false));
        let mut changed = original.clone();
        changed.accounts.iter_mut().find(|account| account.chain == Chain::BitcoinSv).unwrap().derivation_path = "different-root-binding".into();
        assert_eq!(sender.public_funding_payer_matches(&preparation, &changed), Ok(false));
        let source = BsvHomeBsvMultisigPublicFundingSource::Paymail {
            service: sender.clone(),
            preparation: preparation.clone(),
        };
        let preferences = Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default())));
        let eligible = bsv_multisig_public_funding_payers(
            vec![other.clone(), original.clone(), changed],
            preferences,
            Arc::new(MemoryColdWalletStore::default()),
            source,
        )
        .await
        .unwrap();
        assert_eq!(eligible.into_iter().map(|payer| payer.wallet_id).collect::<Vec<_>>(), vec![original.id.id()]);
        let counts = (context.management_request_count(), context.provider.requests.lock().unwrap().len());
        assert_eq!(
            sender.confirm_public_funding_for_payer(preparation.clone(), &other, 84_000.into(), &|| Ok(())).await.err(),
            Some(BsvHomePaymailError::BindingChanged)
        );
        assert_eq!((context.management_request_count(), context.provider.requests.lock().unwrap().len()), counts);
        context.replies(&script(2), None);
        let confirmed = sender.confirm_public_funding_for_payer(preparation, &original, 84_000.into(), &|| Ok(())).await.unwrap();
        assert_eq!(confirmed.amount, "0.00084");
    });
}

#[test]
fn test_actual_public_funding_service_rejects_another_paymail_payer_before_resolution_password_or_keys() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let sender = Arc::new(service(&context));
        let preparation = prepare(&sender, &context, 42_000, false).await;
        let device = Device::with_fixed_wallet(2);
        let service = BsvHomeBsvMultisigPublicFundingService::new(
            device.session.clone(),
            device.gateway.clone(),
            device.addresses.clone(),
            device.signer.clone(),
            Arc::new(RejectedPublicFundingAuthorization),
            device.auth.clone(),
            device.wallet.clone(),
            Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default()))),
            Arc::new(MemoryColdWalletStore::default()),
        )
        .unwrap();
        let counts = (context.management_request_count(), context.provider.requests.lock().unwrap().len());
        let source = BsvHomeBsvMultisigPublicFundingSource::Paymail { service: sender, preparation };
        assert_eq!(service.prepare_funding(source, "0.00084".into()).await.err(), Some(BsvHomeBsvMultisigError::FundingChanged));
        assert_eq!((context.management_request_count(), context.provider.requests.lock().unwrap().len()), counts);
        assert_eq!(device.provider.requests.lock().unwrap().len(), 0);
        assert_eq!(*device.signer.funding_signatures.lock().unwrap(), 0);
    });
}

#[test]
fn test_paymail_payer_selection_does_not_silently_drop_a_corrupt_other_bsv_wallet() {
    block_on(async {
        let context = PublicPaymailContext::new();
        let sender = Arc::new(service(&context));
        let preparation = prepare(&sender, &context, 42_000, false).await;
        let mut invalid = Device::wallet(2);
        invalid.accounts.clear();
        let source = BsvHomeBsvMultisigPublicFundingSource::Paymail { service: sender, preparation };
        let result = bsv_multisig_public_funding_payers(
            vec![context.wallet.clone(), invalid],
            Arc::new(BsvHomePreferencesService::new(Arc::new(MemoryPreferencesStore::default()))),
            Arc::new(MemoryColdWalletStore::default()),
            source,
        )
        .await;
        assert_eq!(result.err(), Some(BsvHomeBsvMultisigError::MissingSigner));
    });
}
