mod chain_factory;
mod cold_wallet;
mod error;
mod multisig_funding;
mod preferences;

pub use chain_factory::ChainClientFactory;
pub use error::GatewayError;
pub(crate) use error::map_network_error;
#[cfg(test)]
pub use preferences::EmptyPreferences;
pub(crate) use preferences::{PreferencesWrapper, SecureStoreWrapper};

use crate::services::chain::BsvHomeRuntimeEnvironment;
use crate::services::chain::rules as chain_rules;
use crate::services::error::BsvHomeServiceError;
use crate::services::node::BsvHomeNodeCheck;
use crate::services::preferences::{BsvHomePreferencesStore, BsvHomeSecureStore};

use crate::alien::{AlienProvider, AlienProviderWrapper, NodeEndpoints, PreferencesNodeEndpoints, coalescing_provider};
use crate::models::*;
use crate::transaction_state::StatusProvider;
use bsv_home_bsv::rpc::ProviderError;
use chain_traits::ChainTraits;
use std::future::Future;
use std::sync::Arc;
use swapper::swapper::BsvHomeSwapper as Swapper;
use yielder::Yielder;

use primitives::TransactionInputType;
use primitives::perpetual::{PerpetualData, PerpetualPositionsSummary};
use primitives::{
    AssetBalance, AssetId, Chain, ChartPeriod, Latency, NodeStatus, Transaction, TransactionDirection, TransactionIdRequest, TransactionType, TransactionUpdate, Wallet,
};

#[derive(uniffi::Object)]
pub struct BsvHomeGateway {
    chain_factory: Arc<ChainClientFactory>,
    yielder: Yielder,
    status_provider: StatusProvider,
}

impl std::fmt::Debug for BsvHomeGateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BsvHomeGateway").finish()
    }
}

impl BsvHomeGateway {
    pub(crate) fn bsv_client(&self) -> bsv_home_bsv::rpc::BsvClient<crate::alien::AlienClient> {
        self.chain_factory.bsv_client()
    }

    pub(crate) fn bsv_read_scope(&self) -> (String, bsv_home_bsv::rpc::BsvClient<crate::alien::AlienClient>) {
        self.chain_factory.bsv_read_scope()
    }

    pub(crate) async fn bsv_transaction_broadcast_checked<F, Fut>(
        &self,
        data: String,
        check: &(dyn Fn() -> Result<(), ProviderError> + Sync),
        before_send: F,
        before_submission: &(dyn Fn() -> Result<(), ProviderError> + Sync),
    ) -> Result<String, ProviderError>
    where
        F: Fn() -> Fut + Sync,
        Fut: Future<Output = Result<(), ProviderError>> + Send,
    {
        self.require_chain(Chain::BitcoinSv)?;
        self.bsv_client().broadcast_checked(data, check, before_send, before_submission).await
    }

    pub(crate) fn allows_chain(&self, chain: Chain) -> bool {
        self.chain_factory.environment.allows_chain(chain)
    }

    pub(crate) fn allows_asset(&self, asset_id: &AssetId) -> bool {
        self.chain_factory.environment.allows_asset(asset_id)
    }

    pub(crate) fn require_asset(&self, asset_id: &AssetId) -> Result<(), BsvHomeServiceError> {
        self.chain_factory.environment.require_asset(asset_id)
    }

    pub(crate) fn visible_wallet(&self, wallet: Wallet) -> Wallet {
        self.chain_factory.environment.visible_wallet(wallet)
    }

    pub(crate) fn require_chain(&self, chain: Chain) -> Result<(), BsvHomeServiceError> {
        self.chain_factory.environment.require_chain(chain)
    }

    pub(crate) fn require_wallet_chain(&self, wallet: &Wallet, chain: Chain) -> Result<(), BsvHomeServiceError> {
        self.require_chain(chain)?;
        if chain.is_testnet() && !self.chain_factory.environment.supports_test_accounts(wallet.wallet_type) {
            return Err(BsvHomeServiceError::Unsupported {
                msg: "test networks require a mnemonic wallet".into(),
            });
        }
        Ok(())
    }

