# Getting Started

Developer setup guide for PredictX smart contract development.

## Prerequisites

### Required Tools

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.70+ | Compiler for Soroban contracts |
| Cargo | latest | Rust package manager |
| wasm-pack | latest | Build Rust to WASM |
| stellar-cli | 22.0.0+ | Interact with Stellar/Soroban |

### Optional Tools

| Tool | Purpose |
|------|---------|
| soroban-cli | Alternative CLI for Soroban |
| VS Code + rust-analyzer | IDE with Rust support |

---

## Installation

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### 2. Add WASM Target

```bash
rustup target add wasm32-unknown-unknown
```

### 3. Install Stellar CLI

```bash
cargo install --locked soroban-cli
```

Verify installation:

```bash
stellar --version
```

---

## Repository Structure

```
predictx-contract/
├── Cargo.toml              # Workspace root
├── packages/
│   └── shared/             # Shared types, errors, constants
│       ├── src/
│       │   ├── lib.rs
│       │   ├── types.rs    # Poll, Match, Stake, etc.
│       │   ├── errors.rs   # PredictXError enum
│       │   └── constants.rs
├── contracts/
│   ├── prediction-market/  # Main staking contract
│   ├── voting-oracle/    # Poll status tracking
│   ├── treasury/          # Fee management
│   └── poll-factory/     # Poll factory
├── tests/
│   └── integration/       # Integration tests
└── docs/                  # Documentation
```

---

## Building Contracts

### Build All Contracts

```bash
cargo build --target wasm32-unknown-unknown --release
```

### Build Specific Contract

```bash
cargo build -p prediction-market --target wasm32-unknown-unknown --release
```

### Build WASM Artifacts

WASM files are output to:

```
contracts/*/src/*.wasm
```

---

## Running Tests

### Run All Tests

```bash
cargo test --all
```

### Run Tests with Output

```bash
cargo test --all -- --nocapture
```

### Run Tests for Specific Contract

```bash
cargo test -p prediction-market
```

### Run Specific Test

```bash
cargo test --all -- test_stake_yes_side_succeeds --nocapture
```

### Run with Coverage

```bash
cargo tarpaulin --workspace --out html
```

---

## Testing with Soroban CLI

### Local Development Network

Start a local Soroban network:

```bash
stellar network add --local testnet \
    --rpc-url http://localhost:8000 \
    --network-passphrase "Local Network"
```

### Deploy Contracts

```bash
# Deploy Token (Stellar Asset Contract)
stellar contract deploy \
    --source account1 \
    --network testnet \
    --wasm target/wasm32-unknown-unknown/release/soroban_token_contract.wasm

# Deploy PredictionMarket
stellar contract deploy \
    --source account1 \
    --network testnet \
    --wasm target/wasm32-unknown-unknown/release/prediction_market.wasm

# Deploy VotingOracle
stellar contract deploy \
    --source account1 \
    --network testnet \
    --wasm target/wasm32-unknown-unknown/release/voting_oracle.wasm

# Deploy Treasury
stellar contract deploy \
    --source account1 \
    --network testnet \
    --wasm target/wasm32-unknown-unknown/release/treasury.wasm
```

### Initialize Contracts

```bash
# Initialize VotingOracle
stellar contract invoke \
    --id $ORACLE_ID \
    --source account1 \
    --network testnet \
    -- \
    initialize \
    --admin GDQEO2HGZCH7TSHB7JWT6UWUMSWGAHFFG4VTC6YMGJLb3JB3TJJHGWA

# Initialize PredictionMarket
stellar contract invoke \
    --id $PM_ID \
    --source account1 \
    --network testnet \
    -- \
    initialize \
    --admin GDQEO2HGZCH7TSHB7JWT6UWUMSWGAHFFG4VTC6YMGJLb3JB3TJJHGWA \
    --voting_oracle $ORACLE_ID \
    --token_address $TOKEN_ID \
    --treasury_address $TREASURY_ID \
    --platform_fee_bps 500
```

---

## IDE Setup

### VS Code

Install extensions:
- `rust-analyzer` — Rust language server
- `CodeLLDB` — Debugger

Recommended `settings.json`:

```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.cargo.buildScripts.overrideCommand": [
    "cargo",
    "build",
    "--message-format=json",
    "--manifest-path=Cargo.toml",
    "--target=wasm32-unknown-unknown"
  ]
}
```

---

## Common Issues

### WASM Build Failures

```bash
# Ensure WASM target is installed
rustup target add wasm32-unknown-unknown

# Clean and rebuild
cargo clean
cargo build --target wasm32-unknown-unknown --release
```

### Contract Not Found Errors

When deploying, ensure the WASM file exists:
```bash
ls -la target/wasm32-unknown-unknown/release/*.wasm
```

### Test Failures

Run tests with `--nocapture` for detailed output:
```bash
cargo test --all -- --nocapture
```

---

## Code Linting

```bash
cargo clippy --all --all-targets -- -D warnings
```

---

## Documentation

Generate documentation:

```bash
cargo doc --no-deps --open
```

Build specific package docs:

```bash
cargo doc -p predictx-shared --no-deps
```

---

## Next Steps

- Read the [Architecture docs](../architecture.md)
- Review the [API references](./contracts/)
- Check the [Deployment guide](./deployment.md)
- Read the [Contributing guidelines](./contributing.md)
