use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use chrono::Utc;
use futures::lock::Mutex;
use primitives::{AssetId, Wallet};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::alien::AlienHttpMethod;
use crate::gateway::BsvHomeGateway;
use crate::services::assets::BsvHomeAssetsService;
use crate::services::preferences::BsvHomeSecureStore;
use crate::services::wallet::addresses::BsvHomeBsvAddressPool;

use super::swap_error::BsvHomePortalSwapError;
use super::swap_model::{
    BsvHomePortalSwapActionNavigation, BsvHomePortalSwapDirection, BsvHomePortalSwapOrder, BsvHomePortalSwapOrderPage, BsvHomePortalSwapPair, BsvHomePortalSwapQuote,
    BsvHomePortalSwapSyncStatus, OrderRequest, PendingOperation, SwapState, WireNavigation, WireOrder, WireOrderPage, WirePair, WireQuote,
};
use super::swap_rules;
use super::swap_store::BsvHomePortalSwapStore;
use super::{BsvHomePortalAuthError, BsvHomePortalAuthService};

#[derive(Clone, Copy, PartialEq)]
enum CreationAttempt {
    Initial,
    Recovery,
}

#[derive(uniffi::Object)]
pub struct BsvHomePortalSwapService {
    pub(super) auth: Arc<BsvHomePortalAuthService>,
    pub(super) store: Arc<dyn BsvHomePortalSwapStore>,
    pub(super) payment_store: Arc<dyn BsvHomeSecureStore>,
    pub(super) assets: Arc<BsvHomeAssetsService>,
    pub(super) gateway: Arc<BsvHomeGateway>,
    pub(super) addresses: Arc<BsvHomeBsvAddressPool>,
    pub(super) mutation: Arc<Mutex<()>>,
    quotes: Mutex<HashMap<String, (String, BsvHomePortalSwapQuote)>>,
}

#[uniffi::export]
impl BsvHomePortalSwapService {
    #[uniffi::constructor]
    pub fn new(
        auth: Arc<BsvHomePortalAuthService>,
        store: Arc<dyn BsvHomePortalSwapStore>,
        payment_store: Arc<dyn BsvHomeSecureStore>,
        assets: Arc<BsvHomeAssetsService>,
        gateway: Arc<BsvHomeGateway>,
        addresses: Arc<BsvHomeBsvAddressPool>,
    ) -> Self {
        Self {
            auth,
            store,
            payment_store,
            assets,
            gateway,
            addresses,
            mutation: Arc::new(Mutex::new(())),
            quotes: Mutex::new(HashMap::new()),
        }
    }

    pub fn direction_for_asset(&self, asset_id: AssetId) -> Option<BsvHomePortalSwapDirection> {
        BsvHomePortalSwapDirection::for_asset(asset_id)
    }

    pub async fn pairs(&self) -> Result<Vec<BsvHomePortalSwapPair>, BsvHomePortalSwapError> {
        let pairs = self.auth.swap_request::<Vec<WirePair>>("/auth/swaps/pairs", AlienHttpMethod::Get, None).await?;
        Ok(swap_rules::pairs(pairs)?.into_iter().map(|(_, pair)| pair).collect())
    }

    pub async fn quote(&self, direction: BsvHomePortalSwapDirection, decimal_amount: String) -> Result<BsvHomePortalSwapQuote, BsvHomePortalSwapError> {
        swap_rules::validate_supported_direction(direction)?;
        let version = self.workflow_version();
        let wallet = self.auth.swap_wallet().await?;
        swap_rules::wallet_addresses(&wallet, direction)?;
        let scope = self.scope(&wallet)?;
        let pairs = self
            .auth
            .swap_wallet_request::<Vec<WirePair>>(&wallet, "/auth/swaps/pairs", AlienHttpMethod::Get, None)
            .await?;
        self.check_workflow(&wallet, version).await?;
        let (pair, _) = swap_rules::pairs(pairs)?
            .into_iter()
            .find(|(_, pair)| pair.direction == direction)
            .ok_or(BsvHomePortalSwapError::UnsupportedPair)?;
        let amount = swap_rules::decimal_amount(&decimal_amount, pair.from.scale)?;
        let request = swap_rules::quote_request(&pair, amount);
        let wire = self
            .auth
            .swap_wallet_request::<WireQuote>(&wallet, "/auth/swaps/quotes", AlienHttpMethod::Post, Some(encode(&request)?))
            .await?;
        self.check_wallet(&wallet).await?;
        self.check_workflow(&wallet, version).await?;
        let quote = swap_rules::quote(wire, &request, &pair, Utc::now())?;
        let mut quotes = self.quotes.lock().await;
        quotes.retain(|_, (_, quote)| quote.expires_at > Utc::now().timestamp());
        quotes.insert(quote.quote_id.clone(), (scope, quote.clone()));
        Ok(quote)
    }

