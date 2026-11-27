use bsv_home_client::{ClientError, build_path_with_query};
use primitives::rewards::RedemptionRequest;
use primitives::{
    AssetId, AuthenticatedRequest, Chain, ChainAddress, ChartPeriod, Device, FiatQuoteType, NFTAssetId, PortfolioAssetsRequest, PriceAlert, ReferralCode, ReportNft,
    ScanTransactionPayload, SupportMessageInput, WalletSubscription, WalletSubscriptionChains,
};
use serde::Serialize;

use crate::method::BsvHomeApiMethod;

#[derive(Clone, Debug)]
pub enum BsvHomeDeviceApiBody {
    Json(Vec<u8>),
    Raw { data: Vec<u8>, content_type: String },
}

#[derive(Clone, Debug)]
pub enum BsvHomeDeviceApiTarget {
    GetDevice,
    AddDevice(Device),
    UpdateDevice(Device),
    IsDeviceRegistered,
    GetAuthNonce,

    GetSubscriptions,
    AddSubscriptions(Vec<WalletSubscription>),
    DeleteSubscriptions(Vec<WalletSubscriptionChains>),

    GetPriceAlerts {
        asset_id: Option<String>,
    },
    AddPriceAlerts(Vec<PriceAlert>),
    DeletePriceAlerts(Vec<PriceAlert>),

    GetTransactions {
        wallet_id: String,
        asset_id: Option<String>,
        from_timestamp: u64,
        limit: usize,
        offset: usize,
    },
    GetAssetsList {
        wallet_id: String,
        from_timestamp: u64,
    },
    GetWalletConfiguration {
        wallet_id: String,
    },

    ScanTransaction(ScanTransactionPayload),

    GetNftAssets {
        wallet_id: String,
    },
    GetNftAsset(NFTAssetId),
    RefreshNftAsset {
        wallet_id: String,
        asset_id: NFTAssetId,
    },
    ReportNft(ReportNft),

    GetSupportMessages {
        from_timestamp: u64,
    },
    SendSupportMessage(SupportMessageInput),
    SendSupportImage {
        image: Vec<u8>,
        file_name: String,
        mime_type: String,
    },

    GetRewards {
        wallet_id: String,
    },
    CreateReferral {
        wallet_id: String,
        request: AuthenticatedRequest<ReferralCode>,
    },
    UseReferralCode {
        wallet_id: String,
        request: AuthenticatedRequest<ReferralCode>,
    },
    RedeemRewards {
        wallet_id: String,
        request: AuthenticatedRequest<RedemptionRequest>,
    },

    GetNotifications {
        from_timestamp: u64,
    },
    MarkNotificationsRead,

    GetFiatQuotes {
        wallet_id: String,
        quote_type: FiatQuoteType,
        asset_id: String,
        amount: f64,
        currency: String,
    },
    GetFiatQuoteUrl {
        wallet_id: String,
        quote_id: String,
    },
    GetFiatTransactions {
        wallet_id: String,
    },

    GetNameRecord {
        name: String,
        chain: String,
    },
    GetAddressNames(Vec<ChainAddress>),

    GetPortfolioAssets {
        period: ChartPeriod,
        request: PortfolioAssetsRequest,
    },
}

impl BsvHomeDeviceApiTarget {
    pub fn method(&self) -> BsvHomeApiMethod {
        match self {
            Self::GetDevice
            | Self::IsDeviceRegistered
            | Self::GetAuthNonce
            | Self::GetSubscriptions
            | Self::GetPriceAlerts { .. }
            | Self::GetTransactions { .. }
            | Self::GetAssetsList { .. }
            | Self::GetWalletConfiguration { .. }
            | Self::GetNftAssets { .. }
            | Self::GetNftAsset(_)
            | Self::GetSupportMessages { .. }
            | Self::GetRewards { .. }
            | Self::GetNotifications { .. }
            | Self::GetFiatQuotes { .. }
            | Self::GetFiatQuoteUrl { .. }
            | Self::GetFiatTransactions { .. }
            | Self::GetNameRecord { .. } => BsvHomeApiMethod::Get,
            Self::UpdateDevice(_) => BsvHomeApiMethod::Put,
            Self::DeleteSubscriptions(_) | Self::DeletePriceAlerts(_) => BsvHomeApiMethod::Delete,
            Self::AddDevice(_)
            | Self::AddSubscriptions(_)
            | Self::AddPriceAlerts(_)
            | Self::ScanTransaction(_)
            | Self::RefreshNftAsset { .. }
            | Self::ReportNft(_)
            | Self::SendSupportMessage(_)
            | Self::SendSupportImage { .. }
            | Self::CreateReferral { .. }
            | Self::UseReferralCode { .. }
            | Self::RedeemRewards { .. }
            | Self::MarkNotificationsRead
            | Self::GetAddressNames(_)
            | Self::GetPortfolioAssets { .. } => BsvHomeApiMethod::Post,
        }
    }

