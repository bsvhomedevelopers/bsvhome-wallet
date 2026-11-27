use std::sync::Arc;

use primitives::WalletId;

use crate::alien::AlienProvider;
use crate::services::error::BsvHomeServiceError;
use crate::services::file::{BsvHomeFileStore, download};
use crate::services::wallet::BsvHomeWalletStore;

const IMAGE_EXTENSION: &str = "png";

#[derive(uniffi::Object)]
pub struct BsvHomeAvatarService {
    wallets: Arc<dyn BsvHomeWalletStore>,
    files: Arc<dyn BsvHomeFileStore>,
    provider: Arc<dyn AlienProvider>,
}

#[uniffi::export]
impl BsvHomeAvatarService {
    #[uniffi::constructor]
    pub fn new(wallets: Arc<dyn BsvHomeWalletStore>, files: Arc<dyn BsvHomeFileStore>, provider: Arc<dyn AlienProvider>) -> Self {
        Self { wallets, files, provider }
    }

    pub async fn set_image(&self, wallet_id: WalletId, image: Vec<u8>) -> Result<(), BsvHomeServiceError> {
        let file_name = self.files.save_file(image, IMAGE_EXTENSION.to_string())?;
        self.remove_previous(&wallet_id).await?;
        self.wallets.set_image_url(wallet_id, Some(file_name)).await
    }

    pub async fn set_image_url(&self, wallet_id: WalletId, url: String) -> Result<(), BsvHomeServiceError> {
        let image = download(&self.provider, url).await?;
        self.set_image(wallet_id, image).await
    }

    pub async fn remove_image(&self, wallet_id: WalletId) -> Result<(), BsvHomeServiceError> {
        self.remove_previous(&wallet_id).await?;
        self.wallets.set_image_url(wallet_id, None).await
    }
}

impl BsvHomeAvatarService {
    async fn remove_previous(&self, wallet_id: &WalletId) -> Result<(), BsvHomeServiceError> {
        match self.wallets.get_wallet(wallet_id.clone()).await?.and_then(|wallet| wallet.image_url) {
            Some(previous) => self.files.remove(previous),
            None => Ok(()),
        }
    }
}
