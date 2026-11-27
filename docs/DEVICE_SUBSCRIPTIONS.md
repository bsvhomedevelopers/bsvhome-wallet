# Device and subscriptions

Every install registers a device with the backend and tells it which supported wallet addresses to watch. Wallet-scoped endpoints such as `/v2/devices/assets` return `404` until that wallet is subscribed. iOS prepares the requested wallet and required chain before a wallet-scoped request, while preserving device request signing.

## Device record

One record per install, defined by the shared `Device` primitive: push token, locale, currency, app version, push and price-alert flags, and `subscriptionsVersion`. The version is bumped when the subscription set changes and lets either side detect that the backend and the client disagree about what is subscribed.

```
GET    /v2/devices/is_registered
GET    /v2/devices
POST   /v2/devices
PUT    /v2/devices
GET    /v2/devices/subscriptions
POST   /v2/devices/subscriptions
DELETE /v2/devices/subscriptions
```

Request signing for all of them: [Device Authentication](./DEVICE_AUTHENTICATION.md).

## Sync flow

```mermaid
flowchart LR
    Start["App start"] --> Sync["One device sync<br/>concurrent callers join it"]
    Triggers["Wallet import · wallet delete<br/>currency · push token · wallet observer"] --> Check{"Local state<br/>changed?"]
    Check -- no --> Skip["No requests"]
    Check -- yes --> Sync
    Sync --> Subs["Reconcile subscriptions"]
    Subs --> SubscriptionCheckpoint["Record converged subscriptions"]
    SubscriptionCheckpoint --> Device["PUT device"]
    Device --> DeviceCheckpoint["Record published device metadata"]
```

Subscriptions are reconciled by diffing local wallets against `GET /v2/devices/subscriptions`: missing addresses are added per wallet grouped by chain, and wallets the backend still knows but the device no longer has are removed in full. Adding a wallet never removes another wallet's subscriptions.

`Chain::supports_bsv_home_backend()` is the shared eligibility rule for subscriptions, the subscription signature, remote wallet discovery, and BsvHome WebSocket prices. BSV and development testnets are currently excluded. BSV balances and transaction history use WhatsOnChain; Shasta uses development RPC and TronGrid. These queries do not depend on BsvHome subscriptions. A wallet containing only excluded accounts skips BsvHome asset, NFT, and wallet configuration queries; mixed wallets retain the BsvHome path for eligible accounts. BSV current prices use DefiLlama.

Registration comes first. If the device is not registered, or registration fails, nothing else may assume the device exists — the next sync retries it.

App start always checks the remote device record so a record lost by the backend is recreated without waiting for a local change. Other triggers use local divergence and avoid a request when nothing changed.

## Ordering

A fresh install imports a wallet and immediately asks for its assets, so the subscription must not race that request. The device API preflight validates the local wallet, registers the device, and ensures the required subscriptions. Asset history and fiat quotes prepare their asset's chain; wallet-wide queries require all eligible chains on that wallet. Preflight adds missing subscriptions only, never deletes subscriptions belonging to another request or wallet, and does not publish a full-install checkpoint. Missing local wallets, unavailable required chains, registration failures, and required subscription failures stop the request.

## Failure and lifetime

Full reconciliation and device metadata publication have separate checkpoints. A successful full subscription reconciliation stores its effective signature and version before the device metadata PUT. If the PUT fails, metadata remains dirty and retries without incrementing the subscription version again. Partial foreground preparation never replaces that full checkpoint. Synchronization is serialized; foreground requests can proceed once their required scope is ready even if unrelated wallets or device metadata remain dirty.

Wallet-wide history runs Shasta, BSV, and BsvHome sources independently, saves successful results, and reports source failures after all applicable sources run. The BsvHome cursor advances only after the complete BsvHome branch succeeds; initial transaction loading completes only after all applicable sources succeed. Failed requests preserve cached history. A stream transaction event independently attempts history and balance refresh. iOS catches each wallet observer synchronization failure inside its loop so later changes continue to synchronize.

Adding accounts invalidates initial history loading. Adding a BsvHome-supported chain also resets the wallet-wide BsvHome history and asset cursors and the asset, NFT, and wallet configuration completion flags, so a wallet previously using only local providers can discover the new chain's older data. Local BSV or testnet changes do not bump the BsvHome subscription version or rewind the BsvHome cursor.

## Shared ownership and platform triggers

[`BsvHomeDeviceService`](../core/bsv_home_core/src/services/device/mod.rs) owns registration, divergence detection, serialization of concurrent syncs, and the published-state checkpoint on both platforms. [`BsvHomeSubscriptionService`](../core/bsv_home_core/src/services/subscription/mod.rs) owns subscription reconciliation. Divergence is derived from the current device plus a deterministic wallet/account signature; mutation sites do not maintain a separate pending flag.

iOS constructs these Core services once. Wallet/account observers call `synchronizeIfNeeded()` for full reconciliation when local subscriptions change. The general device API client runs scoped preparation before wallet requests. The registration client used by the sync services has no preflight, which prevents recursive synchronization. Non-iOS integration is outside the current verification scope.

| Platform | Wallet-change trigger | Platform adapter |
|---|---|---|
| iOS | `SubscriptionsObserver`, consumed by `AppLifecycleService` | `BsvHomeCoreDevicePlatform` |
| Android | `DeviceObserverService` | `GemstoneDevicePlatform` |

## Rules

Changes on either platform must keep these true:

- A wallet-scoped request never runs for a wallet the backend was not told about.
- Concurrent synchronization is serialized; full-sync callers recheck local divergence.
- Full subscription checkpoints are recorded only after successful full reconciliation; device metadata checkpoints follow successful publication.
- Adding a wallet does not remove other wallets' subscriptions; deleting one removes its own.
- Outside app-start and wallet request preparation, nothing changed since the last sync means no requests.

Keep this document current in the same change when the sync triggers, the reconcile rules, or the platform mechanisms above change.

## Code map

- [Core device sync](../core/bsv_home_core/src/services/device/mod.rs)
- [Core subscription reconciliation](../core/bsv_home_core/src/services/subscription/mod.rs)
- [iOS device platform](../ios/Packages/BsvHomeCoreServices/Sources/Device/DevicePlatform.swift)
- [iOS wallet/account trigger](../ios/Packages/FeatureServices/AppService/AppLifecycleService.swift)
- [Android device platform](../android/data/services/gemstone/src/main/kotlin/com/gemwallet/android/data/services/gemstone/device/DevicePlatform.kt)
- [Android wallet trigger](../android/data/services/gemstone/src/main/kotlin/com/gemwallet/android/data/services/gemstone/device/DeviceObserverService.kt)
