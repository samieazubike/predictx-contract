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
├── Cargo.toml              # Workspace manifest
├── predictx-spec.md        # Full product specification
├── rust-toolchain.toml     # Rust toolchain config
├── src/
│   └── lib.rs              # Contract entry point
└── issues/                 # GitHub issue specs (development roadmap)
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

## License

See [LICENSE](LICENSE) for details.
