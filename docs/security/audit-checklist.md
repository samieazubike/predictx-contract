# Pre-Audit Checklist

Comprehensive checklist for security auditing of PredictX smart contracts. Use before Phase 4 mainnet launch.

## Pre-Audit Preparation

### Documentation

- [ ] Architecture documentation complete
- [ ] All public functions documented
- [ ] Security model documented
- [ ] Access control matrix verified against code
- [ ] Upgrade path documented

### Code Quality

- [ ] `cargo clippy --all --all-targets -- -D warnings` passes
- [ ] `cargo fmt` applied consistently
- [ ] No `unsafe` blocks in production code
- [ ] No hardcoded secrets or addresses
- [ ] `TODO` and `FIXME` comments addressed

### Testing

- [ ] All unit tests pass: `cargo test --all`
- [ ] Integration tests cover all public functions
- [ ] Edge case tests for all error paths
- [ ] Concurrent access tests (if applicable)
- [ ] Overflow/underflow tests

---

## Access Control Audit

### Admin Functions

- [ ] Every admin-only function has `require_auth` check
- [ ] Admin address stored during `initialize()`, not constructor
- [ ] Comparison uses `*caller != stored_admin` (not `caller != stored_admin`)
- [ ] `initialize()` can only be called once (checked via `has()`)
- [ ] Unauthorized calls revert with `PredictXError::Unauthorized`

### User Functions

- [ ] All user-write functions validate caller via `require_auth`
- [ ] No function allows caller to modify another user's state without authorization
- [ ] Read functions have no auth requirement (anyone can query)

### Pause Mechanism

- [ ] `pause()` sets `Paused = true`
- [ ] `unpause()` sets `Paused = false`
- [ ] All state-changing user functions call `ensure_not_paused()`
- [ ] `emergency_withdraw` still works when paused
- [ ] Events emitted on pause/unpause

---

## Storage Audit

### DataKey Namespacing

- [ ] All storage keys defined in `DataKey` enum
- [ ] No raw string keys used
- [ ] No key collisions between different data types
- [ ] `Instance` vs `Persistent` storage correctly chosen

### Storage Access Patterns

- [ ] `instance()` used for: config, counters, admin address
- [ ] `persistent()` used for: polls, stakes, matches, user data
- [ ] No `temporary()` storage used for persistent data

---

## Token Handling Audit

### Transfer Operations

- [ ] All token transfers use Soroban token interface
- [ ] `transfer_from` called with correct `from` address
- [ ] `require_auth` called before any token movement
- [ ] Transfer amounts validated before calling token client

### Balance Validation

- [ ] User balance checked before staking
- [ ] Contract balance verified before withdrawals
- [ ] Allowance validated if used

### Checks-Effects-Interactions

- [ ] All state checks happen before external calls
- [ ] State updates happen after external calls
- [ ] No reentrancy vulnerabilities

```rust
// CORRECT order:
if !condition { return Err(); }     // Check
token.transfer(...)?;                 // Interact
storage().set(...);                   // Effect
```

---

## Poll and Stake Audit

### Poll Creation

- [ ] `lock_time` must be in the future
- [ ] `match_id` must reference existing match
- [ ] Max polls per match enforced (50)
- [ ] Poll ID is auto-incremented correctly
- [ ] Event emitted on creation

### Staking

- [ ] Cannot stake twice on same poll (`HasStaked` check)
- [ ] Cannot stake after `lock_time` (`PollLocked` check)
- [ ] Cannot stake on non-Active poll (`PollNotActive` check)
- [ ] Minimum stake amount enforced
- [ ] Token transfer succeeds before state update
- [ ] Pool totals (`yes_pool`, `no_pool`) updated atomically
- [ ] User stake record stored correctly

### Emergency Withdrawal

- [ ] Only eligible after cancellation or 7-day timeout
- [ ] Cannot claim twice (`EmergencyClaimed` check)
- [ ] Full stake amount refunded
- [ ] `total_value_locked` decremented
- [ ] Event emitted

---

## Match Management Audit

### Match Creation