    pub fn path(&self) -> String {
        match self {
            Self::GetDevice | Self::AddDevice(_) | Self::UpdateDevice(_) => "/v2/devices".to_string(),
            Self::IsDeviceRegistered => "/v2/devices/is_registered".to_string(),
            Self::GetAuthNonce => "/v2/devices/auth/nonce".to_string(),
            Self::GetSubscriptions | Self::AddSubscriptions(_) | Self::DeleteSubscriptions(_) => "/v2/devices/subscriptions".to_string(),
            Self::GetPriceAlerts { asset_id } => build_path_with_query("/v2/devices/price_alerts", &PriceAlertsQuery { asset_id: asset_id.clone() }),
            Self::AddPriceAlerts(_) | Self::DeletePriceAlerts(_) => "/v2/devices/price_alerts".to_string(),
            Self::GetTransactions {
                asset_id,
                from_timestamp,
                limit,
                offset,
                ..
            } => build_path_with_query(
                "/v2/devices/transactions",
                &TransactionsQuery {
                    from_timestamp: *from_timestamp,
                    asset_id: asset_id.clone(),
                    limit: *limit,
                    offset: *offset,
                },
            ),
            Self::GetAssetsList { from_timestamp, .. } => format!("/v2/devices/assets?from_timestamp={from_timestamp}"),
            Self::GetWalletConfiguration { .. } => "/v2/devices/wallet_configuration".to_string(),
            Self::ScanTransaction(_) => "/v2/devices/scan/transaction".to_string(),
            Self::GetNftAssets { .. } => "/v2/devices/nft_assets".to_string(),
            Self::GetNftAsset(asset_id) => format!("/v2/devices/nft_assets/{asset_id}"),
            Self::RefreshNftAsset { asset_id, .. } => format!("/v2/devices/nft_assets/{asset_id}/refresh"),
            Self::ReportNft(_) => "/v2/devices/nft/report".to_string(),
            Self::GetSupportMessages { from_timestamp } => format!("/v2/devices/support/messages?from_timestamp={from_timestamp}"),
            Self::SendSupportMessage(_) => "/v2/devices/support/messages".to_string(),
            Self::SendSupportImage { file_name, .. } => format!(
                "/v2/devices/support/messages/images?file_name={}",
                percent_encoding::utf8_percent_encode(file_name, percent_encoding::NON_ALPHANUMERIC)
            ),
            Self::GetRewards { .. } => "/v2/devices/rewards".to_string(),
            Self::CreateReferral { .. } => "/v2/devices/rewards/referrals/create".to_string(),
            Self::UseReferralCode { .. } => "/v2/devices/rewards/referrals/use".to_string(),
            Self::RedeemRewards { .. } => "/v2/devices/rewards/redeem".to_string(),
            Self::GetNotifications { from_timestamp } => format!("/v2/devices/notifications?from_timestamp={from_timestamp}"),
            Self::MarkNotificationsRead => "/v2/devices/notifications/read".to_string(),
            Self::GetFiatQuotes {
                quote_type,
                asset_id,
                amount,
                currency,
                ..
            } => format!("/v2/devices/fiat/quotes/{}/{asset_id}?amount={amount}&currency={currency}", quote_type.as_ref()),
            Self::GetFiatQuoteUrl { quote_id, .. } => format!("/v2/devices/fiat/quotes/{quote_id}/url"),
            Self::GetFiatTransactions { .. } => "/v2/devices/fiat/transactions".to_string(),
            Self::GetNameRecord { name, chain } => format!("/v2/devices/name/resolve/{name}?chain={chain}"),
            Self::GetAddressNames(_) => "/v2/devices/address_names".to_string(),
            Self::GetPortfolioAssets { period, .. } => format!("/v2/devices/portfolio/assets?period={}", period.as_ref()),
        }
    }

    pub fn wallet_id(&self) -> &str {
        match self {
            Self::GetTransactions { wallet_id, .. }
            | Self::GetAssetsList { wallet_id, .. }
            | Self::GetWalletConfiguration { wallet_id }
            | Self::GetNftAssets { wallet_id }
            | Self::RefreshNftAsset { wallet_id, .. }
            | Self::GetRewards { wallet_id }
            | Self::CreateReferral { wallet_id, .. }
            | Self::UseReferralCode { wallet_id, .. }
            | Self::RedeemRewards { wallet_id, .. }
            | Self::GetFiatQuotes { wallet_id, .. }
            | Self::GetFiatQuoteUrl { wallet_id, .. }
            | Self::GetFiatTransactions { wallet_id } => wallet_id.as_str(),
            _ => "",
        }
    }

