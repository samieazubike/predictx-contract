# Gas Costs

Gas cost benchmarks and optimization notes for PredictX smart contracts on Stellar Soroban.

> **Note:** These are estimates based on Soroban SDK patterns. Actual costs should be measured on the target network before mainnet launch. Run the benchmark script (`scripts/gas_benchmarks.sh`) to get exact numbers for your deployment.

## Why Gas Matters

On Stellar/Soroban, gas translates to:
- **Transaction fees** — paid by users for every operation
- **Contract size** — larger WASM = higher deployment cost
- **Storage rent** — persistent storage has ongoing costs in XLM

## Estimated Gas Costs by Operation

### PredictionMarket

| Operation | Type | Estimated Instructions | Notes |
|----------|------|----------------------|-------|
| `initialize` | Admin | ~5,000 | One-time, stores 5 addresses |
| `create_poll` | User | ~8,000 | Creates Poll + updates MatchPolls |
| `stake` | User | ~12,000 | Token transfer + pool update + storage |
| `emergency_withdraw` | User | ~10,000 | Token return + storage flag |
| `cancel_poll` | Admin | ~4,000 | Oracle call + event |
| `pause` / `unpause` | Admin | ~3,000 | Single storage write + event |
| `get_poll` | Read | ~3,000 | Single storage read |
| `get_pool_info` | Read | ~4,000 | Poll + pool aggregation |
| `get_platform_stats` | Read | ~3,500 | Single storage read |

### VotingOracle

| Operation | Type | Estimated Instructions | Notes |
|----------|------|----------------------|-------|
| `initialize` | Admin | ~4,000 | One-time admin storage |
| `set_poll_status` | Admin | ~5,000 | Status + timestamp storage |
| `get_poll_status` | Read | ~2,500 | Single storage read |
| `get_poll_status_updated_at` | Read | ~2,500 | Single storage read |

### Treasury

| Operation | Type | Estimated Instructions | Notes |
|----------|------|----------------------|-------|
| `initialize` | Admin | ~4,000 | One-time admin storage |
| `deposit` | User | ~6,000 | Balance update + validation |
| `balance` | Read | ~2,500 | Single storage read |

### PollFactory

| Operation | Type | Estimated Instructions | Notes |
|----------|------|----------------------|-------|
| `initialize` | Admin | ~4,000 | Admin + NextPollId |
| `create_poll` | User | ~7,000 | Poll creation + storage |
| `get_poll` | Read | ~3,000 | Single storage read |

---

## Storage Costs

### Instance Storage

Per-key cost per ledger entry (approx):

| Type | Cost in bytes | XLM/day (estimated) |
|------|--------------|---------------------|
| Address | 32 | ~0.0001 |
| u64 | 8 | ~0.0001 |
| i128 | 16 | ~0.0001 |
| bool | 1 | ~0.00001 |

### Persistent Storage

Persistent entries have higher rent costs:

| Entry | Size (bytes, approx) | XLM/month (estimated) |
|-------|---------------------|----------------------|
| `Poll` struct | ~400 | ~0.001 |
| `Stake` (per user per poll) | ~150 | ~0.0004 |
| `Match` struct | ~350 | ~0.001 |
| `StoredPollStatus` | ~50 | ~0.0002 |

### Cost Estimation Example

A poll with 100 stakers:
- Poll data: 1 × 0.001 XLM = 0.001 XLM
- Stake records: 100 × 0.0004 XLM = 0.04 XLM
- **Total first month: ~0.041 XLM**

---

## Contract WASM Size

| Contract | WASM Size (KB) | Deployment Cost (XLM, est.) |
|----------|----------------|-------------------------------|
| PredictionMarket | ~45 KB | ~5 XLM |
| VotingOracle | ~15 KB | ~2 XLM |
| Treasury | ~12 KB | ~2 XLM |
| PollFactory | ~14 KB | ~2 XLM |
| **Total** | ~86 KB | ~11 XLM |

### Reducing WASM Size

