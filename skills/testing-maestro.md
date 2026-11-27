# Maestro UI Testing

Maestro (`docs.maestro.dev`) drives a real UI flow against a booted iOS simulator. Use it for reachable iOS journeys that need end-to-end regression coverage under the [iOS-only coding scope](../AGENTS.md#ios-only-coding-scope).

Maestro complements, but does not replace, `ios/BsvHomeUITestsAppTests` (XCUITest, run in CI by `.github/workflows/ios-ui-tests.yml`).

## When to Use What

Decide in one glance:

| Situation | Verify with |
|-----------|-------------|
| iOS user journey (import, receive, send, swap), navigation between screens, or end-to-end regression | **Maestro flow** |
| Pure logic: mappers, formatters, validators, display models, ViewModel state | **Unit test** — see [Quality Checks](quality-checks.md) |
| Permission dialogs, deep links, or XCUITest interruption monitors | **Native UI test** — `ios/BsvHomeUITestsAppTests` (XCUITest) |
| Static screen with no interaction, visual polish/spacing, small interaction, copy/localization-only | **Not worth a flow** |

A Maestro flow earns its cost only when the journey is reachable and regression-prone. If a unit test can assert the same rule, write the unit test. Reach for a flow when the change needs it or when asked — it is not an automatic step on every UI change.

## Setup

- **Install** (official installer): `curl -fsSL "https://get.maestro.mobile.dev" | bash`. Verify: `maestro --version` (validated against `2.6.1`).
- **iOS simulator** needs Xcode Command Line Tools (`xcode-select --install`); Maestro installs its own xctest driver on the first simulator run — no `idb` required on current versions. Physical devices need extra signing setup (out of scope here).
- **Build and install the iOS app first.** Maestro drives the *installed* build, not your source tree. Run `just ios run` before a flow, and reinstall after any structural, rename, or dependency-injection change.

Pass the simulator explicitly:

| Setting | iOS value |
|---------|-----------|
| appId | `com.bsvhome.ios` |
| list devices | `xcrun simctl list devices booted` or `maestro list-devices` |

```bash
maestro --device <id> test flow.yaml
```

`--device` accepts a simulator UDID; `--udid` is the same flag.

## Authoring Rules

These are the core of this doc. A flow that breaks them is worse than no flow.

### Selectors — ids, never text

The app ships ~30 languages. A `text:` selector is a per-locale time bomb; it also collides across elements (Maestro text match is regex, case-insensitive). Select by `id`.

`id` maps to `accessibilityIdentifier` on iOS.

If the element you need has no id, **adding the id to the app is part of writing the flow**:

| Add the id | Existing example |
|------------|------------------|
| `.accessibilityIdentifier("wowPayAssetMore")` | `ios/Features/Assets/Sources/Scenes/AssetScene.swift` |

`id` is regex — keep ids specific so they do not substring-match a neighbor.

### One flow per behavior

Set the iOS app ID in the flow:

```yaml
appId: com.bsvhome.ios
```

Run the flow on a booted iOS simulator.

### Deterministic state

- `launchApp: { clearState: true }` for regression flows. On iOS this is a fresh reinstall. Expect the onboarding or lock screen after it.
- **Never mainnet funds or real keys.** Use a dedicated test seed only (there is already one at `ios/BsvHomeUITestsAppTests/Types/UITestKitConstants.swift`).
- Assert with `assertVisible` / `assertNotVisible` / `takeScreenshot`. **Never `sleep`.** For async work use `extendedWaitUntil` or `waitForAnimationToEnd`, keyed to a visible id — not a fixed delay.

### Biometrics

- **iOS simulator:** Maestro has no built-in Face ID pass, and the BsvHome app re-locks on every `launchApp`. Run flows against a **debug build that bypasses the biometric gate**; otherwise every step after launch lands on the lock screen.

### Text input

Use Maestro `inputText` for text entry.

## Authoring Aids

| Command | Use |
|---------|-----|
| `maestro studio` | Interactive selector/flow explorer (local web UI) |
| `maestro hierarchy` (`--compact` for CSV) | Dump the current screen's element tree to find ids |
| `maestro check-syntax flow.yaml` | Validate a flow before running it |

## Anti-Patterns

| Do not | Instead |
|--------|---------|
| Select by visible text | Select by `id` (add `accessibilityIdentifier` if missing) |
| `sleep` / fixed waits | `extendedWaitUntil` / `waitForAnimationToEnd` on a visible id |
| Assert live prices or market data | Assert structure and ids; keep the flow deterministic |
| Commit flows that need real keys or mainnet funds | Dedicated test seed on testnet/debug build |