    pub(crate) fn logging_path(&self) -> String {
        match self {
            Self::GetNftAsset(_) => "/v2/devices/nft_assets/{asset_id}".to_string(),
            Self::RefreshNftAsset { .. } => "/v2/devices/nft_assets/{asset_id}/refresh".to_string(),
            Self::GetFiatQuotes { .. } => "/v2/devices/fiat/quotes/{type}/{asset_id}".to_string(),
            Self::GetFiatQuoteUrl { .. } => "/v2/devices/fiat/quotes/{quote_id}/url".to_string(),
            Self::GetNameRecord { .. } => "/v2/devices/name/resolve/{name}".to_string(),
            _ => {
                let path = self.path();
                match path.split_once('?') {
                    Some((route, _)) => route.to_string(),
                    None => path,
                }
            }
        }
    }

    pub fn wallet_chain(&self) -> Result<Option<Chain>, ClientError> {
        match self {
            Self::GetTransactions { asset_id: Some(asset_id), .. } | Self::GetFiatQuotes { asset_id, .. } => {
                let asset_id = AssetId::new(asset_id).ok_or_else(|| ClientError::Serialization("invalid wallet request asset id".to_string()))?;
                Ok(Some(asset_id.chain))
            }
            Self::RefreshNftAsset { asset_id, .. } => Ok(Some(asset_id.chain)),
            Self::CreateReferral { request, .. } | Self::UseReferralCode { request, .. } => Ok(Some(request.auth.chain)),
            Self::RedeemRewards { request, .. } => Ok(Some(request.auth.chain)),
            _ => Ok(None),
        }
    }

    pub fn body(&self) -> Result<Option<BsvHomeDeviceApiBody>, ClientError> {
        match self {
            Self::AddDevice(device) | Self::UpdateDevice(device) => json(device),
            Self::AddSubscriptions(subscriptions) => json(subscriptions),
            Self::DeleteSubscriptions(subscriptions) => json(subscriptions),
            Self::AddPriceAlerts(alerts) | Self::DeletePriceAlerts(alerts) => json(alerts),
            Self::ScanTransaction(payload) => json(payload),
            Self::ReportNft(report) => json(report),
            Self::SendSupportMessage(input) => json(input),
            Self::SendSupportImage { image, mime_type, .. } => Ok(Some(BsvHomeDeviceApiBody::Raw {
                data: image.clone(),
                content_type: mime_type.clone(),
            })),
            Self::CreateReferral { request, .. } | Self::UseReferralCode { request, .. } => json(request),
            Self::RedeemRewards { request, .. } => json(request),
            Self::GetAddressNames(requests) => json(requests),
            Self::GetPortfolioAssets { request, .. } => json(request),
            _ => Ok(None),
        }
    }
}

#[derive(Serialize)]
struct PriceAlertsQuery {
    asset_id: Option<String>,
}

#[derive(Serialize)]
struct TransactionsQuery {
    from_timestamp: u64,
    asset_id: Option<String>,
    limit: usize,
    offset: usize,
}

fn json<T: Serialize>(value: &T) -> Result<Option<BsvHomeDeviceApiBody>, ClientError> {
    Ok(Some(BsvHomeDeviceApiBody::Json(serde_json::to_vec(value)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_history_scope_requires_the_requested_asset_chain() {
        for (asset_id, chain) in [(Some(Chain::Tron.as_asset_id().to_string()), Some(Chain::Tron)), (None, None)] {
            let target = BsvHomeDeviceApiTarget::GetTransactions {
                wallet_id: "multicoin_fixture".into(),
                asset_id,
                from_timestamp: 0,
                limit: 100,
                offset: 0,
            };
            assert_eq!(target.wallet_chain().unwrap(), chain);
            assert_eq!(target.wallet_id(), "multicoin_fixture");
        }
        let target = BsvHomeDeviceApiTarget::GetTransactions {
            wallet_id: "multicoin_fixture".into(),
            asset_id: Some("invalid-chain".into()),
            from_timestamp: 0,
            limit: 100,
            offset: 0,
        };
        assert!(target.wallet_chain().is_err());
    }

    #[test]
    fn test_logging_paths_omit_dynamic_identifiers_and_queries() {
        let name = BsvHomeDeviceApiTarget::GetNameRecord {
            name: "private-name".into(),
            chain: "tron".into(),
        };
        let quote = BsvHomeDeviceApiTarget::GetFiatQuoteUrl {
            wallet_id: "private-wallet".into(),
            quote_id: "private-quote".into(),
        };
        let history = BsvHomeDeviceApiTarget::GetTransactions {
            wallet_id: "private-wallet".into(),
            asset_id: Some(Chain::Tron.as_asset_id().to_string()),
            from_timestamp: 42,
            limit: 100,
            offset: 0,
        };
        assert_eq!(name.logging_path(), "/v2/devices/name/resolve/{name}");
        assert_eq!(quote.logging_path(), "/v2/devices/fiat/quotes/{quote_id}/url");
        assert_eq!(history.logging_path(), "/v2/devices/transactions");
    }
}
