use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use primitives::{
    BSVHOME_API_HOST, Latency,
    node_config::{BSV_DEFAULT_NODE_URL, NodeRegion},
};

use crate::BsvHomeCoreError;
use crate::alien::{AlienHttpMethod, AlienProvider, AlienTarget};
use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::price::client::bsv_price_url;

#[uniffi::export]
pub fn service_status_timeout() -> Duration {
    bsv_home_client::DEFAULT_REQUEST_TIMEOUT
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum BsvHomeServiceEndpointType {
    Api,
    BsvHomeNode,
    BitcoinSvRpc,
    DefiLlama,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct BsvHomeServiceEndpoint {
    pub endpoint_type: BsvHomeServiceEndpointType,
    pub host: String,
    pub url: String,
    pub flag: String,
}

impl BsvHomeServiceEndpoint {
    fn new(endpoint_type: BsvHomeServiceEndpointType, host: &str, flag: &str) -> Self {
        Self {
            endpoint_type,
            host: host.to_string(),
            url: format!("https://{host}"),
            flag: flag.to_string(),
        }
    }
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum BsvHomeLatencyStatus {
    Loading,
    Error,
    Result { latency: Latency },
}

#[derive(uniffi::Object)]
pub struct BsvHomeServiceStatus {
    provider: Arc<dyn AlienProvider>,
    environment: BsvHomeRuntimeEnvironment,
}

#[uniffi::export]
impl BsvHomeServiceStatus {
    #[uniffi::constructor]
    pub fn new(provider: Arc<dyn AlienProvider>) -> Self {
        Self::new_with_environment(provider, BsvHomeRuntimeEnvironment::Production)
    }

    #[uniffi::constructor]
    pub fn new_with_environment(provider: Arc<dyn AlienProvider>, environment: BsvHomeRuntimeEnvironment) -> Self {
        Self { provider, environment }
    }

    pub fn get_endpoints(&self) -> Vec<BsvHomeServiceEndpoint> {
        match self.environment {
            BsvHomeRuntimeEnvironment::WowPay => vec![
                BsvHomeServiceEndpoint {
                    endpoint_type: BsvHomeServiceEndpointType::BitcoinSvRpc,
                    host: "api.whatsonchain.com".to_string(),
                    url: format!("{BSV_DEFAULT_NODE_URL}/chain/info"),
                    flag: String::new(),
                },
                BsvHomeServiceEndpoint {
                    endpoint_type: BsvHomeServiceEndpointType::DefiLlama,
                    host: "coins.llama.fi".to_string(),
                    url: bsv_price_url(),
                    flag: String::new(),
                },
            ],
            BsvHomeRuntimeEnvironment::Production | BsvHomeRuntimeEnvironment::LocalDevelopment => {
                [BsvHomeServiceEndpoint::new(BsvHomeServiceEndpointType::Api, BSVHOME_API_HOST, NodeRegion::Us.flag())]
                    .into_iter()
                    .chain(
                        NodeRegion::all()
                            .into_iter()
                            .map(|region| BsvHomeServiceEndpoint::new(BsvHomeServiceEndpointType::BsvHomeNode, region.host(), region.flag())),
                    )
                    .collect()
            }
        }
    }

    pub async fn get_endpoint_status(&self, url: String) -> BsvHomeLatencyStatus {
        match self.get_endpoint_latency(url).await {
            Ok(latency) => BsvHomeLatencyStatus::Result { latency },
            Err(_) => BsvHomeLatencyStatus::Error,
        }
    }
}

impl BsvHomeServiceStatus {
    async fn get_endpoint_latency(&self, url: String) -> Result<Latency, BsvHomeCoreError> {
        let target = AlienTarget {
            url,
            method: AlienHttpMethod::Get,
            headers: Some(HashMap::from([("Cache-Control".to_string(), "no-cache".to_string())])),
            body: None,
        };
        let start_time = Instant::now();
        let response = self.provider.request(target).await?;
        match response.to_rpc_response().status {
            Some(status) if (200..300).contains(&status) => Ok(Latency::from_milliseconds(start_time.elapsed().as_millis() as u64)),
            Some(_) | None => Err("Service endpoint did not return a successful HTTP status".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::alien::{AlienError, AlienResponse};
    use crate::testkit::TestAlienProvider;
    use async_trait::async_trait;
    use futures::executor::block_on;
    use primitives::{BSVHOME_NODES_ASIA_HOST, BSVHOME_NODES_EUROPE_HOST, BSVHOME_NODES_HOST, LatencyType};

    #[derive(Debug)]
    struct OfflineProvider;

    #[async_trait]
    impl AlienProvider for OfflineProvider {
        async fn request(&self, _target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
            Err(AlienError::Offline)
        }
    }

    #[test]
    fn test_endpoint_status_measures_a_reachable_endpoint_and_reports_an_unreachable_one() {
        let reachable = BsvHomeServiceStatus::new(Arc::new(TestAlienProvider::with_status(200)));
        match block_on(reachable.get_endpoint_status("https://api.gemwallet.com".to_string())) {
            BsvHomeLatencyStatus::Result { latency } => assert_eq!(latency.latency_type, LatencyType::Fast),
            other => panic!("a reachable endpoint reports its latency, got {other:?}"),
        }

        let unreachable = BsvHomeServiceStatus::new(Arc::new(OfflineProvider));
        assert_eq!(
            block_on(unreachable.get_endpoint_status("https://api.gemwallet.com".to_string())),
            BsvHomeLatencyStatus::Error
        );
    }

    #[test]
    fn test_non_success_http_status_is_not_a_reachable_service() {
        let provider = Arc::new(TestAlienProvider::with_status(503));
        let service = BsvHomeServiceStatus::new(provider.clone());
        let url = "https://status.example.invalid/health".to_string();

        assert_eq!(block_on(service.get_endpoint_status(url.clone())), BsvHomeLatencyStatus::Error);
        assert_eq!(provider.requested_paths(), vec![url]);
    }

    #[test]
    fn test_missing_http_status_is_not_a_reachable_service() {
        let provider = Arc::new(TestAlienProvider::new(AlienResponse::new(None, Vec::new())));
        let service = BsvHomeServiceStatus::new(provider.clone());
        let url = "https://status.example.invalid/health".to_string();

        assert_eq!(block_on(service.get_endpoint_status(url.clone())), BsvHomeLatencyStatus::Error);
        assert_eq!(provider.requested_paths(), vec![url]);
    }

    type Request = (String, AlienHttpMethod, Option<HashMap<String, String>>, Option<Vec<u8>>);

    #[derive(Debug, Clone, Copy)]
    enum ProviderResponse {
        Http(Option<u16>),
        Offline,
    }

    #[derive(Debug)]
    struct RecordingProvider {
        response: ProviderResponse,
        requests: Mutex<Vec<Request>>,
    }

    impl RecordingProvider {
        fn new(response: ProviderResponse) -> Self {
            Self {
                response,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<Request> {
            self.requests.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl AlienProvider for RecordingProvider {
        async fn request(&self, target: AlienTarget) -> Result<Arc<AlienResponse>, AlienError> {
            self.requests.lock().unwrap().push((target.url, target.method, target.headers, target.body));
            match self.response {
                ProviderResponse::Http(status) => Ok(Arc::new(AlienResponse::new(status, vec![0xff]))),
                ProviderResponse::Offline => Err(AlienError::Offline),
            }
        }
    }

    fn request(url: &str) -> Request {
        (
            url.to_string(),
            AlienHttpMethod::Get,
            Some(HashMap::from([("Cache-Control".to_string(), "no-cache".to_string())])),
            None,
        )
    }

    fn has_latency(status: BsvHomeLatencyStatus) -> bool {
        match status {
            BsvHomeLatencyStatus::Result { .. } => true,
            BsvHomeLatencyStatus::Loading | BsvHomeLatencyStatus::Error => false,
        }
    }

    #[test]
    fn test_wowpay_requests_only_its_two_exact_public_service_targets() {
        let provider = Arc::new(RecordingProvider::new(ProviderResponse::Http(Some(204))));
        let service = BsvHomeServiceStatus::new_with_environment(provider.clone(), BsvHomeRuntimeEnvironment::WowPay);
        let endpoints = service.get_endpoints();
        for endpoint in &endpoints {
            assert!(has_latency(block_on(service.get_endpoint_status(endpoint.url.clone()))));
        }
        assert_eq!(
            provider.requests(),
            vec![
                request("https://api.whatsonchain.com/v1/bsv/main/chain/info"),
                request("https://coins.llama.fi/prices/current/coingecko:bitcoin-cash-sv"),
            ]
        );
        assert_eq!(
            endpoints,
            vec![
                BsvHomeServiceEndpoint {
                    endpoint_type: BsvHomeServiceEndpointType::BitcoinSvRpc,
                    host: "api.whatsonchain.com".to_string(),
                    url: "https://api.whatsonchain.com/v1/bsv/main/chain/info".to_string(),
                    flag: String::new(),
                },
                BsvHomeServiceEndpoint {
                    endpoint_type: BsvHomeServiceEndpointType::DefiLlama,
                    host: "coins.llama.fi".to_string(),
                    url: "https://coins.llama.fi/prices/current/coingecko:bitcoin-cash-sv".to_string(),
                    flag: String::new(),
                },
            ]
        );
    }

    #[test]
    fn test_http_status_and_transport_failures_are_measured_once_without_parsing_the_body_or_retrying() {
        for environment in [
            BsvHomeRuntimeEnvironment::Production,
            BsvHomeRuntimeEnvironment::LocalDevelopment,
            BsvHomeRuntimeEnvironment::WowPay,
        ] {
            for (response, expected) in [
                (ProviderResponse::Http(Some(200)), true),
                (ProviderResponse::Http(Some(204)), true),
                (ProviderResponse::Http(Some(299)), true),
                (ProviderResponse::Http(None), false),
                (ProviderResponse::Http(Some(199)), false),
                (ProviderResponse::Http(Some(300)), false),
                (ProviderResponse::Http(Some(301)), false),
                (ProviderResponse::Http(Some(400)), false),
                (ProviderResponse::Http(Some(401)), false),
                (ProviderResponse::Http(Some(403)), false),
                (ProviderResponse::Http(Some(404)), false),
                (ProviderResponse::Http(Some(429)), false),
                (ProviderResponse::Http(Some(500)), false),
                (ProviderResponse::Http(Some(503)), false),
                (ProviderResponse::Offline, false),
            ] {
                let provider = Arc::new(RecordingProvider::new(response));
                let service = BsvHomeServiceStatus::new_with_environment(provider.clone(), environment);
                let url = "https://status.example.invalid/health";

                assert_eq!(
                    has_latency(block_on(service.get_endpoint_status(url.to_string()))),
                    expected,
                    "{environment:?} {response:?}"
                );
                assert_eq!(provider.requests(), vec![request(url)]);
            }
        }
    }

    #[test]
    fn test_default_production_and_local_development_preserve_legacy_routing_and_public_requests() {
        let endpoints = vec![
            BsvHomeServiceEndpoint::new(BsvHomeServiceEndpointType::Api, BSVHOME_API_HOST, NodeRegion::Us.flag()),
            BsvHomeServiceEndpoint::new(BsvHomeServiceEndpointType::BsvHomeNode, BSVHOME_NODES_HOST, NodeRegion::Us.flag()),
            BsvHomeServiceEndpoint::new(BsvHomeServiceEndpointType::BsvHomeNode, BSVHOME_NODES_ASIA_HOST, NodeRegion::Asia.flag()),
            BsvHomeServiceEndpoint::new(BsvHomeServiceEndpointType::BsvHomeNode, BSVHOME_NODES_EUROPE_HOST, NodeRegion::Eu.flag()),
        ];
        let default = BsvHomeServiceStatus::new(Arc::new(OfflineProvider));
        assert_eq!(default.get_endpoints(), endpoints);
        for environment in [BsvHomeRuntimeEnvironment::Production, BsvHomeRuntimeEnvironment::LocalDevelopment] {
            let provider = Arc::new(RecordingProvider::new(ProviderResponse::Http(Some(200))));
            let service = BsvHomeServiceStatus::new_with_environment(provider.clone(), environment);
            assert_eq!(service.get_endpoints(), endpoints);
            for endpoint in service.get_endpoints() {
                assert!(has_latency(block_on(service.get_endpoint_status(endpoint.url))));
            }
            assert_eq!(provider.requests(), endpoints.iter().map(|endpoint| request(&endpoint.url)).collect::<Vec<_>>());
        }
    }
}
