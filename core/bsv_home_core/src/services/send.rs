use bigdecimal::num_traits::ToPrimitive;
use futures::future::{AbortHandle, Abortable};
use futures::lock::Mutex as AsyncMutex;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use number_formatter::BigNumberFormatter;
use primitives::{Asset, AssetId, Chain, Contact, ContactAddress, Currency, TransactionInputType, Wallet, WalletId};

use crate::models::custom_types::BsvHomeBigInt;
use crate::payment::BsvHomePaymentRecipient;
use crate::services::amount::{BsvHomeAmountService, BsvHomeAmountTransfer};
use crate::services::bsv_multisig::{BsvHomeBsvMultisigPaymentRequest, bsv_multisig_payment_request_from_script};
use crate::services::error::BsvHomeServiceError;
use crate::services::name::BsvHomeNameRecordState;
use crate::services::portal::{
    BsvHomePaymailDestinationService, BsvHomePaymailDestinationSnapshot, BsvHomePaymailError, BsvHomePaymailOutput, BsvHomePortalAuthError, MAX_BSV_PAYMENT_SATOSHIS,
};
use crate::services::recipient::{BsvHomeRecipientError, BsvHomeRecipientScan, BsvHomeRecipientService, BsvHomeRecipientType};
use crate::services::transfer::{BsvHomeRecipient, BsvHomeTransferData};

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSendContactRecipient {
    pub id: String,
    pub contact_id: String,
    pub recipient: BsvHomeRecipient,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BsvHomeSendWalletRecipient {
    pub wallet_id: WalletId,
    pub recipient: BsvHomeRecipient,
}

#[derive(Clone, uniffi::Enum)]
#[allow(clippy::large_enum_variant)]
pub enum BsvHomeSendDestination {
    Transfer { transfer: BsvHomeTransferData },
    PublicFunding { preparation: Arc<BsvHomeSendPublicFundingPreparation> },
}

#[derive(uniffi::Object)]
pub struct BsvHomeSendPublicFundingPreparation {
    id: Uuid,
    owner: Uuid,
    request: BsvHomeBsvMultisigPaymentRequest,
    snapshot: BsvHomePaymailDestinationSnapshot,
    cancelled: AtomicBool,
    abort: Mutex<Option<AbortHandle>>,
    confirmation: AsyncMutex<()>,
}

#[uniffi::export]
impl BsvHomeSendPublicFundingPreparation {
    pub fn id(&self) -> String {
        self.id.to_string()
    }
    pub fn payment_request(&self) -> BsvHomeBsvMultisigPaymentRequest {
        self.request.clone()
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Ok(abort) = self.abort.lock()
            && let Some(abort) = &*abort
        {
            abort.abort();
        }
    }
}

impl BsvHomeSendPublicFundingPreparation {
    fn check(&self) -> Result<(), BsvHomePortalAuthError> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(BsvHomeServiceError::Cancelled.into());
        }
        Ok(())
    }
    fn check_owner(&self, owner: Uuid) -> Result<(), BsvHomePaymailError> {
        self.check()?;
        if self.owner != owner {
            return Err(BsvHomePaymailError::BindingChanged);
        }
        Ok(())
    }
    async fn run<T>(&self, future: impl Future<Output = Result<T, BsvHomePaymailError>>) -> Result<T, BsvHomePaymailError> {
        self.check()?;
        let _confirmation = self.confirmation.lock().await;
        self.check()?;
        let (abort, registration) = AbortHandle::new_pair();
        {
            let mut stored = self.abort.lock().map_err(|_| preparation_lock_error())?;
            self.check()?;
            *stored = Some(abort);
        }
        let result = Abortable::new(future, registration)
            .await
            .map_err(|_| BsvHomePaymailError::from(BsvHomeServiceError::Cancelled))
            .and_then(|result| result);
        *self.abort.lock().map_err(|_| preparation_lock_error())? = None;
        result
    }
}

fn preparation_lock_error() -> BsvHomePaymailError {
    BsvHomeServiceError::Store {
        msg: "public Paymail preparation lock failed".into(),
    }
    .into()
}

#[derive(uniffi::Object)]
pub struct BsvHomeSendService {
    recipient: Arc<BsvHomeRecipientService>,
    amount: Arc<BsvHomeAmountService>,
    paymail: Arc<BsvHomePaymailDestinationService>,
    owner: Uuid,
}

