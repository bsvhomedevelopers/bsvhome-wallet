#![allow(clippy::result_large_err)]

mod authorization;
mod checkpoint;
mod error;
mod model;
pub(crate) mod rules;
#[cfg(test)]
mod scope_tests;
pub(crate) mod screen;
mod session;
mod signer;
#[cfg(test)]
mod single_payer_tests;
#[cfg(test)]
pub(crate) mod testkit;
mod transfer;

use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use authorization::BsvHomeConfirmAuthorization;
use authorization::ExecutionScope;
use bsv_home_bsv::rpc::ProviderError;
pub use checkpoint::{BsvHomeTransactionCheckpoint, BsvHomeTransactionCheckpointData};
pub use error::BsvHomeConfirmError;
pub use model::*;
pub use rules::acquire_asset_flow;
pub use session::BsvHomeConfirmSession;
pub use signer::BsvHomeTransactionSigner;
pub use transfer::BsvHomeConfirmTransferService;

use crate::gateway::{BsvHomeGateway, map_network_error};
use crate::models::asset::chain_fee_asset_ids;
use crate::models::gateway::BsvHomeTransactionPreloadInput;
use crate::models::transaction::{BsvHomeSignedTransaction, BsvHomeTransactionLoadInput};
use crate::services::BsvHomeScanService;
use crate::services::assets::BsvHomeAssetsService;
use crate::services::balance::BsvHomeBalanceService;
use crate::services::bsv_cold_wallet::BsvHomeBsvColdWalletStore;
use crate::services::bsv_cold_wallet::rules::require_hot_wallet_id;
use crate::services::clock::sleep;
use crate::services::confirm::rules::ConfirmInput;
use crate::services::price::BsvHomePriceService;
use crate::services::simulation::{BsvHomeSimulationFormatter, BsvHomeSimulationService};
use crate::services::transaction_state::{BsvHomeTransactionStateService, BsvHomeTransactionStatusService};
use crate::services::transfer::BsvHomeTransferData;
use crate::services::transfer::rules::TransferInput;
use crate::services::wallet::addresses::BsvHomeBsvAddressPool;
use crate::signer::BsvHomeSignerError;
use primitives::TransactionInputType;
use primitives::{Asset, AssetId, Chain, SimulationPayloadFieldDisplay, SimulationResult, Transaction, TransferDataOutputAction, Wallet, WalletId};

#[derive(Clone, Copy)]
enum BsvBroadcastSubmission {
    Ready,
    Unknown,
}

#[derive(uniffi::Object)]
pub struct BsvHomeConfirmService {
    gateway: Arc<BsvHomeGateway>,
    simulation: Arc<BsvHomeSimulationService>,
    scanner: Arc<BsvHomeScanService>,
    transaction_state: Arc<BsvHomeTransactionStateService>,
    balance: Arc<BsvHomeBalanceService>,
    price: Arc<BsvHomePriceService>,
    assets: Arc<BsvHomeAssetsService>,
    transaction_status: Arc<dyn BsvHomeTransactionStatusService>,
    simulation_formatter: BsvHomeSimulationFormatter,
    addresses: Arc<BsvHomeBsvAddressPool>,
    cold_wallet_store: Arc<dyn BsvHomeBsvColdWalletStore>,
}

#[uniffi::export]
impl BsvHomeConfirmService {
    #[uniffi::constructor]
    pub fn new(
        gateway: Arc<BsvHomeGateway>,
        simulation: Arc<BsvHomeSimulationService>,
        scanner: Arc<BsvHomeScanService>,
        transaction_state: Arc<BsvHomeTransactionStateService>,
        balance: Arc<BsvHomeBalanceService>,
        price: Arc<BsvHomePriceService>,
        assets: Arc<BsvHomeAssetsService>,
        transaction_status: Arc<dyn BsvHomeTransactionStatusService>,
        addresses: Arc<BsvHomeBsvAddressPool>,
        cold_wallet_store: Arc<dyn BsvHomeBsvColdWalletStore>,
    ) -> Self {
        Self {
            gateway,
            simulation,
            scanner,
            transaction_state,
            balance,
            price,
            assets,
            transaction_status,
            simulation_formatter: BsvHomeSimulationFormatter::new(),
            addresses,
            cold_wallet_store,
        }
    }

