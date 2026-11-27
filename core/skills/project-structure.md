# Project Structure

Use when you need to find which crate or app owns a capability, or where a new crate belongs.
BsvHome Wallet Core is a Rust-based cryptocurrency wallet backend engine supporting 50+ blockchain networks. It is a Cargo workspace with 60+ crates covering transaction processing, asset management, DeFi integrations, swap operations, and cross-platform mobile support.

## Directory Tree

```
apps/           # Backend services (API, Daemon, Dynode)
bsv_home_core/       # Cross-platform mobile library (UniFFI → iOS/Android)
crates/         # Shared libraries and blockchain implementations
bin/            # Utility binaries
skills/         # Agent guidance documents (this directory)
```

## Applications (`apps/`)

- **API Server** (`apps/api/`): REST API with authenticated device WebSocket streaming
- **Daemon** (`apps/daemon/`): Background services for asset updates, push notifications, transaction indexing
- **Dynode** (`apps/dynode/`): Dynamic blockchain node proxy with caching, monitoring, and metrics

## Cross-Platform Library (`bsv_home_core/`)

Shared Rust library compiled to iOS Swift Package and Android AAR using UniFFI bindings. Contains blockchain RPC clients, swap integrations, payment URI decoding, and message signing.
- Uses `swapper` crate via `bsv_home_core::bsv_home_swapper` module for on-device swap integrations
- Uses `signer` crate for cryptographic signing operations across multiple blockchain types
- **Always use UniFFI remote types for external models** — see [UniFFI Remote and External Types](https://mozilla.github.io/uniffi-rs/latest/types/remote_ext_types.html)

## Blockchain Support

Individual `bsv_home_*` crates for each blockchain with unified RPC client patterns:
- **Bitcoin family** (`bsv_home_bitcoin`): Bitcoin, Bitcoin Cash, Litecoin, Dogecoin
- **EVM chains** (`bsv_home_evm` family crate plus `bsv_home_optimism`, `bsv_home_bsc`, `bsv_home_everstake`, `bsv_home_monad`, `bsv_home_tempo`): Ethereum, Polygon, Arbitrum, Optimism, Base, zkSync, Linea, BSC, Monad, Tempo, and the other chains in `ChainConfig`; see [New Chain Checklist](new-chain-checklist.md) for the crate boundary
- **EVM indexers** (`bsv_home_alchemy`, `bsv_home_ankr`, `bsv_home_blockscout`): address history and token balance providers composed only by `settings_chain`
- **Alternative L1s**: Solana (`bsv_home_solana`), Sui (`bsv_home_sui`), TON (`bsv_home_ton`), Aptos (`bsv_home_aptos`), NEAR (`bsv_home_near`), Stellar (`bsv_home_stellar`), Algorand (`bsv_home_algorand`), Tron (`bsv_home_tron`), XRP (`bsv_home_xrp`), Cardano (`bsv_home_cardano`), Polkadot (`bsv_home_polkadot`)
- **Cosmos ecosystem** (`bsv_home_cosmos`): Cosmos Hub, Osmosis, Celestia, Injective, Sei, Noble

## Utility Binaries (`bin/`)

- **uniffi-bindgen** (`bin/uniffi-bindgen/`): UniFFI bindings generator for iOS and Android
- **generate** (`bin/generate/`): Code generation utilities
- **gas-bench** (`bin/gas-bench/`): Gas benchmarking tool for blockchain operations
- **img-downloader** (`bin/img-downloader/`): Image asset downloader utility
- **cli** (`bin/cli/`): Command-line entry point for Core operations
- **nodecheck** (`bin/nodecheck/`): Node availability and RPC health checks

## Core Crates

### Blockchain Infrastructure
- `bsv_home_client/`: Client trait abstraction; implementations: `ReqwestClient` (backend) and `AlienProvider` (mobile)
- `bsv_home_jsonrpc/`: Internal JSON-RPC client library (replaces external alloy dependencies)
- `bsv_home_hash/`: Hashing utilities for blockchain operations
- `chain_primitives/`: Primitive types specific to blockchain operations
- `chain_traits/`: Common traits for blockchain implementations

### Cross-Chain Operations
- `swapper/`: Standalone swap/exchange integration crate supporting DEX and CEX swaps across multiple chains
- `signer/`: Cryptographic signing operations for transactions across multiple blockchain types

### Data & Storage
- `primitives/`: Central types and models shared across the system
- `storage/`: Database models, migrations, and data access layer using Diesel ORM
- `cacher/`: Caching layer for improved performance

### Pricing & Market Data
- `pricer/`: Asset pricing aggregation and management
- `prices/`: Price provider implementations (CoinGecko, DefiLlama, Jupiter) behind one `PriceProvider` contract
- `coingecko/`: CoinGecko API client

### NFT & Digital Assets
- `nft/`: NFT models, provider factory, and provider clients (OpenSea, Magic Eden, Alchemy, TON)

### Integrations & Services
- `fiat/`: Fiat on-ramp/off-ramp providers (MoonPay, Transak, Mercuryo, Paybis, Flashnet)
- `name_resolver/`: Blockchain naming service integrations (ENS, SNS, etc.)
- `security_provider/`: Security and fraud detection provider integrations
- `api_connector/`: Backend API connector utilities
- `bsv_home_hypercore/`: Perpetuals (perps) trading support via Hyperliquid integration

### Utilities & Support
- `localizer/`: i18n support for 20+ languages using Fluent
- `serde_serializers/`: Custom Serde serializers/deserializers used across crates
- `number_formatter/`: Number and currency formatting utilities
- `job_runner/`: Background job execution framework
- `search_index/`: Search indexing and query capabilities
- `streamer/`: Real-time data streaming utilities
- `tracing/`: Logging and tracing infrastructure
- `settings/`: Configuration management
- `settings_chain/`: Chain-specific configuration settings

## Key Dependency Relationships

```
bsv_home_core/          ← generates UniFFI bindings, builds the Android AAR, and ships a self-contained iOS BsvHomeTest example
  ├── swapper/     ← DEX/CEX swap integrations (uses bsv_home_* chain crates)
  ├── signer/      ← transaction signing (uses chain_primitives, bsv_home_hash)
  └── primitives/  ← central types shared by almost everything

bsv_home_* chain crates (bsv_home_evm, bsv_home_solana, ...)
  ├── bsv_home_client/        ← HTTP/RPC abstraction
  ├── bsv_home_jsonrpc/       ← JSON-RPC protocol layer
  ├── chain_primitives/  ← blockchain primitive types
  └── primitives/        ← shared models

apps/ (api, daemon, dynode)
  ├── storage/     ← Diesel ORM + PostgreSQL
  ├── pricer/      ← pricing aggregation
  ├── bsv_home_client/  ← ReqwestClient for backend HTTP
  └── primitives/  ← shared models
```

`primitives/` is the root dependency — nearly every crate imports it. Changes there ripple everywhere. `bsv_home_client/` has two implementations: `ReqwestClient` (backend) and `AlienProvider` (mobile via bsv_home_core).

## Technology Stack

- **Framework**: Rust workspace with Rocket web framework
- **Database**: PostgreSQL (primary), Redis (caching)
- **Message Queue**: RabbitMQ with Lapin
- **RPC**: Custom `bsv_home_jsonrpc` client library for blockchain interactions
- **Mobile**: UniFFI for iOS/Android bindings
- **Serialization**: Serde with custom serializers
- **Async**: Tokio runtime
- **Testing**: Built-in Rust testing with integration tests