    pub(crate) fn validate_transaction_chain(chain: Chain, input: &TransactionInputType) -> Result<(), GatewayError> {
        if input.get_asset().chain() != chain {
            return Err(GatewayError::PlatformError {
                msg: "requested and transaction networks must match".into(),
            });
        }
        Ok(())
    }

    pub(crate) async fn transaction_by_hash(&self, request: TransactionIdRequest) -> Result<Option<Transaction>, GatewayError> {
        self.with_provider(request.chain, |provider| async move { provider.get_transaction_by_hash(request).await })
            .await
    }

    pub async fn get_address_transactions(&self, chain: Chain, address: String, limit: u32) -> Result<Vec<Transaction>, GatewayError> {
        self.with_provider(chain, |provider| async move {
            let result = provider
                .get_transactions_by_address(chain_traits::TransactionsRequest::new(address.clone(), limit as usize))
                .await?;
            let transactions = match result {
                chain_traits::TransactionsResult::Transactions(transactions) => transactions,
                chain_traits::TransactionsResult::TransactionRequests(requests) => {
                    let mut transactions = Vec::new();
                    for request in requests {
                        let request_chain = request.chain;
                        let hash = request.hash.clone();
                        match provider.get_transaction_by_hash(request).await? {
                            Some(mut transaction) => {
                                if chain.is_testnet() && transaction.transaction_type == TransactionType::Transfer {
                                    transaction.direction = match (transaction.from == address, transaction.to == address) {
                                        (true, true) => TransactionDirection::SelfTransfer,
                                        (true, false) => TransactionDirection::Outgoing,
                                        (false, true) => TransactionDirection::Incoming,
                                        (false, false) => {
                                            tracing::warn!(chain = %request_chain, hash = %hash, "skipped unrelated testnet address history transfer");
                                            continue;
                                        }
                                    };
                                }
                                transactions.push(transaction);
                            }
                            None if chain.is_testnet() => {
                                tracing::warn!(chain = %request_chain, hash = %hash, "skipped testnet address history transaction: unconfirmed or unsupported");
                            }
                            None => return Err("transaction from address history missing".into()),
                        }
                    }
                    transactions
                }
            };
            Ok(transactions.into_iter().map(|transaction| transaction.finalize(vec![address.clone()])).collect())
        })
        .await
    }

    pub async fn get_positions(&self, chain: Chain, address: String) -> Result<PerpetualPositionsSummary, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_positions(address).await }).await
    }

    pub async fn get_perpetuals_data(&self, chain: Chain) -> Result<Vec<PerpetualData>, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_perpetuals_data().await }).await
    }

    pub async fn get_balance_coin(&self, chain: Chain, address: String) -> Result<AssetBalance, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_balance_coin(address).await }).await
    }

    pub async fn get_balance_tokens(&self, chain: Chain, address: String, token_ids: Vec<String>) -> Result<Vec<AssetBalance>, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_balance_tokens(address, token_ids).await })
            .await
    }

    pub async fn get_balance_staking(&self, chain: Chain, address: String) -> Result<Option<AssetBalance>, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_balance_staking(address).await }).await
    }

    pub async fn get_balance_earn(&self, chain: Chain, address: String, token_ids: Vec<String>) -> Result<Vec<AssetBalance>, GatewayError> {
        Ok(self.yielder.get_balance(chain, &address, &token_ids).await)
    }

    pub async fn get_transaction_update(&self, transaction: Transaction) -> Result<TransactionUpdate, GatewayError> {
        Ok(self.status_provider.get_update(&transaction).await?)
    }

    async fn with_provider<T, F, Fut>(&self, chain: Chain, call: F) -> Result<T, GatewayError>
    where
        F: FnOnce(Arc<dyn ChainTraits>) -> Fut,
        Fut: Future<Output = Result<T, Box<dyn std::error::Error + Send + Sync>>>,
    {
        let provider = self.chain_factory.create(chain).await?;
        call(provider).await.map_err(map_network_error)
    }
    pub fn get_earn_providers(&self, asset_id: AssetId) -> Vec<BsvHomeDelegationValidator> {
        self.yielder.get_providers(&asset_id)
    }

    pub async fn get_earn_positions(&self, address: String, asset_id: AssetId) -> Result<Vec<BsvHomeDelegationBase>, GatewayError> {
        self.yielder
            .get_positions(&address, &asset_id)
            .await
            .map_err(|e| GatewayError::NetworkError { msg: e.to_string() })
    }
}