    pub async fn metadata(
        &self,
        wallet_id: WalletId,
        asset_id: AssetId,
        fee_asset_id: AssetId,
        extra_asset_ids: Vec<AssetId>,
    ) -> Result<BsvHomeConfirmMetadata, BsvHomeConfirmError> {
        let asset_ids = rules::metadata_asset_ids(&asset_id, &fee_asset_id, extra_asset_ids);
        let (balances, prices) = futures::join!(self.balance.balances(wallet_id, asset_ids.clone()), self.price.prices(asset_ids));
        rules::build_metadata(asset_id, fee_asset_id, balances?, prices?)
    }

    pub async fn sync_missing_assets(&self, asset_ids: Vec<AssetId>) -> Result<Vec<AssetId>, crate::services::error::BsvHomeServiceError> {
        self.assets.sync_missing_assets(asset_ids).await
    }

    pub async fn load(&self, wallet_id: WalletId, input: BsvHomeConfirmInput, options: BsvHomeConfirmLoadOptions) -> Result<BsvHomeConfirmData, BsvHomeConfirmError> {
        require_hot_wallet_id(wallet_id.clone(), input.transfer.input_type.get_asset().chain(), self.cold_wallet_store.as_ref())?;
        let transfer = &input.transfer;
        let asset = transfer.input_type.get_asset();
        let chain = asset.id.chain;
        self.require_input_assets(&transfer.input_type)?;
        if let Some(asset_id) = &options.fee_asset_id {
            self.gateway.require_asset(asset_id)?;
        }
        if chain == Chain::BitcoinSv {
            if options.fee_asset_id.as_ref().is_some_and(|asset_id| *asset_id != Chain::BitcoinSv.as_asset_id()) {
                return Err(BsvHomeConfirmError::Load {
                    msg: "BSV fees must use the native asset".into(),
                });
            }
            self.addresses.prepare(wallet_id.clone()).await?;
            let wallet = self.addresses.wallet(&wallet_id).await?;
            self.addresses.refresh_reservations(&wallet).await?;
            if let Some(pending) = self
                .addresses
                .pending_input(&wallet, &transfer.recipient.address, &transfer.value.to_string(), transfer.use_max_amount)
                .await?
            {
                if input.from.address != pending.sender_address {
                    return Err(BsvHomeConfirmError::Load {
                        msg: "BSV saved payment sender differs from wallet".into(),
                    });
                }
                if let BsvHomeConfirmFeeSelection::Custom { gas_price } = &options.fee_selection
                    && *gas_price != pending.gas_price.gas_price()
                {
                    return Err(BsvHomeConfirmError::Load {
                        msg: "BSV unfinished payment fee is frozen".into(),
                    });
                }
                let gas_price = pending.input.gas_price.clone();
                return Ok(BsvHomeConfirmData {
                    input,
                    fee: pending.fee.into(),
                    selected_priority: primitives::FeePriority::Normal,
                    fee_rates: vec![primitives::FeeRate::new(primitives::FeePriority::Normal, gas_price).into()],
                    metadata: pending.input.metadata,
                    simulation: None,
                });
            }
        }
        let symbol = asset.symbol.clone();
        let destination = transfer.recipient.address.clone();
        let preload_input = BsvHomeTransactionPreloadInput {
            input_type: transfer.input_type.clone(),
            sender_address: input.from.address.clone(),
            destination_address: destination.clone(),
            references: transfer.recipient.references.clone(),
        };

        // A scanner outage fails open by design: the send continues without a verdict.
        let scan_future = async {
            if chain.is_testnet() {
                None
            } else {
                self.scanner.scan_transaction(rules::scan_payload(preload_input.clone())).await.ok()
            }
        };
        let preload_future = async {
            if chain == Chain::BitcoinSv
                && let Some(metadata) = self.addresses.preload_payer_metadata(wallet_id, &input.from.address, transfer.use_max_amount).await?
            {
                return Ok::<_, BsvHomeConfirmError>(metadata);
            }
            self.gateway.get_transaction_preload(chain, preload_input.clone()).await.map_err(error::load_error)
        };
        let (metadata, fee_rates, scan, simulation) = futures::join!(
            preload_future,
            self.gateway.get_fee_rates(chain, transfer.input_type.clone()),
            scan_future,
            self.simulate(chain, &input),
        );
        let metadata = metadata?;
        let fee_rates = rules::displayed_fee_rates(fee_rates.map_err(error::load_error)?);
        let simulation = simulation?;

        rules::validate_scan(scan.as_ref(), transfer.recipient.memo.as_deref(), &symbol)?;

        let selected = options.fee_selection.select_fee_rate(&fee_rates)?;
        let load = self
            .gateway
            .get_transaction_load(
                chain,
                BsvHomeTransactionLoadInput {
                    input_type: transfer.input_type.clone(),
                    sender_address: input.from.address.clone(),
                    destination_address: destination,
                    value: transfer.value.to_biguint().ok_or_else(|| BsvHomeConfirmError::Load {
                        msg: "negative transfer value".to_string(),
                    })?,
                    gas_price: selected.gas_price_type.clone(),
                    memo: transfer.recipient.memo.clone(),
                    is_max_value: transfer.use_max_amount,
                    metadata,
                },
            )
            .await
            .map_err(error::load_error)?;

        let mut fee = load.fee;
        if let Some(fee_asset_id) = options.fee_asset_id {
            fee.fee_asset = fee_asset_id;
        }

        Ok(BsvHomeConfirmData {
            input,
            fee,
            selected_priority: selected.priority,
            fee_rates,
            metadata: load.metadata,
            simulation,
        })
    }
}

