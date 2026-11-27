# Testing

Use when adding, changing, or running iOS tests, or registering a new test target.
## Execution Rules

- Always run tests through the iOS `justfile`
- Default commands:
  - `just test`
  - `just test <TARGET>`
  - `just build-for-testing` followed by `just test-without-building` for repeated test-debug loops
  - `just test-integration` or `just test-ui` for the iOS integration suite
- Run the narrowest relevant target while iterating, then finish with the appropriate broader validation

Unit tests use two registered test graphs: `BsvHomePackages` runs the package targets in `unit_frameworks.xctestplan` without an App build root; `BsvHome` runs hosted App tests in `unit_hosted.xctestplan`. `test`, `build-for-testing` and `test-without-building` route registered selectors through `scripts/test-unit.py`; an empty selector runs both graphs in sequence. Mixed `-only-testing` selectors are split by target ownership. Unknown targets and custom build settings are rejected by this public route; explicit `_test` calls retain diagnostic control. Test method filters still require checking actual nonzero execution counts.

This separation avoids the observed SwiftBuild graph that loaded a package's static TestKit dependency together with its dynamic product in an independent runner. Keep the normal App/host linkage and inspect actual image definitions when investigating callback or key-path failures; do not apply a global diamond-diagnostic override or replace real Core handles.

## New Test Targets

A test target only runs if it is registered in all three places:

1. `.testTarget` in the package's `Package.swift`
2. The package is referenced in `BsvHome.xcodeproj` (Packages group)
3. An entry in `BsvHomeTests/unit_frameworks.xctestplan`, consumed by the shared `BsvHomePackages` scheme

`swift test` inside the package and Xcode's package scheme bypass the test plan, so green there proves nothing about CI. xcodebuild silently ignores targets missing from the plan and plan entries pointing at deleted targets. After adding a test target, verify with `just test <TARGET>` from `ios/` and confirm the target's tests appear in the output.

## Test Structure

- Keep test names short and descriptive, for example `showManageToken`
- Keep tests concise, usually one behavior with a small number of assertions
- Skip trivial tests that only restate obvious behavior

## Sensitive Navigation

For secret-bearing SwiftUI flows, exercise a real `NavigationStack` in a hosting controller: push an internal page, rebuild the parent, then remove the feature. Root `onDisappear` and representable dismantling can occur during an internal push; neither alone proves the feature exited. A lifetime owner must be installed into retained view state, not constructed as an `@State` candidate whose discarded instance can clear an active model. Cover background/capture cleanup on a pushed page as well as the root. Hosting tests must provide the intended scene phase and await the initial asynchronous load before invoking guarded actions.

SwiftUI can retain a removed feature's state while its sheet is presented. Signing and authorization flows must also invalidate execution synchronously at the navigation or presentation binding that dismisses or replaces them; lifetime destruction is a fallback. Exercise that production owner while keeping the hosting container alive, and verify that internal navigation preserves authorization while feature dismissal cancels it.

## Mocks

- Prefer existing `TestKit` mocks over ad hoc mock services
- Add reusable mocks in the appropriate `TestKit`; keep one-off input construction local to the test
- Prefer `.mock()` style helpers and small deterministic fixtures

## Formatting

- Use direct assertions for short cases
- Break long mock setup into multiline formatting when it improves readability
- Avoid explanatory comments in tests
