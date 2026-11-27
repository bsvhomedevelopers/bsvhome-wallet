use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::channel::oneshot;

use crate::alien::{AlienError, AlienProvider, AlienResponse, AlienTarget};
use crate::services::error::BsvHomeServiceError;
use crate::services::preferences::BsvHomeSecureStore;

use super::{BsvHomePlatformAccountError, BsvHomePlatformAuthenticator, BsvHomePlatformPasskeyOperation};

#[derive(Default)]
pub(crate) struct MemoryPlatformSecureStore {
    pub values: Mutex<HashMap<String, String>>,
    pub fail_reads: Mutex<bool>,
}

impl BsvHomeSecureStore for MemoryPlatformSecureStore {
    fn get(&self, key: String) -> Result<Option<String>, BsvHomeServiceError> {
        if *self.fail_reads.lock().unwrap() {
            return Err(BsvHomeServiceError::Store {
                msg: "test storage failure".into(),
            });
        }
        Ok(self.values.lock().unwrap().get(&key).cloned())
    }
    fn set(&self, key: String, value: String) -> Result<(), BsvHomeServiceError> {
        self.values.lock().unwrap().insert(key, value);
        Ok(())
    }
    fn remove(&self, key: String) -> Result<(), BsvHomeServiceError> {
        self.values.lock().unwrap().remove(&key);
        Ok(())
    }
}

pub(crate) enum PlatformReply {
    Data(u16, Vec<u8>),
    Paused(oneshot::Sender<()>, oneshot::Receiver<()>, u16, Vec<u8>),
}

#[derive(Default)]
pub(crate) struct MemoryPlatformProvider {
    pub requests: Mutex<Vec<AlienTarget>>,
    pub replies: Mutex<VecDeque<PlatformReply>>,
}

impl std::fmt::Debug for MemoryPlatformProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("MemoryPlatformProvider")
    }
}

impl MemoryPlatformProvider {
    pub fn reply(&self, status: u16, body: &str) {
        self.replies.lock().unwrap().push_back(PlatformReply::Data(status, body.as_bytes().to_vec()));
    }
}

#[async_trait]
impl AlienProvider for MemoryPlatformProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        self.requests.lock().unwrap().push(target);
        let response = self.replies.lock().unwrap().pop_front().unwrap();
        let (status, data) = match response {
            PlatformReply::Data(status, data) => (status, data),
            PlatformReply::Paused(started, release, status, data) => {
                started.send(()).unwrap();
                release.await.unwrap();
                (status, data)
            }
        };
        Ok(Arc::new(AlienResponse::new(Some(status), data)))
    }
}

pub(crate) struct MemoryPlatformAuthenticator {
    pub calls: Mutex<Vec<BsvHomePlatformPasskeyOperation>>,
    pub result: Mutex<Result<String, BsvHomePlatformAccountError>>,
    pub verify_result: Mutex<bool>,
}

impl Default for MemoryPlatformAuthenticator {
    fn default() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            result: Mutex::new(Ok(
                r#"{"id":"test","rawId":"dGVzdA","type":"public-key","response":{"clientDataJSON":"eA","authenticatorData":"eA","signature":"eA","attestationObject":"eA"}}"#
                    .into(),
            )),
            verify_result: Mutex::new(false),
        }
    }
}

#[async_trait]
impl BsvHomePlatformAuthenticator for MemoryPlatformAuthenticator {
    async fn authenticate(&self, operation: BsvHomePlatformPasskeyOperation, _: String) -> Result<String, BsvHomePlatformAccountError> {
        self.calls.lock().unwrap().push(operation);
        self.result.lock().unwrap().clone()
    }
    fn verify_offline_proof(&self, _: String, _: String, _: String) -> Result<bool, BsvHomePlatformAccountError> {
        Ok(*self.verify_result.lock().unwrap())
    }
}
