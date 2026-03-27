# PredictionMarket Contract

Main contract for poll creation, staking, match management, and winnings distribution.

**Source:** [`contracts/prediction-market/src/lib.rs`](https://github.com/samieazubike/predictx-contract/blob/main/contracts/prediction-market/src/lib.rs)

## Contract Overview

The `PredictionMarket` contract is the core of the PredictX protocol. It handles:

- Creating prediction polls for football matches
- Accepting and tracking user stakes on Yes/No outcomes
- Managing football match metadata
- Processing emergency withdrawals when polls are cancelled/disputed
- Pausing/unpausing contract for emergencies

## Initialization

### `initialize`

Initializes the contract with admin, oracle, token, and treasury addresses.

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    voting_oracle: Address,
    token_address: Address,
    treasury_address: Address,
    platform_fee_bps: u32,
) -> Result<(), PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| admin | Address | Contract administrator |
| voting_oracle | Address | VotingOracle contract address |
| token_address | Address | Token contract for staking |
| treasury_address | Address | Treasury contract for fees |
| platform_fee_bps | u32 | Platform fee in basis points (e.g., 500 = 5%) |

**Returns:** `Ok(())` on success

**Access:** Admin only (via `require_auth`)

**Errors:**
- `AlreadyInitialized` — Contract already initialized

**Events:** None

**Example (CLI):**
```bash
stellar contract invoke --id $PM_ID --source admin --network testnet \
    -- initialize \
    --admin GDQEO2HGZCH7TSHB7JWT6UWUMSWGAHFFG4VTC6YMGJLb3JB3TJJHGWA \
    --voting_oracle GDQEO2HGZCH7TSHB7JWT6UWUMSWGAHFFG4VTC6YMGJLb3JB3TJJHGWA \
    --token_address GDQEO2HGZCH7TSHB7JWT6UWUMSWGAHFFG4VTC6YMGJLb3JB3TJJHGWA \
    --treasury_address GDQEO2HGZCH7TSHB7JWT6UWUMSWGAHFFG4VTC6YMGJLb3JB3TJJHGWA \
    --platform_fee_bps 500
```

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

### `oracle`

Returns the VotingOracle contract address.

```rust
pub fn oracle(env: Env) -> Result<Address, PredictXError>
```

**Returns:** Oracle address

**Errors:**
- `NotInitialized` — Contract not initialized

---

### `set_oracle`

Updates the VotingOracle contract address.

```rust
pub fn set_oracle(env: Env, voting_oracle: Address) -> Result<(), PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| voting_oracle | Address | New oracle address |

**Access:** Admin only

**Errors:**
- `EmergencyWithdrawNotAllowed` — Contract is paused
- `Unauthorized` — Caller is not admin

**Events:** None

---

### `pause`

Pauses the contract, blocking all user-facing operations.

```rust
pub fn pause(env: Env, admin: Address) -> Result<(), PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| admin | Address | Admin address for authentication |

**Access:** Admin only

**Events:** `ContractPaused { true }`

**Effects:** When paused:
- `stake()` — blocked
- `create_poll()` — blocked
- `create_match()` — blocked
- `set_oracle()` — blocked
- Emergency withdrawal remains allowed

---

### `unpause`

Unpauses the contract, restoring normal operation.

```rust
pub fn unpause(env: Env, admin: Address) -> Result<(), PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| admin | Address | Admin address for authentication |

**Access:** Admin only

**Events:** `ContractUnpaused { false }`

---

### `is_paused`

Returns whether the contract is currently paused.

```rust
pub fn is_paused(env: Env) -> bool
```

**Returns:** `true` if paused, `false` otherwise

---

### `oracle_poll_status`

Queries the VotingOracle for a poll's current status.

```rust
pub fn oracle_poll_status(env: Env, poll_id: u64) -> Result<PollStatus, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| poll_id | u64 | Poll ID to query |

