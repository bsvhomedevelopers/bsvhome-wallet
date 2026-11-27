use std::future::Future;
use std::pin::pin;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::task::{Context as TaskContext, Poll};

use bsv_home_bsv::{
    BsvAddress,
    multisig::{Funding, FundingAddress, Policy, funding_transaction_id, policy_descriptor, review_funding},
};
use futures::{executor::block_on, task::noop_waker};
use primitives::{Address, BsvAddressBranch, BsvAddressLocator, BsvSpendChange, BsvSpendInput, BsvSpendMetadata, Chain, TransactionLoadMetadata, UTXO, Wallet, WalletId};
use tempfile::TempDir;
use uuid::Uuid;

use super::state::PoolState;
use super::testkit::{BsvAddressProvider, MemoryBsvAddressStore, pool_with_keystore};
use super::tests::payer_transfer_input;
use super::{BsvHomeBsvAddressPool, BsvHomeBsvAddressStore, BsvHomeBsvFundingStoredState};
use crate::alien::AlienProvider;
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::keystore::{BsvHomeImportType, BsvHomeKeystore, decode_password};
use crate::services::bsv_multisig::{
    BsvHomeBsvMultisigFundingStatus, BsvHomeBsvMultisigFundingTarget, FundingCheckpoint, LegacyFundingCheckpoint, PayerFundingBinding, PayerFundingCheckpoint,
    bsv_multisig_payment_request_from_script,
};
use crate::services::error::BsvHomeServiceError;
use crate::services::wallet::BsvHomeWalletStore;
use crate::services::wallet::{
    rules as wallet_rules,
    testkit::{MemoryKeystorePassword, MemoryWalletStore, TEST_PASSWORD},
};
use crate::testkit::TestAlienProvider;

const ORIGIN: &str = "https://ledger-fixture.invalid/api/portal";
fn locator(branch: BsvAddressBranch, index: u32) -> BsvAddressLocator {
    BsvAddressLocator { branch, index }
}

