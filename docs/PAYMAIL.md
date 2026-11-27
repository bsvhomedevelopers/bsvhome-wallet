# WowPay Paymail

The iOS integration follows the supplied WowPay Paymail iOS contract dated 2026-10-08. It supports native BSV mainnet P2PKH payments, fixed-default-address registration, deactivation and reactivation. Address replacement, identity rotation, name transfer, token payment and P2P transaction delivery remain outside this implementation.

## Ownership

| Responsibility | Owner |
|---|---|
| Origin-scoped wallet login, raw Authorization token, secure session and wallet-change checks | `core/bsv_home_core/src/services/portal/mod.rs`, `client.rs`, `rules.rs` |
| Configuration, verified bindings, confirmed intent, serialized dual-sign submission and uncertain-result reconciliation | `core/bsv_home_core/src/services/portal/paymail.rs`, `paymail_model.rs`, `paymail_rules.rs` |
| Token-free public discovery, wallet-scoped sender context, delegated HTTPS payment destinations and strict P2PKH locking-script conversion | `core/bsv_home_core/src/services/portal/paymail_destination.rs` |
| Independent identity derivation and local signature recovery checks | `core/bsv_home_core/src/keystore/paymail.rs`, `core/crates/bsv_home_bsv/src/signer.rs` |
| Management loading, explicit confirmation and pending-verification presentation | `ios/Features/WalletTab/Sources/ViewModels/WowPayPaymailViewModel.swift`, `Scenes/WowPayPaymailScene.swift` |
| Dependency creation and environment configuration | `ios/BsvHome/Services/ServicesFactory.swift`, `ViewModelFactory.swift`, `Resources/Info.plist` |
| Verified names on the BSV asset overview and management page | `ios/BsvHome/Navigation/Assets/AssetNavigationView.swift`, `ios/Features/WalletTab/Sources/Scenes/WowPayPaymailScene.swift`; the aligned standalone Receive page has no Paymail card |
| Cohesive recipient validation, native BSV Paymail resolution and exact transfer construction | `core/bsv_home_core/src/services/send.rs` (`BsvHomeSendService`) |
| Input mapping and stale quote invalidation | `ios/Features/Transfer/Sources/ViewModels/WowPaySendViewModel.swift` |
| Simultaneous handle/address confirmation | `ios/Packages/PrimitivesComponents/Sources/Components/AddressListItemView.swift` and its `.nameAndAddress` model mode |

`WowPayPortalOrigin` and `WowPayPaymailTrustedOrigins` are application bundle configuration. The [2026-11-15 alignment](WOWPAY_ANDROID_ALIGNMENT.md) configures ordinary wallet management at `https://paymail.bsvhome.com/api/portal` with its public origin trusted separately from retained platform/MPC. Public discovery on that host returned HTTPS capabilities on the same origin; authenticated configuration compatibility remains an independent check. The management client accepts only the fixed HTTPS Portal origin; `publicBaseUrl` must also match the configured trusted public origins. Public resolution uses the entered Paymail domain and may follow a declared HTTPS capability to another host. Its ephemeral URLSession has no cookies, cache or Portal token, and rejects redirects. Both network paths retain normal system certificate validation and a 30-second resource timeout.

## Binding and signing invariants

Management opens as a pushed page from 我的, the BSV asset overview and the ordinary send form, using the native navigation bar, back action and Refresh item. Returning reloads the overview's verified status or retries the existing send draft's fee preparation. Registration and activation/deactivation still require their existing explicit confirmation sheet; changing the page presentation does not authorize a binding mutation.

The account identity remains the default BSV address. Mnemonic wallets derive it at `m/44'/236'/0'/0/0`; rotating the current HD receive address does not change the login or Paymail identity. Watch wallets cannot register or sign. The existing import capability does not currently advertise new BSV private-key imports; the keystore signing implementation supports an already stored BSV private key.

The independent identity scalar uses HMAC-SHA256 with the raw default BSV signing key, UTF-8 `WowPay/Paymail/Identity/v1` and a four-byte big-endian counter from 0 through 1023. Candidates must be nonzero, below the secp256k1 order and different from the source key. Keys stay inside the keystore, with zeroizing buffers; only compressed public keys and signatures leave it. Binding versions reuse Portal's exact unsigned-integer decoder for JSON integers or decimal-digit strings, then enforce the signed 64-bit range; floating, signed-string and overflowing representations fail validation.

