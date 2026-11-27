use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use bsv_home_bsv::cold_wallet::{Pairing, decode_json};
use bsv_home_derivation::derive_wallet_id_from_account;
use futures::executor::block_on;
use primitives::{BsvAddressBranch, Wallet, WalletSource, WalletType};
use serde_json::{Value, json};

use super::password::BsvHomeOfflineSigningPasswordService;
use super::testkit::Context;
use super::{BsvHomeBsvColdWalletActionIntent, BsvHomeBsvColdWalletRole, BsvHomeBsvColdWalletService, BsvHomeOfflineSigningPasswordDomain};
use crate::gateway::{BsvHomeGateway, EmptyPreferences};
use crate::services::preferences::testkit::MemoryPreferencesStore;
use crate::services::wallet::addresses::{BsvHomeBsvAddressPool, BsvHomeBsvAddressStatus};
use crate::services::wallet::testkit::MemoryKeystorePassword;

fn unified(context: &Context) -> BsvHomeBsvColdWalletService {
    BsvHomeBsvColdWalletService::unified(
        context.wallets.clone(),
        context.addresses.clone(),
        Arc::new(BsvHomeGateway::new(
            context.provider.clone(),
            Arc::new(MemoryPreferencesStore::default()),
            Arc::new(EmptyPreferences),
        )),
        context.keystore.clone(),
        context.store.clone(),
        context.signer.clone(),
    )
}

#[test]
fn test_unified_cold_signer_network_actions_reject_before_provider_authentication_or_key() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        *context.signer.authentication_count.lock().unwrap() = 0;
        let service = unified(&context);
        assert_eq!(service.snapshot(wallet.id.clone()).await.unwrap().role, BsvHomeBsvColdWalletRole::Signer);
        assert!(service.refresh(wallet.id.clone()).await.is_err());
        let scope = service
            .begin_action(
                wallet.id.clone(),
                BsvHomeBsvColdWalletActionIntent::Prepare {
                    recipient: wallet.accounts[0].address.clone(),
                    amount: 1,
                },
            )
            .unwrap();
        assert!(service.prepare(wallet.id.clone(), wallet.accounts[0].address.clone(), 1, scope).await.is_err());
        let scope = service
            .begin_action(
                wallet.id.clone(),
                BsvHomeBsvColdWalletActionIntent::Broadcast {
                    commitment: "not-a-review".into(),
                },
            )
            .unwrap();
        assert!(service.broadcast(wallet.id.clone(), "not-a-review".into(), scope).await.is_err());
        assert!(service.import_signed(wallet.id, "{}".into()).await.is_err());
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
        assert_eq!(*context.signer.authorization_count.lock().unwrap(), 0);
        assert_eq!(*context.signer.authentication_count.lock().unwrap(), 0);
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
    });
}

#[test]
fn test_unified_cold_candidates_exclude_unmarked_hot_wallet_and_qr_import_cannot_silently_mark_it() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let request = context.request(wallet.id.clone(), 75000).await;
        context.store.records.lock().unwrap().remove(&wallet.id);
        *context.signer.authentication_count.lock().unwrap() = 0;
        let service = unified(&context);
        assert_eq!(service.wallet_candidates().await.unwrap().len(), 0);
        assert!(service.import_request(wallet.id.clone(), request).await.is_err());
        let scope = service
            .begin_action(
                wallet.id.clone(),
                BsvHomeBsvColdWalletActionIntent::Sign {
                    commitment: "not-a-review".into(),
                },
            )
            .unwrap();
        assert!(service.sign(wallet.id.clone(), "not-a-review".into(), scope).await.is_err());
        assert!(!context.store.records.lock().unwrap().contains_key(&wallet.id));
        assert_eq!(*context.signer.authorization_count.lock().unwrap(), 0);
        assert_eq!(*context.signer.authentication_count.lock().unwrap(), 0);
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_unified_cold_candidate_metadata_is_not_silently_downgraded_when_its_state_is_corrupt_or_unreadable() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        context.store.records.lock().unwrap().get_mut(&wallet.id).unwrap().value = "{}".into();
        let service = unified(&context);
        let candidates = service.wallet_candidates().await.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].wallet.id, wallet.id);
        assert_eq!(candidates[0].role, BsvHomeBsvColdWalletRole::Signer);
        assert!(service.snapshot(wallet.id).await.is_err());
        *context.store.read_fail.lock().unwrap() = true;
        assert!(service.wallet_candidates().await.is_err());
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
    });
}

