use std::sync::Arc;

use async_trait::async_trait;
use bsv_home_bsv::finalize_wallet_transaction;
use chrono::Utc;
use num_bigint::BigInt;
use num_bigint::BigUint;
use primitives::{AssetType, Chain, Transaction, TransactionIdRequest, TransactionInputType, TransactionState, Wallet};

use crate::BsvHomeCoreError;
use crate::services::confirm::{BsvHomeTransactionCheckpoint, BsvHomeTransactionCheckpointData};
use crate::services::error::BsvHomeServiceError;
use crate::services::transfer::{BsvHomeRecipient, BsvHomeTransferData};

use super::swap_error::BsvHomePortalSwapError;
use super::swap_model::{
    BsvHomePortalSwapOrder, BsvHomePortalSwapOrderStatus, BsvHomePortalSwapPaymentOrder, BsvHomePortalSwapPaymentPreparation, BsvHomePortalSwapPaymentRecovery,
    BsvHomePortalSwapQuote,
};
use super::swap_rules;
use super::{BsvHomePortalAuthError, BsvHomePortalSwapService};

struct OrderPaymentCheckpoint {
    service: Arc<BsvHomePortalSwapService>,
    wallet: Wallet,
    order: BsvHomePortalSwapOrder,
    transfer: BsvHomeTransferData,
    version: u64,
}

#[uniffi::export]
impl BsvHomePortalSwapService {
    pub async fn prepare_payment(self: Arc<Self>, order_id: String) -> Result<BsvHomePortalSwapPaymentPreparation, BsvHomePortalSwapError> {
        let version = self.workflow_version();
        let wallet = self.auth.swap_wallet().await?;
        let order = self.order(order_id).await?;
        self.check_workflow(&wallet, version).await?;
        validate_payment(&wallet, &order)?;
        let transfer = self.transfer(&order).await?;
        self.check_workflow(&wallet, version).await?;
        let payment = BsvHomePortalSwapPaymentOrder {
            order_id: order.order_id.clone(),
            wallet_id: wallet.id.clone(),
            from_asset_id: order.from.as_ref().ok_or(BsvHomePortalSwapError::PaymentUnavailable)?.asset_id.clone(),
            from_amount: order.from_amount.clone(),
            deposit_address: order.deposit_address.clone().ok_or(BsvHomePortalSwapError::PaymentUnavailable)?,
            deposit_extra_id: order.deposit_extra_id.clone(),
            payout_asset_id: order.to.as_ref().ok_or(BsvHomePortalSwapError::PaymentUnavailable)?.asset_id.clone(),
            payout_address: order.payout_address.clone(),
            refund_address: order.refund_address.clone(),
            estimated_to_amount: order.estimated_to_amount.clone(),
            provider_display_name: order.provider_display_name.clone(),
            deposit_expires_at: order.deposit_expires_at,
            version: order.version,
        };
        let checkpoint = Arc::new(OrderPaymentCheckpoint {
            service: self,
            wallet,
            order: order.clone(),
            transfer: transfer.clone(),
            version,
        });
        Ok(BsvHomePortalSwapPaymentPreparation {
            payment,
            order,
            transfer,
            checkpoint,
        })
    }

    pub async fn saved_payment(&self, order_id: String) -> Result<Option<BsvHomeTransactionCheckpointData>, BsvHomePortalSwapError> {
        swap_rules::resource_id(&order_id)?;
        let wallet = self.auth.swap_wallet().await?;
        self.read_payment(&wallet, &order_id)
    }

