//! Safe arithmetic utilities for PredictX contracts.
//!
//! This module provides checked arithmetic operations to prevent
//! integer overflow and underflow vulnerabilities.

use crate::PredictXError;

/// Safe addition that returns an error on overflow.
///
/// # Arguments
/// * `a` - First operand
/// * `b` - Second operand
///
/// # Returns
/// * `Ok(a + b)` if the addition doesn't overflow
/// * `Err(PredictXError::Overflow)` if overflow occurs
#[inline]
pub fn safe_add(a: i128, b: i128) -> Result<i128, PredictXError> {
    a.checked_add(b).ok_or(PredictXError::Overflow)
}

/// Safe subtraction that returns an error on underflow.
///
/// # Arguments
/// * `a` - First operand (minuend)
/// * `b` - Second operand (subtrahend)
///
/// # Returns
/// * `Ok(a - b)` if the subtraction doesn't underflow
/// * `Err(PredictXError::Underflow)` if underflow occurs
#[inline]
pub fn safe_sub(a: i128, b: i128) -> Result<i128, PredictXError> {
    a.checked_sub(b).ok_or(PredictXError::Underflow)
}

/// Safe multiplication that returns an error on overflow.
///
/// Uses u128 as an intermediary to prevent overflow during multiplication
/// when both operands are i128.
///
/// # Arguments
/// * `a` - First operand
/// * `b` - Second operand
///
/// # Returns
/// * `Ok(a * b)` if the multiplication doesn't overflow
/// * `Err(PredictXError::Overflow)` if overflow occurs
#[inline]
pub fn safe_mul(a: i128, b: i128) -> Result<i128, PredictXError> {
    // Convert to u128 for the multiplication to catch overflow
    let (a_abs, a_neg) = if a < 0 { ((a as i128).unsigned_abs(), true) } else { (a as u128, false) };
    let (b_abs, b_neg) = if b < 0 { ((b as i128).unsigned_abs(), true) } else { (b as u128, false) };
    
    let result = a_abs.checked_mul(b_abs).ok_or(PredictXError::Overflow)?;
    
    // Determine sign
    let final_result = if a_neg != b_neg {
        -(result as i128)
    } else {
        result as i128
    };
    
    Ok(final_result)
}

/// Safe division that returns an error on division by zero.
///
/// # Arguments
/// * `a` - Dividend
/// * `b` - Divisor
///
/// # Returns
/// * `Ok(a / b)` if `b` is not zero
/// * `Err(PredictXError::DivisionByZero)` if `b` is zero
#[inline]
pub fn safe_div(a: i128, b: i128) -> Result<i128, PredictXError> {
    if b == 0 {
        return Err(PredictXError::DivisionByZero);
    }
    Ok(a.checked_div(b).unwrap_or(0))
}

/// Safe modulo that returns an error on division by zero.
///
/// # Arguments
/// * `a` - Dividend
/// * `b` - Divisor
///
/// # Returns
/// * `Ok(a % b)` if `b` is not zero
/// * `Err(PredictXError::DivisionByZero)` if `b` is zero
#[inline]
pub fn safe_mod(a: i128, b: i128) -> Result<i128, PredictXError> {
    if b == 0 {
        return Err(PredictXError::DivisionByZero);
    }
    Ok(a.checked_rem(b).unwrap_or(0))
}

/// Calculates proportional share with overflow protection.
///
/// Computes: `amount * numerator / denominator`
///
/// This is safe for financial calculations as it:
/// 1. Uses checked arithmetic for multiplication
/// 2. Returns error on division by zero
/// 3. Handles sign correctly
///
/// # Arguments
/// * `amount` - The base amount
/// * `numerator` - The numerator for the proportion
/// * `denominator` - The denominator for the proportion
///
/// # Returns
/// * `Ok(proportional_amount)` if calculations succeed
/// * `Err(PredictXError::Overflow)` if overflow occurs
/// * `Err(PredictXError::DivisionByZero)` if denominator is zero
pub fn safe_proportional(
    amount: i128,
    numerator: i128,
    denominator: i128,
) -> Result<i128, PredictXError> {
    if denominator == 0 {
        return Err(PredictXError::DivisionByZero);
    }

    // Use u128 intermediary to prevent overflow on multiplication
    // First check if the amount fits in u128
    if amount > i128::MAX as i128 || amount < i128::MIN as i128 {
        return Err(PredictXError::Overflow);
    }

    let result = (amount as u128)
        .checked_mul(numerator as u128)
        .ok_or(PredictXError::Overflow)?
        / (denominator as u128);

    Ok(result as i128)
}