struct Kit {
    _directory: TempDir,
    pool: Arc<BsvHomeBsvAddressPool>,
    store: Arc<MemoryBsvAddressStore>,
    wallets: Arc<MemoryWalletStore>,
    gateway: Arc<BsvHomeGateway>,
    keystore: Arc<BsvHomeKeystore>,
    wallet: Wallet,
    signs: AtomicUsize,
}
impl Kit {
    async fn new(single: bool) -> Self {
        Self::with_provider(single, Arc::new(TestAlienProvider::with_status(403))).await
    }
    async fn with_provider(single: bool, provider: Arc<dyn AlienProvider>) -> Self {
        Self::with_private_key(single, provider, [9; 32]).await
    }
    async fn with_private_key(single: bool, provider: Arc<dyn AlienProvider>, private_key: [u8; 32]) -> Self {
        let directory = TempDir::new().unwrap();
        let keystore = BsvHomeKeystore::new(directory.path().to_string_lossy().into_owned()).unwrap();
        let import = if single {
            BsvHomeImportType::PrivateKey {
                value: ::signer::encode_private_key(&Chain::BitcoinSv, &private_key).unwrap(),
                chain: Chain::BitcoinSv,
            }
        } else {
            BsvHomeImportType::SinglePhrase {
                words: primitives::testkit::ABANDON_PHRASE.split_whitespace().map(String::from).collect(),
                chain: Chain::BitcoinSv,
            }
        };
        let stored = keystore.create_store(import, decode_password(TEST_PASSWORD)).unwrap();
        let wallet = Wallet {
            id: WalletId::from_id(&stored.wallet_id).unwrap(),
            wallet_type: stored.wallet_type,
            accounts: stored.accounts.into_iter().map(wallet_rules::account).collect(),
            ..Wallet::mock()
        };
        let wallets = Arc::new(MemoryWalletStore::default());
        wallets.wallets.lock().unwrap().push(wallet.clone());
        let store = wallets.bsv_addresses.clone();
        let gateway = Arc::new(BsvHomeGateway::new(provider, Arc::new(EmptyPreferences), Arc::new(EmptyPreferences)));
        let pool = pool_with_keystore(
            gateway.clone(),
            wallets.clone(),
            keystore.clone(),
            Arc::new(MemoryKeystorePassword::default()),
            store.clone(),
        );
        pool.prepare(wallet.id.clone()).await.unwrap();
        Self {
            _directory: directory,
            pool,
            store,
            wallets,
            gateway,
            keystore,
            wallet,
            signs: AtomicUsize::new(0),
        }
    }
    fn rebuilt(&self) -> Arc<BsvHomeBsvAddressPool> {
        pool_with_keystore(
            self.gateway.clone(),
            self.wallets.clone(),
            self.keystore.clone(),
            Arc::new(MemoryKeystorePassword::default()),
            self.store.clone(),
        )
    }
    async fn adopt(&self, checkpoint: PayerFundingCheckpoint) -> Result<(), BsvHomeServiceError> {
        let previous = std::mem::replace(&mut *self.store.funding_states.lock().unwrap(), vec![archive(FundingCheckpoint::Current(checkpoint))]);
        let result = self.pool.reconcile_funding(&self.wallet).await;
        *self.store.funding_states.lock().unwrap() = previous;
        result
    }
    async fn hd(&self) -> Option<PoolState> {
        if self.wallet.wallet_type == primitives::WalletType::PrivateKey {
            None
        } else {
            Some(self.pool.required_state(&self.wallet).await.unwrap().1)
        }
    }
    fn root(&self) -> String {
        self.wallet.account(Chain::BitcoinSv).unwrap().address.clone()
    }
    async fn source(&self, at: BsvAddressLocator, tx: char) -> BsvSpendInput {
        let address = match self.hd().await {
            Some(state) => state.address(at).unwrap(),
            None => {
                assert_eq!(at, locator(BsvAddressBranch::Receive, 0));
                self.root()
            }
        };
        BsvSpendInput {
            transaction_id: tx.to_string().repeat(64),
            vout: 0,
            value: 50_000,
            script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
            address,
            locator: at,
        }
    }
    async fn binding(&self, public: bool, at: BsvAddressLocator, tx: char) -> PayerFundingBinding {
        let change = self.hd().await.map(|state| {
            let at = state.next_change().unwrap();
            let address = state.address(at).unwrap();
            BsvSpendChange {
                script_pubkey: hex::encode(BsvAddress::from_str(&address).unwrap().script_pubkey().as_bytes()),
                address,
                locator: at,
            }
        });
        let funding = Funding::from_spend(
            policy(),
            self.root(),
            10_000,
            100,
            BsvSpendMetadata {
                inputs: vec![self.source(at, tx).await],
                change,
            },
        )
        .unwrap();
        self.binding_from_funding(public, funding)
    }
    fn binding_from_funding(&self, public: bool, funding: Funding) -> PayerFundingBinding {
        let descriptor = policy_descriptor(&funding.policy).unwrap();
        let target = if public {
            BsvHomeBsvMultisigFundingTarget::Public {
                request: bsv_multisig_payment_request_from_script("QA collective".into(), descriptor.locking_script, "0.0001".into()).unwrap(),
            }
        } else {
            BsvHomeBsvMultisigFundingTarget::Member { wallet_id: "collective-1".into() }
        };
        let scope = if public {
            format!("{ORIGIN}|{}", self.wallet.id)
        } else {
            format!("{ORIGIN}|{}|collective-1", self.wallet.id)
        };
        PayerFundingBinding {
            scope,
            operation_id: Uuid::new_v4().to_string(),
            payer_wallet_id: self.wallet.id.clone(),
            target,
            review_json: serde_json::to_string(&review_funding(&funding).unwrap()).unwrap(),
            funding_json: serde_json::to_string(&funding).unwrap(),
        }
    }
    fn sign(&self, binding: &PayerFundingBinding) -> PayerFundingCheckpoint {
        self.signs.fetch_add(1, Ordering::SeqCst);
        let raw = self
            .keystore
            .bsv_multisig_funding_sign(
                self.keystore.keystore_id(self.wallet.id.id()),
                binding.funding_json.clone(),
                binding.review_json.clone(),
                decode_password(TEST_PASSWORD),
            )
            .unwrap();
        let (funding, review) = binding.validate().unwrap();
        PayerFundingCheckpoint {
            binding: binding.clone(),
            transaction_id: funding_transaction_id(&funding, &review, &raw).unwrap(),
            raw,
            status: BsvHomeBsvMultisigFundingStatus::Pending,
            error_code: None,
        }
    }
    async fn legacy(&self, at: BsvAddressLocator, tx: char, old_signer: &str) -> (BsvHomeBsvFundingStoredState, PayerFundingCheckpoint) {
        let input = self.source(at, tx).await;
        let funding = Funding {
            chain: Chain::BitcoinSv,
            policy: policy(),
            sender_address: self.root(),
            amount: 10_000,
            fee_rate: 100,
            utxos: vec![UTXO {
                transaction_id: input.transaction_id,
                vout: i32::try_from(input.vout).unwrap(),
                value: input.value.into(),
                address: input.address.clone(),
            }],
            addresses: if at == locator(BsvAddressBranch::Receive, 0) {
                vec![FundingAddress {
                    address: self.root(),
                    locator: at,
                }]
            } else {
                vec![
                    FundingAddress {
                        address: self.root(),
                        locator: locator(BsvAddressBranch::Receive, 0),
                    },
                    FundingAddress {
                        address: input.address,
                        locator: at,
                    },
                ]
            },
            change: None,
        };
        let mut binding = self.binding_from_funding(false, funding.clone());
        binding.scope = format!("{ORIGIN}|{old_signer}|collective-1");
        binding.funding_json = serde_json::to_string_pretty(&funding).unwrap();
        binding.review_json = serde_json::to_string_pretty(&review_funding(&funding).unwrap()).unwrap();
        let signed = self.sign(&binding);
        let legacy = LegacyFundingCheckpoint {
            scope: binding.scope,
            wallet_id: "collective-1".into(),
            payer_wallet_id: self.wallet.id.id(),
            funding_json: binding.funding_json,
            review_json: binding.review_json,
            raw: signed.raw,
            transaction_id: signed.transaction_id,
            status: signed.status,
            error_code: signed.error_code,
        };
        let checkpoint = legacy.validated_checkpoint().unwrap();
        (archive(FundingCheckpoint::Legacy(legacy)), checkpoint)
    }
}
fn policy() -> Policy {
    let data: serde_json::Value = serde_json::from_str(include_str!("../../../../../crates/bsv_home_bsv/testdata/bsv-multisig-interop.json")).unwrap();
    serde_json::from_value(data["proposal"]["policy"].clone()).unwrap()
}
fn archive(checkpoint: FundingCheckpoint) -> BsvHomeBsvFundingStoredState {
    let scope = checkpoint.scope().to_string();
    let parts: Vec<_> = scope.split('|').collect();
    let identifier = hex::encode(bsv_home_hash::sha2::sha256(format!("{}|{}", parts[0], parts[1]).as_bytes()));
    let mut funding = serde_json::Map::new();
    funding.insert(scope, serde_json::to_value(checkpoint).unwrap());
    let json = serde_json::json!({"trusted":{},"memberNames":{},"intentions":{},"funding":funding});
    BsvHomeBsvFundingStoredState {
        identifier,
        revision: 1,
        json: serde_json::to_string(&json).unwrap(),
    }
}
async fn signed_funding(pool: &BsvHomeBsvAddressPool, wallet: &Wallet, operation_id: &str) -> Result<Option<PayerFundingCheckpoint>, BsvHomeServiceError> {
    Ok(pool
        .funding_checkpoints(wallet)
        .await?
        .into_iter()
        .find(|checkpoint| checkpoint.binding.operation_id == operation_id))
}
fn assert_pending<T>(poll: Poll<T>) {
    assert!(poll.is_pending());
}

