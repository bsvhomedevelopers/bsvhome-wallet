use std::collections::HashSet;

use bsv_home_bsv::BsvAddress;
use bsv_home_hash::sha2::sha256;
use num_bigint::BigUint;
use primitives::{
    Address, Asset, AssetBalance, Balance, BsvAddressBranch, BsvAddressLocator, BsvSpendChange, BsvSpendInput, BsvSpendMetadata, Chain, SignerInput, Transaction, TransactionFee,
    TransactionIdRequest, TransactionInputType, TransactionLoadInput, TransactionLoadMetadata, TransactionState, UTXO, Wallet, WalletId,
};

use super::ledger::{PayerState, outpoint};
use super::single::validate_single_checkpoint;
use super::state::{SpendReservation, pool_error, validate_reserved_checkpoint};
use super::{BsvHomeBsvAddressPool, BsvHomeBsvAddressStatus, BsvHomeBsvDiscoveryState, has_hd_account, map_provider_error};
use crate::services::bsv_multisig::BsvMultisigState;
use crate::services::confirm::BsvHomeTransactionCheckpointData;
use crate::services::error::BsvHomeServiceError;

impl BsvHomeBsvAddressPool {
    pub(super) async fn validated_funding_archives(&self) -> Result<Vec<crate::services::bsv_multisig::PayerFundingCheckpoint>, BsvHomeServiceError> {
        let records = self.store.funding_states().await?;
        let mut identifiers = HashSet::new();
        let mut checkpoints = Vec::new();
        for record in records {
            if record.revision == 0
                || record.identifier.len() != 64
                || !record.identifier.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                || !identifiers.insert(record.identifier.clone())
            {
                return Err(pool_error("Invalid BSV funding storage enumeration"));
            }
            let state: BsvMultisigState = serde_json::from_str(&record.json).map_err(pool_error)?;
            for (key, checkpoint) in state.funding {
                let scope = checkpoint.scope();
                let parts: Vec<_> = scope.split('|').collect();
                if parts.len() < 2 || key != scope {
                    return Err(pool_error("Invalid BSV funding archive scope"));
                }
                if hex::encode(sha256(format!("{}|{}", parts[0], parts[1]).as_bytes())) != record.identifier {
                    return Err(pool_error("BSV funding file does not match its original signer scope"));
                }
                checkpoints.push(checkpoint.validated_checkpoint().map_err(pool_error)?);
            }
        }
        Ok(checkpoints)
    }

    pub(super) async fn verify_payer_pending(&self, wallet: &Wallet, state: &PayerState) -> Result<(), BsvHomeServiceError> {
        let known: HashSet<_> = state
            .reservations()
            .iter()
            .filter_map(|record| record.checkpoint.as_ref().map(|saved| saved.transaction_id.clone()))
            .chain(state.funding().iter().filter_map(|record| record.checkpoint().map(|saved| saved.transaction_id)))
            .collect();
        for pending in self.store.unresolved_transactions(wallet.id.clone()).await? {
            if known.contains(pending.hash()) {
                continue;
            }
            let transaction = self
                .gateway
                .transaction_by_hash(TransactionIdRequest::new(Chain::BitcoinSv, pending.hash().to_string(), None))
                .await?;
            if !transaction.is_some_and(|transaction| transaction.hash() == pending.hash() && transaction.state == TransactionState::Confirmed) {
                return Err(pool_error("BSV previous payment requires recovery before new spending"));
            }
        }
        Ok(())
    }