#[test]
fn test_unified_cold_actual_signing_uses_its_own_password_domain_and_has_no_network() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let request = context.request(wallet.id.clone(), 75000).await;
        let service = unified(&context);
        let reviewed = service.import_request(wallet.id.clone(), request).await.unwrap();
        let scope = service
            .begin_action(
                wallet.id.clone(),
                BsvHomeBsvColdWalletActionIntent::Sign {
                    commitment: reviewed.review.commitment.clone(),
                },
            )
            .unwrap();
        assert!(service.sign(wallet.id.clone(), reviewed.review.commitment, scope).await.unwrap().signed_json.is_some());
        let current = BsvHomeOfflineSigningPasswordService::new(
            context.signer.session.clone(),
            context.signer.password_store.clone(),
            wallet.clone(),
            BsvHomeOfflineSigningPasswordDomain::UnifiedCold,
        )
        .unwrap();
        let legacy = BsvHomeOfflineSigningPasswordService::new(
            context.signer.session.clone(),
            context.signer.password_store.clone(),
            wallet,
            BsvHomeOfflineSigningPasswordDomain::LegacyCold,
        )
        .unwrap();
        assert!(current.status().await.unwrap().configured);
        assert!(!legacy.status().await.unwrap().configured);
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 1);
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
    });
}

#[test]
fn test_unified_cold_observer_import_and_candidates_use_real_public_role_without_a_local_secret() {
    block_on(async {
        let offline = Context::new();
        let signer = offline.create().await;
        let pairing = offline.service.pairing(signer.id).await.unwrap();
        let online = Context::new();
        let service = unified(&online);
        let observer = service.import_observer("Public observer".into(), pairing, service.capture_generation()).await.unwrap();
        assert_eq!(observer.role, BsvHomeBsvColdWalletRole::Observer);
        let candidates = service.wallet_candidates().await.unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].wallet.id, observer.wallet.id);
        assert_eq!(candidates[0].role, BsvHomeBsvColdWalletRole::Observer);
        assert!(!online.keystore.has_stored_wallets().unwrap());
        assert_eq!(online.provider.requested_paths(), Vec::<String>::new());
        assert_eq!(*online.signer.signing_count.lock().unwrap(), 0);
    });
}