impl BsvHomeGateway {
    pub async fn get_node_status(&self, chain: Chain, url: &str) -> Result<NodeStatus, GatewayError> {
        let provider = self.chain_factory.create_with_url(chain, url.to_string()).await?;
        provider.get_nodes_status().await.map_err(map_network_error)
    }

    pub async fn check_node(&self, chain: Chain, url: &str) -> Result<BsvHomeNodeCheck, GatewayError> {
        let provider = self.chain_factory.create_with_url(chain, url.to_string()).await?;
        let (chain_id, status) = futures::try_join!(provider.get_chain_id(), provider.get_nodes_status()).map_err(map_network_error)?;
        if let Some(network_id) = chain_rules::mismatched_network_id(chain, chain_id.as_deref()) {
            return Err(GatewayError::NetworkIdMismatch {
                chain: chain.to_string(),
                network_id,
            });
        }
        if let Some(address) = chain_rules::node_verification_address(chain) {
            provider.get_balance_coin(address).await.map_err(map_network_error)?;
        }
        let sync = provider.get_node_status().await.map_err(map_network_error)?;
        Ok(BsvHomeNodeCheck {
            url: url.to_string(),
            chain_id,
            latest_block_number: status.latest_block_number,
            is_in_sync: sync.in_sync,
            latency: Latency::from_milliseconds(status.latency_ms),
        })
    }

    pub async fn get_earn_data(&self, asset_id: AssetId, address: String, value: String, earn_type: BsvHomeEarnType) -> Result<BsvHomeContractCallData, GatewayError> {
        self.yielder
            .get_data(&asset_id, &address, &value, &earn_type)
            .await
            .map_err(|e| GatewayError::NetworkError { msg: e.to_string() })
    }
}

#[uniffi::export]
impl BsvHomeGateway {
    #[uniffi::constructor]
    pub fn new(provider: Arc<dyn AlienProvider>, preferences: Arc<dyn BsvHomePreferencesStore>, secure_preferences: Arc<dyn BsvHomeSecureStore>) -> Self {
        Self::new_with_environment(provider, preferences, secure_preferences, BsvHomeRuntimeEnvironment::Production)
    }

    #[uniffi::constructor]
    pub fn new_with_environment(
        provider: Arc<dyn AlienProvider>,
        preferences: Arc<dyn BsvHomePreferencesStore>,
        secure_preferences: Arc<dyn BsvHomeSecureStore>,
        environment: BsvHomeRuntimeEnvironment,
    ) -> Self {
        let bsv_provider = provider.clone();
        let provider = coalescing_provider(provider);
        let endpoints: Arc<dyn NodeEndpoints> = Arc::new(PreferencesNodeEndpoints::new(preferences.clone()));
        let chain_factory = Arc::new(ChainClientFactory::new_with_bsv_provider(
            provider.clone(),
            bsv_provider,
            preferences,
            secure_preferences,
            environment,
        ));
        let alien_wrapper = Arc::new(AlienProviderWrapper::with_endpoints(provider, endpoints));
        let yielder = Yielder::new(alien_wrapper.clone());
        let swapper = Swapper::new(alien_wrapper);
        let status_provider = StatusProvider::new(chain_factory.clone(), swapper);
        Self {
            chain_factory,
            yielder,
            status_provider,
        }
    }
}