    pub async fn report_saved_broadcast(&self, order_id: String) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
        let saved = self.saved_payment(order_id.clone()).await?.ok_or(BsvHomePortalSwapError::PaymentUnavailable)?;
        self.report_broadcast(order_id, saved.transaction_id).await
    }

    pub async fn recover_payment(&self, order_id: String) -> Result<BsvHomePortalSwapPaymentRecovery, BsvHomePortalSwapError> {
        let version = self.workflow_version();
        let wallet = self.auth.swap_wallet().await?;
        let saved = self.read_payment(&wallet, &order_id)?.ok_or(BsvHomePortalSwapError::PaymentUnavailable)?;
        let order = self.order(order_id.clone()).await?;
        self.check_workflow(&wallet, version).await?;
        let transfer = self.transfer(&order).await?;
        saved.validate_transfer(&wallet, &transfer).map_err(|_| BsvHomePortalSwapError::OrderMismatch)?;
        self.addresses.validate_checkpoint(&wallet, &saved).await?;
        if order.source_txid.as_ref().is_some_and(|transaction_id| transaction_id != &saved.transaction_id) {
            return Err(BsvHomePortalSwapError::BroadcastConflict);
        }
        let transaction = self
            .gateway
            .transaction_by_hash(TransactionIdRequest::new(saved.asset_id.chain, saved.transaction_id.clone(), None))
            .await
            .map_err(BsvHomeServiceError::from)?;
        self.check_workflow(&wallet, version).await?;
        if let Some(transaction) = transaction {
            let owned_addresses = if saved.asset_id.chain == Chain::BitcoinSv {
                self.addresses.owned_addresses(wallet.id.clone()).await?
            } else {
                vec![saved.sender_address.clone()]
            };
            self.check_workflow(&wallet, version).await?;
            let transaction = recovered_transaction(transaction, &owned_addresses, &saved.recipient_address, &saved.value)?;
            let accepted_state = match transaction.state {
                TransactionState::Pending | TransactionState::Confirmed | TransactionState::InTransit => true,
                TransactionState::Failed | TransactionState::Reverted | TransactionState::Refunded => false,
            };
            if transaction.hash() != saved.transaction_id
                || transaction.asset_id != saved.asset_id
                || transaction.to != saved.recipient_address
                || transaction.value != order.from_amount
                || !accepted_state
            {
                return Err(BsvHomePortalSwapError::OrderMismatch);
            }
            let order = self.report_broadcast(order_id, saved.transaction_id).await?;
            return Ok(BsvHomePortalSwapPaymentRecovery { order, can_rebroadcast: false });
        }
        let can_rebroadcast = order.source_txid.is_none() && swap_rules::can_pay(&order, Utc::now());
        Ok(BsvHomePortalSwapPaymentRecovery { order, can_rebroadcast })
    }

    pub async fn verify_payout(&self, order_id: String) -> Result<BsvHomePortalSwapOrder, BsvHomePortalSwapError> {
        let version = self.workflow_version();
        let wallet = self.auth.swap_wallet().await?;
        let mut order = self.order(order_id).await?;
        if order.order_status != BsvHomePortalSwapOrderStatus::Completed {
            return Err(BsvHomePortalSwapError::PaymentUnavailable);
        }
        let direction = order.direction.ok_or(BsvHomePortalSwapError::PaymentUnavailable)?;
        let (payout, refund) = swap_rules::wallet_addresses(&wallet, direction)?;
        if order.payout_address != payout || order.refund_address != refund {
            return Err(BsvHomePortalSwapError::OrderMismatch);
        }
        let destination = order.to.as_ref().ok_or(BsvHomePortalSwapError::PaymentUnavailable)?;
        let hash = order.payout_txid.as_ref().ok_or(BsvHomePortalSwapError::PayoutUnconfirmed)?;
        let amount = order.actual_to_amount.as_ref().ok_or(BsvHomePortalSwapError::PayoutUnconfirmed)?;
        let transaction = self
            .gateway
            .transaction_by_hash(TransactionIdRequest::new(destination.asset_id.chain, hash.clone(), None))
            .await
            .map_err(BsvHomeServiceError::from)?
            .ok_or(BsvHomePortalSwapError::PayoutUnconfirmed)?
            .finalize(vec![payout.clone()]);
        self.check_workflow(&wallet, version).await?;
        if transaction.hash() != hash || transaction.asset_id != destination.asset_id || transaction.to != payout || &transaction.value != amount {
            return Err(BsvHomePortalSwapError::OrderMismatch);
        }
        if transaction.state != TransactionState::Confirmed {
            return Err(BsvHomePortalSwapError::PayoutUnconfirmed);
        }
        order.payout_verified = true;
        let _mutation = self.mutation.lock().await;
        let scope = self.scope(&wallet)?;
        let mut state = self.read_state(&scope)?;
        let stored = state
            .orders
            .iter_mut()
            .find(|stored| stored.order_id == order.order_id)
            .ok_or(BsvHomePortalSwapError::InvalidResponse)?;
        if stored.version != order.version || stored.payout_txid != order.payout_txid || stored.actual_to_amount != order.actual_to_amount {
            return Err(BsvHomePortalSwapError::OrderMismatch);
        }
        *stored = order.clone();
        self.write_state(&scope, &state)?;
        Ok(order)
    }
}

