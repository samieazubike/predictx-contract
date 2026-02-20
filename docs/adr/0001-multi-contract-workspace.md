# ADR 0001: Multi-Contract Soroban Workspace Layout

## Status
Accepted — 2026-02-20

## Context
PredictX is composed of multiple on-chain components (PredictionMarket, PollFactory, VotingOracle, Treasury) that must each compile to an independent Soroban WASM binary.

Additionally:
- Contracts need a shared set of types, errors, and constants to avoid drift/duplication.
- Cross-contract calls in Soroban are performed via generated clients (typically via `soroban_sdk::contractimport!`).
- The build/test pipeline must support compiling and testing all contracts from a single repository root.

## Decision
Adopt a Rust Cargo workspace with:
- `contracts/<contract-name>/` as individual `cdylib` crates (one WASM each)
- `packages/shared/` as a regular Rust library crate (`rlib`) for shared types/errors/constants

Workspace root (`Cargo.toml`) centralizes:
- `soroban-sdk = "22.0.0"` via `[workspace.dependencies]` for version consistency
- Release profiles optimized for Soroban WASM size constraints

## Consequences
### Positive
- Clear separation of concerns per contract; parallel development without merge conflicts.
- Consistent shared types/errors across all contracts.
- Predictable build output: `cargo build --target wasm32-unknown-unknown --release` produces one `.wasm` per contract.
- Cross-contract invocation pattern can be validated early using `contractimport!`-generated clients.

### Trade-offs
- Cross-contract client generation requires access to the callee contract WASM at compile time.
- Workspace-level integration tests are typically hosted within contract crates (unit tests) or in a dedicated test crate if/when needed.