impl BsvHomeConfirmService {
    pub fn simulation(&self, input_type: TransactionInputType, simulation: Option<SimulationResult>, assets: Vec<Asset>) -> Result<BsvHomeConfirmSimulation, BsvHomeConfirmError> {
        let has_critical_warning = simulation.as_ref().map(SimulationResult::has_critical_warning).unwrap_or(false);
        let approval = input_type.approval_value();
        let shows_header = self.simulation_formatter.shows_header(simulation.clone(), approval.is_some());
        let payload_fields = self
            .simulation_formatter
            .payload_fields(simulation.clone().map(|simulation| simulation.payload).unwrap_or_default(), shows_header);
        let header = match approval {
            Some((asset_id, value)) => assets
                .iter()
                .find(|asset| asset.id == asset_id)
                .map(|asset| BsvHomeSimulationValue { asset: asset.clone(), value }),
            None => simulation.as_ref().and_then(|simulation| BsvHomeSimulationValue::from_simulation(simulation, &assets)),
        };
        let balance_changes = self
            .simulation_formatter
            .balance_changes(simulation, assets.iter().map(|asset| asset.id.clone()).collect())
            .into_iter()
            .filter_map(|change| {
                let asset = assets.iter().find(|asset| asset.id == change.asset_id)?.clone();
                Some(BsvHomeSimulationBalanceChange {
                    asset,
                    sign: rules::balance_change_sign(&change.value),
                    value: change.value,
                })
            })
            .collect();
        Ok(BsvHomeConfirmSimulation {
            has_critical_warning,
            primary_fields: payload_fields
                .iter()
                .filter(|field| field.display == SimulationPayloadFieldDisplay::Primary)
                .cloned()
                .collect(),
            secondary_fields: payload_fields
                .iter()
                .filter(|field| field.display == SimulationPayloadFieldDisplay::Secondary)
                .cloned()
                .collect(),
            header,
            balance_changes,
        })
    }
}

