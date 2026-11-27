# Setup

Use this skill for iOS environment setup, bootstrap work, and local tooling prerequisites.

## Prerequisites

1. macOS
2. Xcode
3. `just`
4. Homebrew for local tool installation

Apple Silicon is the supported environment for BsvHomeCore builds. Intel Macs are not supported.

## Initial Setup

```bash
just setup-git
cd ios
just bootstrap
just spm-resolve
```

`just bootstrap` installs the Rust toolchain pieces, typeshare, UniFFI targets, and SwiftFormat needed by the iOS app. It also creates the local BsvHomeCore UniFFI Swift/header sources and iOS Rust static libraries that SwiftPM and Xcode need.

After checkout, the default repo-root workflow is `just generate-core`, then `just run-ios`. The optional `BsvHomeWithCore` Xcode scheme combines cached BsvHomeCore generation with the normal app build for people who prefer staying in Xcode.

## Useful Setup Commands

```bash
just spm-resolve-all
just generate
just generate-core
```

## Notes

- Use the repo root `just setup-git` if submodules are missing or out of date
- Run `just bootstrap` before opening a fresh checkout in Xcode so the local BsvHomeCore UniFFI sources exist
- Xcode latest stable is expected; Apple framework documentation ships inside Xcode at `/Applications/Xcode.app/Contents/PlugIns/IDEIntelligenceChat.framework/Versions/A/Resources/AdditionalDocumentation`