    pub async fn create_order(&self, quote_id: String) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
        let version = self.workflow_version();
        let _mutation = self.mutation.lock().await;
        let wallet = self.auth.swap_wallet().await?;
        let scope = self.scope(&wallet)?;
        let mut state = self.read_state(&scope)?;
        if let Some(operation) = state.operations.iter().find(|operation| operation.request.quote_id == quote_id).cloned() {
            return self.submit_operation(&wallet, &scope, &mut state, operation, version, CreationAttempt::Recovery).await;
        }
        if state.operations.iter().any(|operation| operation.order_id.is_none()) {
            return Err(BsvHomePortalSwapError::PendingCreation);
        }
        let quote = self
            .quotes
            .lock()
            .await
            .get(&quote_id)
            .filter(|(quote_scope, _)| quote_scope == &scope)
            .map(|(_, quote)| quote.clone())
            .ok_or(BsvHomePortalSwapError::QuoteExpired)?;
        if quote.expires_at <= Utc::now().timestamp() + 10 {
            return Err(BsvHomePortalSwapError::QuoteExpired);
        }
        let (payout_address, refund_address) = swap_rules::wallet_addresses(&wallet, quote.direction)?;
        self.check_source_balance(&wallet, &quote).await?;
        self.check_workflow(&wallet, version).await?;
        let operation = PendingOperation {
            request: OrderRequest {
                quote_id,
                client_operation_id: Uuid::new_v4().to_string(),
                payout_address,
                refund_address,
            },
            quote,
            order_id: None,
        };
        state.operations.push(operation.clone());
        self.write_state(&scope, &state)?;
        self.submit_operation(&wallet, &scope, &mut state, operation, version, CreationAttempt::Initial).await
    }

    pub async fn has_pending_order(&self) -> Result<bool, BsvHomePortalSwapError> {
        let wallet = self.auth.swap_wallet().await?;
        Ok(self.read_state(&self.scope(&wallet)?)?.operations.iter().any(|operation| operation.order_id.is_none()))
    }

    pub async fn retry_pending_order(&self) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
        let version = self.workflow_version();
        let _mutation = self.mutation.lock().await;
        let wallet = self.auth.swap_wallet().await?;
        let scope = self.scope(&wallet)?;
        let mut state = self.read_state(&scope)?;
        let operation = state
            .operations
            .iter()
            .find(|operation| operation.order_id.is_none())
            .cloned()
            .ok_or(BsvHomePortalSwapError::PendingCreation)?;
        self.submit_operation(&wallet, &scope, &mut state, operation, version, CreationAttempt::Recovery).await
    }

    pub async fn cached_orders(&self) -> Result<Vec<BsvHomePortalSwapOrder>, BsvHomePortalSwapError> {
        let wallet = self.auth.swap_wallet().await?;
        let mut orders = self.read_state(&self.scope(&wallet)?)?.orders;
        for order in &mut orders {
            order.can_pay = false;
            order.sync_status = BsvHomePortalSwapSyncStatus::Stale;
        }
        orders.sort_by_key(|order| -order.updated_at);
        Ok(orders)
    }

    pub async fn orders(&self, active: bool, page: u32, size: u32) -> Result<BsvHomePortalSwapOrderPage, BsvHomePortalSwapError> {
        let version = self.workflow_version();
        if page == 0 || size == 0 || size > 50 {
            return Err(BsvHomePortalSwapError::InvalidResponse);
        }
        let wallet = self.auth.swap_wallet().await?;
        let scope = self.scope(&wallet)?;
        let path = format!("/auth/swaps/orders?{}page={page}&size={size}", if active { "status=active&" } else { "" });
        let response = self.auth.swap_wallet_request::<WireOrderPage>(&wallet, &path, AlienHttpMethod::Get, None).await?;
        self.check_workflow(&wallet, version).await?;
        let _mutation = self.mutation.lock().await;
        let mut state = self.read_state(&scope)?;
        let records = response.records.into_iter().map(|wire| self.merge_order(&mut state, wire)).collect::<Result<Vec<_>, _>>()?;
        self.write_state(&scope, &state)?;
        Ok(BsvHomePortalSwapOrderPage { total: response.total, records })
    }

    pub async fn order(&self, order_id: String) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
        self.fetch_order(&order_id, false).await
    }

    pub async fn refresh_order(&self, order_id: String) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
        self.fetch_order(&order_id, true).await
    }

    pub async fn resolve_action(&self, order_id: String, action_id: String) -> Result<BsvHomePortalSwapActionNavigation, BsvHomePortalSwapError> {
        let version = self.workflow_version();
        swap_rules::resource_id(&order_id)?;
        swap_rules::resource_id(&action_id)?;
        let wallet = self.auth.swap_wallet().await?;
        let order = self.fetch_order(&order_id, false).await?;
        self.check_workflow(&wallet, version).await?;
        let action = order
            .required_actions
            .iter()
            .find(|action| action.action_id == action_id)
            .ok_or(BsvHomePortalSwapError::ActionUnavailable)?;
        if action.expires_at.is_some_and(|expiry| expiry <= Utc::now().timestamp()) {
            return Err(BsvHomePortalSwapError::ActionUnavailable);
        }
        let path = format!("/auth/swaps/orders/{order_id}/actions/{action_id}/resolve");
        let navigation = self.auth.swap_wallet_request::<WireNavigation>(&wallet, &path, AlienHttpMethod::Post, None).await?;
        self.check_workflow(&wallet, version).await?;
        if navigation.entry_mode != action.entry_mode {
            return Err(BsvHomePortalSwapError::ActionUnavailable);
        }
        swap_rules::navigation(navigation, Utc::now())
    }

    pub fn cancel(&self) {
        self.auth.cancel();
    }
}