    pub(crate) async fn preload_payer_spend(&self, wallet_id: WalletId, sender: &str, is_max: bool) -> Result<BsvSpendMetadata, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if has_hd_account(&wallet) && self.sync(wallet_id.clone()).await?.discovery != BsvHomeBsvDiscoveryState::Complete {
            return Err(pool_error("BSV automatic discovery is incomplete; continue scanning"));
        }
        self.reconcile_funding(&wallet).await?;
        let state = self.payer_state(&wallet).await?;
        require_complete(state.as_ref().map(|(_, state)| state))?;
        if has_hd_account(&wallet) && state.is_none() {
            return Err(pool_error("BSV HD payer ledger is not initialized"));
        }
        let account = wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?;
        if account.address != sender {
            return Err(pool_error("BSV sender does not match stable wallet identity"));
        }
        if let Some((_, state)) = &state {
            self.verify_payer_pending(&wallet, state).await?;
        }
        let addresses = payer_addresses(&wallet, state.as_ref().map(|(_, state)| state))?;
        let reserved = state
            .as_ref()
            .into_iter()
            .flat_map(|(_, state)| state.unconfirmed_inputs())
            .map(outpoint)
            .collect::<Result<HashSet<_>, _>>()?;
        let mut inputs = Vec::new();
        for (address, output) in self.gateway.bsv_client().wallet_native_unspent(&addresses).await.map_err(map_provider_error)? {
            let locator = match state.as_ref().map(|(_, state)| state) {
                Some(PayerState::Hd(state)) => state
                    .records
                    .iter()
                    .find(|record| record.address == address)
                    .map(|record| record.locator)
                    .ok_or_else(|| pool_error("BSV output owner is not in the payer ledger"))?,
                Some(PayerState::SingleAddress(_)) | None => {
                    if address != account.address {
                        return Err(pool_error("BSV single-address output owner differs from payer"));
                    }
                    BsvAddressLocator {
                        branch: BsvAddressBranch::Receive,
                        index: 0,
                    }
                }
            };
            let input = BsvSpendInput {
                transaction_id: output.tx_hash,
                vout: output.tx_pos,
                value: output.value,
                script_pubkey: hex::encode(BsvAddress::from_str(&address).map_err(pool_error)?.script_pubkey().as_bytes()),
                address,
                locator,
            };
            match output.status.as_str() {
                "unconfirmed" => continue,
                "confirmed" if output.height.is_some_and(|height| height > 0) => {}
                _ => return Err(pool_error("Invalid BSV payer UTXO confirmation status")),
            }
            if reserved.contains(&outpoint(&input)?) {
                continue;
            }
            inputs.push(input);
        }
        let change = match state.as_ref().map(|(_, state)| state) {
            Some(PayerState::Hd(state)) if !is_max => state
                .next_change()
                .map(|locator| {
                    let address = state.address(locator)?;
                    let script_pubkey = hex::encode(BsvAddress::from_str(&address).map_err(pool_error)?.script_pubkey().as_bytes());
                    Ok::<_, BsvHomeServiceError>(BsvSpendChange { address, script_pubkey, locator })
                })
                .transpose()?,
            Some(PayerState::Hd(_)) | Some(PayerState::SingleAddress(_)) | None => None,
        };
        self.check_revision(&wallet.id, state.as_ref().map(|(revision, _)| *revision)).await?;
        self.check_payer_wallet(&wallet).await?;
        Ok(BsvSpendMetadata { inputs, change })
    }

    pub(crate) async fn preload_payer_metadata(&self, wallet_id: WalletId, sender: &str, is_max: bool) -> Result<Option<TransactionLoadMetadata>, BsvHomeServiceError> {
        let wallet = self.wallet(&wallet_id).await?;
        if has_hd_account(&wallet) {
            return self
                .preload_payer_spend(wallet_id, sender, is_max)
                .await
                .map(|spend| Some(TransactionLoadMetadata::BitcoinSv { spend }));
        }
        if wallet.wallet_type != primitives::WalletType::PrivateKey {
            return Ok(None);
        }
        let spend = self.preload_payer_spend(wallet_id, sender, is_max).await?;
        Ok(Some(TransactionLoadMetadata::Bitcoin { utxos: single_utxos(&spend)? }))
    }

    pub(super) async fn check_payer_wallet(&self, expected: &Wallet) -> Result<(), BsvHomeServiceError> {
        let current = self.wallet(&expected.id).await?;
        validate_payer_identity(expected, &current)
    }

    pub(super) async fn check_payer_cleanup(&self, expected: &Wallet) -> Result<(), BsvHomeServiceError> {
        if let Some(current) = self.wallets.get_wallet(expected.id.clone()).await? {
            validate_payer_identity(expected, &current)?;
        }
        Ok(())
    }

    pub(super) async fn save_payer_transfer(&self, wallet: &Wallet, id: &str, saved: BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeServiceError> {
        let _mutation = self.mutation.lock().await;
        let (revision, mut state) = self.payer_state(wallet).await?.ok_or_else(|| pool_error("BSV payer ledger missing"))?;
        validate_checkpoint(&state, wallet, &saved)?;
        let reservation = state
            .reservations_mut()
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or_else(|| pool_error("BSV spend reservation missing"))?;
        if !reservation.purpose.is_transfer() {
            return Err(pool_error("BSV funding requires its own typed signed checkpoint"));
        }
        validate_reserved_checkpoint(reservation, &saved)?;
        if reservation.checkpoint.as_ref().is_some_and(|previous| previous != &saved) {
            return Err(pool_error("BSV signed transaction cannot be replaced"));
        }
        reservation.checkpoint = Some(saved);
        let change = reservation.spend.change.as_ref().map(|change| change.locator);
        if let PayerState::Hd(state) = &mut state
            && let Some(locator) = change
            && let Some(record) = state.records.iter_mut().find(|record| record.locator == locator)
        {
            record.status = BsvHomeBsvAddressStatus::Used;
        }
        self.check_payer_wallet(wallet).await?;
        self.save_payer_state(wallet, Some(revision), &state).await
    }

    pub(crate) async fn require_payer_checkpoint(&self, wallet: &Wallet, saved: &BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeServiceError> {
        self.reconcile_funding(wallet).await?;
        let (revision, state) = self.payer_state(wallet).await?.ok_or_else(|| pool_error("BSV durable payer checkpoint missing"))?;
        validate_checkpoint(&state, wallet, saved)?;
        if !state
            .reservations()
            .iter()
            .any(|record| record.purpose.is_transfer() && record.checkpoint.as_ref() == Some(saved))
        {
            return Err(pool_error("BSV signed transaction is not durably recorded by the payer"));
        }
        self.verify_payer_pending(wallet, &state).await?;
        self.check_revision(&wallet.id, Some(revision)).await?;
        self.check_payer_wallet(wallet).await
    }

    pub(super) async fn payer_saved_transfer(
        &self,
        wallet: &Wallet,
        recipient: &str,
        original_value: &str,
        is_max: bool,
    ) -> Result<Option<BsvHomeTransactionCheckpointData>, BsvHomeServiceError> {
        let Some((_, state)) = self.payer_state(wallet).await? else {
            return Ok(None);
        };
        Ok(matching_transfer(state.reservations(), recipient, original_value, is_max)?.and_then(|record| record.checkpoint.clone()))
    }

    pub(super) async fn payer_pending_input(&self, wallet: &Wallet, recipient: &str, original_value: &str, is_max: bool) -> Result<Option<SignerInput>, BsvHomeServiceError> {
        let Some((_, state)) = self.payer_state(wallet).await? else {
            return Ok(None);
        };
        let Some(record) = matching_transfer(state.reservations(), recipient, original_value, is_max)? else {
            return Ok(None);
        };
        if let Some(saved) = &record.checkpoint {
            validate_checkpoint(&state, wallet, saved)?;
            return saved.signer_input().map(Some).map_err(pool_error);
        }
        let metadata = match &state {
            PayerState::Hd(_) => TransactionLoadMetadata::BitcoinSv { spend: record.spend.clone() },
            PayerState::SingleAddress(_) => TransactionLoadMetadata::Bitcoin {
                utxos: single_utxos(&record.spend)?,
            },
        };
        Ok(Some(SignerInput::new(
            TransactionLoadInput {
                input_type: TransactionInputType::Transfer {
                    asset: Asset::from_chain(Chain::BitcoinSv),
                },
                sender_address: wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?.address.clone(),
                destination_address: recipient.to_string(),
                value: record.value.into(),
                gas_price: record.gas_price.clone(),
                memo: None,
                is_max_value: is_max,
                metadata,
            },
            TransactionFee {
                fee: record.fee.into(),
                gas_price_type: record.gas_price.clone(),
                gas_limit: 1.into(),
                options: Default::default(),
                fee_asset: Chain::BitcoinSv.as_asset_id(),
            },
        )))
    }

    pub(super) async fn refresh_payer_reservations(&self, wallet: &Wallet) -> Result<(), BsvHomeServiceError> {
        let Some((revision, mut state)) = self.payer_state(wallet).await? else {
            return Ok(());
        };
        let mut changed = false;
        for reservation in state.reservations_mut() {
            if let Some(saved) = &reservation.checkpoint {
                let transaction = self
                    .gateway
                    .transaction_by_hash(TransactionIdRequest::new(Chain::BitcoinSv, saved.transaction_id.clone(), None))
                    .await?;
                let confirmed = transaction.is_some_and(|transaction| transaction.hash() == saved.transaction_id && transaction.state == TransactionState::Confirmed);
                if reservation.confirmed != confirmed {
                    reservation.confirmed = confirmed;
                    changed = true;
                }
            }
        }
        changed |= self.refresh_funding_confirmations(state.funding_mut()).await?;
        if changed {
            let _mutation = self.mutation.lock().await;
            self.check_payer_wallet(wallet).await?;
            self.save_payer_state(wallet, Some(revision), &state).await?;
        }
        Ok(())
    }

    pub(super) async fn single_wallet_balance(&self, wallet: &Wallet, receipt_hash: Option<&str>) -> Result<(AssetBalance, bool), BsvHomeServiceError> {
        let state = self.payer_state(wallet).await?;
        let reserved = state
            .as_ref()
            .into_iter()
            .flat_map(|(_, state)| state.unconfirmed_inputs())
            .map(outpoint)
            .collect::<Result<HashSet<_>, _>>()?;
        let root = wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?.address.clone();
        let mut available = BigUint::ZERO;
        let mut pending = BigUint::ZERO;
        let mut reserved_value = BigUint::ZERO;
        let mut indexed = receipt_hash.is_none();
        for (address, output) in self
            .gateway
            .bsv_client()
            .wallet_native_unspent(std::slice::from_ref(&root))
            .await
            .map_err(map_provider_error)?
        {
            if address != root {
                return Err(pool_error("BSV single-address balance owner differs from payer"));
            }
            indexed |= receipt_hash == Some(output.tx_hash.as_str());
            let key = (output.tx_hash.parse::<bitcoin::Txid>().map_err(pool_error)?, output.tx_pos);
            if reserved.contains(&key) {
                reserved_value += output.value;
                continue;
            }
            match output.status.as_str() {
                "confirmed" if output.height.is_some_and(|height| height > 0) => available += output.value,
                "unconfirmed" => pending += output.value,
                _ => return Err(pool_error("invalid BSV UTXO confirmation status")),
            }
        }
        self.check_revision(&wallet.id, state.as_ref().map(|(revision, _)| *revision)).await?;
        self.check_payer_wallet(wallet).await?;
        let mut balance = Balance::with_pending_unconfirmed(available, pending);
        balance.reserved = reserved_value;
        Ok((AssetBalance::new_balance(Chain::BitcoinSv.as_asset_id(), balance), indexed))
    }

    pub(super) async fn single_transactions(&self, wallet: &Wallet, limit: u32) -> Result<Vec<Transaction>, BsvHomeServiceError> {
        let state = self.payer_state(wallet).await?;
        let addresses = payer_addresses(wallet, state.as_ref().map(|(_, state)| state))?;
        let mut transactions = self
            .gateway
            .bsv_client()
            .wallet_transactions(&addresses, limit as usize)
            .await
            .map_err(map_provider_error)?;
        if let Some((_, state)) = &state {
            for transaction in &mut transactions {
                if transaction.direction == primitives::TransactionDirection::SelfTransfer
                    && let Some(reservation) = state
                        .reservations()
                        .iter()
                        .find(|record| record.checkpoint.as_ref().is_some_and(|saved| saved.transaction_id == transaction.hash()))
                {
                    let saved = reservation.checkpoint.as_ref().ok_or_else(|| pool_error("BSV checkpoint missing"))?;
                    transaction.to = saved.recipient_address.clone();
                    transaction.value = reservation.value.into();
                }
            }
        }
        self.check_revision(&wallet.id, state.as_ref().map(|(revision, _)| *revision)).await?;
        self.check_payer_wallet(wallet).await?;
        Ok(transactions)
    }
}

pub(super) fn validate_checkpoint(state: &PayerState, wallet: &Wallet, saved: &BsvHomeTransactionCheckpointData) -> Result<(), BsvHomeServiceError> {
    match state {
        PayerState::Hd(state) => state.validate_checkpoint(saved),
        PayerState::SingleAddress(_) => validate_single_checkpoint(wallet, saved),
    }
}

fn matching_transfer<'a>(records: &'a [SpendReservation], recipient: &str, original_value: &str, is_max: bool) -> Result<Option<&'a SpendReservation>, BsvHomeServiceError> {
    let matching: Vec<_> = records
        .iter()
        .filter(|record| !record.confirmed && record.purpose.is_transfer() && record.recipient == recipient && record.original_value == original_value && record.is_max == is_max)
        .collect();
    match matching.as_slice() {
        [] => Ok(None),
        [record] => Ok(Some(*record)),
        _ => Err(pool_error("multiple BSV unfinished payments match this transfer")),
    }
}