impl BsvHomePortalSwapService {
    pub(super) async fn check_source_balance(&self, wallet: &Wallet, quote: &BsvHomePortalSwapQuote) -> Result<(), BsvHomePortalSwapError> {
        let source = wallet.account(quote.from.asset_id.chain).ok_or(BsvHomePortalSwapError::MissingAccount)?;
        let balance = match &quote.from.asset_id.token_id {
            None if quote.from.asset_id.chain == Chain::BitcoinSv => self.addresses.balance(wallet.id.clone(), true).await?,
            None => self
                .gateway
                .get_balance_coin(quote.from.asset_id.chain, source.address.clone())
                .await
                .map_err(BsvHomeServiceError::from)?,
            Some(contract) => self
                .gateway
                .get_balance_tokens(quote.from.asset_id.chain, source.address.clone(), vec![contract.clone()])
                .await
                .map_err(BsvHomeServiceError::from)?
                .into_iter()
                .find(|balance| balance.asset_id == quote.from.asset_id)
                .ok_or(BsvHomePortalSwapError::InvalidResponse)?,
        };
        if balance.asset_id != quote.from.asset_id {
            return Err(BsvHomePortalSwapError::InvalidResponse);
        }
        if balance.balance.available < quote.from_amount {
            return Err(BsvHomePortalSwapError::InsufficientBalance {
                symbol: quote.from.symbol.clone(),
                available_display: swap_rules::display(&balance.balance.available, &quote.from)?,
                required_display: quote.from_amount_display.clone(),
            });
        }
        if quote.from.asset_id.chain == Chain::Tron {
            let fee = self
                .gateway
                .get_balance_coin(Chain::Tron, source.address.clone())
                .await
                .map_err(BsvHomeServiceError::from)?;
            if fee.balance.available == BigUint::from(0u32) {
                let resources = self
                    .gateway
                    .get_balance_staking(Chain::Tron, source.address.clone())
                    .await
                    .map_err(BsvHomeServiceError::from)?;
                let metadata = resources
                    .and_then(|balance| balance.balance.metadata)
                    .ok_or(BsvHomePortalSwapError::NetworkFeeUnavailable)?;
                if metadata.energy_available == 0 || metadata.bandwidth_available == 0 {
                    return Err(BsvHomePortalSwapError::NetworkFeeUnavailable);
                }
            }
        }
        Ok(())
    }

    async fn transfer(&self, order: &BsvHomePortalSwapOrder) -> Result<BsvHomeTransferData, BsvHomePortalSwapError> {
        let from = order.from.as_ref().ok_or(BsvHomePortalSwapError::PaymentUnavailable)?;
        let asset = self.assets.ensure_asset(from.asset_id.clone()).await?;
        let expected_type = if from.asset_id.is_native() { AssetType::NATIVE } else { AssetType::TRC20 };
        if asset.id != from.asset_id || asset.decimals != from.scale as i32 || asset.asset_type != expected_type {
            return Err(BsvHomePortalSwapError::OrderMismatch);
        }
        Ok(BsvHomeTransferData {
            input_type: TransactionInputType::Transfer { asset },
            recipient: BsvHomeRecipient::address(order.deposit_address.clone().ok_or(BsvHomePortalSwapError::PaymentUnavailable)?),
            value: BigInt::from(order.from_amount.clone()),
            use_max_amount: false,
        })
    }

    fn payment_key(&self, wallet: &Wallet, order_id: &str) -> Result<String, BsvHomePortalSwapError> {
        Ok(format!("{}|{order_id}", self.scope(wallet)?))
    }

