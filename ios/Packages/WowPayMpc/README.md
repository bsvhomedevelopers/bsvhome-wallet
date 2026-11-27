# WowPay MPC A for iOS

This package preserves the fixed DKLs23 role-A protocol, encrypted participant state, recovery backup format, and separate Paymail identity. Its Rust workspace and lockfile are independent of BsvHomeCore. It exposes no network, platform authentication, broadcast, or full-private-key reconstruction implementation.

## Build and link

From `ios/`, run `scripts/generate-mpc.sh` for debug device and simulator static libraries, or append `release`. `WOWPAY_MPC_IOS_TARGETS` may select existing Rust targets. The script does not install SDKs or Rust targets.

Link the App and test bundles with the matching `Native/target/<Rust target>/<debug|release>/libwowpay_mpc_ios.a`, alongside the existing BsvHomeCore library. The `WowPayMpcFFI` source target publishes the versioned C header. Do not merge the two Cargo workspaces or change cryptographic dependencies to fix linker errors.

## Swift lifetime

Call `WowPayMpcParticipant.open(walletId:namespace:keychain:rootURL:)` with the complete environment and platform user identity in `namespace`, then `exchange(_:)` and `close()`. Pass the existing Keychain dependency. Default storage is private Application Support; `rootURL` permits an isolated test directory. The package scopes paths and non-synchronizing, device-only seal keys by namespace and wallet. Existing encrypted state with a missing or invalid key is rejected and retained.

The actor sends blocking native work to a serial background queue. Its owner must close it when leaving a protocol, switching account/wallet/environment, locking, or entering the background. Closing does not cancel a server task, prove that a transfer failed, or delete wallet material. A transport timeout closes the native input channel; a new protocol must use a new allowed task/session after reconciliation.

Native replies can contain secret backup material. Keep them in transient authenticated UI state, clear that state when leaving/backgrounding, and never log or persist the entire reply. Only the encrypted `backup` object goes into the user's chosen file. Successful native encryption does not prove that the document picker saved a file or that the backend accepted a backup proof.

C reply storage belongs to Rust. Read it before calling `wowpay_mpc_buffer_free(buffer_id)`; the Swift bridge always frees replies after copying and the native allocator clears the bytes. Buffers use identifiers so repeated frees do not reconstruct or free an already-released pointer. C callers must provide valid readable input pointers for the declared lengths.

## Verification

Run Rust host tests with `cargo test --manifest-path Native/Cargo.toml --locked`, and clippy with `cargo clippy --manifest-path Native/Cargo.toml --locked --all-targets -- -D warnings`. The independent server-B test is explicitly ignored unless `WOWPAY_MPC_PEER_BINARY` points to the correct separately built peer and tests include `--include-ignored`. A normal passing test run does not execute this interoperability test.

The `WowPayMpcTests` target covers protected storage, missing-key preservation, scope isolation, native loading, concurrent-open rejection, and close/reopen. It must be registered in the iOS app's unit test plan and run through the iOS test commands. Device Keychain accessibility, locked-device behavior, actual B interoperability and end-to-end backup/recovery remain separate acceptance requirements.