**Returns:** Current `PollStatus` from oracle

**Errors:**
- `NotInitialized` — Oracle not set

---

### `cancel_poll`

Cancels a poll, marking it as cancelled in the oracle.

```rust
pub fn cancel_poll(env: Env, admin: Address, poll_id: u64) -> Result<(), PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| admin | Address | Admin address |
| poll_id | u64 | Poll to cancel |

**Access:** Admin only

**Events:** `PollCancelled { poll_id }`

**Effects:** Users can then call `emergency_withdraw()` to reclaim stakes.

---

### `check_emergency_eligible`

Checks if a user is eligible for emergency withdrawal on a poll.

```rust
pub fn check_emergency_eligible(env: Env, poll_id: u64) -> bool
```

**Returns:** `true` if eligible, `false` otherwise

**Eligibility conditions:**
- Poll status is `Cancelled`
- Poll status is `Disputed` and 7+ days since last status update
- Poll status is `Locked` and 7+ days since last status update

---

### `emergency_withdraw`

Allows a user to reclaim their stake after a poll is cancelled or after a 7-day timeout on disputed/locked polls.

```rust
pub fn emergency_withdraw(env: Env, user: Address, poll_id: u64) -> Result<i128, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| user | Address | User reclaiming stake |
| poll_id | u64 | Poll to withdraw from |

**Returns:** Amount refunded

**Access:** User (via `require_auth`)

**Errors:**
- `AlreadyClaimed` — User already withdrew
- `NotInitialized` — Oracle not set
- `EmergencyWithdrawNotAllowed` — Not eligible (see `check_emergency_eligible`)
- `NotStaker` — User has no stake on this poll

**Events:** `EmergencyWithdrawal { poll_id, user, amount }`

---

## Poll Management

### `create_poll`

Creates a new prediction poll for a match.

```rust
pub fn create_poll(
    env: Env,
    creator: Address,
    match_id: u64,
    question: String,
    category: PollCategory,
    lock_time: u64,
) -> Result<u64, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| creator | Address | Poll creator |
| match_id | u64 | Associated match ID |
| question | String | Prediction question (max 256 chars) |
| category | PollCategory | Category (PlayerEvent, TeamEvent, etc.) |
| lock_time | u64 | Unix timestamp after which staking is disabled |

**Returns:** New poll ID

**Access:** Any authenticated user

**Errors:**
- `EmergencyWithdrawNotAllowed` — Contract is paused
- `MatchNotFound` — Match does not exist
- `InvalidLockTime` — Lock time is not in the future
- `MaxPollsPerMatchReached` — Match has 50+ polls

**Events:** `PollCreated { poll_id }`

**Constraints:**
- `lock_time > env.ledger().timestamp()`
- Poll count per match ≤ 50

---

### `get_poll`

Retrieves full poll data.

```rust
pub fn get_poll(env: Env, poll_id: u64) -> Result<Poll, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| poll_id | u64 | Poll ID |

**Returns:** `Poll` struct with all poll data

**Errors:**
- `PollNotFound` — Poll does not exist

---

## Staking

### `stake`

Places a stake on a poll outcome.

```rust
pub fn stake(
    env: Env,
    staker: Address,
    poll_id: u64,
    amount: i128,
    side: StakeSide,
) -> Result<Stake, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| staker | Address | User placing the stake |
| poll_id | u64 | Poll to stake on |
| amount | i128 | Token amount (7 decimal places) |
| side | StakeSide | `Yes` or `No` |

**Returns:** Created `Stake` record

**Access:** Staker (via `require_auth`)

**Errors:**
- `EmergencyWithdrawNotAllowed` — Contract is paused
- `StakeAmountZero` — Amount ≤ 0
- `StakeBelowMinimum` — Amount < MIN_STAKE_AMOUNT (10,000,000)
- `PollNotFound` — Poll does not exist
- `PollNotActive` — Poll is not Active
- `PollLocked` — Current time ≥ lock_time
- `AlreadyStaked` — User already staked on this poll
- `InsufficientBalance` — User has insufficient token balance

**Events:** `StakePlaced { poll_id, staker, (amount, side) }`

**Example (CLI):**
```bash
stellar contract invoke --id $PM_ID --source user1 --network testnet \
    -- stake \
    --staker GDQEO2HGZCH7TSHB7JWT6UWUMSWGAHFFG4VTC6YMGJLb3JB3TJJHGWA \
    --poll_id 1 \
    --amount 1000000000 \
    --side Yes
