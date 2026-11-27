use crate::services::bsv_multisig::testkit::ScopedTestActions;
use std::sync::Arc;

use futures::executor::block_on;
use serde_json::Value;

use super::RemotePaymailSettings;
use crate::alien::AlienHttpMethod;
use crate::services::bsv_multisig::testkit::Device;

fn settings(device: &Device, alias: &str, active: bool, can_manage: bool, version: i64) -> RemotePaymailSettings {
    RemotePaymailSettings {
        handle: format!("{alias}@paymail.example"),
        alias: alias.into(),
        domain: "paymail.example".into(),
        active,
        can_manage,
        version,
        configuration_hash: device.service.remote_wallet("team-wallet").unwrap().configuration_hash.unwrap(),
    }
}

async fn trusted(device: &Device) {
    device.service.load().await.unwrap();
    device.service.approve_configuration("team-wallet".into(), 7).await.unwrap();
}

#[test]
fn test_collective_paymail_read_and_update_bind_configuration_version_and_exact_fields() {
    block_on(async {
        let device = Device::new(1);
        trusted(&device).await;
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/paymail/settings", settings(&device, "team", true, true, 4));
        let original = device.service.load_paymail_settings("team-wallet".into()).await.unwrap();
        device
            .provider
            .override_post_data("/auth/multisig/wallets/team-wallet/paymail/settings", settings(&device, "new-team", false, true, 5));
        let updated = device.service.scoped_update_paymail_settings(original.clone(), " NEW-TEAM ".into(), false).await.unwrap();
        assert_eq!(updated.handle, "new-team@paymail.example");
        assert!(!updated.active);
        let post = device.provider.posts().last().unwrap().clone();
        let body: Value = serde_json::from_slice(post.body.as_ref().unwrap()).unwrap();
        assert_eq!(body.get("alias"), Some(&Value::String("new-team".into())));
        assert_eq!(body.get("active"), Some(&Value::Bool(false)));
        assert_eq!(body.get("expectedVersion"), Some(&Value::from(4)));
        assert_eq!(body.get("configurationHash"), Some(&Value::String(original.configuration_hash.clone())));
        assert_eq!(body.as_object().unwrap().len(), 4);
        let count = device.provider.posts().len();
        assert!(device.service.scoped_update_paymail_settings(original, "old".into(), true).await.is_err());
        assert_eq!(device.provider.posts().len(), count);
    });
}

#[test]
fn test_collective_paymail_legacy_fallback_is_only_for_missing_settings_and_cannot_manage() {
    block_on(async {
        let device = Device::new(1);
        trusted(&device).await;
        device.provider.override_error("/auth/multisig/wallets/team-wallet/paymail/settings", "NOT_FOUND");
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/paymail", settings(&device, "multisig-team-wallet", true, false, 0));
        let legacy = device.service.load_paymail_settings("team-wallet".into()).await.unwrap();
        assert!(!legacy.can_manage);
        assert_eq!(legacy.version, 0);
        let posts = device.provider.posts().len();
        assert!(device.service.scoped_update_paymail_settings(legacy, "new".into(), false).await.is_err());
        assert_eq!(device.provider.posts().len(), posts);
        device.provider.override_error("/auth/multisig/wallets/team-wallet/paymail/settings", "FORBIDDEN");
        let before = device.provider.paths().len();
        assert!(device.service.load_paymail_settings("team-wallet".into()).await.is_err());
        assert_eq!(device.provider.paths().len(), before + 1);
    });
}

#[test]
fn test_collective_paymail_write_errors_are_not_replayed_and_require_a_fresh_read() {
    block_on(async {
        let device = Device::new(1);
        trusted(&device).await;
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/paymail/settings", settings(&device, "team", true, true, 4));
        let original = device.service.load_paymail_settings("team-wallet".into()).await.unwrap();
        device
            .provider
            .override_error("POST /auth/multisig/wallets/team-wallet/paymail/settings", "MULTISIG_SERVICE_UNAVAILABLE");
        let before = device.provider.posts().len();
        assert!(device.service.scoped_update_paymail_settings(original.clone(), "changed".into(), true).await.is_err());
        assert_eq!(device.provider.posts().len(), before + 1);
        assert!(device.service.scoped_update_paymail_settings(original, "changed".into(), true).await.is_err());
        assert_eq!(device.provider.posts().len(), before + 1);
    });
}

#[test]
fn test_collective_paymail_rejects_foreign_configuration_bad_domain_and_stale_wallet_before_writing() {
    block_on(async {
        let device = Device::new(1);
        trusted(&device).await;
        for change in ["hash", "handle", "domain", "version"] {
            let mut remote = settings(&device, "team", true, true, 4);
            match change {
                "hash" => remote.configuration_hash = "a".repeat(64),
                "handle" => remote.handle = "different@paymail.example".into(),
                "domain" => {
                    remote.domain = "bad..example".into();
                    remote.handle = "team@bad..example".into();
                }
                "version" => remote.version = -1,
                _ => unreachable!(),
            }
            device.provider.override_data("/auth/multisig/wallets/team-wallet/paymail/settings", remote);
            assert!(device.service.load_paymail_settings("team-wallet".into()).await.is_err());
        }
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/paymail/settings", settings(&device, "team", true, true, 4));
        let original = device.service.load_paymail_settings("team-wallet".into()).await.unwrap();
        let before = device.provider.posts().len();
        for alias in ["", ".team", "team-", "a@b", "a b", "équipe"] {
            assert!(device.service.scoped_update_paymail_settings(original.clone(), alias.into(), true).await.is_err());
        }
        device.provider.wallets.lock().unwrap()[0].configuration_hash = Some("b".repeat(64));
        assert!(device.service.scoped_update_paymail_settings(original.clone(), "new".into(), true).await.is_err());
        assert_eq!(device.provider.posts().len(), before);
        device.service.cancel();
        assert!(device.service.load_paymail_settings("team-wallet".into()).await.is_err());
    });
}

#[test]
fn test_collective_paymail_response_after_cancellation_cannot_publish_or_repeat_the_write() {
    block_on(async {
        let device = Device::new(1);
        trusted(&device).await;
        device
            .provider
            .override_data("/auth/multisig/wallets/team-wallet/paymail/settings", settings(&device, "team", true, true, 4));
        let original = device.service.load_paymail_settings("team-wallet".into()).await.unwrap();
        device
            .provider
            .override_post_data("/auth/multisig/wallets/team-wallet/paymail/settings", settings(&device, "changed", true, true, 5));
        let auth = device.auth.clone();
        *device.provider.on_request.lock().unwrap() = Some(Arc::new(move |target| {
            if target.method == AlienHttpMethod::Post && target.url.ends_with("/paymail/settings") {
                auth.cancel();
            }
        }));
        let before = device.provider.posts().len();
        assert!(device.service.scoped_update_paymail_settings(original.clone(), "changed".into(), true).await.is_err());
        assert_eq!(device.provider.posts().len(), before + 1);
        assert_eq!(device.service.cache().unwrap().paymail_settings["team-wallet"], original);
        assert!(device.service.load_paymail_settings("team-wallet".into()).await.is_err());
        assert_eq!(device.provider.posts().len(), before + 1);
    });
}
