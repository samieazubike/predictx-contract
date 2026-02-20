#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$ROOT_DIR"

echo "Building all contracts (wasm32-unknown-unknown, release)…"
cargo build --target wasm32-unknown-unknown --release

WASM_DIR="$ROOT_DIR/target/wasm32-unknown-unknown/release"
PREDICTION_MARKET_WASM="$WASM_DIR/prediction_market.wasm"
POLL_FACTORY_WASM="$WASM_DIR/poll_factory.wasm"
VOTING_ORACLE_WASM="$WASM_DIR/voting_oracle.wasm"
TREASURY_WASM="$WASM_DIR/treasury.wasm"

echo "Built WASMs:"
ls -lh "$PREDICTION_MARKET_WASM" "$POLL_FACTORY_WASM" "$VOTING_ORACLE_WASM" "$TREASURY_WASM"

cat <<'EOF'

Next: deploy with Soroban CLI.

This repo intentionally does not hardcode network / identity configuration.
Set these environment variables (or adapt the commands) before deploying:

  SOROBAN_NETWORK=<network name>
  SOROBAN_RPC_URL=<rpc url>
  SOROBAN_NETWORK_PASSPHRASE=<passphrase>
  SOROBAN_IDENTITY=<configured identity name>

Example (adjust as needed):

  soroban network add \
    --name "$SOROBAN_NETWORK" \
    --rpc-url "$SOROBAN_RPC_URL" \
    --network-passphrase "$SOROBAN_NETWORK_PASSPHRASE"

  soroban contract deploy --network "$SOROBAN_NETWORK" --source "$SOROBAN_IDENTITY" --wasm target/wasm32-unknown-unknown/release/prediction_market.wasm
  soroban contract deploy --network "$SOROBAN_NETWORK" --source "$SOROBAN_IDENTITY" --wasm target/wasm32-unknown-unknown/release/poll_factory.wasm
  soroban contract deploy --network "$SOROBAN_NETWORK" --source "$SOROBAN_IDENTITY" --wasm target/wasm32-unknown-unknown/release/voting_oracle.wasm
  soroban contract deploy --network "$SOROBAN_NETWORK" --source "$SOROBAN_IDENTITY" --wasm target/wasm32-unknown-unknown/release/treasury.wasm

EOF
