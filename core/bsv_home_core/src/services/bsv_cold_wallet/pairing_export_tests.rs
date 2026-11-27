use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bsv_home_bsv::cold_wallet::{Pairing, decode_json};
use futures::{channel::oneshot, executor::block_on, join};
use primitives::testkit::{ABANDON_PHRASE, signer_mock::TEST_PRIVATE_KEY};
use primitives::{Chain, Wallet, WalletId, WalletSource};
use signer::encode_private_key;

use super::testkit::{Context, MemoryColdWalletStore, sign_action};
use super::{BsvHomeBsvColdWalletRole, BsvHomeBsvColdWalletService, BsvHomeBsvColdWalletStateStamp, BsvHomeBsvColdWalletStore, BsvHomeBsvColdWalletStoredState};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::model::BsvHomeWalletSecretKind;
use crate::services::wallet::{BsvHomeWalletImportResult, BsvHomeWalletImportType, BsvHomeWalletStore};

fn unified(context: &Context, store: Arc<dyn BsvHomeBsvColdWalletStore>) -> BsvHomeBsvColdWalletService {
    BsvHomeBsvColdWalletService::unified(
        context.wallets.clone(),
        context.addresses.clone(),
        Arc::new(BsvHomeGateway::new(context.provider.clone(), Arc::new(EmptyPreferences), Arc::new(EmptyPreferences))),
        context.keystore.clone(),
        store,
        context.signer.clone(),
    )
}

fn imported(result: BsvHomeWalletImportResult) -> Wallet {
    match result {
        BsvHomeWalletImportResult::New { wallet } | BsvHomeWalletImportResult::Existing { wallet } => wallet,
    }
}

async fn phrase(context: &Context) -> Wallet {
    imported(
        context
            .wallets
            .import_wallet(
                "Ordinary HD".into(),
                BsvHomeWalletImportType::MulticoinPhrase {
                    words: ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                    chains: vec![Chain::BitcoinSv],
                },
                WalletSource::Import,
            )
            .await
            .unwrap(),
    )
}

async fn observer(context: &Context, address: &str) -> Wallet {
    imported(
        context
            .wallets
            .import_wallet(
                "Ordinary watch".into(),
                BsvHomeWalletImportType::Address {
                    address: address.into(),
                    chain: Chain::BitcoinSv,
                },
                WalletSource::Import,
            )
            .await
            .unwrap(),
    )
}

#[test]
fn test_explicit_pairing_export_converts_only_the_reviewed_real_hd_wallet_and_claims_selection_after_storage() {
    block_on(async {
        for recovery in [false, true] {
            let context = Context::new();
            let wallet = if recovery {
                imported(
                    context
                        .wallets
                        .bsv_hd_import("Recovered HD".into(), ABANDON_PHRASE.into(), Some("m/44'/236'/1'".into()), None, None, false)
                        .await
                        .unwrap(),
                )
            } else {
                phrase(&context).await
            };
            let previous = observer(&context, "1BitcoinEaterAddressDontSendf59kuE").await;
            context.wallets.set_current_wallet_id(previous.id.clone()).unwrap();
            let service = unified(&context, context.store.clone());
            assert!(service.wallet_candidates().await.unwrap().is_empty());
            assert!(context.store.records.lock().unwrap().is_empty());
            let snapshot = service.export_pairing(wallet.id.clone(), service.capture_generation()).await.unwrap();
            assert_eq!(snapshot.wallet.id, wallet.id);
            assert_eq!(snapshot.role, BsvHomeBsvColdWalletRole::Signer);
            let pairing: Pairing = decode_json(&snapshot.pairing_json).unwrap();
            pairing.validate().unwrap();
            assert_eq!(pairing.first_address, wallet.account(Chain::BitcoinSv).unwrap().address);
            assert!(context.store.records.lock().unwrap().contains_key(&wallet.id));
            assert_eq!(context.wallets.current_wallet_id().unwrap(), Some(wallet.id.clone()));
            assert!(service.consume_wallet_selection(wallet.id.clone()).unwrap());
            assert!(!service.consume_wallet_selection(wallet.id).unwrap());
            assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
            assert_eq!(*context.signer.authentication_count.lock().unwrap(), 0);
            assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
        }
    });
}

