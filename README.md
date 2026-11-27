# BsvHome

[![Core CI](https://github.com/bsvhomedevelopers/bsvhome-wallet/actions/workflows/core-ci.yml/badge.svg)](https://github.com/bsvhomedevelopers/bsvhome-wallet/actions/workflows/core-ci.yml)
[![iOS CI](https://github.com/bsvhomedevelopers/bsvhome-wallet/actions/workflows/ios-ci.yml/badge.svg)](https://github.com/bsvhomedevelopers/bsvhome-wallet/actions/workflows/ios-ci.yml)
[![Android CI](https://github.com/bsvhomedevelopers/bsvhome-wallet/actions/workflows/android-ci.yml/badge.svg)](https://github.com/bsvhomedevelopers/bsvhome-wallet/actions/workflows/android-ci.yml)
[![License](https://badgen.net/github/license/bsvhomedevelopers/bsvhome-wallet)](https://github.com/bsvhomedevelopers/bsvhome-wallet/blob/main/LICENSE)

BsvHome is an open-source mobile wallet for iOS and Android. This repository is the monorepo for both apps and the shared Rust core they build against.

## Features

- Open source, self-custodial wallet with multi-chain support
- Native iOS and Android apps with shared Rust-based blockchain functionality
- Swaps, staking, WalletConnect, fiat on/off ramp, alerts, and market data

## Repository

- `ios/`: SwiftUI application, packages, tests, and iOS-only submodules
- `android/`: Kotlin/Compose application and Android build tooling
- `core/`: shared Rust source used by both mobile apps

## Getting Started

1. Clone the repository with iOS submodules:

```bash
git clone --recursive git@github.com:bsvhomedevelopers/bsvhome-wallet.git
cd bsvhome-wallet
```

2. If needed, initialize iOS submodules later:

```bash
just setup-git
```

### iOS

> [!NOTE]
> iOS builds require macOS and Apple silicon.

```bash
cd ios
just bootstrap
just spm-resolve
just build-for-testing
```

`just bootstrap` generates the BsvHome Core UniFFI bindings and Rust static libraries that the app needs. Run `just generate-core` from `ios/` to refresh those artifacts after changing Core.

### Android

```bash
cd android
just bootstrap
just build-test
```

## Developer Shortcuts

The repo root exposes monorepo commands plus module access to each platform:

```bash
just build
just generate
just localize
just bump patch
just bump minor
just bump major
just ios bootstrap
just ios build
just ios build-for-testing
just ios test-without-building
just android bootstrap
just android build
just android build-test
just android test
```

Platform-specific commands remain available through the [`ios`](ios/justfile) and [`android`](android/justfile) just modules.

## Contributing

- Browse [GitHub Issues](https://github.com/bsvhomedevelopers/bsvhome-wallet/issues)

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution guidelines.

## Community

- [BsvHome website](https://www.bsvhome.com/)
- [BsvHome Community Forum](https://www.bsvhome.com/?page_id=44)

## License

BsvHome is open-sourced software licensed under the [GPL-3.0](LICENSE).