impl BsvHomeConfirmService {
    async fn execute_bsv(
        &self,
        input: SendInput,
        signer: Arc<dyn BsvHomeTransactionSigner>,
        checkpoint: Option<Arc<dyn BsvHomeTransactionCheckpoint>>,
        scope: Option<Arc<ExecutionScope>>,
    ) -> Result<BsvHomeExecuteResult, BsvHomeConfirmError> {
        let transfer = &input.confirm.input.transfer;
        checkpoint::bsv_transfer_asset(transfer)?;
        let saved = self
            .addresses
            .saved_for_transfer(&input.wallet, &transfer.recipient.address, &transfer.value.to_string(), transfer.use_max_amount)
            .await?;
        if let Some(external) = &checkpoint
            && let Some(external_saved) = checkpoint::saved_transaction(&input, external).await?
            && saved.as_ref() != Some(&external_saved)
        {
            return Err(BsvHomeConfirmError::Record {
                msg: "BSV external payment requires validated payer-ledger recovery before broadcast".into(),
            });
        }
        if let Some(saved) = saved {
            saved.validate_confirmation(&input)?;
            authorization::check(scope.as_deref()).await?;
            self.addresses.authorize_broadcast(&input.wallet)?;
            return self.send_bsv_saved(input, saved, checkpoint, scope).await;
        }
        let signer_input = input.signer_input()?;
        let lease = self
            .addresses
            .reserve(&input.wallet, &signer_input.clone().into(), transfer.value.to_string(), transfer.use_max_amount)
            .await?;
        let signing = async {
            authorization::check(scope.as_deref()).await?;
            let transactions = signer
                .sign(input.wallet.clone(), signer_input)
                .await
                .map_err(|error| error::sign_error(Chain::BitcoinSv, error))?;
            authorization::check(scope.as_deref()).await?;
            Ok::<_, BsvHomeConfirmError>(transactions)
        }
        .await;
        let transactions = match signing {
            Ok(transactions) => transactions,
            Err(error) => {
                if let Err(release) = self.addresses.release_unsigned(&input.wallet, &lease.id).await {
                    return Err(BsvHomeConfirmError::Record {
                        msg: format!("{error}; BSV reservation release failed: {release}"),
                    });
                }
                return Err(error);
            }
        };
        let [transaction] = transactions.as_slice() else {
            return Err(BsvHomeConfirmError::Record {
                msg: "BSV spend must produce exactly one signed transaction".into(),
            });
        };
        let saved = BsvHomeTransactionCheckpointData::from_signed(&input, transaction)?;
        self.addresses.validate_checkpoint(&input.wallet, &saved).await?;
        self.addresses.save_signed(&input.wallet, &lease.id, saved.clone()).await?;
        self.send_bsv_saved(input, saved, checkpoint, scope).await
    }

    async fn send_bsv_saved(
        &self,
        input: SendInput,
        saved: BsvHomeTransactionCheckpointData,
        checkpoint: Option<Arc<dyn BsvHomeTransactionCheckpoint>>,
        scope: Option<Arc<ExecutionScope>>,
    ) -> Result<BsvHomeExecuteResult, BsvHomeConfirmError> {
        authorization::check(scope.as_deref()).await?;
        self.addresses.require_payer_checkpoint(&input.wallet, &saved).await?;
        if let Some(checkpoint) = &checkpoint {
            checkpoint.save(saved.clone()).await.map_err(checkpoint::checkpoint_error)?;
        }
        let signed = vec![saved.signed_transaction()];
        authorization::check(scope.as_deref()).await?;
        match input.confirm.input.transfer.input_type.output().output_action {
            TransferDataOutputAction::Sign => Ok(BsvHomeExecuteResult::Signed { data: vec![saved.signed_data] }),
            TransferDataOutputAction::Send => {
                let transactions = self.record(&input, std::slice::from_ref(&saved.transaction_id), &signed).await?;
                if let Some(checkpoint) = checkpoint {
                    checkpoint.authorize_broadcast(saved.clone()).await.map_err(checkpoint::checkpoint_error)?;
                }
                self.addresses.require_payer_checkpoint(&input.wallet, &saved).await?;
                let hashes = self.broadcast(input.confirm.input.transfer.input_type.clone(), signed, scope).await?;
                if hashes != vec![saved.transaction_id] {
                    return Err(BsvHomeConfirmError::Broadcast {
                        hashes,
                        msg: "BSV broadcast transaction id mismatch".into(),
                    });
                }
                self.transaction_status.track(input.wallet.id, transactions.clone());
                Ok(BsvHomeExecuteResult::Sent { hashes, transactions })
            }
        }
    }