    pub(super) fn read_payment(&self, wallet: &Wallet, order_id: &str) -> Result<Option<BsvHomeTransactionCheckpointData>, BsvHomePortalSwapError> {
        self.payment_store
            .get(self.payment_key(wallet, order_id)?)?
            .map(|value| serde_json::from_str(&value).map_err(|_| BsvHomePortalSwapError::InvalidResponse))
            .transpose()
    }
}

fn recovered_transaction(transaction: Transaction, owned_addresses: &[String], recipient: &str, confirmed_value: &BigInt) -> Result<Transaction, BsvHomePortalSwapError> {
    if transaction.asset_id.chain != Chain::BitcoinSv {
        return Ok(transaction.finalize(owned_addresses.to_vec()));
    }
    let mut transaction = finalize_wallet_transaction(transaction, owned_addresses).map_err(|error| BsvHomeServiceError::Core { msg: error.to_string() })?;
    if owned_addresses.iter().any(|address| address == recipient) {
        transaction.value = confirmed_value.to_biguint().ok_or(BsvHomePortalSwapError::OrderMismatch)?;
        transaction.to = recipient.to_string();
    }
    Ok(transaction)
}

fn validate_payment(wallet: &Wallet, order: &BsvHomePortalSwapOrder) -> Result<(), BsvHomePortalSwapError> {
    if !swap_rules::can_pay(order, Utc::now()) || order.deposit_extra_id.as_ref().is_some_and(|value| !value.is_empty()) {
        return Err(BsvHomePortalSwapError::PaymentUnavailable);
    }
    let (payout, refund) = swap_rules::wallet_addresses(wallet, order.direction.ok_or(BsvHomePortalSwapError::PaymentUnavailable)?)?;
    if payout != order.payout_address || refund != order.refund_address {
        return Err(BsvHomePortalSwapError::OrderMismatch);
    }
    Ok(())
}

fn checkpoint_error(error: impl ToString) -> BsvHomeCoreError {
    BsvHomeCoreError::AnyError { msg: error.to_string() }
}

fn checkpoint_workflow_error(error: BsvHomePortalSwapError) -> BsvHomeCoreError {
    match error {
        BsvHomePortalSwapError::Auth {
            error: BsvHomePortalAuthError::Service {
                error: BsvHomeServiceError::Cancelled,
            },
        }
        | BsvHomePortalSwapError::Service {
            error: BsvHomeServiceError::Cancelled,
        } => BsvHomeCoreError::Cancelled,
        error => checkpoint_error(error),
    }
}

impl OrderPaymentCheckpoint {
    fn validate_order(&self, order: &BsvHomePortalSwapOrder) -> Result<(), BsvHomePortalSwapError> {
        validate_payment(&self.wallet, order)?;
        if order.deposit_address != self.order.deposit_address || order.from_amount != self.order.from_amount || order.direction != self.order.direction {
            return Err(BsvHomePortalSwapError::OrderMismatch);
        }
        Ok(())
    }
}