#[test]
fn test_explicit_pairing_export_preserves_existing_complete_signed_state_without_another_signature() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let request = context.request(wallet.id.clone(), 75000).await;
        let pending = context.service.import_request(wallet.id.clone(), request).await.unwrap();
        let signed = sign_action(&context.service, wallet.id.clone(), pending.review.commitment).await.unwrap();
        let before = context.store.records.lock().unwrap().get(&wallet.id).unwrap().clone();
        let signatures = *context.signer.signing_count.lock().unwrap();
        let service = unified(&context, context.store.clone());
        let snapshot = service.export_pairing(wallet.id.clone(), service.capture_generation()).await.unwrap();
        assert!(snapshot.pending == Some(signed));
        assert!(context.store.records.lock().unwrap().get(&wallet.id) == Some(&before));
        assert_eq!(*context.signer.signing_count.lock().unwrap(), signatures);
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_explicit_pairing_export_rejects_private_watch_non_bsv_and_changed_public_profile_before_marker_or_selection() {
    block_on(async {
        for kind in [InvalidSource::Private, InvalidSource::Watch, InvalidSource::NonBsv, InvalidSource::Profile] {
            let context = Context::new();
            let wallet = match kind {
                InvalidSource::Private => imported(
                    context
                        .wallets
                        .import_wallet(
                            "Private fixture".into(),
                            BsvHomeWalletImportType::PrivateKey {
                                value: encode_private_key(&Chain::BitcoinSv, &TEST_PRIVATE_KEY).unwrap(),
                                chain: Chain::BitcoinSv,
                            },
                            WalletSource::Import,
                        )
                        .await
                        .unwrap(),
                ),
                InvalidSource::Watch => observer(&context, "1BoatSLRHtKNngkdXEeobR76b53LETtpyT").await,
                InvalidSource::NonBsv => {
                    let wallet = imported(
                        context
                            .wallets
                            .import_wallet(
                                "Other chain".into(),
                                BsvHomeWalletImportType::SinglePhrase {
                                    words: ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                                    chain: Chain::Ethereum,
                                },
                                WalletSource::Import,
                            )
                            .await
                            .unwrap(),
                    );
                    context
                        .wallet_store
                        .wallets
                        .lock()
                        .unwrap()
                        .iter_mut()
                        .find(|record| record.id == wallet.id)
                        .unwrap()
                        .accounts
                        .retain(|account| account.chain != Chain::BitcoinSv);
                    let wallet = context.wallet_store.get_wallet(wallet.id).await.unwrap().unwrap();
                    assert!(wallet.account(Chain::BitcoinSv).is_none());
                    assert_eq!(context.wallets.secret_kind(wallet.id.clone()).await.unwrap(), Some(BsvHomeWalletSecretKind::Phrase));
                    wallet
                }
                InvalidSource::Profile => {
                    let wallet = phrase(&context).await;
                    context
                        .wallet_store
                        .wallets
                        .lock()
                        .unwrap()
                        .iter_mut()
                        .find(|record| record.id == wallet.id)
                        .unwrap()
                        .accounts
                        .iter_mut()
                        .find(|account| account.chain == Chain::BitcoinSv)
                        .unwrap()
                        .address = "1BitcoinEaterAddressDontSendf59kuE".into();
                    wallet
                }
            };
            let previous = observer(&context, "1BitcoinEaterAddressDontSendf59kuE").await;
            context.wallets.set_current_wallet_id(previous.id.clone()).unwrap();
            let service = unified(&context, context.store.clone());
            let rejected = service.export_pairing(wallet.id.clone(), service.capture_generation()).await.is_err();
            assert!(rejected, "{kind:?}");
            assert!(context.store.records.lock().unwrap().is_empty());
            assert_eq!(context.wallets.current_wallet_id().unwrap(), Some(previous.id));
            assert!(!service.consume_wallet_selection(wallet.id).unwrap());
            assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
            assert_eq!(*context.signer.authentication_count.lock().unwrap(), 0);
        }
    });
}

#[derive(Debug, Clone, Copy)]
enum InvalidSource {
    Private,
    Watch,
    NonBsv,
    Profile,
}