- [ ] Only admin can create matches
- [ ] `kickoff_time` must be in future
- [ ] Match ID auto-incremented
- [ ] Empty poll list initialized

### Match Updates

- [ ] Cannot update after kickoff (`MatchAlreadyStarted` check)
- [ ] Cannot set kickoff time to past
- [ ] All optional fields handled correctly

### Match Completion

- [ ] `finish_match` requires admin
- [ ] Sets `is_finished = true`
- [ ] Cannot finish twice

---

## Cross-Contract Audit

### VotingOracle Interaction

- [ ] `PredictionMarket` stores oracle address at initialization
- [ ] All status queries go through `VotingOracle` client
- [ ] No trust assumptions beyond stored address

### Cross-Contract Reentrancy

- [ ] No callback patterns that could re-enter
- [ ] All cross-contract calls use safe Soroban interface

---

## Integer Safety

### Overflow/Underflow

- [ ] All arithmetic uses `i128` (large enough for token amounts)
- [ ] Release build has overflow checks enabled
- [ ] `saturating_sub` used where appropriate
- [ ] No manual arithmetic without checked operations

### Fee Calculations

- [ ] Fee BPS validated (e.g., 0 < fee < 10_000)
- [ ] Fee calculation uses proper denominator (`BPS_DENOMINATOR`)
- [ ] Fee applied before pool update

---

## Event Audit

### Required Events

- [ ] `PollCreated` — emitted on every poll creation
- [ ] `PollCancelled` — emitted on admin cancellation
- [ ] `StakePlaced` — emitted with amount and side
- [ ] `EmergencyWithdrawal` — emitted with poll_id, user, amount
- [ ] `ContractPaused` / `ContractUnpaused` — emitted on state change
- [ ] `MatchCreated` — emitted on match creation

### Event Quality

- [ ] Events include all relevant state changes
- [ ] Topics are descriptive and unique
- [ ] No sensitive data in events

---

## Error Handling Audit

### Error Codes

- [ ] All error codes defined in `PredictXError` enum
- [ ] No duplicate error codes
- [ ] Error messages are descriptive
- [ ] All error paths tested

### Panic Handling

- [ ] No `panic!` in production code paths
- [ ] No `unwrap()` on user-controlled data
- [ ] `expect()` only on known-good values

---

## Gas and Performance

### Gas Optimization

- [ ] Storage reads minimized in loops
- [ ] No unnecessary storage writes
- [ ] Events used instead of storage for historical data

### Resource Limits

- [ ] Loop bounds validated
- [ ] Vector push bounds checked
- [ ] No unbounded loops

---

## Specific Vulnerability Checks

### Reentrancy

- [ ] No `transfer` before state update
- [ ] No callback mechanisms
- [ ] Soroban token interface confirmed reentrancy-safe

### Front-Running

- [ ] Lock times enforced
- [ ] No sensitive on-chain ordering dependencies

### Access Control

- [ ] `require_auth` on all privileged functions
- [ ] Admin checks on both storage and signature
- [ ] No `tx_source_account` reliance alone

### Input Validation

- [ ] All inputs validated before use
- [ ] No assumptions about caller-provided data
- [ ] String length limits enforced

### Integer Overflow

- [ ] Arithmetic operations checked
- [ ] Fee calculations validated
- [ ] Pool totals cannot overflow

---

## Phase-Specific Checks

### Phase 1 Completeness

- [ ] Oracle is admin-controlled only (no autonomous resolution)
- [ ] Voting is not yet active
- [ ] Dispute resolution not yet active
- [ ] Treasury is accounting-only (no real token movement)

---

## Final Checklist

### Documentation

- [ ] This checklist is complete and passing
- [ ] Architecture docs match implementation
- [ ] All known limitations documented
- [ ] Upgrade path documented

### Code

- [ ] Zero `unsafe` in contract code
- [ ] Zero compiler warnings
- [ ] Test coverage > 80% for critical paths

### External Audit

- [ ] Third-party audit completed
- [ ] All audit findings addressed
- [ ] Bug bounty program launched