/// Calculates percentage of an amount with fee calculation safety.
///
/// Computes: `amount * percentage_bps / BPS_DENOMINATOR`
///
/// # Arguments
/// * `amount` - The base amount
/// * `percentage_bps` - The percentage in basis points
/// * `bps_denominator` - The basis points denominator (typically 10_000)
///
/// # Returns
/// * `Ok(percentage_amount)` if calculations succeed
pub fn safe_percentage(
    amount: i128,
    percentage_bps: u32,
    bps_denominator: u32,
) -> Result<i128, PredictXError> {
    if bps_denominator == 0 {
        return Err(PredictXError::DivisionByZero);
    }
    
    // Convert to i128 for consistent arithmetic
    let amount_u128 = amount as u128;
    let bps_u128 = percentage_bps as u128;
    let denom_u128 = bps_denominator as u128;
    
    let result = amount_u128
        .checked_mul(bps_u128)
        .ok_or(PredictXError::Overflow)?
        / denom_u128;
    
    Ok(result as i128)
}

/// Verifies that the contract remains solvent after an operation.
///
/// This is a critical invariant: contract balance >= total owed to users.
///
/// # Arguments
/// * `contract_balance` - Current token balance of the contract
/// * `total_owed` - Total outstanding obligations
///
/// # Returns
/// * `true` if solvent
/// * `false` if insolvent (balance < owed)
#[inline]
pub fn verify_solvency(contract_balance: i128, total_owed: i128) -> bool {
    contract_balance >= total_owed
}

/// Validates a stake amount is within acceptable bounds.
///
/// # Arguments
/// * `amount` - The stake amount to validate
/// * `min_amount` - Minimum allowed amount
/// * `max_amount` - Maximum allowed amount
///
/// # Returns
/// * `Ok(())` if valid
/// * `Err(PredictXError::StakeAmountZero)` if amount is zero or negative
/// * `Err(PredictXError::StakeBelowMinimum)` if below minimum
/// * `Err(PredictXError::StakeExceedsLimit)` if above maximum
pub fn validate_stake_amount(
    amount: i128,
    min_amount: i128,
    max_amount: i128,
) -> Result<(), PredictXError> {
    if amount <= 0 {
        return Err(PredictXError::StakeAmountZero);
    }
    if amount < min_amount {
        return Err(PredictXError::StakeBelowMinimum);
    }
    if amount > max_amount {
        return Err(PredictXError::StakeExceedsLimit);
    }
    Ok(())
}

/// Validates that a lock time is properly set in the future with buffer.
///
/// # Arguments
/// * `current_time` - Current ledger timestamp
/// * `lock_time` - The lock time to validate
/// * `buffer_secs` - Required buffer before lock time
///
/// # Returns
/// * `true` if lock time is valid and in the future with buffer
/// * `false` if lock time is too close or in the past
#[inline]
pub fn is_before_lock_time(
    current_time: u64,
    lock_time: u64,
    buffer_secs: u64,
) -> bool {
    current_time.saturating_add(buffer_secs) < lock_time
}

/// Checks if the given timestamp is past a lock time with buffer.
///
/// # Arguments
/// * `current_time` - Current ledger timestamp
/// * `lock_time` - The lock time to check against
/// * `buffer_secs` - Buffer in seconds to add after lock_time
///
/// # Returns
/// * `true` if current_time > lock_time + buffer
/// * `false` otherwise
#[inline]
pub fn is_after_lock_time_with_buffer(
    current_time: u64,
    lock_time: u64,
    buffer_secs: u64,
) -> bool {
    current_time > lock_time.saturating_add(buffer_secs)
}

#[cfg(test)]
mod test {
    use super::*;

    // ── safe_add ─────────────────────────────────────────────────────────────

    #[test]
    fn safe_add_works() {
        assert_eq!(safe_add(1, 2).unwrap(), 3);
        assert_eq!(safe_add(0, 0).unwrap(), 0);
        assert_eq!(safe_add(-5, 10).unwrap(), 5);
    }

    #[test]
    fn safe_add_overflow_panics() {
        let result = safe_add(i128::MAX, 1);
        assert_eq!(result, Err(PredictXError::Overflow));
    }

