use std::sync::Arc;

use primitives::{ScanTransaction, ScanTransactionPayload};

use crate::api::{BsvHomeApiError, BsvHomeDeviceApiClient};

#[derive(Debug, uniffi::Object)]
pub struct BsvHomeScanService {
    api: Arc<BsvHomeDeviceApiClient>,
}

#[uniffi::export]
impl BsvHomeScanService {
    #[uniffi::constructor]
    pub fn new(api: Arc<BsvHomeDeviceApiClient>) -> Self {
        Self { api }
    }
}

impl BsvHomeScanService {
    pub async fn scan_transaction(&self, payload: ScanTransactionPayload) -> Result<ScanTransaction, BsvHomeApiError> {
        Ok(self.api.client.scan_transaction(payload).await?)
    }
}
