use std::collections::{HashMap, VecDeque};
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::tests::Context;
use super::{BsvHomePaymailService, BsvHomePortalAuthService};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use bsv_home_bsv::signer::BsvChainSigner;
use chrono::{Duration, SecondsFormat, Utc};
use futures::channel::oneshot;
use primitives::{Chain, Wallet};
use serde::{Deserialize, Serialize};

use crate::alien::{AlienError, AlienProvider, AlienResponse, AlienTarget};

use crate::services::error::BsvHomeServiceError;

use super::BsvHomePortalSwapStore;
use super::paymail_model::{Binding, BindingChallenge, Configuration};

#[derive(Default)]
pub struct MemoryPortalSwapStore {
    pub values: Mutex<HashMap<String, String>>,
    pub fail_write: Mutex<bool>,
}

impl BsvHomePortalSwapStore for MemoryPortalSwapStore {
    fn get(&self, scope: String) -> Result<Option<String>, BsvHomeServiceError> {
        Ok(self.values.lock().unwrap().get(&scope).cloned())
    }

    fn set(&self, scope: String, value: String) -> Result<(), BsvHomeServiceError> {
        if *self.fail_write.lock().unwrap() {
            return Err(BsvHomeServiceError::Store {
                msg: "swap storage unavailable".into(),
            });
        }
        self.values.lock().unwrap().insert(scope, value);
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) enum PaymailSubmission {
    Confirmed,
    OfflineCommitted,
    OfflineMissing,
    OfflineAdvanced,
    Unauthorized,
    InvalidSuccess,
    OfflineUnverified,
}

pub(super) struct MemoryPaymailProvider {
    pub requests: Mutex<Vec<AlienTarget>>,
    pub bindings: Mutex<Vec<Binding>>,
    pub attempted_binding: Mutex<Option<Binding>>,
    pub submission: PaymailSubmission,
    pub challenge_pause: Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
    challenge: Mutex<Option<(String, Binding)>>,
    submitted: Mutex<bool>,
}

impl MemoryPaymailProvider {
    pub fn new(submission: PaymailSubmission) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            bindings: Mutex::new(Vec::new()),
            attempted_binding: Mutex::new(None),
            submission,
            challenge_pause: Mutex::new(None),
            challenge: Mutex::new(None),
            submitted: Mutex::new(false),
        }
    }

    pub fn paths(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.url.strip_prefix("https://api-dev.hj0252.com/api/portal").unwrap().into())
            .collect()
    }
}