impl BsvHomePortalSwapService {
    pub(super) async fn report_broadcast(&self, order_id: String, source_txid: String) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
        let version = self.workflow_version();
        swap_rules::resource_id(&order_id)?;
        swap_rules::transaction_id(&source_txid)?;
        let wallet = self.auth.swap_wallet().await?;
        let scope = self.scope(&wallet)?;
        let saved = self.read_payment(&wallet, &order_id)?.ok_or(BsvHomePortalSwapError::PaymentUnavailable)?;
        if saved.transaction_id != source_txid {
            return Err(BsvHomePortalSwapError::BroadcastConflict);
        }
        {
            let _mutation = self.mutation.lock().await;
            let mut state = self.read_state(&scope)?;
            let order = state
                .orders
                .iter_mut()
                .find(|order| order.order_id == order_id)
                .ok_or(BsvHomePortalSwapError::PaymentUnavailable)?;
            if order.source_txid.as_ref().is_some_and(|value| value != &source_txid) {
                return Err(BsvHomePortalSwapError::BroadcastConflict);
            }
            order.source_txid = Some(source_txid.clone());
            order.broadcast_report_pending = true;
            order.can_pay = false;
            self.write_state(&scope, &state)?;
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Broadcast<'a> {
            source_txid: &'a str,
        }
        self.check_workflow(&wallet, version).await?;
        let path = format!("/auth/swaps/orders/{order_id}/broadcast");
        self.auth
            .swap_wallet_request::<Option<Value>>(&wallet, &path, AlienHttpMethod::Post, Some(encode(&Broadcast { source_txid: &source_txid })?))
            .await?;
        self.check_wallet(&wallet).await?;
        self.fetch_order(&order_id, false).await
    }

    pub(super) fn workflow_version(&self) -> u64 {
        self.auth.connection_version.load(Ordering::SeqCst)
    }

    pub(super) async fn check_workflow(&self, wallet: &Wallet, version: u64) -> Result<(), BsvHomePortalSwapError> {
        self.auth.check_version(version)?;
        self.check_wallet(wallet).await?;
        self.auth.check_version(version)?;
        Ok(())
    }

    pub(super) fn scope(&self, wallet: &Wallet) -> Result<String, BsvHomePortalSwapError> {
        let identity = super::rules::identity(wallet)?;
        Ok(format!("{}|{}|{}", self.auth.client.base_url, identity.wallet_id, identity.address))
    }

    pub(super) fn read_state(&self, scope: &str) -> Result<SwapState, BsvHomePortalSwapError> {
        self.store
            .get(scope.into())?
            .map(|value| serde_json::from_str(&value).map_err(|_| BsvHomePortalSwapError::InvalidResponse))
            .transpose()
            .map(|state| state.unwrap_or_default())
    }

    pub(super) fn write_state(&self, scope: &str, state: &SwapState) -> Result<(), BsvHomePortalSwapError> {
        self.store.set(scope.into(), encode_string(state)?)?;
        Ok(())
    }

    pub(super) async fn check_wallet(&self, wallet: &Wallet) -> Result<(), BsvHomePortalSwapError> {
        let current = self.auth.swap_wallet().await?;
        if current.id != wallet.id || super::rules::identity(&current)? != super::rules::identity(wallet)? {
            return Err(BsvHomePortalAuthError::WalletChanged.into());
        }
        Ok(())
    }

    async fn submit_operation(
        &self,
        wallet: &Wallet,
        scope: &str,
        state: &mut SwapState,
        operation: PendingOperation,
        version: u64,
        attempt: CreationAttempt,
    ) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
        if let Some(order_id) = &operation.order_id {
            return state
                .orders
                .iter()
                .find(|order| &order.order_id == order_id)
                .cloned()
                .ok_or(BsvHomePortalSwapError::InvalidResponse);
        }
        swap_rules::validate_supported_direction(operation.quote.direction)?;
        let (payout, refund) = swap_rules::wallet_addresses(wallet, operation.quote.direction)?;
        if payout != operation.request.payout_address || refund != operation.request.refund_address {
            return Err(BsvHomePortalSwapError::OrderMismatch);
        }
        self.check_workflow(wallet, version).await?;
        let response = self
            .auth
            .swap_wallet_request::<WireOrder>(wallet, "/auth/swaps/orders", AlienHttpMethod::Post, Some(encode(&operation.request)?))
            .await;
        let wire = match response {
            Ok(wire) => wire,
            Err(error) => {
                if attempt == CreationAttempt::Initial
                    && matches!(&error, BsvHomePortalAuthError::Api { code, .. } if matches!(code.as_str(), "SWAP_QUOTE_EXPIRED" | "SWAP_PAIR_UNAVAILABLE" | "SWAP_QUOTE_PROVIDER_MISMATCH" | "SWAP_AMOUNT_OUT_OF_RANGE" | "SYS_REQUEST_PARAM_ERROR"))
                {
                    state
                        .operations
                        .retain(|stored| stored.request.client_operation_id != operation.request.client_operation_id);
                    self.write_state(scope, state)?;
                }
                return Err(error.into());
            }
        };
        let order = swap_rules::order(wire, None, Some(&operation.quote), Some(&operation.request), Utc::now())?;
        let stored = state
            .operations
            .iter_mut()
            .find(|stored| stored.request.client_operation_id == operation.request.client_operation_id)
            .ok_or(BsvHomePortalSwapError::InvalidResponse)?;
        stored.order_id = Some(order.order_id.clone());
        state.orders.push(order.clone());
        self.write_state(scope, state)?;
        self.check_workflow(wallet, version).await?;
        Ok(order)
    }

    async fn fetch_order(&self, order_id: &str, refresh: bool) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
        let version = self.workflow_version();
        swap_rules::resource_id(order_id)?;
        let wallet = self.auth.swap_wallet().await?;
        let scope = self.scope(&wallet)?;
        let (path, method) = if refresh {
            (format!("/auth/swaps/orders/{order_id}/refresh"), AlienHttpMethod::Post)
        } else {
            (format!("/auth/swaps/orders/{order_id}"), AlienHttpMethod::Get)
        };
        let mut wire = self.auth.swap_wallet_request::<WireOrder>(&wallet, &path, method, None).await?;
        if wire.order_id != order_id {
            return Err(BsvHomePortalSwapError::OrderMismatch);
        }
        self.check_workflow(&wallet, version).await?;
        if refresh && wire.source_txid.is_none() {
            let local = self
                .read_state(&scope)?
                .orders
                .into_iter()
                .find(|order| order.order_id == order_id)
                .and_then(|order| order.source_txid);
            if let Some(local) = local {
                let saved = self.read_payment(&wallet, order_id)?.ok_or(BsvHomePortalSwapError::PaymentUnavailable)?;
                if local != saved.transaction_id {
                    return Err(BsvHomePortalSwapError::BroadcastConflict);
                }
                #[derive(Serialize)]
                #[serde(rename_all = "camelCase")]
                struct Broadcast<'a> {
                    source_txid: &'a str,
                }
                self.check_workflow(&wallet, version).await?;
                let broadcast_path = format!("/auth/swaps/orders/{order_id}/broadcast");
                self.auth
                    .swap_wallet_request::<Option<Value>>(&wallet, &broadcast_path, AlienHttpMethod::Post, Some(encode(&Broadcast { source_txid: &local })?))
                    .await?;
                wire = self
                    .auth
                    .swap_wallet_request::<WireOrder>(&wallet, &format!("/auth/swaps/orders/{order_id}"), AlienHttpMethod::Get, None)
                    .await?;
                if wire.order_id != order_id {
                    return Err(BsvHomePortalSwapError::OrderMismatch);
                }
                self.check_workflow(&wallet, version).await?;
            }
        }
        let _mutation = self.mutation.lock().await;
        let mut state = self.read_state(&scope)?;
        let order = self.merge_order(&mut state, wire)?;
        self.write_state(&scope, &state)?;
        Ok(order)
    }

    fn merge_order(&self, state: &mut SwapState, wire: WireOrder) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
        let previous = state.orders.iter().find(|order| order.order_id == wire.order_id);
        if let Some(previous) = previous
            && (wire.version < previous.version || (wire.version == previous.version && wire.updated_at.timestamp() < previous.updated_at))
        {
            return Ok(previous.clone());
        }
        let operation = state
            .operations
            .iter()
            .find(|operation| operation.order_id.as_ref() == Some(&wire.order_id) || wire.client_operation_id.as_ref() == Some(&operation.request.client_operation_id));
        let order = swap_rules::order(
            wire,
            previous,
            operation.map(|operation| &operation.quote),
            operation.map(|operation| &operation.request),
            Utc::now(),
        )?;
        if let Some(stored) = state.orders.iter_mut().find(|stored| stored.order_id == order.order_id) {
            *stored = order.clone();
        } else {
            state.orders.push(order.clone());
        }
        if let Some(operation) = state
            .operations
            .iter_mut()
            .find(|operation| order.client_operation_id.as_ref() == Some(&operation.request.client_operation_id))
        {
            operation.order_id = Some(order.order_id.clone());
        }
        Ok(order)
    }
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, BsvHomePortalSwapError> {
    serde_json::to_vec(value).map_err(|_| BsvHomePortalSwapError::InvalidResponse)
}

fn encode_string<T: Serialize>(value: &T) -> Result<String, BsvHomePortalSwapError> {
    serde_json::to_string(value).map_err(|_| BsvHomePortalSwapError::InvalidResponse)
}
