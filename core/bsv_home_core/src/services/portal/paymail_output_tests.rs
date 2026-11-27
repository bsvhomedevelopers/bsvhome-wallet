use std::sync::Arc;

use bsv_home_bsv::multisig::{Policy, policy_descriptor};
use futures::{executor::block_on, join};

use super::*;
use crate::services::bsv_multisig::bsv_multisig_payment_request_from_script;
use crate::services::bsv_multisig::testkit::member;
use crate::services::portal::testkit::{MemoryPublicPaymailProvider, PublicPaymailContext, public_paymail_replies as replies};

fn policy_script(count: u8) -> String {
    policy_descriptor(&Policy {
        threshold: 2,
        public_keys: (1..=count).map(|index| member(index, true).public_key).collect(),
    })
    .unwrap()
    .locking_script
}

#[test]
fn test_typed_public_paymail_accepts_canonical_p2ms_without_fabricating_an_address_and_preserves_legacy_rejection() {
    block_on(async {
        let provider = Arc::new(MemoryPublicPaymailProvider::default());
        let script = policy_script(2);
        replies(&provider, &script, Some("public-fixture-reference"));
        let service = BsvHomePaymailDestinationService::new(provider.clone());
        assert_eq!(
            service.resolve_output(" Team@Example.com ".into(), None).await.unwrap(),
            BsvHomePaymailResolvedDestination {
                handle: "team@example.com".into(),
                output: BsvHomePaymailOutput::P2ms {
                    request: bsv_multisig_payment_request_from_script("team@example.com".into(), script.clone(), "".into()).unwrap(),
                    output_script: script.clone()
                },
                reference: Some("public-fixture-reference".into()),
            }
        );
        replies(&provider, &script, None);
        assert_eq!(service.destination("team@example.com".into(), None).await, Err(BsvHomePaymailError::UnsupportedOutput));
    });
}

#[test]
fn test_typed_output_accepts_only_sdk_canonical_policies_and_normalizes_hex_encoding() {
    block_on(async {
        for count in 2..=5 {
            let script = policy_script(count);
            let provider = Arc::new(MemoryPublicPaymailProvider::default());
            replies(&provider, &script.to_ascii_uppercase(), None);
            let service = BsvHomePaymailDestinationService::new(provider);
            assert_eq!(
                service.resolve_output("team@example.com".into(), Some(42_000)).await.unwrap().output,
                BsvHomePaymailOutput::P2ms {
                    request: bsv_multisig_payment_request_from_script("team@example.com".into(), script.clone(), "0.00042".into()).unwrap(),
                    output_script: script,
                }
            );
        }
        let valid = policy_script(2);
        let first = &valid[4..70];
        let second = &valid[72..138];
        let invalid = [
            format!("5121{first}21{second}52ae"),
            format!("5221{second}21{first}52ae"),
            format!("5221{first}21{first}52ae"),
            format!("5221{first}2102{}52ae", "ff".repeat(32)),
            format!("524c21{first}21{second}52ae"),
            format!("a914{}87", "00".repeat(20)),
            "6a00".into(),
        ];
        for output in invalid {
            assert_eq!(destination_output("team@example.com", &output, None), Err(BsvHomePaymailError::UnsupportedOutput));
        }
    });
}

#[test]
fn test_public_multisig_paymail_keeps_full_handle_and_rejects_the_existing_payment_name_limit_without_truncation() {
    block_on(async {
        let script = policy_script(2);
        for length in [80, 81] {
            let handle = format!("{}@{}.com", "a".repeat(64), "b".repeat(length - 69));
            assert_eq!(handle.len(), length);
            let provider = Arc::new(MemoryPublicPaymailProvider::default());
            replies(&provider, &script, None);
            let service = BsvHomePaymailDestinationService::new(provider);
            let result = service.resolve_output(handle.clone(), None).await;
            if length == 80 {
                assert_eq!(
                    result.unwrap().output,
                    BsvHomePaymailOutput::P2ms {
                        request: bsv_multisig_payment_request_from_script(handle, script.clone(), "".into()).unwrap(),
                        output_script: script.clone()
                    }
                );
            } else {
                assert_eq!(result, Err(BsvHomePaymailError::UnsupportedOutput));
            }
        }
        let handle = format!("{}@{}.com", "a".repeat(64), "b".repeat(12));
        let provider = Arc::new(MemoryPublicPaymailProvider::default());
        replies(&provider, "76a914000000000000000000000000000000000000000088ac", None);
        let service = BsvHomePaymailDestinationService::new(provider);
        assert_eq!(service.destination(handle.clone(), None).await.unwrap().handle, handle);
    });
}

#[test]
fn test_public_resolution_rechecks_sender_after_cooldown_queue_before_sending_any_public_request() {
    block_on(async {
        let context = PublicPaymailContext::new();
        context.replies(&policy_script(2), None);
        let service = BsvHomePaymailDestinationService::new_for_wallet(context.provider.clone(), context.sender.clone());
        let lock = service.retry_after.lock().await;
        let (result, ()) = join!(service.prepare_for_send("team@example.com".into()), async {
            context.switch_wallet();
            drop(lock);
        });
        assert_eq!(
            result.err(),
            Some(BsvHomePaymailError::Auth {
                error: BsvHomePortalAuthError::WalletChanged
            })
        );
        assert_eq!(context.provider.requests.lock().unwrap().len(), 0);
    });
}
