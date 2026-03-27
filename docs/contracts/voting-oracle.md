# VotingOracle Contract

Manages poll status transitions and tracks voting state for outcome resolution.

**Source:** [`contracts/voting-oracle/src/lib.rs`](https://github.com/samieazubike/predictx-contract/blob/main/contracts/voting-oracle/src/lib.rs)

## Contract Overview

The `VotingOracle` contract tracks the lifecycle status of each poll and is called by `PredictionMarket` to:

- Set poll status (Active, Locked, Voting, Resolved, etc.)
- Query current poll status
- Track when status was last updated

**Current Phase:** Phase 1 (Scaffolding) — Full voting mechanics (community voting, consensus thresholds) are planned for Phase 2.

## Initialization

### `initialize`

Initializes the VotingOracle with an admin address.

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

### `set_poll_status`

Updates the status of a poll (called by PredictionMarket when admin triggers a transition).

```rust
pub fn set_poll_status(env: Env, poll_id: u64, status: PollStatus) -> Result<(), PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| poll_id | u64 | Poll ID to update |
| status | PollStatus | New status |

**Access:** Admin only (via `require_auth`)

**Errors:**
- `Unauthorized` — Caller is not admin

**Effects:**
- Stores `StoredPollStatus { status, updated_at: ledger.timestamp() }`
- Updates timestamp for emergency withdrawal timeout tracking

**Events:** None

**Example (CLI):**
```bash
stellar contract invoke --id $ORACLE_ID --source admin --network testnet \
    -- set_poll_status \
    --poll_id 1 \
    --status Resolved
```

---

## Query Functions

### `get_poll_status`

Returns the current status of a poll.

```rust
pub fn get_poll_status(env: Env, poll_id: u64) -> PollStatus
```

| Parameter | Type | Description |
|-----------|------|-------------|
| poll_id | u64 | Poll ID to query |

**Returns:** Current `PollStatus` (defaults to `Active` if never set)

**Errors:** None

---

### `get_poll_status_updated_at`

Returns the Unix timestamp when the poll status was last updated.

```rust
pub fn get_poll_status_updated_at(env: Env, poll_id: u64) -> u64
```

| Parameter | Type | Description |
|-----------|------|-------------|
| poll_id | u64 | Poll ID to query |

**Returns:** Unix timestamp of last update (0 if never updated)

**Errors:** None

**Use case:** Used by `PredictionMarket` to calculate emergency withdrawal eligibility (7-day timeout).

---

## PollStatus Enum

```rust
enum PollStatus {
    Active = 0,       // Accepting stakes
    Locked = 1,        // Past lock time, no more stakes
    Voting = 2,        // Match ended, community voting in progress
    AdminReview = 3,   // Vote consensus 60-85%, needs admin review
    Disputed = 4,      // Under dispute review
    Resolved = 5,      // Outcome determined, claims open
    Cancelled = 6,     // Emergency cancelled, refunds available
}
```

---

## Data Structures

### StoredPollStatus

Internal struct stored per poll:

```rust
struct StoredPollStatus {
    status: PollStatus,
    updated_at: u64,  // Unix timestamp of last status change
}
```

**Storage Key:** `DataKey::PollStatus(poll_id)`

---

## Cross-Contract Usage

The `VotingOracle` is designed to be called by `PredictionMarket`:

```rust
// In PredictionMarket, when admin calls cancel_poll():
let client = voting_oracle::Client::new(&env, &oracle_id);
client.set_poll_status(&poll_id, &voting_oracle::PollStatus::Cancelled);

// When checking emergency eligibility:
let status = client.get_poll_status(&poll_id);
let updated_at = client.get_poll_status_updated_at(&poll_id);
let elapsed = env.ledger().timestamp().saturating_sub(updated_at);
let eligible = elapsed >= EMERGENCY_TIMEOUT_SECS;
```

---

## Planned Phase 2 Features

The current implementation is scaffolding for Phase 1. Planned Phase 2 additions:

| Feature | Description |
|---------|-------------|
| Community Voting | Allow non-stakers to vote on outcomes |
| Consensus Tracking | Track yes/no/unclear vote counts |
| Auto-Resolution | Auto-resolve at >85% consensus |
| Admin Review | Route 60-85% consensus to admin |
| Dispute Resolution | Multi-sig based dispute handling |
| Voter Rewards | Distribute 0.5-1% of pool to voters |

---

## Storage Design

| Key | Type | Description |
|-----|------|-------------|
| `DataKey::Admin` | Address | Contract admin |
| `DataKey::PollStatus(u64)` | StoredPollStatus | Per-poll status + timestamp |

---

## Errors

| Code | Error | Description |
|------|-------|-------------|
| 1 | `NotInitialized` | Contract not initialized |
| 2 | `AlreadyInitialized` | Contract already initialized |
| 3 | `Unauthorized` | Caller is not admin |
