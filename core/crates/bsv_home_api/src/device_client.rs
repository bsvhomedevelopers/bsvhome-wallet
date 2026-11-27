use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bsv_home_auth::build_device_auth_header;
use bsv_home_client::{ClientError, ContentType, build_request_url, deserialize_response};
use bsv_home_jsonrpc::{RpcClientError, RpcProvider, Target};
use primitives::name::NameRecord;
use primitives::rewards::{RedemptionRequest, RedemptionResult};
use primitives::{
    AddressName, AuthNonce, AuthenticatedRequest, Chain, ChainAddress, ChartPeriod, Device, FiatQuoteType, FiatQuoteUrl, FiatQuotes, FiatTransactionData, InAppNotification,
    NFTAssetData, NFTAssetId, NFTData, PortfolioAssets, PortfolioAssetsRequest, PriceAlert, ReferralCode, ReportNft, Rewards, ScanTransaction, ScanTransactionPayload,
    SupportMessage, SupportMessageInput, TransactionsResponse, WalletConfigurationResult, WalletSubscription, WalletSubscriptionChains, transactions_page_limit,
};
use serde::de::DeserializeOwned;

use crate::device_target::{BsvHomeDeviceApiBody, BsvHomeDeviceApiTarget};

#[async_trait::async_trait]
pub trait WalletRequestPreflight: Send + Sync + std::fmt::Debug {
    async fn prepare(&self, wallet_id: &str, chain: Option<Chain>) -> Result<(), ClientError>;
}

pub trait DeviceKey: Send + Sync + std::fmt::Debug {
    fn private_key(&self) -> Result<Vec<u8>, ClientError>;
}

/// Signs `/v2/devices/*` requests with the device Ed25519 key.
///
/// The key is read when a request is signed, so building the client never reaches secure
/// storage. It is a device-scoped credential, unrelated to wallet keys or recovery material,
/// and its source caches it after the first read.
#[derive(Debug, Clone)]
pub struct BsvHomeDeviceApiClient<E: RpcClientError> {
    base_url: String,
    provider: Arc<dyn RpcProvider<Error = E>>,
    device_key: Arc<dyn DeviceKey>,
    preflight: std::sync::OnceLock<Arc<dyn WalletRequestPreflight>>,
}

impl<E: RpcClientError> BsvHomeDeviceApiClient<E> {
    pub fn new(base_url: String, provider: Arc<dyn RpcProvider<Error = E>>, device_key: Arc<dyn DeviceKey>) -> Self {
        Self {
            base_url,
            provider,
            device_key,
            preflight: std::sync::OnceLock::new(),
        }
    }

    /// Installed after construction because the preflight needs the service that owns this client.
    pub fn set_preflight(&self, preflight: Arc<dyn WalletRequestPreflight>) {
        let _ = self.preflight.set(preflight);
    }