Before signing, Core compares every binding challenge field against the confirmed intent, the trusted origin and the locally derived identity. UUID and raw expiry values are preserved. The binding challenge expiry must be in the future and at most 900 seconds away. Wallet-login challenge timing and device binding follow the separate [Portal authentication protocol](PORTAL_WALLET_AUTH.md#identity-and-signing). Both binding signatures use the original message, low-S compact Bitcoin Signed Message encoding and local recovery validation. Each management service is scoped to the originating screen's wallet ID before authenticated I/O; wallet identity and cancellation are checked again after asynchronous boundaries. Binding operations use the previous version and accept success only for matching fields at exactly previous version plus one.

POST `/bindings` is never automatically repeated. Immediately before submission, Core saves the public intent and identity public key in the existing secure store, scoped to Portal origin, wallet and default address. Any submission failure first rereads `/me`, then configuration. An exact resulting binding is reconciled success; a higher concurrent version is displayed as changed. An unchanged or unavailable result remains pending across refreshes, navigation and app restarts, with its receive handle hidden and further mutations blocked. Only a reconciled version, a detected concurrent change or a definitive rejection clears that checkpoint. Read failures clear the verified receive name and cannot become an empty, supposedly unregistered account. Malformed `/me` records fail the entire read rather than being silently skipped. Deactivation keeps the name reserved and suspends public lookup.

## Sending

The combined native send form holds one `BsvHomeSendService`, which composes the existing recipient, amount and Paymail destination owners. It resolves only BSV native coin Paymail inputs. Ordinary sending now uses the wallet-scoped `BsvHomePaymailDestinationService.newForWallet(provider:sender:)` constructor, with a `BsvHomePaymailService` scoped to the fixed originating wallet ID. Before public discovery, Core loads that wallet's binding, requires a verified snapshot and an active handle, and captures the wallet/workflow generation. A missing active sender produces typed `SenderRequired`; the native form displays static guidance and an explicit management action supplied by App navigation. Returning from management rechecks the same draft rather than accepting a previously resolved destination.

Discovery uses `https://{domain}/.well-known/bsvalias`. The declared HTTPS `paymentDestination` is called without a Portal token and includes the verified `senderHandle` plus `dt` in RFC3339 UTC with milliseconds and a trailing `Z`. The optional `amount` remains omitted by ordinary send; the actual transfer amount still comes from the existing exact integer amount service. Core rechecks the captured sender wallet/workflow after discovery and after destination lookup. The existing generic `new(provider:)` constructor remains available with its original sender-free request interface for retained consumers; it does not gain a wallet, a fabricated handle or new authentication behavior.

Core accepts only a 25-byte `76 a9 14 … 88 ac` locking script and derives its version-zero Base58Check address. It does not use a PKI identity key as a recipient key. An optional response `reference` is preserved in the typed destination result as metadata. It does not replace or validate the output address, authorize signing, prove delivery or establish payment success; ordinary transfer construction continues to use the normalized handle and validated address.

Recipient, amount, asset and account changes cancel and invalidate previous resolution and fee results. HTTP 429 and business `RATE_LIMITED` responses trigger a cooldown; the iOS transport also honors numeric and HTTP-date `Retry-After` headers. Confirmation displays the normalized original Paymail and the complete resolved BSV address alongside the existing network, amount and fee information. The existing authentication, simulation, signing and broadcast flow owns transaction submission; destination lookup alone never reports payment success.

## Verification

The wallet-scoped sender/time/reference addition is phase 4 implementation in progress. Full Native and runtime acceptance and the new deployment's non-empty active binding/public destination behavior remain pending at this checkpoint. The phase 3 live read verified configuration and empty `/me`, not an active sender, binding/deactivation or a funded Paymail transfer. Historical results below do not discharge those current gates; see [the alignment record](WOWPAY_ANDROID_ALIGNMENT.md#phase-4-implementation-in-progress).

Current deterministic sender tests change the wallet during both public discovery and final destination I/O. An intentional removal of the final check returned an old-wallet destination as `Ok` where `WalletChanged` was required; the [negative test](../ios/build/phase4-paymail-final-sender-guard-inverted.log) failed. The guard was restored and the [Portal suite](../ios/build/phase4-core-final-portal.log) then passed 148 tests, zero failed, with seven historical live tests ignored. The latest [Portal run](../ios/build/phase4-transfer-password/portal-final-core.log) passed 151 after adding three password-harness local rejection cases; eight live tests were ignored, comprising the seven historical tests and one new password lifecycle. They were not executed or passed, and the two Portal rounds are not additive. The [send suite](../ios/build/phase4-core-final-send.log) passed eight tests. Localization generation passed; the executed Native batch and subsequent focused navigation checks have not established a complete pass. None of this is non-empty live binding/destination or funded-transfer acceptance.

Deterministic Core tests cover identity vectors, compact signatures, challenge tampering, Origin and response validation, token isolation, binding versions, submission reconciliation and wallet races. iOS model tests cover explicit confirmation, pending-operation blocking, read failure, cancelled late responses, stale send quotes and handle/address presentation.

Executed on 2026-10-08:

| Directory | Command | Result |
|---|---|---|
| Repository root | `just ios localize` | Passed; generated 31 iOS localizations |
| Repository root | `just ios generate-core` | Passed; host, iOS Simulator and iOS device bindings/libraries generated |
| `core` | `just format` | Passed; formatting limited to task-owned changes |
| `core` | `just test bsv_home_core` | 873 passed, 0 failed, 3 gated live tests ignored |
| `core` | `just test bsv_home_hash` | 5 passed, 0 failed |
| `core` | `just test bsv_home_bsv` | 43 passed, 0 failed |
| `core` | `cargo clippy --locked -p bsv_home_core --all-features --all-targets -- -D warnings` | Passed |
| `core` | `cargo clippy --locked -p bsv_home_hash -p bsv_home_bsv --all-features -- -D warnings` | Passed |
| `ios` | `just test TransferTests` | Passed; Paymail native-coin restriction, stale-result cancellation and confirmation covered |
| `ios` | `just test WalletTabTests` | Passed; confirmation-before-signing, pending verification, failed refresh and cancellation covered |
| `ios` | `just test NativeProviderServiceTests` | Passed; redirect rejection and immediate cooldown without additional transport covered |
| `ios` | `just test PrimitivesComponentsTests` | Passed; full handle and address remain visible together |
| `ios` | `just build` | Passed; complete iOS Simulator application |
| Repository root | `git diff --check` | Passed |

The built application was installed and launched on an existing iPhone 17 / iOS 26.4 simulator. The BSV send screen accepted `alice@example.com` while the zero amount kept confirmation disabled; leaving the screen returned to Home. This smoke check did not submit a transaction or register the simulator user's Paymail. Interactive management-sheet lifecycle acceptance remains separate from the passing model tests and full application build.

The gated live lifecycle test uses an independently generated unfunded temporary wallet and random name. `PAYMAIL_LIVE_TESTS=1 cargo test --locked -p bsv_home_core --lib test_live_paymail_register_lookup_disable_restore_and_cleanup -- --ignored --nocapture`, run from `core`, passed the registration, refresh, public resolution, deactivation and same-identity reactivation assertions, but exited 101 after a final-cleanup refresh transport failure. The final deactivation submission had already returned success. The encrypted fixture was retained; rerunning the same command with `PAYMAIL_LIVE_FIXTURE_DIR` set to that retained directory performed cleanup-only recovery and passed (1 test, exit 0). That recovery read configuration and `/me`, verified the binding was disabled and both public PKI and payment-destination endpoints returned 404, then removed the fixture. It submitted no new binding or original challenge replay. This is a recovered live cleanup, not a passing result for the original command.

`cargo test --locked -p bsv_home_core --lib test_retained_paymail_fixture_restores_encrypted_identity_without_session_tokens` passed (1 test). The fixture is saved before mutation, uses a private directory and files, retains encrypted identity plus public reconciliation metadata, and excludes session tokens and plaintext key material. If a fixture is lost before cleanup, a deployment administrator must find and disable the test binding; the client cannot reconstruct the random identity.

Live tests use test password/store adapters rather than iOS Keychain or biometric hardware and never broadcast funds. Real-device biometric behavior, funded payment submission and third-party provider interoperability require separate acceptance checks. Shared Core impact on Android is unverified under the repository's iOS-only scope.