#[test]
fn test_explicit_pairing_export_rejects_stale_first_poll_and_store_failure_without_changing_the_current_wallet() {
    block_on(async {
        for stale in [false, true] {
            let context = Context::new();
            let wallet = phrase(&context).await;
            let previous = observer(&context, "1BitcoinEaterAddressDontSendf59kuE").await;
            context.wallets.set_current_wallet_id(previous.id.clone()).unwrap();
            let service = unified(&context, context.store.clone());
            let generation = service.capture_generation();
            if stale {
                service.invalidate_actions().unwrap();
            } else {
                *context.store.fail.lock().unwrap() = true;
            }
            assert!(service.export_pairing(wallet.id.clone(), generation).await.is_err());
            assert!(context.store.records.lock().unwrap().is_empty());
            assert_eq!(context.wallets.current_wallet_id().unwrap(), Some(previous.id));
            assert!(!service.consume_wallet_selection(wallet.id).unwrap());
            assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
        }
    });
}

#[test]
fn test_explicit_pairing_export_rechecks_cancel_selection_and_wallet_after_real_store_awaits_and_never_claims_late_selection() {
    block_on(async {
        for committed in [false, true] {
            for change in [Change::Cancel, Change::Selection, Change::Account] {
                let context = Context::new();
                let wallet = phrase(&context).await;
                let previous = observer(&context, "1BoatSLRHtKNngkdXEeobR76b53LETtpyT").await;
                let other = observer(&context, "1BitcoinEaterAddressDontSendf59kuE").await;
                context.wallets.set_current_wallet_id(previous.id.clone()).unwrap();
                let (entered, waiting) = oneshot::channel();
                let (release, gate) = oneshot::channel();
                let store = Arc::new(PairingGateStore {
                    inner: context.store.clone(),
                    committed,
                    gate: Mutex::new(Some((entered, gate))),
                });
                let service = unified(&context, store);
                let exporting = service.export_pairing(wallet.id.clone(), service.capture_generation());
                let changing = async {
                    waiting.await.unwrap();
                    match change {
                        Change::Cancel => service.invalidate_actions().unwrap(),
                        Change::Selection => context.wallets.set_current_wallet_id(other.id.clone()).unwrap(),
                        Change::Account => {
                            context
                                .wallet_store
                                .wallets
                                .lock()
                                .unwrap()
                                .iter_mut()
                                .find(|record| record.id == wallet.id)
                                .unwrap()
                                .accounts[0]
                                .address = other.accounts[0].address.clone()
                        }
                    }
                    release.send(()).unwrap();
                };
                let (result, ()) = join!(exporting, changing);
                assert!(result.is_err());
                assert_eq!(context.store.records.lock().unwrap().contains_key(&wallet.id), committed);
                let current = match change {
                    Change::Selection => other.id,
                    Change::Cancel | Change::Account => previous.id,
                };
                assert_eq!(context.wallets.current_wallet_id().unwrap(), Some(current));
                assert!(!service.consume_wallet_selection(wallet.id).unwrap());
                assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
                assert_eq!(*context.signer.authentication_count.lock().unwrap(), 0);
                assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
            }
        }
    });
}

#[derive(Clone, Copy)]
enum Change {
    Cancel,
    Selection,
    Account,
}

type PairingGate = (oneshot::Sender<()>, oneshot::Receiver<()>);

struct PairingGateStore {
    inner: Arc<MemoryColdWalletStore>,
    committed: bool,
    gate: Mutex<Option<PairingGate>>,
}

impl PairingGateStore {
    async fn pause(&self, committed: bool) {
        if self.committed != committed {
            return;
        }
        let gate = self.gate.lock().unwrap().take();
        if let Some((entered, release)) = gate {
            entered.send(()).unwrap();
            release.await.unwrap();
        }
    }
}

#[async_trait]
impl BsvHomeBsvColdWalletStore for PairingGateStore {
    fn state_stamp(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvColdWalletStateStamp>, BsvHomeServiceError> {
        self.inner.state_stamp(wallet_id)
    }
    fn has_state(&self, wallet_id: WalletId) -> Result<bool, BsvHomeServiceError> {
        self.inner.has_state(wallet_id)
    }
    async fn get(&self, wallet_id: WalletId) -> Result<Option<BsvHomeBsvColdWalletStoredState>, BsvHomeServiceError> {
        let result = self.inner.get(wallet_id).await?;
        self.pause(false).await;
        Ok(result)
    }
    async fn compare_and_swap(&self, wallet_id: WalletId, revision: Option<u64>, state: BsvHomeBsvColdWalletStoredState) -> Result<bool, BsvHomeServiceError> {
        let result = self.inner.compare_and_swap(wallet_id, revision, state).await?;
        if result {
            self.pause(true).await;
        }
        Ok(result)
    }
}
