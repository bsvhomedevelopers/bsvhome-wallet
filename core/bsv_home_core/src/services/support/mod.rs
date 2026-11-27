pub mod model;
pub mod rules;
pub mod store;

use crate::services::error::BsvHomeServiceError;
use std::future::Future;
use std::sync::Arc;

use chrono::Utc;
use primitives::{SupportMessage, SupportMessageInput, SupportMessageStatus};
use uuid::Uuid;

use crate::alien::AlienProvider;
use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};
use crate::services::file::{BsvHomeFileStore, download};

pub use model::BsvHomeSupportChatGroup;
pub use store::BsvHomeSupportStore;

#[derive(uniffi::Object)]
pub struct BsvHomeSupportService {
    api: Arc<BsvHomeDeviceApiClient>,
    store: Arc<dyn BsvHomeSupportStore>,
    files: Arc<dyn BsvHomeFileStore>,
    provider: Arc<dyn AlienProvider>,
}

#[uniffi::export]
impl BsvHomeSupportService {
    #[uniffi::constructor]
    pub fn new(api: Arc<BsvHomeDeviceApiClient>, store: Arc<dyn BsvHomeSupportStore>, files: Arc<dyn BsvHomeFileStore>, provider: Arc<dyn AlienProvider>) -> Self {
        Self { api, store, files, provider }
    }

    pub async fn image_file(&self, url: String) -> Result<String, BsvHomeServiceError> {
        let file_name = rules::image_file_name(&url);
        if self.files.exists(file_name.clone()) {
            return Ok(self.files.path(file_name));
        }
        let image = download(&self.provider, url).await?;
        self.files.save_named_file(image, file_name)
    }

    pub async fn sync_messages(&self, from_timestamp: u64) -> Result<(), BsvHomeServiceError> {
        let messages = self.api.client.get_support_messages(from_timestamp).await.map_err(BsvHomeApiError::from)?;
        self.store.save_messages(messages).await
    }

    pub async fn send_text(&self, content: String) -> Result<(), BsvHomeServiceError> {
        let message = rules::pending_message(Uuid::new_v4().to_string(), content.clone(), vec![], Utc::now());
        self.deliver(message, self.api.client.send_support_message(SupportMessageInput { content })).await
    }

    pub async fn send_image(&self, image: Vec<u8>, file_name: String, mime_type: String) -> Result<(), BsvHomeServiceError> {
        let id = Uuid::new_v4().to_string();
        let pending_image = rules::pending_image(id.clone(), file_name.clone(), image.len() as u64);
        let message = rules::pending_message(id, String::new(), vec![pending_image], Utc::now());
        self.deliver(message, self.api.client.send_support_image(image, file_name, mime_type)).await
    }

    pub async fn retry_message(&self, message: SupportMessage) -> Result<(), BsvHomeServiceError> {
        if !message.images.is_empty() {
            return Err(BsvHomeServiceError::Unsupported {
                msg: "image messages cannot be retried".to_string(),
            });
        }
        let content = message.content.clone();
        let message = rules::with_status(message, SupportMessageStatus::Sending);
        self.deliver(message, self.api.client.send_support_message(SupportMessageInput { content })).await
    }
}

impl BsvHomeSupportService {
    async fn deliver<F, E>(&self, message: SupportMessage, send: F) -> Result<(), BsvHomeServiceError>
    where
        F: Future<Output = Result<SupportMessage, E>>,
        BsvHomeApiError: From<E>,
    {
        self.store.save_messages(vec![message.clone()]).await?;
        match send.await {
            Ok(sent) => self.store.save_message(message.id, sent).await,
            Err(error) => {
                self.store.save_messages(vec![rules::with_status(message, SupportMessageStatus::Failed)]).await?;
                Err(BsvHomeApiError::from(error).into())
            }
        }
    }
}