#[test]
fn test_typed_funding_real_keystore_durability_and_reopen_for_hd_single_member_public() {
    block_on(async {
        for single in [false, true] {
            for public in [false, true] {
                let kit = Kit::new(single).await;
                let binding = kit.binding(public, locator(BsvAddressBranch::Receive, 0), '1').await;
                let lease = kit.pool.reserve_funding(&kit.wallet, binding.clone()).await.unwrap();
                assert!(kit.store.get(kit.wallet.id.clone()).await.unwrap().is_some());
                assert_eq!(kit.signs.load(Ordering::SeqCst), 0);
                assert_eq!(kit.rebuilt().unsubmitted_funding(&kit.wallet).await.unwrap(), vec![binding.clone()]);
                let checkpoint = kit.sign(&binding);
                kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint.clone()).await.unwrap();
                drop(lease);
                kit.rebuilt().require_funding_checkpoint(&kit.wallet, &checkpoint).await.unwrap();
                assert_eq!(signed_funding(&kit.rebuilt(), &kit.wallet, &binding.operation_id).await.unwrap(), Some(checkpoint.clone()));
                let unsigned = kit.rebuilt().unsubmitted_funding(&kit.wallet).await.unwrap();
                assert!(unsigned.is_empty());
                kit.rebuilt().release_funding_unsigned(&kit.wallet, &binding).await.unwrap();
                assert_eq!(kit.rebuilt().funding_checkpoints(&kit.wallet).await.unwrap(), vec![checkpoint]);
                assert_eq!(kit.signs.load(Ordering::SeqCst), 1);
            }
        }
    });
}

#[test]
fn test_global_funding_lease_blocks_other_pool_at_cas_and_real_keystore_gates() {
    block_on(async {
        for single in [false, true] {
            let kit = Kit::new(single).await;
            let other = kit.rebuilt();
            let binding = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
            *kit.store.hold_next_save.lock().unwrap() = true;
            let mut reserve = Box::pin(kit.pool.reserve_funding(&kit.wallet, binding.clone()));
            let waker = noop_waker();
            let mut task = TaskContext::from_waker(&waker);
            assert_pending(reserve.as_mut().poll(&mut task));
            assert_eq!(kit.store.held_save_count(), 1);
            assert_eq!(
                other.release_funding_unsigned(&kit.wallet, &binding).await.unwrap_err().to_string(),
                "BSV funding is still being signed"
            );
            assert!(other.reserve_funding(&kit.wallet, binding.clone()).await.is_err());
            assert_eq!(kit.signs.load(Ordering::SeqCst), 0);
            assert!(kit.store.complete_held_save().unwrap());
            let lease = reserve.await.unwrap();
            let (release, gate) = futures::channel::oneshot::channel::<()>();
            let mut signing = pin!(async {
                gate.await.unwrap();
                kit.sign(&binding)
            });
            assert_pending(signing.as_mut().poll(&mut task));
            assert_eq!(
                other.release_funding_unsigned(&kit.wallet, &binding).await.unwrap_err().to_string(),
                "BSV funding is still being signed"
            );
            release.send(()).unwrap();
            let checkpoint = signing.await;
            kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint.clone()).await.unwrap();
            drop(lease);
            other.release_funding_unsigned(&kit.wallet, &binding).await.unwrap();
            assert_eq!(signed_funding(&other, &kit.wallet, &binding.operation_id).await.unwrap(), Some(checkpoint));
        }
    });
}

#[test]
fn test_cancel_marker_rejects_foreign_reserve_or_signed_save_that_commits_after_abort() {
    block_on(async {
        for single in [false, true] {
            for signed_save in [false, true] {
                let kit = Kit::new(single).await;
                let other = kit.rebuilt();
                let binding = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
                let waker = noop_waker();
                let mut task = TaskContext::from_waker(&waker);
                if signed_save {
                    let lease = kit.pool.reserve_funding(&kit.wallet, binding.clone()).await.unwrap();
                    let checkpoint = kit.sign(&binding);
                    *kit.store.hold_next_save.lock().unwrap() = true;
                    let mut save = Box::pin(kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint));
                    assert_pending(save.as_mut().poll(&mut task));
                    drop(save);
                    drop(lease);
                } else {
                    *kit.store.hold_next_save.lock().unwrap() = true;
                    let mut reserve = Box::pin(kit.pool.reserve_funding(&kit.wallet, binding.clone()));
                    assert_pending(reserve.as_mut().poll(&mut task));
                    drop(reserve);
                }
                other.release_funding_unsigned(&kit.wallet, &binding).await.unwrap();
                assert!(!kit.store.complete_held_save().unwrap());
                assert!(other.unsubmitted_funding(&kit.wallet).await.unwrap().is_empty());
                assert!(signed_funding(&other, &kit.wallet, &binding.operation_id).await.unwrap().is_none());
                assert!(other.reserve_funding(&kit.wallet, binding).await.is_err());
            }
        }
    });
}