#[uniffi::export]
impl BsvHomeSendService {
    #[uniffi::constructor]
    pub fn new(recipient: Arc<BsvHomeRecipientService>, amount: Arc<BsvHomeAmountService>, paymail: Arc<BsvHomePaymailDestinationService>) -> Self {
        Self {
            recipient,
            amount,
            paymail,
            owner: Uuid::new_v4(),
        }
    }

    pub fn get_currency(&self) -> Currency {
        self.amount.get_currency()
    }

    pub fn is_paymail_input(&self, input: String) -> bool {
        is_paymail_input(&input)
    }

    pub fn supports_paymail(&self, asset: Asset) -> bool {
        supports_paymail(&asset)
    }

    pub fn contact_recipients(&self, asset: Asset, contacts: Vec<Contact>, addresses: Vec<ContactAddress>) -> Result<Vec<BsvHomeSendContactRecipient>, BsvHomeRecipientError> {
        self.amount.require_asset(&asset.id).map_err(|_| BsvHomeRecipientError::InvalidAddress)?;
        let mut recipients = Vec::new();
        for contact in contacts {
            for address in addresses
                .iter()
                .filter(|address| address.contact_id == contact.id && address.chain == asset.chain() && address.memo.is_none())
            {
                if let Ok(recipient) = self.recipient(asset.clone(), address.address.clone(), BsvHomeNameRecordState::None, Some(contact.name.clone())) {
                    recipients.push(BsvHomeSendContactRecipient {
                        id: address.id.clone(),
                        contact_id: contact.id.clone(),
                        recipient,
                    });
                }
            }
        }
        Ok(recipients)
    }

    pub fn wallet_recipients(&self, asset: Asset, wallets: Vec<Wallet>, source_wallet_id: WalletId) -> Result<Vec<BsvHomeSendWalletRecipient>, BsvHomeRecipientError> {
        self.amount.require_asset(&asset.id).map_err(|_| BsvHomeRecipientError::InvalidAddress)?;
        wallets
            .into_iter()
            .filter(|wallet| wallet.id != source_wallet_id)
            .filter_map(|wallet| {
                wallet.account(asset.chain()).map(|account| {
                    self.recipient(asset.clone(), account.address.clone(), BsvHomeNameRecordState::None, Some(wallet.name.clone()))
                        .map(|recipient| BsvHomeSendWalletRecipient {
                            wallet_id: wallet.id.clone(),
                            recipient,
                        })
                })
            })
            .collect()
    }

    pub fn recipient(&self, asset: Asset, input: String, state: BsvHomeNameRecordState, name: Option<String>) -> Result<BsvHomeRecipient, BsvHomeRecipientError> {
        self.amount.require_asset(&asset.id).map_err(|_| BsvHomeRecipientError::InvalidAddress)?;
        let mut recipient = self.recipient.recipient(asset.chain(), input, state, None, Vec::new())?;
        if let Some(name) = name {
            recipient.name = Some(name);
        }
        Ok(recipient)
    }

    pub fn scan(&self, asset: Asset, input: String) -> Result<BsvHomePaymentRecipient, BsvHomeRecipientError> {
        self.amount.require_asset(&asset.id).map_err(|_| BsvHomeRecipientError::InvalidAddress)?;
        if is_paymail_input(&input) && !input.contains(':') {
            paymail_input(&asset, &input).map_err(|_| BsvHomeRecipientError::InvalidAddress)?;
            return Ok(BsvHomePaymentRecipient {
                recipient: BsvHomeRecipient::address(input),
                amount: None,
            });
        }
        let payment = match self.recipient.scan(input, BsvHomeRecipientType::Asset { asset: asset.clone() })? {
            BsvHomeRecipientScan::Recipient { payment } => payment,
            BsvHomeRecipientScan::Confirm { transfer } => scanned_transfer(&asset, transfer)?,
        };
        if payment.recipient.memo.is_some() || !payment.recipient.references.is_empty() {
            return Err(BsvHomeRecipientError::InvalidAddress);
        }
        Ok(BsvHomePaymentRecipient {
            recipient: self.recipient(asset, payment.recipient.address, BsvHomeNameRecordState::None, payment.recipient.name)?,
            amount: payment.amount,
        })
    }