impl BsvHomeGateway {
    pub async fn get_staking_validators(&self, chain: Chain, apy: Option<f64>) -> Result<Vec<BsvHomeDelegationValidator>, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_staking_validators(apy).await }).await
    }

    pub async fn get_staking_delegation_validators(&self, chain: Chain, address: String) -> Result<Vec<BsvHomeDelegationValidator>, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_staking_delegation_validators(address).await })
            .await
    }

    pub async fn get_staking_delegations(&self, chain: Chain, address: String) -> Result<Vec<BsvHomeDelegationBase>, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_staking_delegations(address).await }).await
    }

    pub async fn get_perpetual_account_mode(&self, chain: Chain, address: String) -> Result<BsvHomePerpetualAccountMode, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_perpetual_account_mode(address).await })
            .await
    }

    pub async fn get_perpetual_candlesticks(&self, chain: Chain, symbol: String, period: String) -> Result<Vec<BsvHomeChartCandleStick>, GatewayError> {
        let chart_period = ChartPeriod::new(period.clone()).ok_or(GatewayError::PlatformError {
            msg: format!("Unknown chart period: {}", period),
        })?;
        self.with_provider(chain, |provider| async move { provider.get_perpetual_candlesticks(symbol, chart_period).await })
            .await
    }

    pub async fn get_perpetual_portfolio(&self, chain: Chain, address: String) -> Result<primitives::portfolio::PerpetualPortfolio, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_perpetual_portfolio(address).await }).await
    }
}