    pub async fn get_device(&self) -> Result<Option<Device>, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetDevice).await
    }

    pub async fn add_device(&self, device: Device) -> Result<Device, ClientError> {
        self.send(BsvHomeDeviceApiTarget::AddDevice(device)).await
    }

    pub async fn update_device(&self, device: Device) -> Result<Device, ClientError> {
        self.send(BsvHomeDeviceApiTarget::UpdateDevice(device)).await
    }

    pub async fn is_device_registered(&self) -> Result<bool, ClientError> {
        self.send(BsvHomeDeviceApiTarget::IsDeviceRegistered).await
    }

    pub async fn get_auth_nonce(&self) -> Result<AuthNonce, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetAuthNonce).await
    }

    pub async fn get_subscriptions(&self) -> Result<Vec<WalletSubscriptionChains>, ClientError> {
        let subscriptions: Option<Vec<WalletSubscriptionChains>> = self.send(BsvHomeDeviceApiTarget::GetSubscriptions).await?;
        Ok(subscriptions.unwrap_or_default())
    }

    pub async fn add_subscriptions(&self, subscriptions: Vec<WalletSubscription>) -> Result<(), ClientError> {
        self.send::<serde_json::Value>(BsvHomeDeviceApiTarget::AddSubscriptions(subscriptions)).await?;
        Ok(())
    }

    pub async fn delete_subscriptions(&self, subscriptions: Vec<WalletSubscriptionChains>) -> Result<(), ClientError> {
        self.send::<serde_json::Value>(BsvHomeDeviceApiTarget::DeleteSubscriptions(subscriptions)).await?;
        Ok(())
    }

    pub async fn get_price_alerts(&self, asset_id: Option<String>) -> Result<Vec<PriceAlert>, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetPriceAlerts { asset_id }).await
    }

    pub async fn add_price_alerts(&self, alerts: Vec<PriceAlert>) -> Result<(), ClientError> {
        self.send::<serde_json::Value>(BsvHomeDeviceApiTarget::AddPriceAlerts(alerts)).await?;
        Ok(())
    }

    pub async fn delete_price_alerts(&self, alerts: Vec<PriceAlert>) -> Result<(), ClientError> {
        self.send::<serde_json::Value>(BsvHomeDeviceApiTarget::DeletePriceAlerts(alerts)).await?;
        Ok(())
    }

    pub async fn get_transactions(&self, wallet_id: String, asset_id: Option<String>, from_timestamp: u64) -> Result<TransactionsResponse, ClientError> {
        let mut transactions = Vec::new();
        let mut address_names = Vec::new();

        while let Some(limit) = transactions_page_limit(transactions.len()) {
            let offset = transactions.len();
            let response: TransactionsResponse = self
                .send(BsvHomeDeviceApiTarget::GetTransactions {
                    wallet_id: wallet_id.clone(),
                    asset_id: asset_id.clone(),
                    from_timestamp,
                    limit,
                    offset,
                })
                .await?;
            let page_size = response.transactions.len();

            transactions.extend(response.transactions);
            for address_name in response.address_names {
                if !address_names.contains(&address_name) {
                    address_names.push(address_name);
                }
            }

            if page_size < limit {
                break;
            }
        }

        Ok(TransactionsResponse::new(transactions, address_names))
    }

    pub async fn get_assets_list(&self, wallet_id: String, from_timestamp: u64) -> Result<Vec<String>, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetAssetsList { wallet_id, from_timestamp }).await
    }

    pub async fn get_wallet_configuration(&self, wallet_id: String) -> Result<WalletConfigurationResult, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetWalletConfiguration { wallet_id }).await
    }

    pub async fn scan_transaction(&self, payload: ScanTransactionPayload) -> Result<ScanTransaction, ClientError> {
        self.send(BsvHomeDeviceApiTarget::ScanTransaction(payload)).await
    }

    pub async fn get_nft_assets(&self, wallet_id: String) -> Result<Vec<NFTData>, ClientError> {
        Ok(self
            .send::<Option<Vec<NFTData>>>(BsvHomeDeviceApiTarget::GetNftAssets { wallet_id })
            .await?
            .unwrap_or_default())
    }

    pub async fn get_nft_asset(&self, asset_id: NFTAssetId) -> Result<NFTAssetData, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetNftAsset(asset_id)).await
    }

    pub async fn refresh_nft_asset(&self, wallet_id: String, asset_id: NFTAssetId) -> Result<bool, ClientError> {
        self.send(BsvHomeDeviceApiTarget::RefreshNftAsset { wallet_id, asset_id }).await
    }

    pub async fn report_nft(&self, report: ReportNft) -> Result<bool, ClientError> {
        self.send(BsvHomeDeviceApiTarget::ReportNft(report)).await
    }

    pub async fn get_support_messages(&self, from_timestamp: u64) -> Result<Vec<SupportMessage>, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetSupportMessages { from_timestamp }).await
    }

    pub async fn send_support_message(&self, input: SupportMessageInput) -> Result<SupportMessage, ClientError> {
        self.send(BsvHomeDeviceApiTarget::SendSupportMessage(input)).await
    }

    pub async fn send_support_image(&self, image: Vec<u8>, file_name: String, mime_type: String) -> Result<SupportMessage, ClientError> {
        self.send(BsvHomeDeviceApiTarget::SendSupportImage { image, file_name, mime_type }).await
    }

    pub async fn get_rewards(&self, wallet_id: String) -> Result<Rewards, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetRewards { wallet_id }).await
    }

    pub async fn create_referral(&self, wallet_id: String, request: AuthenticatedRequest<ReferralCode>) -> Result<Rewards, ClientError> {
        self.send(BsvHomeDeviceApiTarget::CreateReferral { wallet_id, request }).await
    }

    pub async fn use_referral_code(&self, wallet_id: String, request: AuthenticatedRequest<ReferralCode>) -> Result<bool, ClientError> {
        self.send(BsvHomeDeviceApiTarget::UseReferralCode { wallet_id, request }).await
    }

    pub async fn redeem_rewards(&self, wallet_id: String, request: AuthenticatedRequest<RedemptionRequest>) -> Result<RedemptionResult, ClientError> {
        self.send(BsvHomeDeviceApiTarget::RedeemRewards { wallet_id, request }).await
    }

    pub async fn get_notifications(&self, from_timestamp: u64) -> Result<Vec<InAppNotification>, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetNotifications { from_timestamp }).await
    }

    pub async fn mark_notifications_read(&self) -> Result<(), ClientError> {
        self.send_ignoring_body(BsvHomeDeviceApiTarget::MarkNotificationsRead).await
    }

    pub async fn get_fiat_quotes(&self, wallet_id: String, quote_type: FiatQuoteType, asset_id: String, amount: f64, currency: String) -> Result<FiatQuotes, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetFiatQuotes {
            wallet_id,
            quote_type,
            asset_id,
            amount,
            currency,
        })
        .await
    }

    pub async fn get_fiat_quote_url(&self, wallet_id: String, quote_id: String) -> Result<FiatQuoteUrl, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetFiatQuoteUrl { wallet_id, quote_id }).await
    }

    pub async fn get_fiat_transactions(&self, wallet_id: String) -> Result<Vec<FiatTransactionData>, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetFiatTransactions { wallet_id }).await
    }

    pub async fn get_name_record(&self, name: String, chain: String) -> Result<Option<NameRecord>, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetNameRecord { name, chain }).await
    }

    pub async fn get_address_names(&self, requests: Vec<ChainAddress>) -> Result<Vec<AddressName>, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetAddressNames(requests)).await
    }

    pub async fn get_portfolio_assets(&self, period: ChartPeriod, request: PortfolioAssetsRequest) -> Result<PortfolioAssets, ClientError> {
        self.send(BsvHomeDeviceApiTarget::GetPortfolioAssets { period, request }).await
    }

    async fn send<R: DeserializeOwned>(&self, target: BsvHomeDeviceApiTarget) -> Result<R, ClientError> {
        let response = self.request(target).await?;
        deserialize_response(&response)
    }

    async fn send_ignoring_body(&self, target: BsvHomeDeviceApiTarget) -> Result<(), ClientError> {
        self.request(target).await.map(|_| ())
    }

    async fn request(&self, target: BsvHomeDeviceApiTarget) -> Result<bsv_home_client::Response, ClientError> {
        if let Some(preflight) = self.preflight.get().filter(|_| !target.wallet_id().is_empty()) {
            preflight.prepare(target.wallet_id(), target.wallet_chain()?).await?;
        }
        let path = target.path();
        let body = target.body()?;
        let (bytes, content_type) = match &body {
            Some(BsvHomeDeviceApiBody::Json(bytes)) => (bytes.as_slice(), ContentType::ApplicationJson.as_str().to_string()),
            Some(BsvHomeDeviceApiBody::Raw { data, content_type }) => (data.as_slice(), content_type.clone()),
            None => (&[][..], ContentType::ApplicationJson.as_str().to_string()),
        };

        let mut headers = self.authorization(&target, bytes)?;
        headers.insert("Content-Type".to_string(), content_type);

        let request = Target {
            url: build_request_url(&self.base_url, &path),
            method: target.method().into(),
            headers: Some(headers),
            body: body.map(|body| match body {
                BsvHomeDeviceApiBody::Json(bytes) => bytes,
                BsvHomeDeviceApiBody::Raw { data, .. } => data,
            }),
        };
        let response = self.provider.request(request).await.map_err(RpcClientError::into_client_error)?;
        if let Some(status) = response.status.filter(|status| !(200..300).contains(status)) {
            tracing::error!(
                stage = "device_api",
                method = target.method().as_ref(),
                path = target.logging_path(),
                status,
                chain = ?target.wallet_chain().ok().flatten(),
                "device API request failed"
            );
            return Err(ClientError::Http { status, body: response.data });
        }
        Ok(response)
    }

    fn authorization(&self, target: &BsvHomeDeviceApiTarget, body: &[u8]) -> Result<HashMap<String, String>, ClientError> {
        let path = target.path();
        let signed_path = path.split('?').next().unwrap_or(&path);
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| ClientError::Serialization(error.to_string()))?
            .as_millis() as u64;

        let header = build_device_auth_header(
            &self.device_key.private_key()?,
            target.method().as_ref(),
            signed_path,
            target.wallet_id(),
            body,
            timestamp_ms,
        )
        .map_err(|error| ClientError::Serialization(error.to_string()))?;

        Ok(HashMap::from([("Authorization".to_string(), header)]))
    }
}