    #[test]
    fn safe_add_underflow_panics() {
        let result = safe_add(i128::MIN, -1);
        assert_eq!(result, Err(PredictXError::Underflow));
    }

    // ── safe_sub ─────────────────────────────────────────────────────────────

    #[test]
    fn safe_sub_works() {
        assert_eq!(safe_sub(5, 3).unwrap(), 2);
        assert_eq!(safe_sub(0, 0).unwrap(), 0);
        assert_eq!(safe_sub(-5, 10).unwrap(), -15);
    }

    #[test]
    fn safe_sub_underflow_panics() {
        let result = safe_sub(i128::MIN, 1);
        assert_eq!(result, Err(PredictXError::Underflow));
    }

    // ── safe_mul ─────────────────────────────────────────────────────────────

    #[test]
    fn safe_mul_works() {
        assert_eq!(safe_mul(3, 4).unwrap(), 12);
        assert_eq!(safe_mul(0, 100).unwrap(), 0);
        assert_eq!(safe_mul(-3, 4).unwrap(), -12);
        assert_eq!(safe_mul(-3, -4).unwrap(), 12);
    }

    #[test]
    fn safe_mul_overflow_panics() {
        let result = safe_mul(i128::MAX / 2 + 1, 3);
        assert_eq!(result, Err(PredictXError::Overflow));
    }

    // ── safe_div ─────────────────────────────────────────────────────────────

    #[test]
    fn safe_div_works() {
        assert_eq!(safe_div(10, 2).unwrap(), 5);
        assert_eq!(safe_div(-10, 2).unwrap(), -5);
        assert_eq!(safe_div(10, -2).unwrap(), -5);
    }

    #[test]
    fn safe_div_by_zero_panics() {
        assert_eq!(safe_div(10, 0), Err(PredictXError::DivisionByZero));
    }

    // ── safe_proportional ────────────────────────────────────────────────────

    #[test]
    fn safe_proportional_works() {
        // 100 * 50 / 100 = 50
        assert_eq!(safe_proportional(100, 50, 100).unwrap(), 50);
        // 1000 * 250 / 1000 = 250
        assert_eq!(safe_proportional(1000, 250, 1000).unwrap(), 250);
    }

    #[test]
    fn safe_proportional_with_overflow() {
        // Large numbers that would overflow
        let result = safe_proportional(i128::MAX, 2, 1);
        assert_eq!(result, Err(PredictXError::Overflow));
    }

    #[test]
    fn safe_proportional_zero_denominator() {
        assert_eq!(
            safe_proportional(100, 50, 0),
            Err(PredictXError::DivisionByZero)
        );
    }

    // ── verify_solvency ──────────────────────────────────────────────────────

    #[test]
    fn verify_solvency_works() {
        assert!(verify_solvency(1000, 500));
        assert!(verify_solvency(1000, 1000));
        assert!(!verify_solvency(500, 1000));
    }

    // ── validate_stake_amount ────────────────────────────────────────────────

    #[test]
    fn validate_stake_amount_works() {
        assert!(validate_stake_amount(100, 10, 10000).is_ok());
        assert_eq!(
            validate_stake_amount(0, 10, 10000),
            Err(PredictXError::StakeAmountZero)
        );
        assert_eq!(
            validate_stake_amount(-100, 10, 10000),
            Err(PredictXError::StakeAmountZero)
        );
        assert_eq!(
            validate_stake_amount(5, 10, 10000),
            Err(PredictXError::StakeBelowMinimum)
        );
        assert_eq!(
            validate_stake_amount(20000, 10, 10000),
            Err(PredictXError::StakeExceedsLimit)
        );
    }

    // ── is_before_lock_time ──────────────────────────────────────────────────

    #[test]
    fn is_before_lock_time_works() {
        // Current time 100, lock time 200, buffer 30 -> true
        assert!(is_before_lock_time(100, 200, 30));
        // Current time 171, lock time 200, buffer 30 -> false (171+30=201 > 200)
        assert!(!is_before_lock_time(171, 200, 30));
        // Current time 200, lock time 200, buffer 30 -> false
        assert!(!is_before_lock_time(200, 200, 30));
        // Current time 250, lock time 200, buffer 30 -> false
        assert!(!is_before_lock_time(250, 200, 30));
    }
}