#[test]
fn test_signed_save_winning_before_cancel_retains_the_exact_durable_transaction() {
    block_on(async {
        let kit = Kit::new(true).await;
        let other = kit.rebuilt();
        let binding = kit.binding(true, locator(BsvAddressBranch::Receive, 0), '1').await;
        let lease = kit.pool.reserve_funding(&kit.wallet, binding.clone()).await.unwrap();
        let checkpoint = kit.sign(&binding);
        *kit.store.hold_next_save.lock().unwrap() = true;
        let mut save = Box::pin(kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint.clone()));
        let waker = noop_waker();
        let mut task = TaskContext::from_waker(&waker);
        assert_pending(save.as_mut().poll(&mut task));
        assert!(kit.store.complete_held_save().unwrap());
        drop(save);
        drop(lease);
        other.release_funding_unsigned(&kit.wallet, &binding).await.unwrap();
        assert_eq!(signed_funding(&other, &kit.wallet, &binding.operation_id).await.unwrap(), Some(checkpoint));
        assert_eq!(kit.signs.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn test_legacy_deleted_signer_high_hd_and_single_none_adoption_never_replans_or_resigns() {
    block_on(async {
        let deleted = WalletId::Single(Chain::BitcoinSv, "1BitcoinEaterAddressDontSendf59kuE".into()).id();
        for (single, at) in [
            (false, locator(BsvAddressBranch::Receive, 57)),
            (false, locator(BsvAddressBranch::Change, 44)),
            (true, locator(BsvAddressBranch::Receive, 0)),
        ] {
            let kit = Kit::new(single).await;
            let before = kit.hd().await;
            let (record, expected) = kit.legacy(at, '1', &deleted).await;
            *kit.store.funding_states.lock().unwrap() = vec![record.clone()];
            kit.pool.reconcile_funding(&kit.wallet).await.unwrap();
            let saved = kit.rebuilt().funding_checkpoints(&kit.wallet).await.unwrap();
            assert_eq!(saved, vec![expected.clone()]);
            assert_eq!(*kit.store.funding_states.lock().unwrap(), vec![record]);
            assert_eq!(kit.signs.load(Ordering::SeqCst), 1);
            assert_eq!(kit.wallet.account(Chain::BitcoinSv).unwrap().address, kit.root());
            if let Some(before) = before {
                let after = kit.pool.required_state(&kit.wallet).await.unwrap().1;
                assert_eq!(
                    (after.receive_count, after.selected_index, after.account_xpub),
                    (before.receive_count, before.selected_index, before.account_xpub)
                );
                assert!(after.records.iter().any(|record| record.locator == at && !record.confirmed_history));
                assert!(after.reservations.is_empty());
            }
            let stored = kit.store.get(kit.wallet.id.clone()).await.unwrap();
            kit.pool.reconcile_funding(&kit.wallet).await.unwrap();
            assert_eq!(kit.store.get(kit.wallet.id.clone()).await.unwrap(), stored);
            assert_eq!(signed_funding(&kit.rebuilt(), &kit.wallet, &expected.binding.operation_id).await.unwrap(), Some(expected));
        }
    });
}

#[test]
fn test_hd_single_ordinary_member_public_share_one_outpoint_authority_in_every_winner_order() {
    block_on(async {
        for single in [false, true] {
            for winner in [0, 1, 2] {
                let kit = Kit::new(single).await;
                let other = kit.rebuilt();
                let hd = kit.hd().await;
                let ordinary = payer_transfer_input(&kit.wallet, hd.as_ref());
                let member = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
                let public = kit.binding(true, locator(BsvAddressBranch::Receive, 0), '1').await;
                let lease = match winner {
                    0 => kit.pool.reserve(&kit.wallet, &ordinary, "10000".into(), false).await.unwrap(),
                    1 => kit.pool.reserve_funding(&kit.wallet, member.clone()).await.unwrap(),
                    2 => kit.pool.reserve_funding(&kit.wallet, public.clone()).await.unwrap(),
                    _ => unreachable!(),
                };
                if winner != 0 {
                    assert!(other.reserve(&kit.wallet, &ordinary, "10000".into(), false).await.is_err());
                }
                if winner != 1 {
                    assert!(other.reserve_funding(&kit.wallet, member.clone()).await.is_err());
                }
                if winner != 2 {
                    assert!(other.reserve_funding(&kit.wallet, public.clone()).await.is_err());
                }
                if winner == 0 {
                    kit.signs.fetch_add(1, Ordering::SeqCst);
                    let signed = kit
                        .keystore
                        .sign(
                            kit.keystore.keystore_id(kit.wallet.id.id()),
                            Chain::BitcoinSv,
                            ordinary.into(),
                            decode_password(TEST_PASSWORD),
                        )
                        .unwrap();
                    assert_eq!(signed.len(), 1);
                } else {
                    let checkpoint = kit.sign(if winner == 1 { &member } else { &public });
                    kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint.clone()).await.unwrap();
                    kit.rebuilt().require_funding_checkpoint(&kit.wallet, &checkpoint).await.unwrap();
                }
                assert_eq!(kit.signs.load(Ordering::SeqCst), 1);
                drop(lease);
            }
        }
    });
}

#[test]
fn test_sdk_funding_binding_and_actual_change_are_required_before_reservation_or_key_access() {
    block_on(async {
        for single in [false, true] {
            for corruption in ["payer", "target", "fee", "source", "locator"] {
                let kit = Kit::new(single).await;
                let mut binding = kit.binding(true, locator(BsvAddressBranch::Receive, 0), '1').await;
                let before = kit.store.get(kit.wallet.id.clone()).await.unwrap();
                match corruption {
                    "payer" => binding.payer_wallet_id = WalletId::PrivateKey(Chain::BitcoinSv, "1BitcoinEaterAddressDontSendf59kuE".into()),
                    "target" => {
                        let BsvHomeBsvMultisigFundingTarget::Public { request } = &mut binding.target else {
                            unreachable!()
                        };
                        request.amount = "0.0002".into();
                    }
                    "fee" => {
                        let mut funding: Funding = serde_json::from_str(&binding.funding_json).unwrap();
                        funding.fee_rate += 1;
                        binding.funding_json = serde_json::to_string(&funding).unwrap();
                    }
                    "source" => {
                        let mut funding: Funding = serde_json::from_str(&binding.funding_json).unwrap();
                        funding.addresses[0].address = "1BitcoinEaterAddressDontSendf59kuE".into();
                        binding.funding_json = serde_json::to_string(&funding).unwrap();
                    }
                    "locator" => {
                        let mut funding: Funding = serde_json::from_str(&binding.funding_json).unwrap();
                        funding.addresses[0].locator.index = 1;
                        binding.funding_json = serde_json::to_string(&funding).unwrap();
                    }
                    _ => unreachable!(),
                }
                assert!(kit.pool.reserve_funding(&kit.wallet, binding).await.is_err());
                assert_eq!(kit.store.get(kit.wallet.id.clone()).await.unwrap(), before);
                assert_eq!(kit.signs.load(Ordering::SeqCst), 0);
            }
        }
        let kit = Kit::new(false).await;
        let mut binding = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
        let mut funding: Funding = serde_json::from_str(&binding.funding_json).unwrap();
        funding.change = None;
        binding.funding_json = serde_json::to_string(&funding).unwrap();
        binding.review_json = serde_json::to_string(&review_funding(&funding).unwrap()).unwrap();
        assert_eq!(
            kit.pool.reserve_funding(&kit.wallet, binding).await.err().unwrap().to_string(),
            "New HD funding requires a real allocated change address"
        );
        assert_eq!(kit.signs.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn test_funding_save_failure_cannot_claim_durability_or_replace_the_authoritative_raw() {
    block_on(async {
        for single in [false, true] {
            let kit = Kit::new(single).await;
            let binding = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
            let lease = kit.pool.reserve_funding(&kit.wallet, binding.clone()).await.unwrap();
            let checkpoint = kit.sign(&binding);
            let before = kit.store.get(kit.wallet.id.clone()).await.unwrap();
            *kit.store.save_error.lock().unwrap() = Some(BsvHomeServiceError::Store {
                msg: "fixture durable write failed".into(),
            });
            assert!(kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint.clone()).await.is_err());
            assert!(kit.pool.require_funding_checkpoint(&kit.wallet, &checkpoint).await.is_err());
            assert_eq!(kit.store.get(kit.wallet.id.clone()).await.unwrap(), before);
            *kit.store.save_error.lock().unwrap() = None;
            kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint.clone()).await.unwrap();
            drop(lease);
            let mut changed = checkpoint.clone();
            changed.binding.review_json.push(' ');
            assert!(kit.pool.require_funding_checkpoint(&kit.wallet, &changed).await.is_err());
            let mut invalid = checkpoint.clone();
            invalid.raw = "00".into();
            assert!(kit.adopt(invalid).await.is_err());
            assert_eq!(kit.rebuilt().funding_checkpoints(&kit.wallet).await.unwrap(), vec![checkpoint]);
            assert_eq!(kit.signs.load(Ordering::SeqCst), 1);
        }
    });
}

#[test]
fn test_all_scope_legacy_adoption_is_atomic_and_preserves_corrupt_conflicting_or_unsaved_archives() {
    block_on(async {
        let deleted = WalletId::Single(Chain::BitcoinSv, "1BitcoinEaterAddressDontSendf59kuE".into()).id();
        for single in [false, true] {
            for failure in ["corrupt", "conflict", "save", "duplicate"] {
                let kit = Kit::new(single).await;
                let (record, _) = kit.legacy(locator(BsvAddressBranch::Receive, 0), '1', &deleted).await;
                let mut second = record.clone();
                match failure {
                    "corrupt" => second.json = "{".into(),
                    "conflict" => {
                        let mut value: serde_json::Value = serde_json::from_str(&record.json).unwrap();
                        let old = value["funding"].as_object().unwrap().keys().next().unwrap().clone();
                        let mut checkpoint = value["funding"].as_object_mut().unwrap().remove(&old).unwrap();
                        let scope = old.replace("collective-1", "collective-2");
                        checkpoint["scope"] = scope.clone().into();
                        checkpoint["walletId"] = "collective-2".into();
                        value["funding"].as_object_mut().unwrap().insert(scope, checkpoint);
                        second.json = serde_json::to_string(&value).unwrap();
                    }
                    "save" => {
                        *kit.store.save_error.lock().unwrap() = Some(BsvHomeServiceError::Store {
                            msg: "fixture recovery write failed".into(),
                        })
                    }
                    "duplicate" => {}
                    _ => unreachable!(),
                }
                let records = if failure == "conflict" {
                    let mut value: serde_json::Value = serde_json::from_str(&record.json).unwrap();
                    let other: serde_json::Value = serde_json::from_str(&second.json).unwrap();
                    value["funding"].as_object_mut().unwrap().extend(other["funding"].as_object().unwrap().clone());
                    vec![BsvHomeBsvFundingStoredState {
                        json: serde_json::to_string(&value).unwrap(),
                        ..record
                    }]
                } else if failure == "save" {
                    vec![record]
                } else if failure == "corrupt" {
                    vec![second]
                } else {
                    vec![record, second]
                };
                *kit.store.funding_states.lock().unwrap() = records.clone();
                let before = kit.store.get(kit.wallet.id.clone()).await.unwrap();
                assert!(kit.pool.reconcile_funding(&kit.wallet).await.is_err());
                assert_eq!(kit.store.get(kit.wallet.id.clone()).await.unwrap(), before);
                assert_eq!(*kit.store.funding_states.lock().unwrap(), records);
                assert_eq!(kit.signs.load(Ordering::SeqCst), 1);
            }
        }
    });
}

#[test]
fn test_public_payer_can_explicitly_discard_unsigned_member_after_signer_deletion_without_touching_signed_records() {
    block_on(async {
        for single in [false, true] {
            let kit = Kit::new(single).await;
            let mut binding = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
            let deleted = WalletId::Single(Chain::BitcoinSv, "1BitcoinEaterAddressDontSendf59kuE".into());
            binding.scope = format!("{ORIGIN}|{deleted}|collective-1");
            let lease = kit.pool.reserve_funding(&kit.wallet, binding.clone()).await.unwrap();
            drop(lease);
            let other = kit.rebuilt();
            assert_eq!(other.unsubmitted_funding(&kit.wallet).await.unwrap(), vec![binding.clone()]);
            let mut changed = binding.clone();
            changed.review_json.push(' ');
            assert!(other.release_funding_unsigned(&kit.wallet, &changed).await.is_err());
            assert_eq!(other.unsubmitted_funding(&kit.wallet).await.unwrap(), vec![binding.clone()]);
            other.release_funding_unsigned(&kit.wallet, &binding).await.unwrap();
            assert!(other.unsubmitted_funding(&kit.wallet).await.unwrap().is_empty());
            let next = kit.binding(true, locator(BsvAddressBranch::Receive, 0), '1').await;
            let lease = other.reserve_funding(&kit.wallet, next.clone()).await.unwrap();
            let checkpoint = kit.sign(&next);
            other.save_funding_signed(&kit.wallet, &lease.id, checkpoint.clone()).await.unwrap();
            drop(lease);
            other.release_funding_unsigned(&kit.wallet, &next).await.unwrap();
            assert_eq!(other.funding_checkpoints(&kit.wallet).await.unwrap(), vec![checkpoint]);
        }
    });
}

#[test]
fn test_only_matching_confirmed_chain_transaction_releases_funding_and_stale_archive_does_not_downgrade_outcome() {
    block_on(async {
        for single in [false, true] {
            let provider = Arc::new(BsvAddressProvider::default());
            let kit = Kit::with_provider(single, provider.clone()).await;
            let parent = bitcoin::Transaction {
                version: bitcoin::transaction::Version::TWO,
                lock_time: bitcoin::absolute::LockTime::ZERO,
                input: vec![bitcoin::TxIn {
                    previous_output: bitcoin::OutPoint::null(),
                    script_sig: bitcoin::ScriptBuf::from_bytes(vec![1, 7]),
                    sequence: bitcoin::Sequence::MAX,
                    witness: bitcoin::Witness::new(),
                }],
                output: vec![bitcoin::TxOut {
                    value: bitcoin::Amount::from_sat(50_000),
                    script_pubkey: BsvAddress::from_str(&kit.root()).unwrap().script_pubkey(),
                }],
            };
            let parent_raw = hex::encode(bitcoin::consensus::serialize(&parent));
            let parent_id = provider.add_raw_transaction(&parent_raw, true);
            assert_eq!(parent_id, parent.compute_txid().to_string());
            let mut binding = kit.binding(true, locator(BsvAddressBranch::Receive, 0), '1').await;
            let mut funding: Funding = serde_json::from_str(&binding.funding_json).unwrap();
            funding.utxos[0].transaction_id = parent_id;
            binding.review_json = serde_json::to_string(&review_funding(&funding).unwrap()).unwrap();
            binding.funding_json = serde_json::to_string(&funding).unwrap();
            let lease = kit.pool.reserve_funding(&kit.wallet, binding.clone()).await.unwrap();
            let checkpoint = kit.sign(&binding);
            kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint.clone()).await.unwrap();
            drop(lease);
            let mut accepted = checkpoint.clone();
            accepted.status = BsvHomeBsvMultisigFundingStatus::SyncFailed;
            accepted.error_code = Some("SYNC_FAILED".into());
            assert_eq!(kit.pool.record_funding_outcome(&kit.wallet, &accepted).await.unwrap(), accepted);
            assert_eq!(kit.pool.record_funding_outcome(&kit.wallet, &checkpoint).await.unwrap(), accepted);
            *kit.store.funding_states.lock().unwrap() = vec![archive(FundingCheckpoint::Current(checkpoint.clone()))];
            kit.pool.reconcile_funding(&kit.wallet).await.unwrap();
            assert_eq!(kit.pool.funding_checkpoints(&kit.wallet).await.unwrap(), vec![accepted.clone()]);
            assert_eq!(kit.pool.payer_state(&kit.wallet).await.unwrap().unwrap().1.unconfirmed_inputs().count(), 1);
            assert_eq!(provider.add_raw_transaction(&checkpoint.raw, false), checkpoint.transaction_id);
            kit.pool.refresh_reservations(&kit.wallet).await.unwrap();
            assert_eq!(kit.pool.payer_state(&kit.wallet).await.unwrap().unwrap().1.unconfirmed_inputs().count(), 1);
            assert_eq!(provider.add_raw_transaction(&checkpoint.raw, true), checkpoint.transaction_id);
            kit.pool.refresh_reservations(&kit.wallet).await.unwrap();
            assert_eq!(kit.pool.payer_state(&kit.wallet).await.unwrap().unwrap().1.unconfirmed_inputs().count(), 0);
            assert_eq!(kit.pool.funding_checkpoints(&kit.wallet).await.unwrap(), vec![accepted]);
            assert_eq!(provider.add_raw_transaction(&checkpoint.raw, false), checkpoint.transaction_id);
            kit.pool.refresh_reservations(&kit.wallet).await.unwrap();
            assert_eq!(kit.pool.payer_state(&kit.wallet).await.unwrap().unwrap().1.unconfirmed_inputs().count(), 1);
            assert_eq!(kit.signs.load(Ordering::SeqCst), 1);
        }
    });
}

#[test]
fn test_durable_funding_rows_reject_ambiguous_family_or_tampered_plan_without_repairing_storage() {
    block_on(async {
        for single in [false, true] {
            for corruption in ["null-kind", "duplicate-kind", "spend", "unsigned-confirmed", "raw", "cancellation"] {
                let kit = Kit::new(single).await;
                let binding = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
                let lease = kit.pool.reserve_funding(&kit.wallet, binding.clone()).await.unwrap();
                if corruption == "raw" {
                    let checkpoint = kit.sign(&binding);
                    kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint).await.unwrap();
                }
                drop(lease);
                let mut stored = kit.store.get(kit.wallet.id.clone()).await.unwrap().unwrap();
                let mut value: serde_json::Value = serde_json::from_str(&stored.value).unwrap();
                match corruption {
                    "null-kind" => value["kind"] = serde_json::Value::Null,
                    "duplicate-kind" => {}
                    "spend" => value["funding_reservations"][0]["spend"]["inputs"][0]["value"] = 49_999.into(),
                    "unsigned-confirmed" => value["funding_reservations"][0]["confirmed"] = true.into(),
                    "raw" => value["funding_reservations"][0]["signed"]["raw"] = "00".into(),
                    "cancellation" => value["cancelled_funding"] = serde_json::json!([{"operationId":binding.operation_id,"bindingHash":"x"}]),
                    _ => unreachable!(),
                }
                stored.value = if corruption == "duplicate-kind" {
                    format!("{{\"kind\":\"singleAddress\",\"kind\":\"singleAddress\",{}", &stored.value[1..])
                } else {
                    serde_json::to_string(&value).unwrap()
                };
                kit.store.states.lock().unwrap().insert(kit.wallet.id.clone(), stored.clone());
                assert!(kit.pool.funding_checkpoints(&kit.wallet).await.is_err());
                assert_eq!(kit.store.get(kit.wallet.id.clone()).await.unwrap(), Some(stored));
            }
        }
    });
}

#[test]
fn test_cancelled_operation_late_cas_never_overwrites_a_different_signing_operation_even_after_marker_eviction() {
    block_on(async {
        for single in [false, true] {
            let kit = Kit::new(single).await;
            let other = kit.rebuilt();
            let abandoned = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
            let winner = kit.binding(true, locator(BsvAddressBranch::Receive, 0), '1').await;
            *kit.store.hold_next_save.lock().unwrap() = true;
            let mut late = Box::pin(kit.pool.reserve_funding(&kit.wallet, abandoned.clone()));
            let waker = noop_waker();
            let mut task = TaskContext::from_waker(&waker);
            assert_pending(late.as_mut().poll(&mut task));
            let lease = other.reserve_funding(&kit.wallet, winner.clone()).await.unwrap();
            drop(late);
            other.release_funding_unsigned(&kit.wallet, &abandoned).await.unwrap();
            assert_eq!(other.unsubmitted_funding(&kit.wallet).await.unwrap(), vec![winner.clone()]);
            for _ in 0..65 {
                let mut cancelled = abandoned.clone();
                cancelled.operation_id = Uuid::new_v4().to_string();
                other.release_funding_unsigned(&kit.wallet, &cancelled).await.unwrap();
            }
            assert!(!kit.store.complete_held_save().unwrap());
            assert_eq!(other.unsubmitted_funding(&kit.wallet).await.unwrap(), vec![winner.clone()]);
            let checkpoint = kit.sign(&winner);
            other.save_funding_signed(&kit.wallet, &lease.id, checkpoint.clone()).await.unwrap();
            drop(lease);
            assert_eq!(other.funding_checkpoints(&kit.wallet).await.unwrap(), vec![checkpoint]);
            assert_eq!(kit.signs.load(Ordering::SeqCst), 1);
        }
    });
}

#[test]
fn test_two_pools_cannot_sign_the_same_ordinary_intent_and_real_keystore_failure_releases_for_retry() {
    block_on(async {
        for single in [false, true] {
            let kit = Kit::new(single).await;
            let other = kit.rebuilt();
            let hd = kit.hd().await;
            let input = payer_transfer_input(&kit.wallet, hd.as_ref());
            let lease = kit.pool.reserve(&kit.wallet, &input, "10000".into(), false).await.unwrap();
            let (release, gate) = futures::channel::oneshot::channel::<()>();
            let mut signing = Box::pin(async {
                gate.await.unwrap();
                kit.signs.fetch_add(1, Ordering::SeqCst);
                kit.keystore.sign(
                    kit.keystore.keystore_id(kit.wallet.id.id()),
                    Chain::BitcoinSv,
                    input.clone().into(),
                    decode_password("incorrect fixture password"),
                )
            });
            let waker = noop_waker();
            let mut task = TaskContext::from_waker(&waker);
            assert_pending(signing.as_mut().poll(&mut task));
            let error = other.reserve(&kit.wallet, &input, "10000".into(), false).await.err().unwrap();
            assert_eq!(error.to_string(), "BSV payment is already being signed");
            assert_eq!(kit.signs.load(Ordering::SeqCst), 0);
            let independent = Kit::with_private_key(true, Arc::new(TestAlienProvider::with_status(403)), [11; 32]).await;
            let mut independent_input = payer_transfer_input(&independent.wallet, None);
            let TransactionLoadMetadata::Bitcoin { utxos } = &mut independent_input.input.metadata else {
                unreachable!()
            };
            utxos[0].transaction_id = "2".repeat(64);
            let independent_lease = independent.pool.reserve(&independent.wallet, &independent_input, "10000".into(), false).await.unwrap();
            let signed = independent
                .keystore
                .sign(
                    independent.keystore.keystore_id(independent.wallet.id.id()),
                    Chain::BitcoinSv,
                    independent_input.into(),
                    decode_password(TEST_PASSWORD),
                )
                .unwrap();
            assert_eq!(signed.len(), 1);
            drop(independent_lease);
            release.send(()).unwrap();
            assert!(signing.await.is_err());
            kit.pool.release_unsigned(&kit.wallet, &lease.id).await.unwrap();
            drop(lease);
            let retry = other.reserve(&kit.wallet, &input, "10000".into(), false).await.unwrap();
            assert!(kit.pool.reserve(&kit.wallet, &input, "10000".into(), false).await.is_err());
            kit.signs.fetch_add(1, Ordering::SeqCst);
            let signed = kit
                .keystore
                .sign(kit.keystore.keystore_id(kit.wallet.id.id()), Chain::BitcoinSv, input.into(), decode_password(TEST_PASSWORD))
                .unwrap();
            assert_eq!(signed.len(), 1);
            assert_eq!(kit.signs.load(Ordering::SeqCst), 2);
            drop(retry);
        }
    });
}

#[test]
fn test_deleted_payer_unsigned_cleanup_blocks_a_foreign_late_reserve_or_signed_save_for_both_families() {
    block_on(async {
        for single in [false, true] {
            for signed_save in [false, true] {
                let kit = Kit::new(single).await;
                let other = kit.rebuilt();
                let binding = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
                let waker = noop_waker();
                let mut task = TaskContext::from_waker(&waker);
                if signed_save {
                    let lease = kit.pool.reserve_funding(&kit.wallet, binding.clone()).await.unwrap();
                    let checkpoint = kit.sign(&binding);
                    *kit.store.hold_next_save.lock().unwrap() = true;
                    let mut save = Box::pin(kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint));
                    assert_pending(save.as_mut().poll(&mut task));
                    drop(save);
                    drop(lease);
                } else {
                    *kit.store.hold_next_save.lock().unwrap() = true;
                    let mut reserve = Box::pin(kit.pool.reserve_funding(&kit.wallet, binding.clone()));
                    assert_pending(reserve.as_mut().poll(&mut task));
                    drop(reserve);
                }
                kit.wallets.delete_wallet(kit.wallet.id.clone()).await.unwrap();
                other.release_funding_unsigned(&kit.wallet, &binding).await.unwrap();
                assert!(!kit.store.complete_held_save().unwrap());
                let state = kit.store.get(kit.wallet.id.clone()).await.unwrap().unwrap();
                let value: serde_json::Value = serde_json::from_str(&state.value).unwrap();
                assert!(value.get("funding_reservations").is_none());
                assert_eq!(value["cancelled_funding"].as_array().unwrap().len(), 1);
                kit.wallets.add_wallet(kit.wallet.clone()).await.unwrap();
                assert!(other.reserve_funding(&kit.wallet, binding).await.is_err());
                assert_eq!(kit.store.get(kit.wallet.id.clone()).await.unwrap(), Some(state));
            }
        }
    });
}

#[test]
fn test_deleted_payer_cleanup_preserves_signed_rows_and_reports_real_storage_or_cas_failure() {
    block_on(async {
        for single in [false, true] {
            for failure in ["signed", "io", "cas"] {
                let kit = Kit::new(single).await;
                let binding = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
                let lease = kit.pool.reserve_funding(&kit.wallet, binding.clone()).await.unwrap();
                if failure == "signed" {
                    let checkpoint = kit.sign(&binding);
                    kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint).await.unwrap();
                }
                drop(lease);
                let before = kit.store.get(kit.wallet.id.clone()).await.unwrap().unwrap();
                kit.wallets.delete_wallet(kit.wallet.id.clone()).await.unwrap();
                match failure {
                    "signed" => {
                        kit.pool.release_funding_unsigned(&kit.wallet, &binding).await.unwrap();
                        assert_eq!(kit.store.get(kit.wallet.id.clone()).await.unwrap(), Some(before));
                    }
                    "io" | "cas" => {
                        if failure == "io" {
                            *kit.store.save_error.lock().unwrap() = Some(BsvHomeServiceError::Store { msg: "cleanup IO failed".into() });
                        } else {
                            *kit.store.conflict_on_save.lock().unwrap() = true;
                        }
                        assert!(kit.pool.release_funding_unsigned(&kit.wallet, &binding).await.is_err());
                        let after = kit.store.get(kit.wallet.id.clone()).await.unwrap().unwrap();
                        assert_eq!(after.value, before.value);
                        *kit.store.save_error.lock().unwrap() = None;
                        kit.pool.release_funding_unsigned(&kit.wallet, &binding).await.unwrap();
                        let value: serde_json::Value = serde_json::from_str(&kit.store.get(kit.wallet.id.clone()).await.unwrap().unwrap().value).unwrap();
                        assert!(value.get("funding_reservations").is_none());
                        assert_eq!(value["cancelled_funding"].as_array().unwrap().len(), 1);
                    }
                    _ => unreachable!(),
                }
            }
        }
    });
}