#[async_trait]
impl BsvHomeTransactionCheckpoint for OrderPaymentCheckpoint {
    async fn authorize_broadcast(&self, data: BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeCoreError> {
        self.service.check_workflow(&self.wallet, self.version).await.map_err(checkpoint_workflow_error)?;
        data.validate_transfer(&self.wallet, &self.transfer).map_err(checkpoint_error)?;
        self.service
            .addresses
            .validate_checkpoint(&self.wallet, &data)
            .await
            .map_err(|error| checkpoint_workflow_error(error.into()))?;
        let order = self.service.order(self.order.order_id.clone()).await.map_err(checkpoint_workflow_error)?;
        self.service.check_workflow(&self.wallet, self.version).await.map_err(checkpoint_workflow_error)?;
        self.validate_order(&order).map_err(checkpoint_error)?;
        let saved = self.service.read_payment(&self.wallet, &self.order.order_id).map_err(checkpoint_error)?;
        if saved.as_ref() != Some(&data) {
            return Err(checkpoint_error(BsvHomePortalSwapError::BroadcastConflict));
        }
        self.service.check_workflow(&self.wallet, self.version).await.map_err(checkpoint_workflow_error)
    }

    async fn load(&self) -> Result<Option<BsvHomeTransactionCheckpointData>, BsvHomeCoreError> {
        self.service.check_workflow(&self.wallet, self.version).await.map_err(checkpoint_workflow_error)?;
        let saved = self.service.read_payment(&self.wallet, &self.order.order_id).map_err(checkpoint_error)?;
        if let Some(saved) = &saved {
            saved.validate_transfer(&self.wallet, &self.transfer).map_err(checkpoint_error)?;
            self.service
                .addresses
                .validate_checkpoint(&self.wallet, saved)
                .await
                .map_err(|error| checkpoint_workflow_error(error.into()))?;
            if !self
                .service
                .recover_payment(self.order.order_id.clone())
                .await
                .map_err(checkpoint_workflow_error)?
                .can_rebroadcast
            {
                return Err(checkpoint_error(BsvHomePortalSwapError::PaymentUnavailable));
            }
        } else {
            let scope = self.service.scope(&self.wallet).map_err(checkpoint_error)?;
            let state = self.service.read_state(&scope).map_err(checkpoint_error)?;
            let order = state
                .orders
                .iter()
                .find(|order| order.order_id == self.order.order_id)
                .ok_or_else(|| checkpoint_error(BsvHomePortalSwapError::PaymentUnavailable))?;
            self.validate_order(order).map_err(checkpoint_error)?;
        }
        Ok(saved)
    }

    async fn save(&self, data: BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeCoreError> {
        self.service.check_workflow(&self.wallet, self.version).await.map_err(checkpoint_workflow_error)?;
        data.validate_transfer(&self.wallet, &self.transfer).map_err(checkpoint_error)?;
        self.service
            .addresses
            .validate_checkpoint(&self.wallet, &data)
            .await
            .map_err(|error| checkpoint_workflow_error(error.into()))?;
        let _mutation = self.service.mutation.lock().await;
        if let Some(saved) = self.service.read_payment(&self.wallet, &self.order.order_id).map_err(checkpoint_error)? {
            if saved == data {
                return Ok(());
            }
            return Err(checkpoint_error(BsvHomePortalSwapError::BroadcastConflict));
        }
        let scope = self.service.scope(&self.wallet).map_err(checkpoint_error)?;
        let state = self.service.read_state(&scope).map_err(checkpoint_error)?;
        let order = state
            .orders
            .iter()
            .find(|order| order.order_id == self.order.order_id)
            .ok_or_else(|| checkpoint_error(BsvHomePortalSwapError::PaymentUnavailable))?;
        self.validate_order(order).map_err(checkpoint_error)?;
        let key = self.service.payment_key(&self.wallet, &self.order.order_id).map_err(checkpoint_error)?;
        let value = serde_json::to_string(&data).map_err(checkpoint_error)?;
        self.service.payment_store.set(key, value).map_err(checkpoint_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use primitives::{TransactionDirection, TransactionUtxoInput};

    use super::*;

    #[test]
    fn test_recovered_bsv_payment_projects_all_owned_addresses_and_uses_confirmed_internal_amount() {
        let input = TransactionUtxoInput::new("input".to_string(), 50_000u32.into());
        let payment = TransactionUtxoInput::new("recipient".to_string(), 10_000u32.into());
        let change = TransactionUtxoInput::new("change".to_string(), 39_900u32.into());
        let mut transaction = Transaction::mock_utxo(vec![input], vec![change, payment]);
        transaction.asset_id = Chain::BitcoinSv.as_asset_id();
        let owned_addresses = vec!["input".to_string(), "change".to_string()];
        let external = recovered_transaction(transaction.clone(), &owned_addresses, "recipient", &10_000.into()).unwrap();
        assert_eq!(
            (external.direction, external.to, external.value),
            (TransactionDirection::Outgoing, "recipient".to_string(), 10_000u32.into())
        );

        let internal_addresses = [owned_addresses, vec!["recipient".to_string()]].concat();
        let internal = recovered_transaction(transaction.clone(), &internal_addresses, "recipient", &10_000.into()).unwrap();
        assert_eq!(
            (internal.direction, internal.to, internal.value),
            (TransactionDirection::SelfTransfer, "recipient".to_string(), 10_000u32.into())
        );
        assert_eq!(
            recovered_transaction(transaction, &internal_addresses, "recipient", &(-1).into()).unwrap_err(),
            BsvHomePortalSwapError::OrderMismatch
        );
    }
}
