# Security Model

Threat model, mitigations, and security considerations for PredictX contracts.

## Overview

PredictX is a smart contract system managing real financial value. Security is paramount. This document outlines the threat model, mitigations, and best practices.

---

## Trust Model

### Trusted Parties

| Party | Trust Level | Rationale |
|-------|-------------|-----------|
| Contract Admin | Full trust | Can pause, cancel polls, update oracle |
| Users | Trust assumptions | Can stake, vote, claim |
| Oracle | Trust assumptions | Returns accurate poll status |

### Trust Boundaries

```
Users ──token transfers──▶ Contract ──status queries──▶ Oracle
                            │
                            └───fee collection──▶ Treasury
```

---

## Threat Model

### T1: Unauthorized Admin Actions

**Description:** Non-admin calls admin-only functions.

**Impact:** Could pause contract, cancel polls, drain funds.

**Mitigation:**
- All admin functions use `require_auth` and check stored admin address
- Admin address stored during initialization, not constructor

**Code:**
```rust
let stored_admin = get_admin(&env)?;
if admin != stored_admin {
    return Err(PredictXError::Unauthorized);
}
admin.require_auth();
```

**Severity:** Critical

---

### T2: Double Staking

**Description:** User stakes twice on the same poll.

**Impact:** Unfair advantage, pool imbalance.

**Mitigation:**
- `HasStaked(poll_id, user)` flag checked before staking
- Error returned: `AlreadyStaked`

**Code:**
```rust
if env.storage().persistent().has(&DataKey::HasStaked(poll_id, staker.clone())) {
    return Err(PredictXError::AlreadyStaked);
}
```

**Severity:** High

---

### T3: Staking After Lock Time

**Description:** User stakes after poll's lock time.

**Impact:** Late stakers gain unfair advantage.

**Mitigation:**
- Timestamp check: `env.ledger().timestamp() >= poll.lock_time`
- Error returned: `PollLocked`

**Code:**
```rust
if env.ledger().timestamp() >= poll.lock_time {
    return Err(PredictXError::PollLocked);
}
```

**Severity:** High

---

### T4: Insufficient Balance

**Description:** User stakes more than their token balance.

**Impact:** Failed transfers, state inconsistency.

**Mitigation:**
- Soroban token `transfer_from` validates balance automatically
- Custom check: `InsufficientBalance` error

**Code:**
```rust
token_utils::transfer_to_contract(env, &staker, amount)?;
```

**Severity:** High

---

### T5: Reentrancy

**Description:** Malicious contract calls back during token transfer.

**Mitigation:**
- Soroban token interface is reentrancy-safe
- Checks-Effects-Interactions pattern followed
- No callback mechanisms in contracts

**Severity:** Low (Soroban handles this)

---

### T6: Integer Overflow

**Description:** Arithmetic overflow in pool calculations.

**Mitigation:**
- Rust 2021+ edition with overflow checks (default in debug)
- Release builds use `overflow-checks = true`
- Use `i128` for token amounts (sufficient range)

**Severity:** Medium

---

### T7: Front-Running

**Description:** Transaction ordering manipulated to disadvantage users.

**Mitigation:**
- Lock times enforced on-chain
- No sensitive timing dependencies
- Voting window starts after match end

**Severity:** Low

---

### T8: Emergency Withdrawal Abuse

**Description:** User claims emergency withdrawal multiple times.

**Mitigation:**
- `EmergencyClaimed(poll_id, user)` flag prevents double withdrawal
- Error returned: `AlreadyClaimed`

**Code:**
```rust
if has_emergency_claimed(&env, poll_id, &user) {
    return Err(PredictXError::AlreadyClaimed);
}
```

**Severity:** High

---

### T9: Contract Pause Abuse

**Description:** Admin pauses contract indefinitely.

**Impact:** Denial of service for users.

**Mitigation:**
- Emergency withdrawal still allowed when paused
- Transparent events emitted for pauses
- Multi-sig governance planned for Phase 2

**Severity:** Medium

---

## Access Control Matrix

| Function | Admin | User | Oracle | Anyone |
|----------|-------|------|--------|--------|
| `initialize` | ✅ | ❌ | ❌ | ❌ |
| `pause` | ✅ | ❌ | ❌ | ❌ |
| `unpause` | ✅ | ❌ | ❌ | ❌ |
| `set_oracle` | ✅ | ❌ | ❌ | ❌ |
| `cancel_poll` | ✅ | ❌ | ❌ | ❌ |
| `create_poll` | ❌ | ✅ | ❌ | ❌ |
| `stake` | ❌ | ✅ | ❌ | ❌ |
| `emergency_withdraw` | ❌ | ✅ | ❌ | ❌ |
| `create_match` | ✅ | ❌ | ❌ | ❌ |
| `set_poll_status` | ✅ | ❌ | ❌ | ❌ |

---

## Storage Security

### Persistent vs Instance Storage

| Type | Persistence | Use Case |
|------|-------------|----------|
| Instance | Forever | Config, counters |
| Persistent | Forever (with rent) | User data, stakes |

### DataKey Namespacing

All storage keys are namespaced via enum:

```rust
enum DataKey {
    Admin,
    Poll(u64),
    Stake(u64, Address),
    // ...
}
```

This prevents key collisions between different data types.

---

## Token Security

### Token Interface

Contracts interact with tokens via Soroban's token interface:

```rust
token_utils::transfer_to_contract(&env, &staker, amount)?;
```

### Validation

- Contracts validate all inputs before token operations
- Token client performs balance/allowance checks
- No arbitrary token transfers without user consent (`require_auth`)

---

## Emergency Procedures

### If Contract is Paused

1. Users can still call `emergency_withdraw`
2. Admin monitors situation
3. Resolution within 7 days expected

### If Bug is Discovered

1. Admin can pause contract
2. Emergency withdrawal available after timeout
3. Bug bounty program (planned)

### Emergency Withdrawal Timeline

```
Poll Cancelled ──┬─── Immediate: User can emergency_withdraw
                 │
Poll Disputed ───┤
                 │
Poll Locked ─────┘
                 
                 └─── After 7 days: User can emergency_withdraw
```

---

## Known Limitations

| Limitation | Description | Mitigation |
|------------|-------------|------------|
| Admin key single point of failure | Single admin can pause/cancel | Multi-sig governance planned |
| Oracle is admin-controlled | Truth depends on admin honesty | Community voting in Phase 2 |
| No upgrade mechanism | Contracts cannot be upgraded | Deploy new contracts if needed |
| No trustless resolution | Depends on off-chain data | Community + admin verification |

---

## Security Best Practices

### For Users

1. **Verify contract addresses** before interacting
2. **Check poll status** before staking
3. **Monitor emergency withdrawals** for your polls
4. **Keep private keys secure**

### For Developers

1. **Always use `require_auth`** for privileged functions
2. **Follow CEI pattern** (Checks-Effects-Interactions)
3. **Validate all inputs** even if token interface validates
4. **Emit events** for all state changes
5. **Write comprehensive tests** covering error paths

---

## Audit Checklist

Before mainnet launch:

- [ ] All admin functions use `require_auth`
- [ ] All error paths tested
- [ ] Integer overflow checked
- [ ] Reentrancy protection verified
- [ ] Double-stake protection confirmed
- [ ] Lock time enforcement tested
- [ ] Emergency withdrawal timeout verified
- [ ] Pause/unpause tested
- [ ] Token transfers tested with real tokens
- [ ] All events emit correctly
- [ ] No storage key collisions
- [ ] Documentation complete
