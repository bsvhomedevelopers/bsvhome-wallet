use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::channel::oneshot;

use crate::alien::{AlienError, AlienProvider, AlienResponse, AlienTarget};
use crate::services::portal::testkit::{MemoryPaymailProvider, PaymailSubmission};

pub struct PasswordProvider {
    pub requests: Mutex<Vec<AlienTarget>>,
    responses: Mutex<VecDeque<Arc<AlienResponse>>>,
    pause: Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
    authentication: MemoryPaymailProvider,
}

impl PasswordProvider {
    pub fn new(responses: &[(u16, &str)]) -> Arc<Self> {
        Arc::new(Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(
                responses
                    .iter()
                    .map(|(status, data)| Arc::new(AlienResponse::new(Some(*status), data.as_bytes().to_vec())))
                    .collect(),
            ),
            pause: Mutex::new(None),
            authentication: MemoryPaymailProvider::new(PaymailSubmission::Confirmed),
        })
    }

    pub fn paths(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|request| request.url.strip_prefix("https://api-dev.hj0252.com/api/portal").unwrap().to_string())
            .collect()
    }

    pub fn pause_next(&self) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (started, entered) = oneshot::channel();
        let (release, wait) = oneshot::channel();
        *self.pause.lock().unwrap() = Some((started, wait));
        (entered, release)
    }
}

impl Debug for PasswordProvider {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PasswordProvider")
    }
}

#[async_trait]
impl AlienProvider for PasswordProvider {
    async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
        self.requests.lock().unwrap().push(target.clone());
        if target.url.ends_with("/wallet-auth/challenge") || target.url.ends_with("/wallet-auth/login") {
            return self.authentication.request(target).await;
        }
        let pause = self.pause.lock().unwrap().take();
        if let Some((started, release)) = pause {
            started.send(()).unwrap();
            release.await.unwrap();
        }
        Ok(self.responses.lock().unwrap().pop_front().unwrap())
    }
}