    pub async fn execute(&self, input: SendInput, signer: Arc<dyn BsvHomeTransactionSigner>) -> Result<BsvHomeExecuteResult, BsvHomeConfirmError> {
        self.execute_with_checkpoint(input, signer, None).await
    }

    pub(super) async fn execute_with_checkpoint(
        &self,
        input: SendInput,
        signer: Arc<dyn BsvHomeTransactionSigner>,
        checkpoint: Option<Arc<dyn BsvHomeTransactionCheckpoint>>,
    ) -> Result<BsvHomeExecuteResult, BsvHomeConfirmError> {
        self.execute_scoped(input, signer, checkpoint, None).await
    }

    async fn execute_scoped(
        &self,
        input: SendInput,
        signer: Arc<dyn BsvHomeTransactionSigner>,
        checkpoint: Option<Arc<dyn BsvHomeTransactionCheckpoint>>,
        scope: Option<Arc<ExecutionScope>>,
    ) -> Result<BsvHomeExecuteResult, BsvHomeConfirmError> {
        let chain = input.confirm.input.transfer.input_type.get_asset().chain();
        let scope = Some(scope.unwrap_or_else(|| Arc::new(ExecutionScope::unscoped(input.wallet.clone(), chain, self.cold_wallet_store.clone()))));
        authorization::check(scope.as_deref()).await?;
        self.require_input_assets(&input.confirm.input.transfer.input_type)?;
        self.gateway.require_asset(&input.confirm.fee.fee_asset)?;
        self.gateway.require_wallet_chain(&input.wallet, chain)?;
        if chain == Chain::BitcoinSv && input.wallet.wallet_type == primitives::WalletType::PrivateKey {
            return self.execute_bsv(input, signer, checkpoint, scope).await;
        }
        if let primitives::TransactionLoadMetadata::BitcoinSv { .. } = &input.confirm.metadata {
            return self.execute_bsv(input, signer, checkpoint, scope).await;
        }
        if let Some(checkpoint) = &checkpoint
            && let Some(saved) = checkpoint::saved_transaction(&input, checkpoint).await?
        {
            return self.send_checkpoint(input, saved, checkpoint.clone(), scope).await;
        }
        let signer_input = input.signer_input()?;
        authorization::check(scope.as_deref()).await?;
        let transactions = signer.sign(input.wallet.clone(), signer_input).await.map_err(|error| error::sign_error(chain, error))?;
        authorization::check(scope.as_deref()).await?;
        if transactions.is_empty() {
            return Err(BsvHomeConfirmError::Sign {
                error: BsvHomeSignerError::SigningError("no signed transactions".to_string()),
                chain,
                msg: "no signed transactions".to_string(),
            });
        }
        input.confirm.input.transfer.input_type.validate_approvals(&transactions)?;
        if let Some(checkpoint) = checkpoint {
            let [transaction] = transactions.as_slice() else {
                return Err(BsvHomeConfirmError::Record {
                    msg: "checkpoint payment must contain exactly one transaction".to_string(),
                });
            };
            let saved = BsvHomeTransactionCheckpointData::from_signed(&input, transaction)?;
            checkpoint.save(saved.clone()).await.map_err(checkpoint::checkpoint_error)?;
            return self.send_checkpoint(input, saved, checkpoint, scope).await;
        }
        match input.confirm.input.transfer.input_type.output().output_action {
            TransferDataOutputAction::Sign => Ok(BsvHomeExecuteResult::Signed {
                data: transactions.into_iter().map(|transaction| transaction.data).collect(),
            }),
            TransferDataOutputAction::Send => {
                let result = self.send(input, transactions, scope).await?;
                Ok(BsvHomeExecuteResult::Sent {
                    hashes: result.hashes,
                    transactions: result.transactions,
                })
            }
        }
    }
}