```

---

### `get_stake_info`

Retrieves a user's stake record for a poll.

```rust
pub fn get_stake_info(env: Env, poll_id: u64, user: Address) -> Result<Stake, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| poll_id | u64 | Poll ID |
| user | Address | User address |

**Returns:** User's `Stake` record

**Errors:**
- `NotStaker` — User has no stake on this poll

---

### `get_user_stakes`

Lists all poll IDs a user has staked on.

```rust
pub fn get_user_stakes(env: Env, user: Address) -> Vec<u64>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| user | Address | User address |

**Returns:** Vector of poll IDs

---

### `has_user_staked`

Checks if a user has already staked on a poll.

```rust
pub fn has_user_staked(env: Env, poll_id: u64, user: Address) -> bool
```

| Parameter | Type | Description |
|-----------|------|-------------|
| poll_id | u64 | Poll ID |
| user | Address | User address |

**Returns:** `true` if user has staked

---

### `calculate_potential_winnings`

Calculates potential winnings for a stake amount (read-only, for UI preview).

```rust
pub fn calculate_potential_winnings(
    env: Env,
    poll_id: u64,
    side: StakeSide,
    amount: i128,
) -> Result<i128, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| poll_id | u64 | Poll ID |
| side | StakeSide | Proposed stake side |
| amount | i128 | Proposed stake amount |

**Returns:** Estimated winnings (not guaranteed due to integer rounding)

**Formula:**
```
winnings = amount * total_pool_after * (BPS_DENOMINATOR - fee_bps)
            / (pool_on_side_after * BPS_DENOMINATOR)
```

**Errors:**
- `StakeAmountZero` — Amount ≤ 0
- `PollNotFound` — Poll does not exist

---

### `get_pool_info`

Returns current pool amounts and participant counts.

```rust
pub fn get_pool_info(env: Env, poll_id: u64) -> Result<PoolInfo, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| poll_id | u64 | Poll ID |

**Returns:** `PoolInfo { yes_pool, no_pool, yes_count, no_count }`

**Errors:**
- `PollNotFound` — Poll does not exist

---

### `get_platform_stats`

Returns aggregate platform statistics.

```rust
pub fn get_platform_stats(env: Env) -> PlatformStats
```

**Returns:** `PlatformStats` with totals:
- `total_value_locked` — Sum of all active stakes
- `total_polls_created` — Total polls ever created
- `total_stakes_placed` — Total stakes ever placed
- `total_payouts` — Total payouts distributed
- `total_users` — Total unique users

---

## Token Functions

### `get_token_address`

Returns the staking token contract address.

```rust
pub fn get_token_address(env: Env) -> Result<Address, PredictXError>
```

---

### `get_treasury_address`

Returns the treasury contract address.

```rust
pub fn get_treasury_address(env: Env) -> Result<Address, PredictXError>
```

---

### `get_platform_fee_bps`

Returns the configured platform fee in basis points.

```rust
pub fn get_platform_fee_bps(env: Env) -> u32
```

**Returns:** Fee in BPS (e.g., 500 = 5%)

---

### `get_contract_balance`

Returns the contract's token balance.

```rust
pub fn get_contract_balance(env: Env) -> Result<i128, PredictXError>
```

**Returns:** Total tokens held by contract

---

## Match Management

### `create_match`

Creates a new football match record.

```rust
pub fn create_match(
    env: Env,
    admin: Address,
    home_team: String,
    away_team: String,
    league: String,
    venue: String,
    kickoff_time: u64,
) -> Result<u64, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| admin | Address | Admin address |
| home_team | String | Home team name |
| away_team | String | Away team name |
| league | String | League/competition name |
| venue | String | Stadium/venue name |
| kickoff_time | u64 | Unix timestamp for kickoff |

