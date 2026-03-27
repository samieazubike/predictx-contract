# PollFactory Contract

Factory contract for modular poll creation and discovery.

**Source:** [`contracts/poll-factory/src/lib.rs`](https://github.com/samieazubike/predictx-contract/blob/main/contracts/poll-factory/src/lib.rs)

## Contract Overview

The `PollFactory` contract provides a factory pattern for creating polls independently from the `PredictionMarket`. It:

- Creates polls with creator authentication
- Stores polls persistently
- Provides poll retrieval by ID

**Current Phase:** Phase 1 (Scaffolding) — This is a standalone factory that mirrors `PredictionMarket`'s poll creation. In later phases, it may become the primary poll creation entry point with `PredictionMarket` delegating.

**Note:** The `PredictionMarket` contract has primary poll creation logic with match linking. `PollFactory` creates polls with `match_id = 0` (not linked to matches).

---

## Initialization

### `initialize`

Initializes the PollFactory with an admin address.

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

## Poll Creation

### `create_poll`

Creates a new standalone poll (not linked to a match).

```rust
pub fn create_poll(
    env: Env,
    creator: Address,
    question: String,
    lock_timestamp: u64,
) -> Result<u64, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| creator | Address | Poll creator |
| question | String | Prediction question |
| lock_timestamp | u64 | Unix timestamp when staking locks |

**Returns:** New poll ID

**Access:** Any authenticated user (via `require_auth`)

**Errors:**
- `NotInitialized` — Contract not initialized

**Events:** None

**Note:** Polls created here have `match_id = 0` and `category = Other`. For match-linked polls with categories, use `PredictionMarket.create_poll()`.

---

### `get_poll`

Retrieves a poll by ID.

```rust
pub fn get_poll(env: Env, poll_id: u64) -> Result<Poll, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| poll_id | u64 | Poll ID |

**Returns:** `Poll` struct

**Errors:**
- `PollNotFound` — Poll does not exist

---

## Poll Structure

Polls created via PollFactory have these initial values:

```rust
Poll {
    poll_id: auto-incremented,
    match_id: 0,                    // Not linked to a match
    creator: creator,                // User who created
    question: provided,              // User-provided question
    category: PollCategory::Other,   // Default category
    lock_time: lock_timestamp,       // User-specified lock time
    yes_pool: 0,
    no_pool: 0,
    yes_count: 0,
    no_count: 0,
    status: PollStatus::Active,
    outcome: None,
    resolution_time: 0,
    created_at: env.ledger().timestamp(),
}
```

---

## Storage Design

| Key | Type | Description |
|-----|------|-------------|
| `DataKey::Admin` | Address | Contract admin |
| `DataKey::NextPollId` | u64 | Auto-incrementing poll ID |
| `DataKey::Poll(u64)` | Poll | Per-poll data |

---

## Comparison: PollFactory vs PredictionMarket

| Feature | PollFactory | PredictionMarket |
|---------|-------------|------------------|
| Match-linked polls | No (match_id = 0) | Yes |
| Category support | No (Other only) | Yes (PlayerEvent, TeamEvent, etc.) |
| Poll creation | Standalone | Linked to match |
| Staking | Not supported | Full staking support |
| Match management | No | Yes (create_match, etc.) |
| Emergency withdrawal | No | Yes |

---

## Errors

| Code | Error | Description |
|------|-------|-------------|
| 1 | `NotInitialized` | Contract not initialized |
| 2 | `AlreadyInitialized` | Contract already initialized |
| 3 | `Unauthorized` | Caller is not admin |
| 4 | `PollNotFound` | Poll does not exist |

---

## Example Usage

```bash
# Create a poll
stellar contract invoke --id $PF_ID --source user1 --network testnet \
    -- create_poll \
    --creator GDQEO2HGZCH7TSHB7JWT6UWUMSWGAHFFG4VTC6YMGJLb3JB3TJJHGWA \
    --question "Will BTC reach $100k by end of 2024?" \
    --lock_timestamp 1735689600

# Retrieve the poll
stellar contract invoke --id $PF_ID --source user1 --network testnet \
    -- get_poll \
    --poll_id 1
```

---

## Planned Phase 2+ Features

| Feature | Description |
|---------|-------------|
| Match Integration | Link polls to PredictionMarket matches |
| Category Support | Full category support (PlayerEvent, TeamEvent, etc.) |
| Discovery | Browse/list all polls across contracts |
| Poll Validation | Enforce question quality standards |
