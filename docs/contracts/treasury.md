# Treasury Contract

Holds platform fees and manages per-user balance accounting.

**Source:** [`contracts/treasury/src/lib.rs`](https://github.com/samieazubike/predictx-contract/blob/main/contracts/treasury/src/lib.rs)

## Contract Overview

The `Treasury` contract is responsible for:

- Tracking per-user balance accounting
- Accepting deposits from users
- Holding platform fees collected from prediction polls

**Current Phase:** Phase 1 (Scaffolding) — Real token transfers via Soroban token interface are planned for Phase 2.

---

## Initialization

### `initialize`

Initializes the Treasury contract with an admin address.

```rust
pub fn initialize(env: Env, admin: Address) -> Result<(), PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| admin | Address | Contract administrator |

**Access:** Admin only (via `require_auth`)

**Errors:**
- `AlreadyInitialized` — Contract already initialized

**Events:** None

---

## Admin Functions

### `admin`

Returns the contract administrator address.

```rust
pub fn admin(env: Env) -> Result<Address, PredictXError>
```

**Returns:** Admin address

**Errors:**
- `NotInitialized` — Contract not initialized

---

## User Functions

### `deposit`

Records a deposit to a user's internal balance (placeholder for Phase 1).

```rust
pub fn deposit(env: Env, from: Address, amount: i128) -> Result<i128, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| from | Address | User depositing |
| amount | i128 | Amount to deposit |

**Access:** User (via `require_auth`)

**Returns:** New total balance for user

**Errors:**
- `StakeAmountZero` — Amount ≤ 0
- `NotInitialized` — Contract not initialized

**Events:** None

**Note:** This is a placeholder accounting method. Real token transfers will be integrated in Phase 2.

---

### `balance`

Returns a user's current balance.

```rust
pub fn balance(env: Env, who: Address) -> Result<i128, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| who | Address | User to query |

**Returns:** User's current balance

**Errors:**
- `NotInitialized` — Contract not initialized

---

## Planned Phase 2 Features

The current implementation is scaffolding for Phase 1. Planned Phase 2 additions:

| Feature | Description |
|---------|-------------|
| Real Token Deposits | Accept actual token transfers |
| Fee Collection | Receive 5% platform fee from PredictionMarket |
| Voter Rewards Distribution | Distribute 0.5-1% of pool to voters |
| Admin Withdrawal | Allow admin to withdraw accumulated fees |
| Fee Percentage Configuration | Make fee configurable by admin |

---

## Storage Design

| Key | Type | Description |
|-----|------|-------------|
| `DataKey::Admin` | Address | Contract admin |
| `DataKey::Balance(Address)` | i128 | Per-user balance |

---

## Data Structures

### Balance Tracking

```rust
fn get_balance(env: &Env, who: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Balance(who.clone()))
        .unwrap_or(0_i128)
}
```

---

## Errors

| Code | Error | Description |
|------|-------|-------------|
| 1 | `NotInitialized` | Contract not initialized |
| 2 | `AlreadyInitialized` | Contract already initialized |
| 10 | `StakeAmountZero` | Deposit amount must be > 0 |

---

## Example Usage

```bash
# Deposit tokens (Phase 1 - internal accounting only)
stellar contract invoke --id $TREASURY_ID --source user1 --network testnet \
    -- deposit \
    --from GDQEO2HGZCH7TSHB7JWT6UWUMSWGAHFFG4VTC6YMGJLb3JB3TJJHGWA \
    --amount 1000000000

# Check balance
stellar contract invoke --id $TREASURY_ID --source user1 --network testnet \
    -- balance \
    --who GDQEO2HGZCH7TSHB7JWT6UWUMSWGAHFFG4VTC6YMGJLb3JB3TJJHGWA
```