impl Debug for MemoryPaymailProvider {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("MemoryPaymailProvider")
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaymailEnvelope<T> {
    success: bool,
    data: T,
}

fn paymail_response<T: Serialize>(data: T) -> Arc<AlienResponse> {
    Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&PaymailEnvelope { success: true, data }).unwrap()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PaymailChallengeRequest {
    alias: String,
    domain: String,
    address: String,
    identity_public_key: String,
    active: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PaymailBindingRequest {
    challenge_id: String,
    address_signature: String,
    identity_signature: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaymailLoginRequest {
    address: String,
    device_id: String,
    device_type: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaymailLoginChallenge {
    challenge_id: String,
    address: String,
    chain: String,
    message: String,
    expires_at: String,
}

#[derive(Serialize)]
struct PaymailLogin {
    token: String,
    address: String,
    chain: String,
}

#[async_trait]
impl AlienProvider for MemoryPaymailProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        self.requests.lock().unwrap().push(target.clone());
        let path = target.url.strip_prefix("https://api-dev.hj0252.com/api/portal").unwrap();
        match path {
            "/wallet-auth/challenge" => {
                let identity: PaymailLoginRequest = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                let now = Utc::now();
                let expiry = (now + Duration::minutes(5)).to_rfc3339_opts(SecondsFormat::AutoSi, true);
                Ok(paymail_response(PaymailLoginChallenge {
                    challenge_id: "C0123456789abcdef0123456789abcdef".into(),
                    message: format!(
                        "WowPay Wallet Auth\nAddress: {}\nNonce: 0123456789abcdef0123456789abcdef\nIssued At: {}\nExpiration Time: {expiry}\nDevice: {}\nDevice Type: {}",
                        identity.address,
                        now.to_rfc3339_opts(SecondsFormat::AutoSi, true),
                        identity.device_id,
                        identity.device_type
                    ),
                    address: identity.address,
                    chain: "BSV".into(),
                    expires_at: expiry,
                }))
            }
            "/wallet-auth/login" => {
                let identity: PaymailLoginRequest = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                Ok(paymail_response(PaymailLogin {
                    token: "paymail-test-token".into(),
                    address: identity.address,
                    chain: "BSV".into(),
                }))
            }
            "/auth/paymail/config" => Ok(paymail_response(Configuration {
                domains: vec!["api-dev.hj0252.com".into()],
                public_base_url: "https://api-dev.hj0252.com".into(),
                network: "BSV_MAINNET".into(),
                address_type: "P2PKH".into(),
                identity_key_rotation: false,
            })),
            "/auth/paymail/me" => {
                if *self.submitted.lock().unwrap() && matches!(self.submission, PaymailSubmission::OfflineUnverified) {
                    return Err(AlienError::Offline);
                }
                Ok(paymail_response(self.bindings.lock().unwrap().clone()))
            }
            "/auth/paymail/challenges" => {
                let pause = self.challenge_pause.lock().unwrap().take();
                if let Some((started, release)) = pause {
                    started.send(()).unwrap();
                    release.await.unwrap();
                }
                let request: PaymailChallengeRequest = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                let version = self
                    .bindings
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|binding| binding.handle.ends_with(&format!("@{}", request.domain)))
                    .map_or(0, |binding| binding.version);
                let expires_at = (Utc::now() + Duration::minutes(5)).to_rfc3339();
                let challenge = BindingChallenge {
                    challenge_id: "11111111-1111-4111-8111-111111111111".into(),
                    message: format!(
                        "WowPay Paymail Binding\nOrigin: https://api-dev.hj0252.com\nUser: 1\nHandle: {}@{}\nNetwork: BSV_MAINNET\nAddress: {}\nIdentity Key: {}\nActive: {}\nVersion: {version}\nNonce: 11111111-1111-4111-8111-111111111111\nExpires: {expires_at}",
                        request.alias, request.domain, request.address, request.identity_public_key, request.active
                    ),
                    expires_at,
                };
                *self.challenge.lock().unwrap() = Some((
                    challenge.message.clone(),
                    Binding {
                        handle: format!("{}@{}", request.alias, request.domain),
                        address: request.address,
                        identity_public_key: request.identity_public_key,
                        active: request.active,
                        version: version + 1,
                    },
                ));
                Ok(paymail_response(challenge))
            }
            "/auth/paymail/bindings" => {
                *self.submitted.lock().unwrap() = true;
                let request: PaymailBindingRequest = serde_json::from_slice(target.body.as_ref().unwrap()).unwrap();
                assert_eq!(request.challenge_id, "11111111-1111-4111-8111-111111111111");
                let (message, mut binding) = self.challenge.lock().unwrap().take().unwrap();
                assert_eq!(
                    BsvChainSigner::recover_message_address(&message, &BASE64.decode(&request.address_signature).unwrap()).unwrap(),
                    binding.address
                );
                assert_eq!(
                    hex::encode(BsvChainSigner::recover_message_public_key(&message, &BASE64.decode(&request.identity_signature).unwrap()).unwrap()),
                    binding.identity_public_key
                );
                *self.attempted_binding.lock().unwrap() = Some(binding.clone());
                match self.submission {
                    PaymailSubmission::OfflineMissing | PaymailSubmission::OfflineUnverified => return Err(AlienError::Offline),
                    PaymailSubmission::Unauthorized => return Ok(Arc::new(AlienResponse::new(Some(401), br#"{"code":"USER_SESSION_EXPIRED","message":"expired"}"#.to_vec()))),
                    PaymailSubmission::OfflineAdvanced => {
                        binding.version += 1;
                        binding.active = !binding.active;
                    }
                    PaymailSubmission::Confirmed | PaymailSubmission::OfflineCommitted | PaymailSubmission::InvalidSuccess => {}
                }
                let mut bindings = self.bindings.lock().unwrap();
                bindings.retain(|previous| previous.handle != binding.handle);
                bindings.push(binding.clone());
                match self.submission {
                    PaymailSubmission::OfflineCommitted | PaymailSubmission::OfflineAdvanced => Err(AlienError::Offline),
                    PaymailSubmission::InvalidSuccess => Ok(Arc::new(AlienResponse::new(Some(200), b"{}".to_vec()))),
                    PaymailSubmission::Confirmed => Ok(paymail_response(binding)),
                    PaymailSubmission::OfflineMissing | PaymailSubmission::Unauthorized | PaymailSubmission::OfflineUnverified => unreachable!(),
                }
            }
            _ => panic!("unexpected Paymail endpoint"),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct MemoryPublicPaymailProvider {
    pub requests: Mutex<Vec<AlienTarget>>,
    pub request_times: Mutex<Vec<Instant>>,
    pub replies: Mutex<VecDeque<Arc<AlienResponse>>>,
    pub request_pause: Mutex<Option<(usize, oneshot::Sender<()>, oneshot::Receiver<()>)>>,
}

#[async_trait]
impl AlienProvider for MemoryPublicPaymailProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        self.request_times.lock().unwrap().push(Instant::now());
        let request_count = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(target);
            requests.len()
        };
        let response = self.replies.lock().unwrap().pop_front().unwrap();
        let pause = {
            let mut pause = self.request_pause.lock().unwrap();
            if pause.as_ref().is_some_and(|(index, _, _)| *index == request_count) {
                pause.take()
            } else {
                None
            }
        };
        if let Some((_, reached, resume)) = pause {
            let _ = reached.send(());
            let _ = resume.await;
        }
        Ok(response)
    }
}

pub(crate) const TEST_PAYMAIL_PUBLIC_KEY: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

pub(crate) struct PublicPaymailContext {
    context: Context,
    management: Arc<MemoryPaymailProvider>,
    pub auth: Arc<BsvHomePortalAuthService>,
    pub provider: Arc<MemoryPublicPaymailProvider>,
    pub sender: Arc<BsvHomePaymailService>,
    pub wallet: Wallet,
}

impl PublicPaymailContext {
    pub fn new() -> Self {
        let context = Context::new();
        let management = Arc::new(MemoryPaymailProvider::new(PaymailSubmission::Confirmed));
        management.bindings.lock().unwrap().push(Binding {
            handle: "sender@api-dev.hj0252.com".into(),
            address: context.wallet.account(Chain::BitcoinSv).unwrap().address.clone(),
            identity_public_key: TEST_PAYMAIL_PUBLIC_KEY.into(),
            active: true,
            version: 1,
        });
        let auth = context.auth_service(management.clone());
        let sender = Arc::new(BsvHomePaymailService::new(auth.clone(), vec!["https://api-dev.hj0252.com".into()], context.wallet.id.id()).unwrap());
        let wallet = context.wallet.clone();
        Self {
            context,
            management,
            auth,
            sender,
            wallet,
            provider: Arc::new(MemoryPublicPaymailProvider::default()),
        }
    }

    pub fn switch_wallet(&self) {
        self.context.switch_wallet();
    }
    pub fn replace_wallet(&self, wallet: Option<Wallet>) {
        self.context.replace_wallet(wallet);
    }
    pub fn set_sender_handle(&self, handle: String) {
        self.management.bindings.lock().unwrap()[0].handle = handle;
    }
    pub fn management_request_count(&self) -> usize {
        self.management.requests.lock().unwrap().len()
    }
    pub fn replies(&self, output: &str, reference: Option<&str>) {
        public_paymail_replies(&self.provider, output, reference);
    }
}

#[derive(Serialize)]
struct PublicDestinationOutput<'a> {
    output: &'a str,
    reference: Option<&'a str>,
}

pub(crate) fn public_paymail_replies(provider: &MemoryPublicPaymailProvider, output: &str, reference: Option<&str>) {
    provider.replies.lock().unwrap().extend([
        Arc::new(AlienResponse::new(
            Some(200),
            br#"{"bsvalias":"1.0","capabilities":{"paymentDestination":"https://delegate.example/{alias}@{domain.tld}"}}"#.to_vec(),
        )),
        Arc::new(AlienResponse::new(Some(200), serde_json::to_vec(&PublicDestinationOutput { output, reference }).unwrap())),
    ]);
}