#[test]
fn test_cold_native_public_scope_fixture_uses_the_actual_wallet_id_and_pool_codec() {
    let pairing: Pairing = decode_json(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/bsv-cold-native-pairing.json"))).unwrap();
    pairing.validate().unwrap();
    let account = pairing.account().unwrap();
    let wallet = Wallet {
        id: derive_wallet_id_from_account(&account, WalletType::Single).unwrap(),
        external_id: None,
        name: "Public cold action scope fixture".into(),
        index: 0,
        wallet_type: WalletType::Single,
        accounts: vec![account.clone()],
        is_pinned: false,
        image_url: None,
        source: WalletSource::Import,
    };
    let state = BsvHomeBsvAddressPool::initial_public_record(&wallet, &account).unwrap().unwrap();
    assert_eq!(state.revision, 0);
    if env::var("BSVHOME_EXPORT_COLD_PUBLIC_SCOPE_FIXTURE").ok().as_deref() == Some("1") {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ios/build/phase7");
        fs::create_dir_all(&directory).unwrap();
        let payload = json!({"formatVersion":1,"wallet":wallet,"addressRevision":state.revision,"addressValue":state.value});
        fs::write(directory.join("bsv-cold-native-action-scope.json"), serde_json::to_vec_pretty(&payload).unwrap()).unwrap();
    }
}

#[test]
fn test_unified_receive_shares_ordinary_selection_addition_and_rebuilt_pool_without_changing_legacy_selection() {
    block_on(async {
        let offline = Context::new();
        let signer = offline.create().await;
        let pairing = offline.service.pairing(signer.id).await.unwrap();
        let context = Context::new();
        let service = unified(&context);
        let observer = service
            .import_observer("Public observer".into(), pairing.clone(), service.capture_generation())
            .await
            .unwrap();
        let wallet_id = observer.wallet.id.clone();
        let import_authentications = *context.signer.authentication_count.lock().unwrap();
        assert_eq!(import_authentications, 1);
        let legacy = context.service.receive(wallet_id.clone()).await.unwrap();
        let ordinary = context.addresses.select_address(wallet_id.clone(), 3).await.unwrap();
        let received = service.receive(wallet_id.clone()).await.unwrap();
        let selected = received.selected_index;
        assert_eq!(selected, ordinary.selected_index);
        assert_eq!(
            received.addresses.iter().find(|record| record.is_current).map(|record| record.address.clone()),
            ordinary.current_address
        );
        assert_eq!(service.snapshot(wallet_id.clone()).await.unwrap().addresses, received);
        let selected = service.select_address(wallet_id.clone(), 7).await.unwrap();
        let ordinary = context.addresses.snapshot(wallet_id.clone()).await.unwrap().unwrap();
        let selected_index = selected.selected_index;
        assert_eq!(selected_index, Some(7));
        assert_eq!(ordinary.selected_index, selected_index);
        assert_eq!(
            ordinary.current_address,
            selected.addresses.iter().find(|record| record.is_current).map(|record| record.address.clone())
        );
        let added = service.add_address(wallet_id.clone()).await.unwrap();
        let added_index = added.selected_index;
        assert_eq!(added_index, Some(10));
        let gateway = Arc::new(BsvHomeGateway::new(
            context.provider.clone(),
            Arc::new(MemoryPreferencesStore::default()),
            Arc::new(EmptyPreferences),
        ));
        let rebuilt = BsvHomeBsvAddressPool::new(
            gateway,
            context.wallet_store.clone(),
            context.wallet_store.bsv_addresses.clone(),
            context.keystore.clone(),
            Arc::new(MemoryKeystorePassword::default()),
        );
        let ordinary = rebuilt.snapshot(wallet_id.clone()).await.unwrap().unwrap();
        assert_eq!((ordinary.receive_count, ordinary.selected_index), (11, Some(10)));
        assert_eq!(
            ordinary.current_address,
            added.addresses.iter().find(|record| record.is_current).map(|record| record.address.clone())
        );
        assert_eq!(context.service.receive(wallet_id.clone()).await.unwrap(), legacy);
        assert_eq!(service.pairing(wallet_id.clone()).await.unwrap(), pairing);
        assert_eq!(context.wallets.current_wallet_id().unwrap(), Some(wallet_id));
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
        assert_eq!(*context.signer.authentication_count.lock().unwrap(), import_authentications);
        assert_eq!(*context.signer.authorization_count.lock().unwrap(), 0);
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
        assert!(!context.keystore.has_stored_wallets().unwrap());
    });
}

#[test]
fn test_unified_receive_without_a_current_address_keeps_the_real_pending_and_public_issued_list() {
    block_on(async {
        let context = Context::new();
        let wallet = context.create().await;
        let request = context.request(wallet.id.clone(), 75000).await;
        let pending = context.service.import_request(wallet.id.clone(), request).await.unwrap();
        let authentication_count = *context.signer.authentication_count.lock().unwrap();
        let before_cold = context.store.records.lock().unwrap().get(&wallet.id).unwrap().clone();
        let before_pool = {
            let mut records = context.wallet_store.bsv_addresses.states.lock().unwrap();
            let record = records.get_mut(&wallet.id).unwrap();
            let mut state: Value = serde_json::from_str(&record.value).unwrap();
            state["receive_count"] = Value::Null;
            state["selected_index"] = Value::Null;
            for record in state["records"].as_array_mut().unwrap() {
                if record["locator"]["branch"] == serde_json::to_value(BsvAddressBranch::Receive).unwrap() {
                    record["status"] = serde_json::to_value(BsvHomeBsvAddressStatus::Used).unwrap();
                }
            }
            record.value = serde_json::to_string(&state).unwrap();
            record.clone()
        };
        let ordinary = context.addresses.snapshot(wallet.id.clone()).await.unwrap().unwrap();
        assert_eq!(ordinary.current_address, None);
        let service = unified(&context);
        let snapshot = service.snapshot(wallet.id.clone()).await.unwrap();
        let selected = snapshot.addresses.selected_index;
        assert_eq!(selected, None);
        assert_eq!(snapshot.pending, Some(pending.clone()));
        assert_eq!(snapshot.addresses.addresses.len(), ordinary.receive_count as usize);
        assert_eq!(snapshot.addresses.addresses.iter().filter(|record| record.is_current).count(), 0);
        assert_eq!(serde_json::to_value(&snapshot.wallet).unwrap(), serde_json::to_value(&wallet).unwrap());
        assert_eq!(context.wallet_store.bsv_addresses.states.lock().unwrap().get(&wallet.id).unwrap(), &before_pool);
        assert_eq!(context.store.records.lock().unwrap().get(&wallet.id).unwrap(), &before_cold);
        let chosen = service.select_address(wallet.id.clone(), 3).await.unwrap();
        let selected = chosen.selected_index;
        assert_eq!(selected, Some(3));
        assert_eq!(context.addresses.snapshot(wallet.id.clone()).await.unwrap().unwrap().selected_index, Some(3));
        assert_eq!(service.snapshot(wallet.id).await.unwrap().pending, Some(pending));
        assert_eq!(context.provider.requested_paths(), Vec::<String>::new());
        assert_eq!(*context.signer.authentication_count.lock().unwrap(), authentication_count);
        assert_eq!(*context.signer.authorization_count.lock().unwrap(), 0);
        assert_eq!(*context.signer.signing_count.lock().unwrap(), 0);
    });
}