fn require_complete(state: Option<&PayerState>) -> Result<(), BsvHomeServiceError> {
    if let Some(PayerState::Hd(state)) = state
        && state.discovery != BsvHomeBsvDiscoveryState::Complete
    {
        return Err(pool_error("BSV automatic discovery is incomplete; continue scanning"));
    }
    Ok(())
}

pub(super) fn payer_addresses(wallet: &Wallet, state: Option<&PayerState>) -> Result<Vec<String>, BsvHomeServiceError> {
    match state {
        Some(PayerState::Hd(state)) => Ok(state.records.iter().map(|record| record.address.clone()).collect()),
        Some(PayerState::SingleAddress(_)) | None => Ok(vec![
            wallet.account(Chain::BitcoinSv).ok_or_else(|| pool_error("wallet has no BSV account"))?.address.clone(),
        ]),
    }
}

fn single_utxos(spend: &BsvSpendMetadata) -> Result<Vec<UTXO>, BsvHomeServiceError> {
    spend
        .inputs
        .iter()
        .map(|source| {
            Ok(UTXO {
                transaction_id: source.transaction_id.clone(),
                vout: i32::try_from(source.vout).map_err(pool_error)?,
                value: source.value.into(),
                address: source.address.clone(),
            })
        })
        .collect()
}

fn validate_payer_identity(expected: &Wallet, current: &Wallet) -> Result<(), BsvHomeServiceError> {
    if current.id != expected.id || current.wallet_type != expected.wallet_type || current.account(Chain::BitcoinSv) != expected.account(Chain::BitcoinSv) {
        return Err(pool_error("BSV payer ownership changed during operation"));
    }
    Ok(())
}
