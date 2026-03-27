# PredictX Events Reference

Complete reference for all events emitted by PredictX smart contracts.

## PredictionMarket Events

### PollCreated

Emitted when a new prediction poll is created.

**Topic:** `(Symbol: "PollCreated", poll_id: u64)`

**Data:** `()`

```rust
env.events().publish(
    (Symbol::new(&env, "PollCreated"), poll_id),
    ()
);
```

| Field | Type | Description |
|-------|------|-------------|
| poll_id | u64 | Unique poll identifier |

**Example CLI:**
```bash
stellar contract events --source-account admin --network testnet \
    --type PollCreated
```

---

### PollCancelled

Emitted when an admin cancels a poll (via `cancel_poll`).

**Topic:** `(Symbol: "PollCancelled")`

**Data:** `poll_id: u64`

```rust
env.events().publish(
    (Symbol::new(&env, "PollCancelled"),),
    poll_id
);
```

| Field | Type | Description |
|-------|------|-------------|
| poll_id | u64 | ID of cancelled poll |

---

### EmergencyWithdrawal

Emitted when a user successfully withdraws their stake via emergency withdrawal.

**Topic:** `(Symbol: "EmergencyWithdrawal", poll_id: u64, user: Address)`

**Data:** `amount: i128` — Amount refunded to user

```rust
env.events().publish(
    (Symbol::new(&env, "EmergencyWithdrawal"), poll_id, user.clone()),
    stake.amount
);
```

| Field | Type | Description |
|-------|------|-------------|
| poll_id | u64 | Poll ID |
| user | Address | User who withdrew |
| amount | i128 | Token amount refunded |

---

### ContractPaused

Emitted when the contract enters paused state.

**Topic:** `(Symbol: "ContractPaused")`

**Data:** `true: bool`

```rust
env.events().publish(
    (Symbol::new(&env, "ContractPaused"),),
    true
);
```

**Effects:** When paused, all user-facing functions (`stake`, `create_poll`, etc.) are blocked. Only `unpause` and `emergency_withdraw` remain callable.

---

### ContractUnpaused

Emitted when the contract exits paused state.

**Topic:** `(Symbol: "ContractUnpaused")`

**Data:** `false: bool`

```rust
env.events().publish(
    (Symbol::new(&env, "ContractUnpaused"),),
    false
);
```

---

### StakePlaced

Emitted when a user successfully places a stake on a poll.

**Topic:** `(Symbol: "StakePlaced", poll_id: u64, staker: Address)`

**Data:** `(amount: i128, side: StakeSide)`

```rust
env.events().publish(
    (Symbol::new(&env, "StakePlaced"), poll_id, staker),
    (amount, side)
);
```

| Field | Type | Description |
|-------|------|-------------|
| poll_id | u64 | Poll ID |
| staker | Address | User who staked |
| amount | i128 | Stake amount in token units |
| side | StakeSide | `Yes = 0` or `No = 1` |

**Example CLI:**
```bash
stellar contract invoke --id $PM_ID --source user1 --network testnet \
    -- stake --staker user1 --poll_id 1 --amount 1000000000 --side Yes

# Event output:
# StakePlaced(poll_id=1, staker=GBXXX..., amount=1000000000, side=Yes)
```

---

## VotingOracle Events

The VotingOracle contract currently does not emit custom events beyond standard Soroban storage events. Status transitions are managed internally via `StoredPollStatus` and queried via `get_poll_status`.

### Future Events (Phase 2)

Planned events for full voting implementation:

| Event | Description |
|-------|-------------|
| `VotingStarted` | Emitted when voting opens for a poll |
| `VoteCast` | Emitted when a user casts a vote |
| `AutoResolved` | Emitted when poll auto-resolves via consensus |
| `AdminVerified` | Emitted when admin reviews and verifies outcome |
| `DisputeInitiated` | Emitted when a dispute is raised |
| `DisputeResolved` | Emitted when dispute is resolved via multi-sig |

---

## Treasury Events

The Treasury contract currently does not emit custom events. Balance tracking is internal via `DataKey::Balance`.

### Future Events (Phase 2)

Planned events for full Treasury integration:

| Event | Description |
|-------|-------------|
| `FeesDeposited` | Emitted when platform fees are deposited |
| `VoterRewardDistributed` | Emitted when voter rewards are distributed |
| `AdminWithdrawal` | Emitted when admin withdraws fees |

---

## Event Subscribing

### Using Stellar CLI

```bash
# Subscribe to all PredictX events
stellar contract events --network testnet --source-account user1

# Filter by event type
stellar contract events --network testnet --type StakePlaced

# Filter by contract
stellar contract events --network testnet --contract-id $PM_ID
```

### Using Soroban SDK

```rust
use soroban_sdk::Events;

env.events().publish(
    (Symbol::new(&env, "StakePlaced"), poll_id, staker),
    (amount, side)
);

// Subscribe via Client
let events = client.events()?;
```

---

## Event Data Schemas

### StakePlaced Data Structure

```
Topics:   ["StakePlaced", poll_id: u64, staker: Address]
Data:     [amount: i128, side: u32]
```

### PollCreated Data Structure

```
Topics:   ["PollCreated", poll_id: u64]
Data:     []
```

### EmergencyWithdrawal Data Structure

```
Topics:   ["EmergencyWithdrawal", poll_id: u64, user: Address]
Data:     [amount: i128]
```

---

## Event Best Practices

1. **Always subscribe to events** rather than polling contract state
2. **Use topic filters** to reduce event stream noise
3. **Handle missing events** — blockchain reorganizations can skip events
4. **Store event cursor** — save last processed event cursor for resume
5. **Verify event authenticity** — always check the emitting contract address