**Access:** Admin only

**Returns:** New match ID

**Errors:**
- `Unauthorized` — Caller is not admin
- `InvalidLockTime` — Kickoff time is not in the future

---

### `update_match`

Updates match details (only before match starts).

```rust
pub fn update_match(
    env: Env,
    admin: Address,
    match_id: u64,
    home_team: Option<String>,
    away_team: Option<String>,
    league: Option<String>,
    venue: Option<String>,
    kickoff_time: Option<u64>,
) -> Result<Match, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| admin | Address | Admin address |
| match_id | u64 | Match to update |
| home_team | Option<String> | New home team (or None to skip) |
| away_team | Option<String> | New away team |
| league | Option<String> | New league |
| venue | Option<String> | New venue |
| kickoff_time | Option<u64> | New kickoff time |

**Access:** Admin only

**Returns:** Updated `Match` struct

**Errors:**
- `Unauthorized` — Caller is not admin
- `MatchNotFound` — Match does not exist
- `MatchAlreadyStarted` — Match kickoff has passed

---

### `finish_match`

Marks a match as finished (prerequisite for poll resolution).

```rust
pub fn finish_match(env: Env, admin: Address, match_id: u64) -> Result<(), PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| admin | Address | Admin address |
| match_id | u64 | Match to finish |

**Access:** Admin only

**Errors:**
- `Unauthorized` — Caller is not admin
- `MatchNotFound` — Match does not exist

---

### `get_match`

Retrieves match data.

```rust
pub fn get_match(env: Env, match_id: u64) -> Result<Match, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| match_id | u64 | Match ID |

**Returns:** `Match` struct

**Errors:**
- `MatchNotFound` — Match does not exist

---

### `get_match_polls`

Lists all poll IDs associated with a match.

```rust
pub fn get_match_polls(env: Env, match_id: u64) -> Result<Vec<u64>, PredictXError>
```

| Parameter | Type | Description |
|-----------|------|-------------|
| match_id | u64 | Match ID |

**Returns:** Vector of poll IDs

**Errors:**
- `MatchNotFound` — Match does not exist

---

### `get_match_count`

Returns total number of matches created.

```rust
pub fn get_match_count(env: Env) -> u64
```

**Returns:** Match count

---

## Data Structures

### Poll

```rust
struct Poll {
    pub poll_id: u64,
    pub match_id: u64,
    pub creator: Address,
    pub question: String,
    pub category: PollCategory,
    pub lock_time: u64,
    pub yes_pool: i128,
    pub no_pool: i128,
    pub yes_count: u32,
    pub no_count: u32,
    pub status: PollStatus,
    pub outcome: Option<bool>,
    pub resolution_time: u64,
    pub created_at: u64,
}
```

### Stake

```rust
struct Stake {
    pub user: Address,
    pub poll_id: u64,
    pub amount: i128,
    pub side: StakeSide,
    pub claimed: bool,
    pub staked_at: u64,
}
```

### PoolInfo

```rust
struct PoolInfo {
    pub yes_pool: i128,
    pub no_pool: i128,
    pub yes_count: u32,
    pub no_count: u32,
}
```

### Match

```rust
struct Match {
    pub match_id: u64,
    pub home_team: String,
    pub away_team: String,
    pub league: String,
    pub venue: String,
    pub kickoff_time: u64,
    pub created_by: Address,
    pub is_finished: bool,
}
```
