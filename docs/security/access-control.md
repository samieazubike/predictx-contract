# Access Control

Role definitions, permission matrix, and enforcement details for PredictX contracts.

## Overview

Access control in PredictX follows the principle of **least privilege**: each actor can only perform actions within their designated scope.

---

## Role Definitions

### Admin

**Definition:** The primary operator address set during contract initialization.

| Capability | Details |
|------------|---------|
| Initialize contracts | Sets up initial contract state |
| Pause/unpause | Halts all user operations in emergencies |
| Update oracle | Changes `VotingOracle` contract address |
| Cancel polls | Forcefully closes polls (triggers emergency withdrawal) |
| Create matches | Registers football matches in the system |
| Update matches | Modifies match details before kickoff |
| Finish matches | Marks matches as ended (prerequisite for resolution) |
| Set poll status | (VotingOracle only) Manually transitions poll status |

**Admin is NOT able to:**
- Steal user funds
- Withdraw from Treasury
- Modify individual stake records
- Bypass `require_auth` checks on user functions

---

### User

**Definition:** Any Stellar account interacting with the protocol.

| Capability | Details |
|------------|---------|
| Create polls | Any authenticated user can create a poll |
| Stake | Place tokens on Yes/No outcomes |
| Claim winnings | Withdraw payout after resolution |
| Emergency withdraw | Retrieve stake after 7-day oracle timeout |
| Vote | (Phase 2) Cast votes on poll outcomes |

---

### Oracle (Contract)

**Definition:** The `VotingOracle` contract, queried by `PredictionMarket` for poll status.

| Capability | Details |
|------------|---------|
| Set poll status | Called by PredictionMarket admin actions |
| Return poll status | Read-only query for `PredictionMarket` |

---

## Permission Matrix

### PredictionMarket

| Function | Admin | User | Oracle | Anyone |
|---------|-------|------|--------|--------|
| `initialize` | ✅ | ❌ | ❌ | ❌ |
| `pause` | ✅ | ❌ | ❌ | ❌ |
| `unpause` | ✅ | ❌ | ❌ | ❌ |
| `set_oracle` | ✅ | ❌ | ❌ | ❌ |
| `cancel_poll` | ✅ | ❌ | ❌ | ❌ |
| `create_match` | ✅ | ❌ | ❌ | ❌ |
| `update_match` | ✅ | ❌ | ❌ | ❌ |
| `finish_match` | ✅ | ❌ | ❌ | ❌ |
| `create_poll` | ❌ | ✅ | ❌ | ❌ |
| `stake` | ❌ | ✅ | ❌ | ❌ |
| `emergency_withdraw` | ❌ | ✅ | ❌ | ❌ |
| `claim_winnings` | ❌ | ✅ | ❌ | ❌ |
| `get_poll` | ❌ | ✅ | ❌ | ✅ |
| `get_match` | ❌ | ✅ | ❌ | ✅ |
| `get_pool_info` | ❌ | ✅ | ❌ | ✅ |
| `oracle_poll_status` | ❌ | ✅ | ❌ | ✅ |
| `check_emergency_eligible` | ❌ | ✅ | ❌ | ✅ |

### VotingOracle

| Function | Admin | User | Anyone |
|---------|-------|------|--------|
| `initialize` | ✅ | ❌ | ❌ |
| `set_poll_status` | ✅ | ❌ | ❌ |
| `get_poll_status` | ✅ | ✅ | ✅ |
| `get_poll_status_updated_at` | ✅ | ✅ | ✅ |
| `admin` | ✅ | ✅ | ✅ |

### Treasury

| Function | Admin | User | Anyone |
|---------|-------|------|--------|
| `initialize` | ✅ | ❌ | ❌ |
| `deposit` | ❌ | ✅ | ❌ |
| `balance` | ✅ | ✅ | ✅ |
| `admin` | ✅ | ✅ | ✅ |

### PollFactory

| Function | Admin | User | Anyone |
|---------|-------|------|--------|
| `initialize` | ✅ | ❌ | ❌ |
| `create_poll` | ❌ | ✅ | ❌ |
| `get_poll` | ✅ | ✅ | ✅ |
| `admin` | ✅ | ✅ | ✅ |

---

## Enforcement Mechanisms

### `require_auth`

All privileged functions use Soroban's `require_auth()`:

```rust
admin.require_auth();  // Verifies caller signed this invocation
```

This prevents:
- Transactions submitted by non-admin addresses
- Replay attacks (each invocation requires fresh authorization)

### Admin Stored Reference

The admin address is stored during `initialize()`:

```rust
env.storage().instance().set(&DataKey::Admin, &admin);
```

Comparisons are always `*caller != stored_admin`.

### Pause Guard

When paused, sensitive operations return `EmergencyWithdrawNotAllowed`:

```rust
pub(crate) fn ensure_not_paused(env: &Env) -> Result<(), PredictXError> {
    if is_paused(env) {
        return Err(PredictXError::EmergencyWithdrawNotAllowed);
    }
    Ok(())
}
```

---

## Known Limitations

| Limitation | Impact | Mitigation |
|-----------|--------|------------|
| Single admin key | Single point of failure | Multi-sig governance planned for Phase 2 |
| Admin can pause indefinitely | Denial of service | Emergency withdrawal still available; timelock governance planned |
| Oracle truthfulness relies on admin | Requires trust in admin | Community voting in Phase 2 reduces trust dependency |

---

## Future Access Control (Phase 2-3)

### Multi-Sig Admin

Phase 2 will introduce 3-of-5 multi-sig for:
- `cancel_poll`
- `set_oracle`
- `set_poll_status`

### Governance Token

Phase 3 will transfer upgrade authority from admin to PRED token holders via on-chain governance.

### Role-Based Access (Planned)

Future contracts may introduce finer-grained roles:

```rust
enum Role {
    Admin,        // Full admin rights
    Oracle,       // Status management only
    Upgrader,    // Contract upgrades only
    Treasury,     // Fee withdrawal only
}
```