impl BsvHomeConfirmService {
    pub(super) fn require_input_assets(&self, input_type: &TransactionInputType) -> Result<(), BsvHomeConfirmError> {
        let asset_ids = rules::metadata_asset_ids(&input_type.get_asset().id, &input_type.fee_asset().id, input_type.asset_ids());
        for asset_id in asset_ids {
            self.gateway.require_asset(&asset_id)?;
        }
        if let TransactionInputType::TransferNft { nft_asset, .. } = input_type {
            self.gateway.require_asset(&AssetId::from_token(nft_asset.chain, &nft_asset.id.contract_address))?;
        }
        Ok(())
    }

    async fn ensure_simulation_assets(&self, asset_ids: Vec<AssetId>) -> Result<Vec<Asset>, crate::services::error::BsvHomeServiceError> {
        self.assets.ensure_simulation_assets(asset_ids).await
    }

    pub async fn fee_assets(&self, wallet_id: WalletId, chain: Chain) -> Result<Vec<BsvHomeFeeAsset>, BsvHomeConfirmError> {
        self.gateway.require_chain(chain)?;
        let fee_asset_ids = chain_fee_asset_ids(chain);
        if fee_asset_ids.is_empty() {
            return Ok(Vec::new());
        }
        let (assets, balances, prices) = futures::join!(
            self.assets.assets(fee_asset_ids.clone()),
            self.balance.balances(wallet_id, fee_asset_ids.clone()),
            self.price.prices(fee_asset_ids),
        );
        Ok(rules::selectable_fee_assets(assets?, balances?, prices?))
    }
    pub async fn preload(&self, wallet_id: WalletId, input: BsvHomeConfirmInput, options: BsvHomeConfirmLoadOptions) -> Result<BsvHomeConfirmFeeLoad, BsvHomeConfirmError> {
        let confirm_data = self.load(wallet_id.clone(), input, options).await?;
        let fee_asset_id = confirm_data.fee.fee_asset.clone();
        let metadata = self
            .input_metadata(wallet_id.clone(), &confirm_data.input.transfer.input_type, fee_asset_id.clone())
            .await?;
        let fee_asset = self
            .assets
            .assets(vec![fee_asset_id.clone()])
            .await?
            .into_iter()
            .next()
            .ok_or(BsvHomeConfirmError::BalanceMissing { asset_id: fee_asset_id.clone() })?;
        let amount = if confirm_data.input.transfer.input_type.get_asset().chain() == Chain::BitcoinSv {
            let wallet = self.addresses.wallet(&wallet_id).await?;
            match self
                .addresses
                .pending_input(
                    &wallet,
                    &confirm_data.input.transfer.recipient.address,
                    &confirm_data.input.transfer.value.to_string(),
                    confirm_data.input.transfer.use_max_amount,
                )
                .await?
            {
                Some(pending) => BsvHomeTransferAmountResult::Amount {
                    amount: crate::transfer_amount::BsvHomeTransferAmount {
                        value: pending.input.value.into(),
                        network_fee: pending.fee.fee,
                        is_max_amount: confirm_data.input.transfer.use_max_amount,
                    },
                },
                None => confirm_data.preload_amount(&metadata, &fee_asset)?,
            }
        } else {
            confirm_data.preload_amount(&metadata, &fee_asset)?
        };
        Ok(BsvHomeConfirmFeeLoad {
            fee_asset,
            metadata,
            preload: BsvHomeConfirmPreload { confirm_data, amount },
        })
    }
}

