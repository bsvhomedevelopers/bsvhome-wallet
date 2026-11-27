# Development Commands

From the repository root, use the iOS `justfile` through `just ios <recipe>` for app workflows. Do not run root recipes that invoke Android, including `just build`, `just test`, `just lint`, `just test-integration`, `just clean`, `just generate`, and `just localize`; see the [iOS-only coding scope](../AGENTS.md#ios-only-coding-scope).

## iOS Commands

```bash
just ios build          # build iOS
just ios test           # run iOS unit tests
just ios test-ui        # run iOS integration/UI tests
just ios generate-models # regenerate iOS TypeShare models
just ios generate-core # regenerate iOS UniFFI bindings
just ios localize       # update iOS localization output
just run-ios            # build, install, and run iOS
```

Load the iOS guide before dropping into platform-specific commands or tooling details. Load the Core guide when iOS work requires shared Core changes.
