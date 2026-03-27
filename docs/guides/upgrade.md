# Upgrade Guide

Procedures for upgrading PredictX smart contracts on Stellar.

## Current Upgrade Status

> **Phase 1:** Upgrade mechanisms are **not yet implemented**. Contracts are deployed as immutable WASM blobs. This document describes the planned upgrade path for Phase 3/4.

## Upgrade Philosophy

PredictX follows a **"security over agility"** principle:
- Contracts are designed to be correct on first deployment
- Upgrades require careful governance, not unilateral admin action
- Any upgrade path includes a time-lock to protect users

---

## Planned Upgrade Mechanism

### Soroban Contract Upgrade Capability

Stellar Soroban supports contract upgrades via the `deploy()` function with upgradeable WASM. The planned architecture:

```
┌─────────────────────────────────────────────────────────────┐
│                  Upgrade Governance Layer                      │
├─────────────────────────────────────────────────────────────┤
│  1. Admin proposes new WASM hash                            │
│  2. 48-hour timelock countdown begins                        │
│  3. After timelock: upgrade executes                         │
│  4. Emergency: multi-sig can bypass timelock (3/5 admins)  │
└─────────────────────────────────────────────────────────────┘
```

### Upgrade Authorization Levels

| Action | Authority | Timelock |
|--------|-----------|----------|
| Patch (bug fix, same API) | Multi-sig 3/5 | 24 hours |
| Minor (new function, backwards compatible) | Admin + timelock | 48 hours |
| Major (breaking API changes) | Governance token vote | 7 days |

---

## Current Mitigation: Contract Design

Since Phase 1 has no upgrade mechanism, contracts are designed to minimize the need for upgrades:

### Immutability Design Principles

1. **Initialization over constructor** — Admin address set via `initialize()`, not constructor, allowing deployment without immediate configuration
2. **Oracle pattern** — `VotingOracle` address stored in `PredictionMarket`, allowing oracle logic to change without redeploying the main contract
3. **Treasury separation** — Fees go to separate `Treasury` contract, can be upgraded independently
4. **Pause mechanism** — Admin can halt contract without upgrading

### Pause vs Upgrade

| Scenario | Action |
|----------|--------|
| Bug in staking logic | `pause()` + emergency withdrawal |
| Bug in oracle status | `set_oracle()` to new oracle |
| Want to change fee | Deploy new Treasury + redirect |
| Breaking API change | Deploy new contract version |

---

## Phase 3: Governance Token

Planned for Phase 3:

```
PredictX Token (PRED)
├── Vote on protocol upgrades
├── Stake to participate in governance
├── Fee discounts for token holders
└── Revenue share for stakers
```

Upgrade governance will transfer from admin multi-sig to PRED token holders.

---

## Phase 4: Formal Upgrade Process

### Upgrade Proposal

```markdown
# PredictX Upgrade Proposal

## Summary
Brief description of why an upgrade is needed.

## Changes
- What functions change?
- What is the new WASM hash?
- Is it backwards compatible?

## Risk Assessment
- What can go wrong?
- What is the rollback plan?

## Timeline
- T+0: Proposal submitted
- T+48h: Timelock expires
- T+48h: Upgrade executed

## Rollback Plan
How to revert if the upgrade causes issues.
```

### Emergency Upgrade

For critical vulnerabilities:

1. Multi-sig council (3/5 admins) approves emergency action
2. Timelock waived
3. Upgrade deployed within 4 hours of critical issue confirmed
4. Full incident report within 48 hours

---

## Deployment of New Contracts

### Without Upgrade Mechanism (Phase 1-2)

Since contracts cannot be upgraded, new versions are deployed as entirely new contract IDs:

```bash
# Deploy new version
NEW_PM_ID=$(stellar contract deploy \
    --source $ADMIN \
    --network testnet \
    --wasm target/wasm32-unknown-unknown/release/prediction_market_v2.wasm)

# Initialize with same parameters
stellar contract invoke \
    --id $NEW_PM_ID \
    --source $ADMIN \
    --network testnet \
    -- \
    initialize \
    --admin $ADMIN \
    --voting_oracle $ORACLE_ID \
    --token_address $TOKEN_ID \
    --treasury_address $TREASURY_ID \
    --platform_fee_bps 500

# Frontend updates contract ID
```

### Data Migration

When deploying new contracts, user data (stakes, polls) stays in the old contract. There is no automatic migration in Phase 1.

**Migration path:**
1. Pause old contract (`pause()`)
2. Allow emergency withdrawals for all active stakes
3. Deploy new contract
4. Users interact with new contract

---

## Rollback Procedures

### If Bug is Discovered Post-Deployment

1. **Immediate:** Call `pause()` on the affected contract
2. **Users:** Call `emergency_withdraw()` to retrieve stakes
3. **Analysis:** Identify the bug and scope of impact
4. **Fix:** Deploy corrected contract (new contract ID)
5. **Communication:** Publish post-mortem within 48 hours

### Rollback Decision Tree

```
Bug discovered
├── Is user funds at risk?
│   ├── YES → Emergency: pause() + emergency_withdraw()
│   └── NO  → Can it wait for scheduled upgrade?
│       ├── YES → Include in next scheduled upgrade
│       └── NO  → Emergency upgrade process
```

---

## Storage Migration

Soroban contract storage is tightly coupled to the WASM logic. Storage migration requires:

1. Read all relevant state from old contract
2. Deploy new contract
3. Call migration functions to populate new storage

### Planned Migration Helpers (Phase 3)

```rust
/// Migration function to transfer polls from old to new contract.
pub fn migrate_polls(env: Env, old_contract: Address, poll_ids: Vec<u64>) {
    for poll_id in poll_ids {
        let poll: Poll = old_contract.get_poll(poll_id);
        env.storage().persistent().set(&DataKey::Poll(poll_id), &poll);
    }
}
```

---

## Verification

After any upgrade, verify:

```bash
# 1. Check new WASM is deployed
stellar contract info --id $CONTRACT_ID --network testnet

# 2. Verify admin is unchanged
stellar contract invoke --id $CONTRACT_ID --source $ADMIN --network testnet -- admin

# 3. Verify state is intact
stellar contract invoke --id $CONTRACT_ID --source $ADMIN --network testnet -- get_platform_stats

# 4. Test a stake
stellar contract invoke --id $CONTRACT_ID --source $USER --network testnet -- \
    stake --staker $USER --poll_id 1 --amount 1000000000 --side Yes
```

---

## Upgrade Checklist

- [ ] New WASM built and verified
- [ ] Storage migration plan documented
- [ ] Timelock initiated (if applicable)
- [ ] Multi-sig approvals obtained (if applicable)
- [ ] Rollback plan documented
- [ ] Frontend team notified of contract ID change
- [ ] Community notified
- [ ] Post-upgrade verification run
- [ ] Incident report if emergency upgrade