impl BsvHomeGateway {
    pub async fn transaction_broadcast(&self, chain: Chain, data: String, options: BsvHomeBroadcastOptions) -> Result<String, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.transaction_broadcast(data, options).await })
            .await
    }
    pub async fn get_fee_rates(&self, chain: Chain, input: TransactionInputType) -> Result<Vec<BsvHomeFeeRate>, GatewayError> {
        Self::validate_transaction_chain(chain, &input)?;
        let fees = self.with_provider(chain, |provider| async move { provider.get_transaction_fee_rates(input).await }).await?;
        Ok(fees.into_iter().map(|f| f.into()).collect())
    }
    pub async fn get_transaction_preload(&self, chain: Chain, input: BsvHomeTransactionPreloadInput) -> Result<BsvHomeTransactionLoadMetadata, GatewayError> {
        Self::validate_transaction_chain(chain, &input.input_type)?;
        let preload_input: primitives::TransactionPreloadInput = input.into();
        let metadata = self
            .with_provider(chain, |provider| async move { provider.get_transaction_preload(preload_input).await })
            .await?;
        Ok(metadata)
    }
    pub async fn get_transaction_load(&self, chain: Chain, input: BsvHomeTransactionLoadInput) -> Result<BsvHomeTransactionData, GatewayError> {
        Self::validate_transaction_chain(chain, &input.input_type)?;
        let load_data = self
            .with_provider(chain, |chain_provider| async move { chain_provider.get_transaction_load(input.into()).await })
            .await?;

        Ok(BsvHomeTransactionData {
            fee: load_data.fee.into(),
            metadata: load_data.metadata,
        })
    }
    pub async fn get_token_data(&self, chain: Chain, token_id: String) -> Result<BsvHomeAsset, GatewayError> {
        self.with_provider(chain, |provider| async move { provider.get_token_data(token_id).await }).await
    }
    pub async fn get_is_token_address(&self, chain: Chain, token_id: String) -> Result<bool, GatewayError> {
        Ok(self.chain_factory.create(chain).await?.get_is_token_address(&token_id))
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use crate::alien::AlienResponse;
    use crate::services::preferences::testkit::MemoryPreferencesStore;
    use crate::testkit::TestAlienProvider;
    use bsv_home_tron::address::TronAddress;
    use futures::executor::block_on;
    use primitives::{Address, Asset, TransactionLoadInput, TransactionPreloadInput, known_assets::TRON_SHASTA_USDT};
    use serde_json::{Value, json};

    const HISTORY_NODE: &str = "https://history.example";
    const HISTORY_ADDRESS: &str = "TEB39Rt69QkgD1BKhqaRNqGxfQzCarkRCb";
    const MISSING_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const UNSUPPORTED_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const NATIVE_HASH: &str = "5a9935a1b7be0150a511111582bbfed62ddb873333b3986bd712e6105fe90ad5";
    const TOKEN_HASH: &str = "d61c72e80f48b6d014301fc088f5d7f5a512d08b17cc6208d6b9993aeea12b0a";

    fn history_gateway(chain: Chain, responses: Vec<(String, AlienResponse)>) -> (BsvHomeGateway, Arc<TestAlienProvider>) {
        let alien = Arc::new(TestAlienProvider::with_responses(responses));
        let preferences = Arc::new(MemoryPreferencesStore::default());
        preferences.set(format!("node_{chain}"), HISTORY_NODE.to_string()).unwrap();
        let gateway = BsvHomeGateway::new_with_environment(alien.clone(), preferences, Arc::new(EmptyPreferences {}), BsvHomeRuntimeEnvironment::LocalDevelopment);
        (gateway, alien)
    }

    fn json_response(path: &str, status: u16, body: Value) -> (String, AlienResponse) {
        (format!("{HISTORY_NODE}{path}"), AlienResponse::new(Some(status), serde_json::to_vec(&body).unwrap()))
    }

    fn shasta_history_responses(native_status: u16) -> Vec<(String, AlienResponse)> {
        let native: Value = serde_json::from_str(include_str!("../../../crates/bsv_home_tron/testdata/transaction_coin_transfer.json")).unwrap();
        let native_receipt: Value = serde_json::from_str(include_str!("../../../crates/bsv_home_tron/testdata/transaction_coin_transfer_receipt.json")).unwrap();
        let mut unsupported = native.clone();
        unsupported["txID"] = json!(UNSUPPORTED_HASH);
        unsupported["raw_data"]["contract"][0]["type"] = json!("AccountPermissionUpdateContract");
        let mut unsupported_receipt = native_receipt.clone();
        unsupported_receipt["id"] = json!(UNSUPPORTED_HASH);
        let mut token: Value = serde_json::from_str(include_str!("../../../crates/bsv_home_tron/testdata/transaction_incoming_token_transfer.json")).unwrap();
        let mut token_receipt: Value = serde_json::from_str(include_str!("../../../crates/bsv_home_tron/testdata/transaction_incoming_token_transfer_receipt.json")).unwrap();
        let token_address = TronAddress::parse(TRON_SHASTA_USDT.id.token_id.as_deref().unwrap()).unwrap();
        let token_hex = hex::encode(token_address.as_bytes());
        token["raw_data"]["contract"][0]["parameter"]["value"]["contract_address"] = json!(token_hex);
        let recipient = TronAddress::parse(HISTORY_ADDRESS).unwrap();
        token["raw_data"]["contract"][0]["parameter"]["value"]["data"] = json!(hex::encode(bsv_home_tron::trc20::encode_transfer(&recipient, "10894000000").unwrap()));
        token_receipt["contract_address"] = json!(token_hex);
        token_receipt["log"][0]["address"] = json!(hex::encode(token_address.account_id()));
        token_receipt["log"][0]["topics"][2] = json!(recipient.abi_address_parameter());

        vec![
            json_response(
                "/wallet/getblockbynum?num=0",
                200,
                json!({"blockID": "0000000000000000de1aa88295e1fcf982742f773e0419c5a9c134c994a9059e"}),
            ),
            json_response(
                &format!("/v1/accounts/{HISTORY_ADDRESS}/transactions?limit=4"),
                200,
                json!({"data": [
                    {"txID": MISSING_HASH, "block_timestamp": 4},
                    {"txID": UNSUPPORTED_HASH, "block_timestamp": 3},
                    {"txID": NATIVE_HASH, "block_timestamp": 2}
                ]}),
            ),
            json_response(
                &format!("/v1/accounts/{HISTORY_ADDRESS}/transactions/trc20?limit=4"),
                200,
                json!({"data": [{"transaction_id": TOKEN_HASH, "block_timestamp": 1}]}),
            ),
            json_response(&format!("/wallet/gettransactioninfobyid?value={MISSING_HASH}"), 200, json!({})),
            json_response(&format!("/wallet/gettransactioninfobyid?value={UNSUPPORTED_HASH}"), 200, unsupported_receipt),
            json_response(&format!("/wallet/gettransactionbyid?value={UNSUPPORTED_HASH}"), 200, unsupported),
            json_response(&format!("/wallet/gettransactioninfobyid?value={NATIVE_HASH}"), native_status, native_receipt),
            json_response(&format!("/wallet/gettransactionbyid?value={NATIVE_HASH}"), 200, native),
            json_response(&format!("/wallet/gettransactioninfobyid?value={TOKEN_HASH}"), 200, token_receipt),
            json_response(&format!("/wallet/gettransactionbyid?value={TOKEN_HASH}"), 200, token),
        ]
    }

    #[test]
    fn test_shasta_transaction_input_rejects_mainnet_gateway_before_http() {
        let alien = Arc::new(TestAlienProvider::with_status(200));
        let gateway = BsvHomeGateway::new(alien.clone(), Arc::new(EmptyPreferences {}), Arc::new(EmptyPreferences {}));
        let message = "Platform error: requested and transaction networks must match";

        for asset in [Asset::from_chain(Chain::TronShasta), TRON_SHASTA_USDT.clone()] {
            let input_type = TransactionInputType::Transfer { asset };
            assert_eq!(block_on(gateway.get_fee_rates(Chain::Tron, input_type.clone())).unwrap_err().to_string(), message);
            assert_eq!(
                block_on(gateway.get_transaction_preload(Chain::Tron, TransactionPreloadInput::mock_with_input_type(input_type.clone()).into()))
                    .unwrap_err()
                    .to_string(),
                message
            );
            assert_eq!(
                block_on(gateway.get_transaction_load(Chain::Tron, TransactionLoadInput::mock_with_input_type(input_type).into()))
                    .unwrap_err()
                    .to_string(),
                message
            );
        }
        assert!(alien.requested_paths().is_empty());
    }

    #[test]
    fn test_shasta_history_skips_unconfirmed_and_unsupported_records() {
        let (gateway, alien) = history_gateway(Chain::TronShasta, shasta_history_responses(200));
        let result = block_on(gateway.get_address_transactions(Chain::TronShasta, HISTORY_ADDRESS.to_string(), 4));
        if !cfg!(debug_assertions) {
            assert_eq!(result.unwrap_err().to_string(), "Platform error: network unavailable in this environment: tronshasta");
            assert!(alien.requested_paths().is_empty());
            return;
        }
        let transactions = result.unwrap();

        assert_eq!(transactions.iter().map(Transaction::hash).collect::<Vec<_>>(), vec![NATIVE_HASH, TOKEN_HASH]);
        assert_eq!(transactions[0].asset_id, Chain::TronShasta.as_asset_id());
        assert_eq!(transactions[0].value.to_string(), "25000000");
        assert_eq!(transactions[0].direction, TransactionDirection::Outgoing);
        assert_eq!(transactions[1].asset_id, TRON_SHASTA_USDT.id);
        assert_eq!(transactions[1].value.to_string(), "10894000000");
        assert_eq!(transactions[1].direction, TransactionDirection::Incoming);
        assert!(
            transactions
                .iter()
                .all(|transaction| transaction.id.chain == Chain::TronShasta && transaction.fee_asset_id == Chain::TronShasta.as_asset_id())
        );
        assert_eq!(alien.requested_paths().len(), 10);
        assert!(!alien.requested_paths().contains(&format!("{HISTORY_NODE}/wallet/gettransactionbyid?value={MISSING_HASH}")));
    }

    #[test]
    fn test_shasta_history_propagates_rpc_errors_after_skipped_records() {
        let (gateway, alien) = history_gateway(Chain::TronShasta, shasta_history_responses(500));
        let error = block_on(gateway.get_address_transactions(Chain::TronShasta, HISTORY_ADDRESS.to_string(), 4)).unwrap_err();
        if cfg!(debug_assertions) {
            assert_eq!(error.to_string(), "Network error: HTTP error: status 500");
            assert!(
                alien
                    .requested_paths()
                    .contains(&format!("{HISTORY_NODE}/wallet/gettransactioninfobyid?value={NATIVE_HASH}"))
            );
        } else {
            assert_eq!(error.to_string(), "Platform error: network unavailable in this environment: tronshasta");
            assert!(alien.requested_paths().is_empty());
        }
    }

    #[test]
    fn test_shasta_history_projects_native_and_usdt_transfers_for_the_requested_wallet() {
        let counterparty = "THTR75o8xXAgCTQqpiot2AFRAjvW1tSbVV";
        let unrelated_recipient = "TJRyWwFs9wTFGZg3JbrVriFbNfCug5tDeC";
        let cases = [
            (false, counterparty, HISTORY_ADDRESS, TransactionDirection::Incoming),
            (false, HISTORY_ADDRESS, counterparty, TransactionDirection::Outgoing),
            (false, HISTORY_ADDRESS, HISTORY_ADDRESS, TransactionDirection::SelfTransfer),
            (true, counterparty, HISTORY_ADDRESS, TransactionDirection::Incoming),
            (true, HISTORY_ADDRESS, counterparty, TransactionDirection::Outgoing),
            (true, HISTORY_ADDRESS, HISTORY_ADDRESS, TransactionDirection::SelfTransfer),
        ];
        let native: Value = serde_json::from_str(include_str!("../../../crates/bsv_home_tron/testdata/transaction_coin_transfer.json")).unwrap();
        let native_receipt: Value = serde_json::from_str(include_str!("../../../crates/bsv_home_tron/testdata/transaction_coin_transfer_receipt.json")).unwrap();
        let token: Value = serde_json::from_str(include_str!("../../../crates/bsv_home_tron/testdata/transaction_incoming_token_transfer.json")).unwrap();
        let token_receipt: Value = serde_json::from_str(include_str!("../../../crates/bsv_home_tron/testdata/transaction_incoming_token_transfer_receipt.json")).unwrap();
        let token_address = TronAddress::parse(TRON_SHASTA_USDT.id.token_id.as_deref().unwrap()).unwrap();
        let mut responses = shasta_history_responses(200)
            .into_iter()
            .filter(|(url, _)| url.contains("getblockbynum"))
            .collect::<Vec<_>>();
        let mut native_history = Vec::new();
        let mut token_history = Vec::new();

        for (index, (is_token, from, to)) in cases
            .iter()
            .map(|(is_token, from, to, _)| (*is_token, *from, *to))
            .chain(std::iter::once((false, counterparty, unrelated_recipient)))
            .enumerate()
        {
            let hash = format!("{index:064x}");
            let sender = TronAddress::parse(from).unwrap();
            let recipient = TronAddress::parse(to).unwrap();
            let mut transaction = if is_token { token.clone() } else { native.clone() };
            let mut receipt = if is_token { token_receipt.clone() } else { native_receipt.clone() };
            transaction["txID"] = json!(hash);
            receipt["id"] = json!(hash);
            let contract = &mut transaction["raw_data"]["contract"][0]["parameter"]["value"];
            contract["owner_address"] = json!(hex::encode(sender.as_bytes()));
            if is_token {
                contract["contract_address"] = json!(hex::encode(token_address.as_bytes()));
                contract["data"] = json!(hex::encode(bsv_home_tron::trc20::encode_transfer(&recipient, "10894000000").unwrap()));
                receipt["contract_address"] = json!(hex::encode(token_address.as_bytes()));
                receipt["log"][0]["address"] = json!(hex::encode(token_address.account_id()));
                receipt["log"][0]["topics"][1] = json!(sender.abi_address_parameter());
                receipt["log"][0]["topics"][2] = json!(recipient.abi_address_parameter());
                token_history.push(json!({"transaction_id": hash, "block_timestamp": index}));
            } else {
                contract["to_address"] = json!(hex::encode(recipient.as_bytes()));
                native_history.push(json!({"txID": hash, "block_timestamp": index}));
            }
            responses.push(json_response(&format!("/wallet/gettransactioninfobyid?value={hash}"), 200, receipt));
            responses.push(json_response(&format!("/wallet/gettransactionbyid?value={hash}"), 200, transaction));
        }
        responses.push(json_response(
            &format!("/v1/accounts/{HISTORY_ADDRESS}/transactions?limit=7"),
            200,
            json!({"data": native_history}),
        ));
        responses.push(json_response(
            &format!("/v1/accounts/{HISTORY_ADDRESS}/transactions/trc20?limit=7"),
            200,
            json!({"data": token_history}),
        ));
        let (gateway, _) = history_gateway(Chain::TronShasta, responses);
        let result = block_on(gateway.get_address_transactions(Chain::TronShasta, HISTORY_ADDRESS.to_string(), 7));
        if !cfg!(debug_assertions) {
            assert_eq!(result.unwrap_err().to_string(), "Platform error: network unavailable in this environment: tronshasta");
            return;
        }
        let transactions = result.unwrap();

        assert_eq!(transactions.len(), 6);
        for (index, (is_token, from, to, direction)) in cases.iter().enumerate() {
            let hash = format!("{index:064x}");
            let transaction = transactions.iter().find(|transaction| transaction.hash() == hash).unwrap();
            assert_eq!(transaction.direction, *direction);
            assert_eq!(transaction.from, *from);
            assert_eq!(transaction.to, *to);
            assert_eq!(transaction.asset_id, if *is_token { TRON_SHASTA_USDT.id.clone() } else { Chain::TronShasta.as_asset_id() });
            assert_eq!(transaction.value.to_string(), if *is_token { "10894000000" } else { "25000000" });
            assert_eq!(transaction.fee_asset_id, Chain::TronShasta.as_asset_id());
        }
    }

    #[test]
    fn test_bsv_history_preserves_missing_transaction_errors() {
        let address = "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5";
        let mut history: Value = serde_json::from_str(include_str!("../../testdata/bsv/address_history.json")).unwrap();
        history[0]["address"] = Value::String(address.to_string());
        history[0]["confirmed"]["result"][0]["tx_hash"] = Value::String(MISSING_HASH.to_string());
        let responses = vec![
            json_response("/addresses/history/all", 200, history),
            json_response(&format!("/tx/hash/{MISSING_HASH}"), 404, Value::Object(Default::default())),
        ];
        let (gateway, _) = history_gateway(Chain::BitcoinSv, responses);

        assert_eq!(
            block_on(gateway.get_address_transactions(Chain::BitcoinSv, address.to_string(), 4))
                .unwrap_err()
                .to_string(),
            "Network error: BSV history transaction missing"
        );
    }
}

#[cfg(all(test, feature = "reqwest_provider"))]
mod tests {
    use super::*;
    use crate::testkit::TestAlienProvider;

    #[test]
    fn test_get_node_status_http_404_error() {
        let provider: Arc<dyn AlienProvider> = Arc::new(TestAlienProvider::with_status(404));
        let preferences: Arc<dyn BsvHomePreferencesStore> = Arc::new(EmptyPreferences {});
        let secure: Arc<dyn BsvHomeSecureStore> = Arc::new(EmptyPreferences {});
        let gateway = BsvHomeGateway::new(provider, preferences.clone(), secure);

        let result = futures::executor::block_on(gateway.get_node_status(Chain::Bitcoin, "https://httpbin.org/status/404"));

        match result {
            Ok(status) => panic!("expected network error for 404 response, got {:?}", status),
            Err(GatewayError::NetworkError { msg }) => assert_eq!(msg, "HTTP error: status 404"),
            Err(error) => panic!("expected NetworkError, got {error:?}"),
        }
    }
}
