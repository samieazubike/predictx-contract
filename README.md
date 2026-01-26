# PredictX Contract

A Stellar smart contract written in Rust using the Soroban SDK.

## Prerequisites

- Rust (latest stable version)
- Cargo
- Stellar CLI (optional, for deployment)

## Building

```bash
cargo build --target wasm32-unknown-unknown --release
```

## Testing

```bash
cargo test
```

## Development

This project uses the Soroban SDK for Stellar smart contract development. The contract includes:

- Basic initialization functionality
- Counter increment operations
- State management using Soroban storage

## Contract Structure

The contract is structured as a library crate (`lib`) that compiles to WebAssembly (`cdylib`). The main contract logic is in `src/lib.rs`.

## License

See LICENSE file for details.