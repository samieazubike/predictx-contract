# PredictX Contract

A decentralized football prediction market built on the **Stellar blockchain** using the [Soroban SDK](https://soroban.stellar.org/). Users create prediction polls on specific match events, stake tokens on outcomes, and winners split the pool proportionally — all enforced on-chain.

## Overview

PredictX is **not a traditional betting platform**. It's a community-driven prediction game where:

- Anyone can create granular prediction polls (e.g. "Will Palmer score a goal?")
- Users stake crypto on **Yes** or **No** sides of each poll
- Outcomes are resolved through **community voting + admin verification**
- Winners receive proportional payouts from the losing pool (minus a 5% platform fee)
- Everything runs transparently on-chain

## Architecture

The platform is composed of four core smart contracts:

| Contract             | Purpose                                                                          |
| -------------------- | -------------------------------------------------------------------------------- |
| **PredictionMarket** | Core logic — poll creation, staking, winnings distribution, emergency withdrawal |
| **VotingOracle**     | Outcome resolution — community voting, admin verification, dispute handling      |
| **PollFactory**      | Registry & factory — modular poll creation, discovery, categorization            |
| **Treasury**         | Fee management — platform fee collection, admin payouts, revenue tracking        |

A shared types crate (`predictx-shared`) provides common data structures, error types, and constants used across all contracts.

> For the full product specification, see [`predictx-spec.md`](predictx-spec.md).

## Key Features

- **Pool-Based Staking** — Multiple users stake on each side; rewards distributed proportionally
- **Hybrid Oracle** — Community voting (2-hour window) + admin review for contested outcomes
- **Dispute Resolution** — 24-hour challenge window with multi-sig escalation
- **Voter Incentives** — 0.5–1% of the pool rewards honest voters
- **Time-Locks** — Polls auto-lock at match kickoff, halftime, or custom times
- **On-Chain Analytics** — User stats, platform-wide metrics, and history tracking

## Tech Stack

- **Blockchain**: Stellar (Soroban)
- **Language**: Rust
- **SDK**: `soroban-sdk 22.0.0`
- **Target**: `wasm32-unknown-unknown`
- **Toolchain**: Stable Rust

## Workspace Layout

This repository is a **multi-contract Cargo workspace**. Each contract is its
own `cdylib` crate and compiles to a separate WASM binary.

```
predictx-contract/
├── Cargo.toml
├── rust-toolchain.toml
├── contracts/
│   ├── prediction-market/
│   ├── poll-factory/
│   ├── voting-oracle/
│   └── treasury/
├── packages/
│   └── shared/
├── scripts/
│   └── deploy.sh
└── docs/
    └── adr/
        └── 0001-multi-contract-workspace.md
```

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- Soroban CLI — `cargo install --locked soroban-cli`
- `wasm32-unknown-unknown` target — `rustup target add wasm32-unknown-unknown`

## Building

```bash
# Build all contracts
cargo build --target wasm32-unknown-unknown --release

# Build with debug logs (for testnet)
cargo build --target wasm32-unknown-unknown --profile release-with-logs
```

## Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

## Project Structure

```
predictx-contract/
├── Cargo.toml                 # Workspace manifest
├── predictx-spec.md           # Full product specification
├── rust-toolchain.toml        # Rust toolchain config
├── contracts/                 # Soroban contracts (one WASM each)
├── packages/shared/           # Shared types/errors/constants
├── scripts/deploy.sh          # Build + deploy helper
└── docs/adr/                  # Architecture decision records
```

## Development Roadmap

Development is tracked across 28 GitHub issues organized into four phases:

| Phase       | Focus                                                  | Issues                            |
| ----------- | ------------------------------------------------------ | --------------------------------- |
| **Phase 1** | MVP — Core staking, polls, token integration, security | #1 – #8, #15 – #17, #20, #27, #28 |
| **Phase 2** | Voting — Oracle, disputes, voter rewards, user stats   | #9 – #12, #18, #19, #21           |
| **Phase 3** | Advanced — Factory, treasury, optimization, SDK        | #13, #14, #22, #23, #25           |
| **Phase 4** | Launch — Deployment pipeline, documentation            | #24, #26                          |

## Specification

The full product specification document is available at [`predictx-spec.md`](predictx-spec.md). It covers:

- Product overview and problem statement
- Detailed user flows with worked examples
- Smart contract specifications (functions, storage, events)
- Resolution mechanics (voting thresholds, dispute process)
- Security considerations and attack mitigations
- Development phases and milestones

**Read this document before contributing.**

## Contract Initialization & Configuration

This section explains how to properly initialize and configure the PredictX contracts after deployment.

### Initialization Order

The contracts must be initialized in a specific order to establish cross-contract references correctly:

1. **Deploy all contracts first** - Get the contract addresses for all four contracts
2. **Initialize Treasury** - Sets up token and market references
3. **Initialize VotingOracle** - Configures voting parameters and market reference  
4. **Initialize PredictionMarket** - Main contract with platform configuration
5. **Initialize PollFactory** - Sets up poll creation limits and market reference
6. **Set cross-contract addresses** - Link contracts together

### Initialization Parameters

#### Treasury
```rust
pub fn initialize(
    env: Env,
    admin: Address,
    token_address: Address,
    prediction_market_address: Address,
) -> Result<(), PredictXError>
```

#### VotingOracle
```rust
pub fn initialize(
    env: Env,
    admin: Address,
    prediction_market_address: Address,
    voting_window_secs: u64,        // 1800-14400 (30min-4hr)
    consensus_threshold_bps: u32,    // 7000-9500 (70%-95%)
    admin_review_threshold_bps: u32, // 5000-8000 (50%-80%)
    dispute_window_secs: u64,       // 43200-172800 (12hr-48hr)
    dispute_fee: i128,              // Must be >= 0
) -> Result<(), PredictXError>
```

#### PredictionMarket
```rust
pub fn initialize(
    env: Env,
    admin: Address,
    token_address: Address,
    treasury_address: Address,
    platform_fee_bps: u32,          // 0-1000 (0%-10%)
    voter_reward_bps: u32,          // 0-200 (0%-2%)
    min_stake_amount: i128,         // Must be > 0
) -> Result<(), PredictXError>
```

#### PollFactory
```rust
pub fn initialize(
    env: Env,
    admin: Address,
    prediction_market_address: Address,
    max_polls_per_creator_per_day: u32, // 1-50
) -> Result<(), PredictXError>
```

### Cross-Contract Address Setting

After initialization, set the cross-contract references:

```rust
// In PredictionMarket - set oracle address
prediction_market.set_oracle_address(&admin, &oracle_address);

// In VotingOracle - set market address  
voting_oracle.set_market_address(&admin, &market_address);

// In Treasury - set market address
treasury.set_market_address(&admin, &market_address);

// In PollFactory - set market address
poll_factory.set_market_address(&admin, &market_address);
```

### Configuration Management

All contracts support runtime configuration updates by the super admin:

#### Update Configuration
```rust
pub fn update_config(
    env: Env,
    admin: Address,
    key: ConfigKey,
    value: ConfigValue,
) -> Result<(), PredictXError>
```

#### Query Configuration
```rust
// PredictionMarket only
pub fn get_config(env: Env) -> PlatformConfig
```

#### Supported Configuration Keys

| ConfigKey | Type | Range | Description |
|-----------|------|-------|-------------|
| PlatformFeeBps | u32 | 0-1000 BPS | Platform fee percentage |
| VoterRewardBps | u32 | 0-200 BPS | Voter reward percentage |
| VotingWindowSecs | u64 | 1800-14400 | Voting window duration |
| ConsensusThresholdBps | u32 | 7000-9500 | Auto-resolve threshold |
| AdminReviewThresholdBps | u32 | 5000-8000 | Admin review threshold |
| DisputeWindowSecs | u64 | 43200-172800 | Dispute window duration |
| DisputeFee | i128 | >= 0 | Cost to open dispute |
| MinStakeAmount | i128 | > 0 | Minimum stake amount |
| MaxPollsPerMatch | u32 | 10-100 | Max polls per match |

### Default Values

| Parameter | Default | Range |
|-----------|---------|-------|
| Platform fee | 500 BPS (5%) | 0-1000 BPS |
| Voter reward | 100 BPS (1%) | 0-200 BPS |
| Voting window | 7200 sec (2h) | 1800-14400 |
| Auto-resolve threshold | 8500 BPS (85%) | 7000-9500 |
| Admin review threshold | 6000 BPS (60%) | 5000-8000 |
| Dispute window | 86400 sec (24h) | 43200-172800 |
| Min stake amount | Configurable | > 0 |
| Max polls per match | 50 | 10-100 |
| Max polls per creator/day | 10 | 1-50 |

### Security Notes

- **Re-initialization is prevented** - Once initialized, contracts cannot be re-initialized
- **Admin-only configuration** - Only the super admin can update configuration parameters
- **Range validation** - All configuration updates are validated against allowed ranges
- **Event emission** - All configuration changes emit `ConfigUpdated` events with old/new values
- **Version tracking** - Configuration version increments on each update

### Example Initialization Sequence

```rust
// 1. Deploy contracts and get addresses
let treasury_address = deploy_treasury();
let oracle_address = deploy_voting_oracle();  
let market_address = deploy_prediction_market();
let factory_address = deploy_poll_factory();

// 2. Initialize contracts
treasury.initialize(&admin, &token_address, &market_address);
oracle.initialize(&admin, &market_address, 7200, 8500, 6000, 86400, 10000000);
market.initialize(&admin, &token_address, &treasury_address, 500, 100, 10000000);
factory.initialize(&admin, &market_address, 10);

// 3. Set cross-contract addresses
market.set_oracle_address(&admin, &oracle_address);
oracle.set_market_address(&admin, &market_address);
treasury.set_market_address(&admin, &market_address);
factory.set_market_address(&admin, &market_address);
```

## License

See [LICENSE](LICENSE) for details.