impl BsvHomeConfirmService {
    async fn send_checkpoint(
        &self,
        input: SendInput,
        saved: BsvHomeTransactionCheckpointData,
        checkpoint: Arc<dyn BsvHomeTransactionCheckpoint>,
        scope: Option<Arc<ExecutionScope>>,
    ) -> Result<BsvHomeExecuteResult, BsvHomeConfirmError> {
        authorization::check(scope.as_deref()).await?;
        self.addresses.validate_checkpoint(&input.wallet, &saved).await?;
        let signed = vec![saved.signed_transaction()];
        let transactions = self.record(&input, std::slice::from_ref(&saved.transaction_id), &signed).await?;
        checkpoint.authorize_broadcast(saved.clone()).await.map_err(checkpoint::checkpoint_error)?;
        let hashes = self.broadcast(input.confirm.input.transfer.input_type.clone(), signed, scope).await?;
        if hashes != vec![saved.transaction_id] {
            return Err(BsvHomeConfirmError::Broadcast {
                hashes,
                msg: "payment broadcast transaction id mismatch".to_string(),
            });
        }
        self.transaction_status.track(input.wallet.id, transactions.clone());
        Ok(BsvHomeExecuteResult::Sent { hashes, transactions })
    }

    async fn broadcast_saved_transaction(
        &self,
        wallet: Wallet,
        transfer: BsvHomeTransferData,
        saved: BsvHomeTransactionCheckpointData,
        checkpoint: Arc<dyn BsvHomeTransactionCheckpoint>,
    ) -> Result<String, BsvHomeConfirmError> {
        let scope = Arc::new(ExecutionScope::unscoped(
            wallet.clone(),
            transfer.input_type.get_asset().chain(),
            self.cold_wallet_store.clone(),
        ));
        scope.check().await?;
        self.require_input_assets(&transfer.input_type)?;
        self.gateway.require_asset(&saved.fee_asset_id)?;
        self.gateway.require_wallet_chain(&wallet, transfer.input_type.get_asset().chain())?;
        saved.validate_transfer(&wallet, &transfer)?;
        self.addresses.validate_checkpoint(&wallet, &saved).await?;
        checkpoint.authorize_broadcast(saved.clone()).await.map_err(checkpoint::checkpoint_error)?;
        let hashes = self.broadcast(transfer.input_type, vec![saved.signed_transaction()], Some(scope)).await?;
        if hashes != vec![saved.transaction_id.clone()] {
            return Err(BsvHomeConfirmError::Broadcast {
                hashes,
                msg: "saved payment broadcast transaction id mismatch".to_string(),
            });
        }
        Ok(saved.transaction_id)
    }

    async fn send(&self, input: SendInput, signed: Vec<BsvHomeSignedTransaction>, scope: Option<Arc<ExecutionScope>>) -> Result<BsvHomeSendResult, BsvHomeConfirmError> {
        match self.broadcast(input.confirm.input.transfer.input_type.clone(), signed.clone(), scope).await {
            Ok(hashes) => {
                let transactions = self.store_pending(&input, &hashes, &signed).await;
                Ok(BsvHomeSendResult { hashes, transactions })
            }
            Err(BsvHomeConfirmError::Broadcast { hashes, msg }) => {
                self.store_pending(&input, &hashes, &signed).await;
                Err(BsvHomeConfirmError::Broadcast { hashes, msg })
            }
            Err(error) => Err(error),
        }
    }

    async fn store_pending(&self, input: &SendInput, hashes: &[String], signed: &[BsvHomeSignedTransaction]) -> Vec<Transaction> {
        let stored = self.record(input, hashes, signed).await.unwrap_or_default();
        self.transaction_status.track(input.wallet.id.clone(), stored.clone());
        stored
    }

