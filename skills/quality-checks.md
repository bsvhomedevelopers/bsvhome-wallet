# Quality Checks

Run the checks that match the area you touched. Use the narrowest meaningful command while iterating, then finish with the broader validation required by the risk of the change.

For SwiftUI work, pure presentation changes should not spend most of the loop in full app builds. Apply the [iOS-only coding scope](../AGENTS.md#ios-only-coding-scope) throughout these checks.

Before running checks, confirm the active checkout/worktree and command directory. Use the feature flags that compile the changed path, and verify that filtered commands selected the intended tests. Tool-specific caveats (Cargo test filters, lock contention) live in the platform development-commands skills.

Run `just ios ...` commands from the repository root; run `cd ios && just ...` commands from the iOS directory.

## Iteration Matrix

| Change Type | Inner Loop Checks |
|-------------|-------------------|
| iOS presentation-only SwiftUI | `cd ios && just build-package <PACKAGE>` |
| iOS ViewModel, formatter, validation, or display-model behavior | `cd ios && just build-package <PACKAGE>`<br>`cd ios && just test <TARGET>` when a targeted test exists or is added |
| Core change needed for iOS with no iOS API impact | `cd core && just test <CRATE>` |
| Core change that affects iOS bindings or shared models | `cd core && just test <CRATE>`<br>`just ios generate-models` and/or `just ios generate-core` as applicable<br>then targeted iOS compile until app integration is ready |
| Localization input change used by iOS | `just ios localize`<br>then targeted iOS app/package compile where generated strings are consumed |

## Closing Matrix

| Change Type | Minimum Closing Checks |
|-------------|------------------------|
| iOS presentation-only SwiftUI | `cd ios && just build-package <PACKAGE>`<br>Simulator/device smoke when the changed flow is reachable |
| iOS ViewModel, navigation, app wiring, or behavioral UI change | `cd ios && just build`<br>`cd ios && just test <TARGET>` or `cd ios && just test` |
| iOS UI flow or navigation change | Optional: a Maestro flow on a booted simulator when requested — see [Maestro UI Testing](testing-maestro.md) |
| Core change needed for iOS with no iOS API impact | `cd core && just test <CRATE>`<br>`cd core && cargo clippy -p <crate> --all-features -- -D warnings`<br>`cd core && just format` |
| Core change that affects iOS bindings or shared models | `cd core && just test <CRATE>`<br>`cd core && cargo clippy -p <crate> --all-features -- -D warnings`<br>`cd core && just format`<br>`just ios generate-models` and/or `just ios generate-core` as applicable<br>`just ios build` |
| Localization input change used by iOS | `just ios localize`<br>Rebuild iOS if the generated strings are consumed by the change |
| Documentation-only change | `git diff --check`<br>Inspect changed links, paths, commands, and instructions |

Navigation, app wiring, wallet-critical UI, security-sensitive code, signing, transaction construction, wallet import/export, seed phrases, private keys, and auth flows are never presentation-only. Use the stricter iOS and security checks for those tasks.

For Core crates with `default = []`, per-crate `cargo clippy -p <crate>` and `cargo test -p <crate>` skip feature-gated modules and pass in seconds. Add `--all-features` or the gating feature (`just test <CRATE>` already does); see [Core Development Commands](../core/skills/development-commands.md).

Except for documentation-only changes, closing a task requires at least one real build or test command for the changed area. Do not substitute `git diff`, static inspection, or reasoning for execution. If execution is blocked by unrelated repo state, include the exact command and the blocking failure in the handoff.

Compiling a gated integration test with `--no-run` proves build compatibility, not live provider behavior. Report deterministic tests, gated live tests, and checks that were compiled but not executed as separate results.

If a broad suite fails outside the changed path, rerun the narrow affected check to separate a regression from an environmental or pre-existing failure. Report both results; a targeted pass does not turn the failed broad suite into a pass.

## Ready-to-Commit Batch

Do not run the closing matrix after every edit. Once the implementation is stable and no more code edits are expected, run the applicable closing checks as one batch:

1. Regenerate models/bindings or localization if the changed inputs require it.
2. Run the targeted tests that cover the changed behavior.
3. Build the affected package/module/app according to the closing matrix.
4. Exercise the changed UI flow when the platform guide requires a simulator, emulator, or device smoke check.

If any step modifies source files or forces a compile fix, return to the narrow iteration loop, then run the affected final checks again.

If you change shared models or bindings used by iOS, run the applicable iOS generation steps and validate iOS.

If a shared Core change may affect other consumers, report that impact as unverified.

For detailed platform-specific commands, flags, and workflows see:
- [iOS Development Commands](../ios/skills/development-commands.md)
- [Core Development Commands](../core/skills/development-commands.md)