    pub async fn resolve_destination(
        &self,
        asset: Asset,
        input: String,
        state: BsvHomeNameRecordState,
        name: Option<String>,
        value: BsvHomeBigInt,
        use_max_amount: bool,
    ) -> Result<BsvHomeSendDestination, BsvHomePaymailError> {
        self.amount.require_asset(&asset.id).map_err(BsvHomePaymailError::from)?;
        if value <= BsvHomeBigInt::from(0) {
            return Err(BsvHomePaymailError::InvalidAmount);
        }
        if !paymail_input(&asset, &input)? {
            return Ok(BsvHomeSendDestination::Transfer {
                transfer: self.transfer_data(asset, input, state, name, value, use_max_amount).await?,
            });
        }
        let amount = exact_bsv_satoshis(&value)?;
        let snapshot = self.paymail.prepare_for_send(input).await?;
        match &snapshot.result.output {
            BsvHomePaymailOutput::P2pkh { address, .. } => {
                let recipient = BsvHomeRecipient::named(address.clone(), snapshot.result.handle.clone());
                Ok(BsvHomeSendDestination::Transfer {
                    transfer: self.transfer_to_recipient(asset, recipient, value, use_max_amount).await?,
                })
            }
            BsvHomePaymailOutput::P2ms { output_script, .. } => {
                let amount_bsv = BigNumberFormatter::big_decimal_value(&amount.to_string(), 8)
                    .map_err(|_| BsvHomePaymailError::InvalidAmount)?
                    .normalized()
                    .to_plain_string();
                let request = bsv_multisig_payment_request_from_script(snapshot.result.handle.clone(), output_script.clone(), amount_bsv)
                    .map_err(|_| BsvHomePaymailError::UnsupportedOutput)?;
                Ok(BsvHomeSendDestination::PublicFunding {
                    preparation: Arc::new(BsvHomeSendPublicFundingPreparation {
                        id: Uuid::new_v4(),
                        owner: self.owner,
                        request,
                        snapshot,
                        cancelled: AtomicBool::new(false),
                        abort: Mutex::new(None),
                        confirmation: AsyncMutex::new(()),
                    }),
                })
            }
        }
    }

    pub async fn confirm_public_funding(
        &self,
        preparation: Arc<BsvHomeSendPublicFundingPreparation>,
        value: BsvHomeBigInt,
    ) -> Result<BsvHomeBsvMultisigPaymentRequest, BsvHomePaymailError> {
        self.confirm_public_funding_with_checkpoint(preparation, value, &|| Ok(())).await
    }

    pub async fn transfer_data(
        &self,
        asset: Asset,
        input: String,
        state: BsvHomeNameRecordState,
        name: Option<String>,
        value: BsvHomeBigInt,
        use_max_amount: bool,
    ) -> Result<BsvHomeTransferData, BsvHomePaymailError> {
        self.amount.require_asset(&asset.id).map_err(BsvHomePaymailError::from)?;
        if value <= BsvHomeBigInt::from(0) {
            return Err(BsvHomePaymailError::InvalidAmount);
        }
        let recipient = if paymail_input(&asset, &input)? {
            let destination = self.paymail.destination(input, None).await?;
            BsvHomeRecipient::named(destination.address, destination.handle)
        } else {
            self.recipient(asset.clone(), input, state, name).map_err(|_| BsvHomePaymailError::InvalidHandle)?
        };
        self.transfer_to_recipient(asset, recipient, value, use_max_amount).await
    }
}

impl BsvHomeSendService {
    pub(crate) fn check_public_funding_scope(&self, preparation: &BsvHomeSendPublicFundingPreparation) -> Result<(), BsvHomePaymailError> {
        preparation.check_owner(self.owner)?;
        self.paymail.check_prepared_sender_scope(&preparation.snapshot)
    }

    pub(crate) fn public_funding_payer_matches(&self, preparation: &BsvHomeSendPublicFundingPreparation, payer: &Wallet) -> Result<bool, BsvHomePaymailError> {
        self.check_public_funding_scope(preparation)?;
        self.paymail.prepared_sender_matches_payer(&preparation.snapshot, payer)
    }