```bash
# Profile WASM size
wasm-objdump -h target/wasm32-unknown-unknown/release/prediction_market.wasm

# Show what's contributing most
wasm-objdump -d target/wasm32-unknown-unknown/release/prediction_market.wasm | \
    grep -E "func\[.*\]" | sort -k3 -rn | head -20
```

Optimization tips:
- Use `#[inline]` for small helper functions
- Avoid large match arms with duplicate code
- Prefer `enum` over `bool` flags where possible

---

## User-Facing Gas Costs

### Staking a Poll

Total gas for a complete stake transaction:

```
Base transaction fee:     ~100 stroops (0.0001 XLM)
Stake operation:          ~12,000 instructions
  - Auth validation:      ~500
  - Poll lookup:         ~800
  - Token transfer:       ~4,000
  - Pool update:         ~2,000
  - Stake storage:       ~3,000
  - Event emission:      ~500
  - Platform stats:       ~1,200
────────────────────────────────────────
Estimated total:         ~12,000-15,000 instructions
```

### Claiming Winnings

```
Claim operation:         ~15,000 instructions
  - Auth validation:      ~500
  - Poll lookup:         ~800
  - Stake validation:     ~1,000
  - Winner check:       ~500
  - Winnings calculation: ~2,000
  - Token transfer:      ~4,000
  - Fee to treasury:     ~2,000
  - State update:        ~2,000
  - Event emission:      ~500
────────────────────────────────────────
Estimated total:         ~15,000-18,000 instructions
```

---

## Gas Optimization Tips

### For Developers

1. **Batch storage reads** — Read all needed data in one function rather than multiple small reads
2. **Use `get()` over `update()` pattern** — Read-modify-write in one call is cheaper than separate operations
3. **Avoid duplicate storage access** — Store frequently-read values in instance storage
4. **Limit event data** — Events cost gas; don't duplicate storage in events

### Patterns to Avoid

```rust
// BAD: Multiple storage reads
let a = env.storage().get(&Key::A)?;
let b = env.storage().get(&Key::B)?;
let c = env.storage().get(&Key::C)?;

// GOOD: Single read if possible (struct)
let state = env.storage().get(&Key::State)?;
let (a, b, c) = (state.a, state.b, state.c);
```

### Storage vs Memory

```rust
// BAD for frequently accessed data
fn get_foo() {
    let data = env.storage().get(&DataKey::Foo)?; // Storage read every call
}

// GOOD: Cache in instance if data changes rarely
fn get_foo() -> Foo {
    env.storage().instance().get(&DataKey::Foo)
}
```

---

## Benchmarking on Testnet

Run the gas benchmark script to measure actual costs:

```bash
# Build in release mode
cargo build --target wasm32-unknown-unknown --release

# Deploy to testnet (requires funded account)
./scripts/gas_benchmarks.sh testnet

# Output shows actual instruction counts per operation
```

### Manual Benchmark via CLI

```bash
# Measure invoke cost
stellar contract invoke \
    --id $PM_ID \
    --source user1 \
    --network testnet \
    --cost
```

---

## Cost Projection for Mainnet

Assuming 1 XLM = $0.50 and 1 operation = 15,000 instructions:

| Operation | Instructions | Cost (USD) |
|-----------|-------------|-------------|
| Stake | 15,000 | ~$0.0003 |
| Create Poll | 10,000 | ~$0.0002 |
| Claim | 18,000 | ~$0.0004 |
| Emergency Withdraw | 12,000 | ~$0.0002 |

For a user placing 10 stakes per month:
- Monthly cost: ~$0.003
- **PredictX is extremely cost-effective compared to Ethereum-based alternatives**

---

## Phase 2 Gas Considerations

When voting and disputes are added:

| New Operation | Estimated Cost | Notes |
|--------------|---------------|-------|
| Cast vote | ~8,000 | Write vote record |
| Resolve dispute | ~25,000 | Multi-sig + complex logic |
| Auto-resolve | ~20,000 | Tally + threshold check |