#[test]
fn test_unsigned_cleanup_still_rejects_a_present_wallet_with_changed_identity_even_for_signed_noop() {
    block_on(async {
        for signed in [false, true] {
            for change in ["type", "address", "path"] {
                let kit = Kit::new(false).await;
                let binding = kit.binding(false, locator(BsvAddressBranch::Receive, 0), '1').await;
                let lease = kit.pool.reserve_funding(&kit.wallet, binding.clone()).await.unwrap();
                if signed {
                    let checkpoint = kit.sign(&binding);
                    kit.pool.save_funding_signed(&kit.wallet, &lease.id, checkpoint).await.unwrap();
                }
                drop(lease);
                let before = kit.store.get(kit.wallet.id.clone()).await.unwrap();
                {
                    let mut wallets = kit.wallets.wallets.lock().unwrap();
                    let wallet = wallets.iter_mut().find(|wallet| wallet.id == kit.wallet.id).unwrap();
                    match change {
                        "type" => wallet.wallet_type = primitives::WalletType::View,
                        "address" => wallet.accounts[0].address = "1BitcoinEaterAddressDontSendf59kuE".into(),
                        "path" => wallet.accounts[0].derivation_path = "m/44'/236'/1'/0/0".into(),
                        _ => unreachable!(),
                    }
                }
                assert!(kit.pool.release_funding_unsigned(&kit.wallet, &binding).await.is_err());
                assert_eq!(kit.store.get(kit.wallet.id.clone()).await.unwrap(), before);
            }
        }
    });
}