    pub(crate) async fn confirm_public_funding_for_payer(
        &self,
        preparation: Arc<BsvHomeSendPublicFundingPreparation>,
        payer: &Wallet,
        value: BsvHomeBigInt,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<BsvHomeBsvMultisigPaymentRequest, BsvHomePaymailError> {
        let payer_check = || {
            check()?;
            if !self.public_funding_payer_matches(&preparation, payer).map_err(|_| BsvHomePortalAuthError::WalletChanged)? {
                return Err(BsvHomePortalAuthError::WalletChanged);
            }
            Ok(())
        };
        if !self.public_funding_payer_matches(&preparation, payer)? {
            return Err(BsvHomePaymailError::BindingChanged);
        }
        self.confirm_public_funding_with_checkpoint(preparation.clone(), value, &payer_check).await
    }

    pub(crate) async fn check_public_funding_source(
        &self,
        preparation: &BsvHomeSendPublicFundingPreparation,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<(), BsvHomePaymailError> {
        check()?;
        self.check_public_funding_scope(preparation)?;
        self.paymail.check_prepared_sender(&preparation.snapshot, check).await?;
        self.check_public_funding_scope(preparation)?;
        check()?;
        Ok(())
    }

    pub(crate) async fn confirm_public_funding_with_checkpoint(
        &self,
        preparation: Arc<BsvHomeSendPublicFundingPreparation>,
        value: BsvHomeBigInt,
        check: &(dyn Fn() -> Result<(), BsvHomePortalAuthError> + Sync),
    ) -> Result<BsvHomeBsvMultisigPaymentRequest, BsvHomePaymailError> {
        check()?;
        self.check_public_funding_scope(&preparation)?;
        let amount = exact_bsv_satoshis(&value)?;
        let prepared_check = || {
            preparation.check()?;
            check()
        };
        preparation
            .run(async {
                self.check_public_funding_source(&preparation, &prepared_check).await?;
                let resolved = self.paymail.confirm_for_send(&preparation.snapshot, amount, &prepared_check).await?;
                self.check_public_funding_source(&preparation, &prepared_check).await?;
                match (&preparation.snapshot.result.output, resolved.output) {
                    (BsvHomePaymailOutput::P2ms { output_script: original, .. }, BsvHomePaymailOutput::P2ms { output_script, request }) if *original == output_script => {
                        Ok(request)
                    }
                    (BsvHomePaymailOutput::P2ms { .. }, BsvHomePaymailOutput::P2ms { .. } | BsvHomePaymailOutput::P2pkh { .. })
                    | (BsvHomePaymailOutput::P2pkh { .. }, BsvHomePaymailOutput::P2pkh { .. } | BsvHomePaymailOutput::P2ms { .. }) => Err(BsvHomePaymailError::BindingChanged),
                }
            })
            .await
    }

    async fn transfer_to_recipient(
        &self,
        asset: Asset,
        recipient: BsvHomeRecipient,
        value: BsvHomeBigInt,
        use_max_amount: bool,
    ) -> Result<BsvHomeTransferData, BsvHomePaymailError> {
        self.amount
            .transfer_data(
                asset,
                BsvHomeAmountTransfer::Send {
                    payment: BsvHomePaymentRecipient { recipient, amount: None },
                },
                value,
                use_max_amount,
            )
            .await
            .map_err(BsvHomePaymailError::from)
    }
}

fn exact_bsv_satoshis(value: &BsvHomeBigInt) -> Result<u64, BsvHomePaymailError> {
    value
        .to_u64()
        .filter(|amount| *amount > 0 && *amount <= MAX_BSV_PAYMENT_SATOSHIS)
        .ok_or(BsvHomePaymailError::InvalidAmount)
}

fn is_paymail_input(input: &str) -> bool {
    input.contains('@')
}

fn supports_paymail(asset: &Asset) -> bool {
    asset.id == AssetId::from_chain(Chain::BitcoinSv)
}

fn paymail_input(asset: &Asset, input: &str) -> Result<bool, BsvHomePaymailError> {
    if !is_paymail_input(input) {
        return Ok(false);
    }
    if !supports_paymail(asset) {
        return Err(BsvHomePaymailError::UnsupportedCapability);
    }
    Ok(true)
}

fn scanned_transfer(asset: &Asset, transfer: BsvHomeTransferData) -> Result<BsvHomePaymentRecipient, BsvHomeRecipientError> {
    let valid_asset = match &transfer.input_type {
        TransactionInputType::Transfer { asset: scanned_asset } => scanned_asset.id == asset.id,
        TransactionInputType::Deposit { .. }
        | TransactionInputType::Withdrawal { .. }
        | TransactionInputType::Swap { .. }
        | TransactionInputType::Stake { .. }
        | TransactionInputType::TokenApprove { .. }
        | TransactionInputType::Generic { .. }
        | TransactionInputType::TransferNft { .. }
        | TransactionInputType::Account { .. }
        | TransactionInputType::Perpetual { .. }
        | TransactionInputType::Earn { .. } => false,
    };
    if !valid_asset || transfer.use_max_amount || transfer.value <= BsvHomeBigInt::from(0) {
        return Err(BsvHomeRecipientError::InvalidAddress);
    }
    let amount = BigNumberFormatter::value(&transfer.value.to_string(), asset.decimals).map_err(|_| BsvHomeRecipientError::InvalidAddress)?;
    Ok(BsvHomePaymentRecipient {
        recipient: transfer.recipient,
        amount: Some(amount),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use futures::executor::block_on;
    use primitives::Account;
    use primitives::known_assets::TRON_USDT;

    use crate::alien::AlienResponse;
    use crate::api::{BsvHomeDeviceApiClient, BsvHomeStaticApiClient};
    use crate::gateway::{BsvHomeGateway, EmptyPreferences};
    use crate::payment::BsvHomePaymentService;
    use crate::services::chain::BsvHomeRuntimeEnvironment;
    use crate::services::confirm::testkit::UnusedStakeStore;
    use crate::services::device::BsvHomeDeviceKeyService;
    use crate::services::explorer::BsvHomeExplorerService;
    use crate::services::name::BsvHomeNameService;
    use crate::services::portal::testkit::MemoryPublicPaymailProvider;
    use crate::services::preferences::{BsvHomePreferencesService, testkit::MemoryPreferencesStore};
    use crate::services::stake::BsvHomeStakeService;
    use crate::services::wallet::testkit::{MemoryAddressStore, MemoryWalletStore};
    use crate::services::wallet_session::{BsvHomeWalletSessionService, testkit::MemoryWalletSessionStore};

    fn service(provider: Arc<MemoryPublicPaymailProvider>) -> BsvHomeSendService {
        service_with_environment(provider, crate::services::chain::BsvHomeRuntimeEnvironment::Production)
    }

    fn service_with_environment(provider: Arc<MemoryPublicPaymailProvider>, environment: crate::services::chain::BsvHomeRuntimeEnvironment) -> BsvHomeSendService {
        let paymail = Arc::new(BsvHomePaymailDestinationService::new(provider.clone()));
        service_with_resolver(provider, environment, paymail)
    }

    pub(super) fn service_with_resolver(
        provider: Arc<MemoryPublicPaymailProvider>,
        environment: crate::services::chain::BsvHomeRuntimeEnvironment,
        paymail: Arc<BsvHomePaymailDestinationService>,
    ) -> BsvHomeSendService {
        let preference_store = Arc::new(MemoryPreferencesStore::default());
        let preferences = Arc::new(BsvHomePreferencesService::new(preference_store.clone()));
        let session = Arc::new(BsvHomeWalletSessionService::new_with_environment(
            Arc::new(MemoryWalletSessionStore::default()),
            Arc::new(MemoryWalletStore::default()),
            environment,
        ));
        let address_store = Arc::new(MemoryAddressStore::default());
        let device_api = Arc::new(BsvHomeDeviceApiClient::new(
            provider.clone(),
            Arc::new(BsvHomeDeviceKeyService::new(Arc::new(EmptyPreferences))),
        ));
        let recipient = Arc::new(BsvHomeRecipientService::new(
            Arc::new(BsvHomeNameService::new(device_api, address_store.clone())),
            Arc::new(BsvHomePaymentService::new(provider.clone())),
            session.clone(),
        ));
        let stake = Arc::new(BsvHomeStakeService::new(
            Arc::new(BsvHomeGateway::new(provider.clone(), preference_store, Arc::new(EmptyPreferences))),
            Arc::new(BsvHomeStaticApiClient::new(provider.clone())),
            Arc::new(UnusedStakeStore),
            address_store,
            Arc::new(BsvHomeExplorerService::new(preferences.clone())),
            preferences.clone(),
            session.clone(),
        ));
        BsvHomeSendService::new(recipient, Arc::new(BsvHomeAmountService::new(stake, preferences, session)), paymail)
    }

    #[test]
    fn test_multisig_configuration_is_not_a_normal_send_address() {
        let service = service(Arc::new(MemoryPublicPaymailProvider::default()));
        for input in ["WOWPAY-MSIG:1:invalid", "P2MS:script-hash", "{\"network\":\"BSV_MAINNET\",\"type\":\"P2MS\"}"] {
            assert!(service.scan(Asset::from_chain(Chain::BitcoinSv), input.into()).is_err());
            assert!(
                service
                    .recipient(Asset::from_chain(Chain::BitcoinSv), input.into(), BsvHomeNameRecordState::None, None)
                    .is_err()
            );
        }
    }

    #[test]
    fn test_wowpay_send_and_amount_reject_hidden_assets_before_resolution() {
        block_on(async {
            let provider = Arc::new(MemoryPublicPaymailProvider::default());
            let service = service_with_environment(provider.clone(), crate::services::chain::BsvHomeRuntimeEnvironment::WowPay);
            for asset in [
                Asset::from_chain(Chain::Ethereum),
                Asset::from_chain(Chain::Tron),
                primitives::known_assets::TRON_USDT.clone(),
                Asset::from_chain(Chain::TronShasta),
                Asset {
                    id: AssetId::from_token(Chain::Tron, "counterfeit-usdt"),
                    ..primitives::known_assets::TRON_USDT.clone()
                },
            ] {
                assert!(service.recipient(asset.clone(), "receiver".into(), BsvHomeNameRecordState::None, None).is_err());
                assert!(service.scan(asset.clone(), "alice@example.com".into()).is_err());
                assert!(
                    service
                        .transfer_data(asset.clone(), "alice@example.com".into(), BsvHomeNameRecordState::None, None, 1.into(), false)
                        .await
                        .is_err()
                );
                assert!(
                    service
                        .amount
                        .transfer_data(
                            asset,
                            BsvHomeAmountTransfer::Send {
                                payment: BsvHomePaymentRecipient {
                                    recipient: BsvHomeRecipient::address("receiver".into()),
                                    amount: None
                                }
                            },
                            1.into(),
                            false
                        )
                        .await
                        .is_err()
                );
            }
            let asset = Asset::from_chain(Chain::BitcoinSv);
            let transfer = service
                .amount
                .transfer_data(
                    asset.clone(),
                    BsvHomeAmountTransfer::Send {
                        payment: BsvHomePaymentRecipient {
                            recipient: BsvHomeRecipient::address("receiver".into()),
                            amount: None,
                        },
                    },
                    1.into(),
                    false,
                )
                .await
                .unwrap();
            assert_eq!(transfer.input_type.get_asset().id, asset.id);
            assert!(provider.requests.lock().unwrap().is_empty());
        });
    }

    #[test]
    fn test_wallet_recipients_bind_fixed_source_and_reject_damaged_or_hidden_accounts() {
        let provider = Arc::new(MemoryPublicPaymailProvider::default());
        let scoped = service_with_environment(provider.clone(), BsvHomeRuntimeEnvironment::WowPay);
        let asset = Asset::from_chain(Chain::BitcoinSv);
        let source = Wallet::mock_with_accounts(vec![Account::mock(Chain::BitcoinSv, "1BitcoinEaterAddressDontSendf59kuE")]);
        let target = Wallet {
            id: WalletId::Multicoin("target-wallet".into()),
            name: "Target wallet".into(),
            accounts: vec![Account::mock(Chain::BitcoinSv, "1CounterpartyXXXXXXXXXXXXXXXUWLpVr")],
            ..source.clone()
        };
        let unsupported = Wallet {
            id: WalletId::Multicoin("unsupported-wallet".into()),
            accounts: vec![Account::mock(Chain::Tron, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t")],
            ..source.clone()
        };
        assert_eq!(
            scoped
                .wallet_recipients(asset.clone(), vec![source.clone(), target.clone(), unsupported], source.id.clone())
                .unwrap(),
            vec![BsvHomeSendWalletRecipient {
                wallet_id: target.id.clone(),
                recipient: BsvHomeRecipient::named("1CounterpartyXXXXXXXXXXXXXXXUWLpVr".into(), "Target wallet".into()),
            }]
        );
        let damaged = Wallet {
            accounts: vec![Account::mock(Chain::BitcoinSv, "invalid-account")],
            ..target.clone()
        };
        assert_eq!(
            scoped.wallet_recipients(asset, vec![damaged], source.id.clone()),
            Err(BsvHomeRecipientError::InvalidAddress)
        );
        assert_eq!(
            scoped.wallet_recipients(Asset::from_chain(Chain::Tron), vec![target], source.id),
            Err(BsvHomeRecipientError::InvalidAddress)
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_contact_recipients_bind_owner_chain_and_supported_destination_without_resolving_names() {
        let provider = Arc::new(MemoryPublicPaymailProvider::default());
        let scoped = service_with_environment(provider.clone(), BsvHomeRuntimeEnvironment::WowPay);
        let created = Utc.timestamp_opt(0, 0).unwrap();
        let alice = Contact {
            id: "alice".into(),
            name: "Alice".into(),
            description: None,
            image_url: None,
            created_at: created,
            updated_at: created,
        };
        let bob = Contact {
            id: "bob".into(),
            name: "Bob".into(),
            ..alice.clone()
        };
        let address = |id: &str, owner: &str, chain: Chain, value: &str, memo: Option<&str>| ContactAddress {
            id: id.into(),
            contact_id: owner.into(),
            chain,
            address: value.into(),
            memo: memo.map(str::to_string),
        };
        let addresses = vec![
            address("alice-main", "alice", Chain::BitcoinSv, "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5", None),
            address("bob-main", "bob", Chain::BitcoinSv, "1BoatSLRHtKNngkdXEeobR76b53LETtpyT", None),
            address("unknown-owner", "unknown", Chain::BitcoinSv, "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5", None),
            address("wrong-chain", "alice", Chain::Tron, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t", None),
            address("memo", "alice", Chain::BitcoinSv, "1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5", Some("")),
            address("script", "alice", Chain::BitcoinSv, "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy", None),
            address("testnet", "alice", Chain::BitcoinSv, "mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn", None),
            address("unresolved-name", "alice", Chain::BitcoinSv, "alice@example.com", None),
            address("invalid", "alice", Chain::BitcoinSv, "not-a-valid-address", None),
        ];
        let contacts = vec![alice.clone(), bob];
        let rows = scoped.contact_recipients(Asset::from_chain(Chain::BitcoinSv), contacts.clone(), addresses.clone()).unwrap();
        assert_eq!(
            rows,
            vec![
                BsvHomeSendContactRecipient {
                    id: "alice-main".into(),
                    contact_id: "alice".into(),
                    recipient: BsvHomeRecipient::named("1L2F8wYxTRagCZLnsm2engg8ngGECSeuE5".into(), "Alice".into())
                },
                BsvHomeSendContactRecipient {
                    id: "bob-main".into(),
                    contact_id: "bob".into(),
                    recipient: BsvHomeRecipient::named("1BoatSLRHtKNngkdXEeobR76b53LETtpyT".into(), "Bob".into())
                },
            ]
        );
        assert_eq!(
            scoped.contact_recipients(Asset::from_chain(Chain::Tron), contacts.clone(), addresses.clone()),
            Err(BsvHomeRecipientError::InvalidAddress)
        );
        let production = service(provider.clone());
        assert_eq!(
            production.contact_recipients(TRON_USDT.clone(), vec![alice], addresses).unwrap(),
            vec![BsvHomeSendContactRecipient {
                id: "wrong-chain".into(),
                contact_id: "alice".into(),
                recipient: BsvHomeRecipient::named("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t".into(), "Alice".into())
            }]
        );
        assert_eq!(provider.requests.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_transfer_data_resolves_paymail_and_preserves_exact_amount_and_handle() {
        block_on(async {
            let provider = Arc::new(MemoryPublicPaymailProvider::default());
            provider.replies.lock().unwrap().extend([
                Arc::new(AlienResponse::new(
                    Some(200),
                    br#"{"bsvalias":"1.0","capabilities":{"paymentDestination":"https://receiver.example/address/{alias}@{domain.tld}"}}"#.to_vec(),
                )),
                Arc::new(AlienResponse::new(
                    Some(200),
                    br#"{"output":"76a914000000000000000000000000000000000000000088ac"}"#.to_vec(),
                )),
            ]);
            let service = service(provider.clone());
            let asset = Asset::from_chain(Chain::BitcoinSv);
            let transfer = service
                .transfer_data(asset.clone(), " Alice@Example.COM ".into(), BsvHomeNameRecordState::None, None, 30_000_000.into(), false)
                .await
                .unwrap();
            let TransactionInputType::Transfer { asset: transfer_asset } = transfer.input_type else {
                panic!("expected native BSV transfer");
            };
            assert_eq!(transfer_asset, asset);
            assert_eq!(
                transfer.recipient,
                BsvHomeRecipient::named("1111111111111111111114oLvT2".into(), "alice@example.com".into())
            );
            assert_eq!(transfer.value, BsvHomeBigInt::from(30_000_000));
            assert!(!transfer.use_max_amount);
            let requests = provider.requests.lock().unwrap();
            assert_eq!(requests.len(), 2);
            assert_eq!(requests[1].body, Some(b"{}".to_vec()));
        });
    }

    #[test]
    fn test_transfer_data_rejects_unsupported_paymail_assets_before_network_request() {
        block_on(async {
            let provider = Arc::new(MemoryPublicPaymailProvider::default());
            let service = service(provider.clone());
            let bsv = Asset::from_chain(Chain::BitcoinSv);
            for asset in [
                Asset::from_chain(Chain::Tron),
                Asset {
                    id: AssetId::from_token(Chain::BitcoinSv, "token"),
                    ..bsv
                },
            ] {
                assert_eq!(
                    service
                        .transfer_data(asset, "alice@example.com".into(), BsvHomeNameRecordState::None, None, 30_000_000.into(), false)
                        .await
                        .err(),
                    Some(BsvHomePaymailError::UnsupportedCapability)
                );
            }
            assert_eq!(provider.requests.lock().unwrap().len(), 0);
        });
    }

    #[test]
    fn test_paymail_is_limited_to_the_native_bsv_asset() {
        let bsv = Asset::from_chain(Chain::BitcoinSv);
        let token = Asset {
            id: AssetId::from_token(Chain::BitcoinSv, "token"),
            ..bsv.clone()
        };
        assert_eq!(paymail_input(&bsv, "alice@example.com"), Ok(true));
        for asset in [token, Asset::from_chain(Chain::Tron), Asset::mock_spl_token()] {
            assert_eq!(paymail_input(&asset, "alice@example.com"), Err(BsvHomePaymailError::UnsupportedCapability));
        }
        assert_eq!(paymail_input(&bsv, "1BoatSLRHtKNngkdXEeobR76b53LETtpyT"), Ok(false));
    }

    #[test]
    fn test_scanned_payment_preserves_exact_satoshis_and_rejects_other_transfers() {
        let asset = Asset::from_chain(Chain::BitcoinSv);
        let transfer = BsvHomeTransferData {
            input_type: TransactionInputType::Transfer { asset: asset.clone() },
            recipient: BsvHomeRecipient::address("1BoatSLRHtKNngkdXEeobR76b53LETtpyT".into()),
            value: 30_000_000.into(),
            use_max_amount: false,
        };
        assert_eq!(scanned_transfer(&asset, transfer.clone()).unwrap().amount, Some("0.3".into()));
        assert_eq!(
            scanned_transfer(&Asset::from_chain(Chain::Tron), transfer.clone()),
            Err(BsvHomeRecipientError::InvalidAddress)
        );
        assert_eq!(
            scanned_transfer(
                &asset,
                BsvHomeTransferData {
                    use_max_amount: true,
                    ..transfer.clone()
                }
            ),
            Err(BsvHomeRecipientError::InvalidAddress)
        );
        assert_eq!(
            scanned_transfer(&asset, BsvHomeTransferData { value: 0.into(), ..transfer }),
            Err(BsvHomeRecipientError::InvalidAddress)
        );
    }
}

#[cfg(test)]
#[path = "send_public_paymail_tests.rs"]
mod public_paymail_tests;
