use std::sync::atomic::AtomicBool;

use super::*;
use crate::services::portal::paymail_tests::configuration;

fn checkpoint(active: &AtomicBool) -> Result<(), BsvHomePortalAuthError> {
    if active.load(Ordering::SeqCst) {
        Ok(())
    } else {
        Err(BsvHomeServiceError::Cancelled.into())
    }
}

fn write() -> BsvMultisigTarget {
    BsvMultisigTarget::Create {
        body: Value::Object(Default::default()),
    }
}

#[test]
fn test_action_checkpoint_rejects_first_poll_and_connection_queue_before_login_without_cancelling_auth() {
    block_on(async {
        for queued in [false, true] {
            let context = Context::new();
            let auth = context.service();
            let active = AtomicBool::new(true);
            let check = || checkpoint(&active);
            let lock = auth.connection.lock().await;
            let request = auth.multisig_checkpoint_request_at_generation::<Value>(&context.wallet, write(), 0, &check, &|| Ok(()));
            let result = if queued {
                let (result, ()) = join!(request, async {
                    active.store(false, Ordering::SeqCst);
                    drop(lock);
                });
                result
            } else {
                active.store(false, Ordering::SeqCst);
                drop(lock);
                request.await
            };
            assert_eq!(result, Err(BsvHomeServiceError::Cancelled.into()));
            assert_eq!(auth.multisig_version(), 0);
            assert_eq!(context.provider.paths(), Vec::<String>::new());
            assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
            assert_eq!(context.store.writes.load(Ordering::Relaxed), 0);
        }
    });
}

#[test]
fn test_action_checkpoint_rechecks_challenge_login_and_read_retry_without_new_business_requests() {
    block_on(async {
        for pause in 0..3 {
            let context = Context::new();
            let auth = context.service();
            let active = AtomicBool::new(true);
            let check = || checkpoint(&active);
            let (mut step, started, release) = Step::paused();
            if pause == 2 {
                context.cache(context.cached_session(None));
                step.reply = Reply::Rejected {
                    status: 401,
                    success: false,
                    code: "USER_SESSION_EXPIRED",
                    force_logout: true,
                };
            }
            *context.provider.steps.lock().unwrap() = (0..usize::from(pause == 1)).map(|_| Step::reply(Reply::Success)).chain([step]).collect();
            let (result, ()) = join!(
                auth.multisig_checkpoint_request_at_generation::<Value>(&context.wallet, BsvMultisigTarget::Wallets, 0, &check, &|| Ok(())),
                async {
                    started.await.unwrap();
                    active.store(false, Ordering::SeqCst);
                    release.send(()).unwrap();
                }
            );
            assert_eq!(result, Err(BsvHomeServiceError::Cancelled.into()));
            assert_eq!(
                context.provider.paths(),
                if pause == 2 {
                    vec![MULTISIG_WALLETS_PATH]
                } else {
                    vec![CHALLENGE_PATH, LOGIN_PATH][..=pause].to_vec()
                }
            );
            assert_eq!(context.password.create_requests.lock().unwrap().len(), usize::from(pause == 1));
            assert_eq!(context.store.writes.load(Ordering::Relaxed), 0);
        }
    });
}

struct CancellingPassword {
    active: Arc<AtomicBool>,
    calls: AtomicUsize,
}

impl BsvHomeKeystorePassword for CancellingPassword {
    fn get_password(&self, _: bool) -> Result<String, BsvHomeServiceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.active.store(false, Ordering::SeqCst);
        Ok(TEST_PASSWORD.into())
    }
    fn get_wallet_password(&self, _: WalletId) -> Result<Option<String>, BsvHomeServiceError> {
        Err(BsvHomeServiceError::Cancelled)
    }
    fn delete_wallet_password(&self, _: WalletId) -> Result<(), BsvHomeServiceError> {
        Err(BsvHomeServiceError::Cancelled)
    }
    fn authentication(&self) -> Result<BsvHomeKeystoreAuthentication, BsvHomeServiceError> {
        Err(BsvHomeServiceError::Cancelled)
    }
}

#[test]
fn test_action_checkpoint_rechecks_password_callback_before_challenge_signing_and_business_submission() {
    block_on(async {
        let context = Context::new();
        let active = Arc::new(AtomicBool::new(true));
        let password = Arc::new(CancellingPassword {
            active: active.clone(),
            calls: AtomicUsize::new(0),
        });
        let auth = context.service_with_password(password.clone());
        let check = || checkpoint(&active);
        assert_eq!(
            auth.multisig_checkpoint_request_at_generation::<Value>(&context.wallet, write(), 0, &check, &|| Ok(()))
                .await,
            Err(BsvHomeServiceError::Cancelled.into())
        );
        assert_eq!(password.calls.load(Ordering::SeqCst), 1);
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH]);
        assert_eq!(context.store.writes.load(Ordering::Relaxed), 0);
    });
}