    async fn broadcast(
        &self,
        input_type: TransactionInputType,
        transactions: Vec<BsvHomeSignedTransaction>,
        scope: Option<Arc<ExecutionScope>>,
    ) -> Result<Vec<String>, BsvHomeConfirmError> {
        let chain = input_type.get_asset().id.chain;
        let options = input_type.broadcast_options();
        let delay = rules::broadcast_delay_milliseconds(chain);
        let mut hashes: Vec<String> = Vec::with_capacity(transactions.len());

        for (index, transaction) in transactions.iter().enumerate() {
            authorization::check(scope.as_deref()).await?;
            let hash = if chain == Chain::BitcoinSv {
                self.broadcast_bsv(transaction.data.clone(), scope.as_deref(), &hashes).await?
            } else {
                self.gateway
                    .transaction_broadcast(chain, transaction.data.clone(), options.clone())
                    .await
                    .map_err(|error| error::broadcast_error(hashes.clone(), error))?
            };
            hashes.push(hash);
            if index < transactions.len() - 1 && delay > 0 {
                sleep(Duration::from_millis(delay)).await;
            }
        }

        Ok(hashes)
    }

    async fn broadcast_bsv(&self, data: String, scope: Option<&ExecutionScope>, hashes: &[String]) -> Result<String, BsvHomeConfirmError> {
        let submission = Mutex::new(BsvBroadcastSubmission::Ready);
        let check = || {
            if let Some(scope) = scope {
                scope.check_signing().map_err(|error| Box::new(error) as ProviderError)?;
            }
            Ok(())
        };
        let before_send = || async { authorization::check(scope).await.map_err(|error| Box::new(error) as ProviderError) };
        let before_submission = || {
            check()?;
            *submission.lock().map_err(|_| BsvHomeConfirmError::Record {
                msg: "BSV broadcast state is unavailable".into(),
            })? = BsvBroadcastSubmission::Unknown;
            Ok(())
        };
        match self.gateway.bsv_transaction_broadcast_checked(data, &check, before_send, &before_submission).await {
            Ok(hash) => Ok(hash),
            Err(error) => {
                let submission = *submission.lock().map_err(|_| BsvHomeConfirmError::Record {
                    msg: "BSV broadcast state is unavailable".into(),
                })?;
                match submission {
                    BsvBroadcastSubmission::Ready => match error.downcast::<BsvHomeConfirmError>() {
                        Ok(error) => Err(*error),
                        Err(error) => Err(error::broadcast_error(hashes.to_vec(), map_network_error(error))),
                    },
                    BsvBroadcastSubmission::Unknown => Err(BsvHomeConfirmError::Broadcast {
                        hashes: hashes.to_vec(),
                        msg: "BSV broadcast result is uncertain; the saved payment remains available for recovery".into(),
                    }),
                }
            }
        }
    }

    async fn record(&self, input: &SendInput, hashes: &[String], signed: &[BsvHomeSignedTransaction]) -> Result<Vec<Transaction>, BsvHomeConfirmError> {
        let pending = input.pending_transactions(hashes, signed)?;
        if pending.is_empty() {
            return Ok(pending);
        }
        self.transaction_state
            .add_transactions(input.wallet.id.clone(), pending.clone())
            .await
            .map_err(|error| BsvHomeConfirmError::Record { msg: error.to_string() })?;
        Ok(pending)
    }

    async fn simulate(&self, chain: Chain, input: &BsvHomeConfirmInput) -> Result<Option<SimulationResult>, BsvHomeConfirmError> {
        let Some(transaction) = input.transfer.input_type.simulation_payload() else {
            return Ok(None);
        };
        let simulation = self
            .simulation
            .simulate_transaction(chain, transaction, Some(input.from.address.clone()))
            .await
            .map_err(|error| BsvHomeConfirmError::Load { msg: error.to_string() })?;
        input.transfer.input_type.validate_simulation(&simulation)?;
        Ok(Some(simulation))
    }
}

impl BsvHomeConfirmService {
    async fn input_metadata(&self, wallet_id: WalletId, input_type: &TransactionInputType, fee_asset_id: AssetId) -> Result<BsvHomeConfirmMetadata, BsvHomeConfirmError> {
        self.metadata(wallet_id, input_type.transaction_asset().id, fee_asset_id, input_type.asset_ids()).await
    }
}
