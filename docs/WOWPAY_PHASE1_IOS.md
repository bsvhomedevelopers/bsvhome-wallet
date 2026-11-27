# WowPay phase one iOS integration

The 2026-11-15 user-approved [Android alignment](WOWPAY_ANDROID_ALIGNMENT.md) supersedes this document's production entry-point scope: platform/Passkey/MPC/independent identity extensions are to be hidden, with implementation and data retained. The records below describe the earlier integration and do not certify current Android parity or acceptance. Preserve its independent environment and security/storage boundaries while applying the newer scope.

## Contract and ownership

The user-approved phase-one scope is BSV with Home, Assets and Profile navigation. It supersedes the earlier four-tab BSV/TRX/USDT presentation. Existing wallets, encrypted keystores, accounts, assets, contacts and transaction records remain stored. Core's `BsvHomeRuntimeEnvironment::WowPay` supplies the BSV policy; the apps do not maintain another chain list. Other Core environments remain available to their consumers.

The reference is the supplied 2026-10-31 iOS integration document and Android commit `32047f8a5441a5674db974331269a5b90060a257`. The reference repository is read-only after the explicitly requested branch switch. No Android generation, source modification or build is part of this delivery. Shared Core changes may affect other consumers; those effects are unverified.

The current visual reference is the [prototype package supplied on 2026-11-15](WOWPAY_UI_REFACTOR.md#current-prototype--2026-11-15). That replacement updates prototype links and documentation only. Its collective-wallet and signing-identity proposals do not amend this phase-one contract, enable its retained legacy/demo pages or establish native visual acceptance.

`core/bsv_home_core/src/services/portal/platform` owns platform account transport, session leases, email, system Passkey orchestration, profile, devices, account closure/reopening, email changes, public HD accounts and independent identity recovery. iOS implements the system authenticator, Keychain and P-256 device proof ports. Ordinary BSV wallet authentication remains a separate service and token namespace. Session errors from the HD, alternative-identity and MPC pages refresh one account projection from Core, including when another account read is busy. Fast Core cancellation completes synchronously before starting another challenge, so a queued old cancellation cannot invalidate a later login.

`core/bsv_home_core/src/services/portal/mpc` owns the public local wallet directory, task routing, backup proofs, immutable payment intents, independent review/verification, Paymail binding and recovery/epoch commit. The directory is scoped to the complete environment and account. Its public wallet metadata, pending intents, proposals and signed transaction receipts currently share one encrypted Keychain record, limited to 4,000,000 bytes. Reaching this limit fails closed without discarding history. This bounded storage needs a separate migration before supporting an unbounded transaction archive. Email login does not discover every historical MPC wallet. Imported backup context supplies the original wallet identifier for recovery.

`core/crates/bsv_home_bsv/src/mpc.rs` owns transaction review and complete signed-transaction verification. The two exported bindings are `bsv_mpc_review` and `bsv_mpc_verify_signed`. They consume the fixed proposal and expected recipient/amount/maximum fee, verify complete previous transactions and all inputs, and reject unsupported scripts, duplicate inputs, changed outputs, invalid DER, high-S signatures and any hash type other than `0x41`.

## Native A and protected material

`ios/Packages/WowPayMpc/Native` is an independent Rust workspace. Its DKLs23 upstream revision, cryptographic dependency versions, lockfile and third-party notices remain separate from BsvHomeCore. `ios/scripts/generate-mpc.sh` builds ARM64 device and simulator static libraries. The `BsvHome` target links the matching archive, and only the daily device flow initializes MPC services. The C ABI provides versioned open/exchange/close/free operations with explicit allocation ownership and panic boundaries. Swift serializes native access outside the main thread. Native file locks enforce exclusivity across actors and processes.

Every wallet's seal key is non-synchronizing, device-bound Keychain material. A ciphertext state with a missing seal key cannot create a replacement key. A files and candidate epochs use protected, atomic, backup-excluded storage. The installation identifier is public metadata; reinstalling creates a new device identity rather than reusing an old device proof. Device proof uses P-256 SPKI DER/Base64URL and SHA256withECDSA over the original UTF-8 message text. Key creation and signing run serially within its background actor. The `privateKeyUsage` access-control flag is supplied only for Secure Enclave keys, following [Apple’s flag contract](https://developer.apple.com/documentation/security/secaccesscontrolcreateflags/privatekeyusage). System Keychain integration tests use the application-hosted `BsvHomeTests` target.

Backup generation alone does not activate a wallet. Exported bytes contain only the encrypted backup object; the recovery key stays in temporary protected UI state. Actual file saving, a re-entered recovery key, local decryption/context verification and a remote backup proof precede ACTIVE. File receipts and UI callbacks must remain bound to their original account, wallet, epoch, commitment and presentation revision. Leaving the complete backup route clears recovery material even while the system export sheet is active; both user cancellation and a detached picker are safe, and a late save cannot authorize a new wallet or epoch.

On a device configured for cold wallet use, `BsvHome` opens a separate root with protected storage and a provider that rejects network requests. `OfflineIdentity` manages an independent P-256 identity factor from that root. Its public challenge and proof files and PBKDF2/AES-GCM backup format follow the phase-one interoperability contract. The daily device's identity recovery flow accepts only public keys and proofs. BSV signing and MPC recovery use their own keys and recovery rules. [Cold wallet setup](BSV_COLD_WALLET.md#device-setup-and-import)

## Payments and recovery

Before the first prepare request, Core stores the original draft, resolved address and idempotency key. Retrying an uncertain prepare uses those original values. Paymail resolution never silently changes the target of an existing pending preparation. Confirmation binds the complete proposal commitment; a wallet or draft change invalidates the old presentation and authorization.

Signing and broadcasting are separate user actions and separate Passkey grants. Signed transactions are independently verified and retained with the original transfer ID/TXID. PREPARED, SIGNING, SIGNED, SUBMITTING, SUBMITTED, CONFIRMED, UNKNOWN, CONFLICTED and terminal rejection/cancellation states remain distinct. Closing a page or suspending the app does not declare a payment failed or cancelled. Pending transfers remain queryable across restarts.

Restoring A enters RESTORED_REFRESH_REQUIRED. The original trusted Passkey, new device proof, business review, notification and server waiting conditions still apply. Refresh preserves the original public key/address. A candidate epoch requires a newly saved backup and proof. COMMITTING is retained until remote completion; only then can local A commit. Cancellation is limited to uncommitted states and cannot roll back B after commit.

Task preparation itself has no supplied idempotency or list endpoint. If that write loses its receipt before a task ID is known, Core preserves an explicit uncertain-prepare checkpoint and blocks automatic creation of another task. A server-side reconciliation contract is required to resolve this case; the client does not invent an ID or erase the checkpoint.

## Implemented phase map

| Phase | Local implementation | Acceptance still requiring external systems |
| --- | --- | --- |
| I0 | Pinned reference, typed routes and ownership contracts | Current server Controller/DTO snapshot |
| I1 | V2 account, email, system Passkey adapter, Keychain session, profile, devices and credentials | Verified RP, AASA, origin, test email and device channel |
| I2 | Existing BSV creation/import/custom paths, send/receive, scanner and shared contacts | Physical-device regression |
| I3 | Separate native A, protected state, C ABI, all-input BSV review/verification | Independent B executable and cross-party interoperability |
| I4 | DKG orchestration, public identity checks, protected export receipt and backup proof | Complete A/B DKG and real server activation |
| I5 | Balances/history, persisted fixed drafts, signing, broadcast authorization and uncertain-state continuation | Real chain provider/server lifecycle; no mainnet broadcast performed |
| I6 | Separate traditional/MPC Paymail bindings, public resolution and preserved send drafts | V2 business records and configured public Paymail service |
| I7 | Import, device proof, review timeline, refresh, new backup and retained COMMITTING | Business review, waiting and B commit integration |
| I8 | Request/cancel/wait/final closure and two-factor reopening | Real server state transitions |
| I9 | BSV three-tab navigation, separate local/MPC selection, existing multisig and cold wallet flow | Native visual comparison remains pending; the current reference package is available, while its collective-wallet proposals remain outside this implementation record |
| I10 | Email change, verified public HD enrollment/scanning, alternative identity and cold-only offline factor | Server policy, two reviewers and completed recovery |

These entries describe source integration rather than a claim that externally gated acceptance has passed.

## Environment and verification boundaries

The mobile V2 base is `https://api-dev.hj0252.com/v2/api/`. Legacy wallet-authenticated Paymail/multisig management uses the same V2 namespace, with separate session storage and the same 40-second/4-MiB/no-redirect transport policy. MPC Paymail configuration and binding queries use the platform account session on the V2 origin. Public Paymail discovery and destination resolution use token-free requests to the public Paymail service. V1 business records are not migrated, recreated or removed automatically.

The admin site is environment information, not a client API or a source for embedding administrative permissions. No administrative credentials belong in source, documentation, fixtures or the app. This implementation does not deploy services, change server flags, issue real email, bypass waiting conditions or broadcast mainnet funds.

The `BsvHome` app declares `webcredentials:api-dev.hj0252.com` for the development V2 Passkey RP. The origin and Apple CDN served AASA JSON listing `U2D6XV88CZ.com.bsvhome.ios`, which matches the iOS signing settings. Simulator login opened the system cross-device Passkey sheet and returned to the platform account screen on cancellation. Signed-device registration and login still require a test account to verify the authenticator origin accepted by the server. The client passes through the system's `clientDataJSON`. The current server Controller/DTO snapshot and independent B executable remain external acceptance prerequisites.

New phase-one text has simplified Chinese and English source wording. Other existing locales retain English fallback for these additions; this does not certify native translation review. Existing locale/theme selection remains available.

### Reproducible checks

Use `just ios generate-core`, `just ios generate-mpc`, `just ios localize`, `just ios build` and selected `just ios test` targets from the repository root. The `BsvHome` test plan includes `PhaseOneTests`, `WowPayMpcTests` and `OfflineIdentityTests`. Native host tests run with `cargo test --manifest-path ios/Packages/WowPayMpc/Native/Cargo.toml --locked`. The external-B test also requires `WOWPAY_MPC_PEER_BINARY` and explicit ignored-test selection.

Historical results from 2026-11-02, before the move to one app:

| Command / scope | Result |
| --- | --- |
| `cd core && cargo test --locked -p bsv_home_core --lib --all-features` | 1,018 passed, 0 failed, 3 explicitly gated live tests ignored |
| `cd core && cargo test --locked -p bsv_home_bsv --all-features` | 81 passed, 0 failed, 0 ignored; includes MPC, multisig and cold-wallet checks |
| `cd core && cargo clippy --locked -p bsv_home_bsv -p bsv_home_core --all-features --lib --offline -- -D warnings` | Passed for production libraries |
| `just ios generate-core`, `just ios generate-mpc`, `just ios localize` | Passed; ARM64 device and simulator static libraries generated |
| `just ios build` | Passed for the integrated online app |
| `just ios build-cold-wallet` and `just ios build-cold-wallet-device` | Passed for simulator and generic ARM64 iOS; device compilation used `CODE_SIGNING_ALLOWED=NO` |
| Post-presentation-fix `OfflineIdentityTests` on the cold scheme | 5 passed, 0 failed, 0 skipped; simulator/device builds repeated successfully |
| Cold-wallet test plan, 4 selected targets | 89 distinct tests selected: 88 passed, 0 failed, 1 physical-file-protection test skipped |
| `just ios test 'BsvHomeTests/MpcWalletNavigationTests -only-testing PhaseOneTests'` | 15 passed, 0 failed, 0 skipped (13 service/UI-state tests and 2 application-hosted system/navigation tests) |
| `just ios test BsvHomeTests/MpcDeviceProofTests` after host isolation | 1 passed, 0 failed, 0 skipped; real simulator Keychain access retained |
| iOS wallet regression batch, 11 selected targets | 662 distinct tests selected: 660 passed, 0 failed, 2 checks skipped (physical file protection and gated live HTTPS/ATS); 44 parameterized tests expand to 136 runs |

Cold tests used the new isolated `WowPay Offline Identity QA` simulator with `SIMULATOR_NAME='WowPay Offline Identity QA' just ios _test test '' BsvHomeColdWallet build/ColdWalletDerivedData`. The four targets were `OfflineIdentityTests`, `BsvHomeCoreServicesTests`, `BsvColdWalletTests` and `QRScannerTests`.

HD selection deliberately omits view-only, private-key and non-BSV wallets without deleting them. Contact projection omits unsupported chains, Memo-bearing addresses and invalid destinations. MPC balance degradation is limited to explicit chain-unavailable/not-indexed errors and stays visibly STALE/UNAVAILABLE; other failures propagate as safe errors.

The iOS regression selection was `just ios test 'BsvHomeTests -only-testing WowPayMpcTests -only-testing OfflineIdentityTests -only-testing BsvHomeCoreServicesTests -only-testing NativeProviderServiceTests -only-testing TransferTests -only-testing OnboardingTest -only-testing WalletTabTests -only-testing AssetsTests -only-testing PrimitivesComponentsTests -only-testing BsvMultisigTests'`. The final phase-one batch uses the application-hosted `BsvHomeTests` target for real system picker/navigation coverage and `PhaseOneTests` for deterministic state handling. The two hosted tests share their fixtures with the package and are excluded from the framework-only package test target.

A host macOS `swift build` attempt for the UI packages failed in existing UIKit-dependent Style code; it is not counted as iOS verification or a macOS compatibility result. All Swift build/test results above use explicit Xcode iOS destinations.

The production-library clippy result does not certify test targets: an earlier `--all-targets` attempt also reported existing test-only lints for a cold-wallet mutex guard held across an await and boolean comparisons in wallet-address tests. Those unrelated tests were not reformatted or altered to satisfy clippy.

The main app supplies a blank, real UIApplication/UIWindowScene only when a DEBUG XCTest host is detected; its resolver and business startup are not constructed in this mode. This keeps hosted tests away from existing wallets, automatic provider requests and system startup prompts. Release builds always use normal startup. Normal simulator startup was checked again from the final integrated build outside XCTest and displayed the real account screen; system Keychain integration passed again in the isolated host.

The iOS skips were `BsvMultisigStoreTests.fileAndDirectoryUseCompleteProtection` (physical-device-only) and `PortalHttpsLiveTests.configuredPortalHttpsPassesAts` (explicit live-test environment not enabled). The three ignored Core live tests cover real Portal authentication, read-only Portal swap discovery and mutable Paymail registration/lifecycle. They were intentionally not enabled. Native A host tests passed 7 cases, with the independent-B case ignored because `WOWPAY_MPC_PEER_BINARY` was not supplied; its ignored status is not interoperability acceptance. The iOS device-proof test uses the real simulator Security Keychain in the application-hosted `BsvHomeTests` target and validates concurrent access, SPKI/DER encoding, UTF-8 action signing and persistent reopening. Physical Secure Enclave and lock-screen behavior remain untested.

Cold-tool presentation uses a single optional identifiable model, so the first presentation cannot capture an empty model from a separate Boolean state.

The first broad iOS batch exposed the old three-asset expectations and a `noHandle` HD test fixture calling an unoverridden FFI method. The fixture and scope tests were corrected; historical TRON/testnet rows and negative send/import cases remain asserted. Incremental Xcode resource bundles in the existing derived-data directories needed their stale generated inner copy rebuilt to include the new public P-256 fixture; no source fallback or test skip was added.

Manual smoke used a new isolated iPhone SE simulator on iOS 26.4: account entry, original wallet import terms entry and return, unauthenticated recovery/error page, and two increases of the preferred text size. The cold-tool entry was also launched after the item-presentation fix and reached real LocalAuthentication device-passcode UI; authentication was cancelled rather than supplying a credential. Successful unlocking of that simulator tool was not observed. Feature-level tests use deterministic authentication fixtures; they do not certify physical device authentication. No terms were accepted, no real credential was used and no wallet funds were created or sent. Authenticated MPC flows await the external prerequisites above.

Earlier Android and cold-wallet records are references, not new iOS acceptance results. Shared changes are unverified on Android.