#[test]
fn test_action_business_checkpoint_marks_only_business_requests_and_never_replays_mutations() {
    block_on(async {
        let context = Context::new();
        let auth = context.service();
        let sends = AtomicUsize::new(0);
        let before = || {
            sends.fetch_add(1, Ordering::SeqCst);
            Ok(())
        };
        context.provider.replies(vec![
            Reply::Success,
            Reply::Success,
            Reply::Rejected {
                status: 401,
                success: false,
                code: "USER_SESSION_EXPIRED",
                force_logout: true,
            },
        ]);
        let result = auth
            .multisig_checkpoint_request_at_generation::<Value>(&context.wallet, write(), 0, &|| Ok(()), &before)
            .await;
        assert!(result.is_err());
        assert_eq!(sends.load(Ordering::SeqCst), 1);
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, MULTISIG_WALLETS_PATH]);
        assert_eq!(context.password.create_requests.lock().unwrap().len(), 1);
        let context = Context::new();
        let auth = context.service();
        assert_eq!(
            auth.multisig_checkpoint_request_at_generation::<Value>(&context.wallet, write(), 0, &|| Ok(()), &|| Err(BsvHomeServiceError::Cancelled.into()))
                .await,
            Err(BsvHomeServiceError::Cancelled.into())
        );
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH]);
        let context = Context::new();
        let auth = context.service();
        assert_eq!(
            auth.multisig_checkpoint_request_at_generation::<Vec<Value>>(
                &context.wallet,
                BsvMultisigTarget::Wallets,
                0,
                &|| Ok(()),
                &|| Err(BsvHomeServiceError::Cancelled.into())
            )
            .await,
            Ok(Vec::new())
        );
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH, MULTISIG_WALLETS_PATH]);
    });
}

#[test]
fn test_action_cancellation_during_session_commit_removes_only_that_session_and_never_submits() {
    block_on(async {
        let context = Context::new();
        let auth = context.service();
        let active = Arc::new(AtomicBool::new(true));
        let cancelled = active.clone();
        *context.store.on_write.lock().unwrap() = Some(Box::new(move || cancelled.store(false, Ordering::SeqCst)));
        let check = || checkpoint(&active);
        assert_eq!(
            auth.multisig_checkpoint_request_at_generation::<Value>(&context.wallet, write(), 0, &check, &|| Ok(()))
                .await,
            Err(BsvHomeServiceError::Cancelled.into())
        );
        assert_eq!(context.provider.paths(), [CHALLENGE_PATH, LOGIN_PATH]);
        assert_eq!(context.store.writes.load(Ordering::SeqCst), 1);
        assert_eq!(context.store.get(SESSION.into()).unwrap(), None);
        assert_eq!(auth.multisig_version(), 0);
    });
}

#[test]
fn test_paymail_checked_load_rejects_cancel_before_poll_and_after_auth_queue_without_auth_side_effects() {
    block_on(async {
        for queued in [false, true] {
            let context = Context::new();
            context.provider.replies(vec![
                Reply::Success,
                Reply::Success,
                Reply::Raw {
                    status: 200,
                    data: response(200, configuration()).to_rpc_response().data,
                },
            ]);
            let auth = Arc::new(context.service());
            let sender = BsvHomePaymailService::new(auth.clone(), vec!["https://api-dev.hj0252.com".into()], context.wallet.id.id()).unwrap();
            let active = AtomicBool::new(true);
            let check = || checkpoint(&active);
            let lock = auth.connection.lock().await;
            let operation = sender.load_with_checkpoint(&check);
            let result = if queued {
                let (result, ()) = join!(operation, async {
                    active.store(false, Ordering::SeqCst);
                    drop(lock);
                });
                result
            } else {
                active.store(false, Ordering::SeqCst);
                drop(lock);
                operation.await
            };
            assert_eq!(result, Err(BsvHomeServiceError::Cancelled.into()));
            assert_eq!(context.provider.paths(), Vec::<String>::new());
            assert_eq!(*context.password.create_requests.lock().unwrap(), Vec::<bool>::new());
            assert_eq!(auth.multisig_version(), 0);
        }
    });
}
