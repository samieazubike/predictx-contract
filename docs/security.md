# PredictX Security Review

_Last updated: March 28, 2026_

## Overview
This document summarizes the security hardening and attack prevention measures implemented in the PredictX smart contracts. It covers all critical vectors identified in the PRD and describes the on-chain mitigations.

## Attack Vectors & Mitigations

### 1. Frontrunning
- Strict lock time enforcement with buffer (`LOCK_TIME_BUFFER_SECS`)
- No stakes accepted within buffer window before lock

### 2. Oracle Manipulation
- Non-participant voting requirement
- Voter stake requirement (skin in the game)
- Admin review for non-consensus polls
- Dispute mechanism

### 3. Admin Centralization
- Multi-sig for critical operations
- Time-delayed admin actions
- Super admin key rotation
- Admin action logging

### 4. Reentrancy
- Reentrancy guard on all financial functions
- Checks-Effects-Interactions pattern
- `claimed` flag set before transfer

### 5. Arithmetic Safety
- All financial calculations use checked arithmetic
- Overflow/underflow returns error, never panics
- Division by zero handled gracefully

## Invariants
- Solvency: contract balance >= total owed
- No panics in production code
- All errors return Result types

## Test Coverage
- Reentrancy attack simulation
- Double-claim attempt
- Overflow/underflow with i128 edge cases
- Staking after lock time
- Voting as a staker
- Admin actions from non-admin
- Emergency withdrawal before timeout
- Frontrunning simulation

## Recommendations
- Review all public entrypoints for input validation
- Run `cargo audit` before deployment
- Require 2+ reviewers for all security PRs

---

_This document is a living record. Update after every major security change or audit._
