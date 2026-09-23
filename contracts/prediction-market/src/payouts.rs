use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::{env, near_bindgen, AccountId, NearToken, PromiseOrValue, log};

/// Represents the outcome of a prediction market poll.
pub enum Outcome {
    Yes,
    No,
}

/// Represents the result of a payout calculation.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct PayoutResult {
    /// The amount to be paid out to the winner(s).
    pub amount: NearToken,
    /// The outcome that won.
    pub outcome: Outcome,
}

/// Calculates the payout for a given poll resolution.
///
/// If the losing pool is empty (0 staked), it is treated as a no-contest.
/// In this case, stakers on the winning side receive their exact stake back
/// with no platform fee deducted.
///
/// # Arguments
///
/// * `total_pool_yes` - Total amount staked on Yes.
/// * `total_pool_no` - Total amount staked on No.
/// * `winning_outcome` - The outcome that won the poll.
/// * `platform_fee_bps` - Platform fee in basis points (e.g., 250 for 2.5%).
///
/// # Returns
///
/// A `PayoutResult` containing the calculated payout amount and the winning outcome.
pub fn calculate_payout(
    total_pool_yes: NearToken,
    total_pool_no: NearToken,
    winning_outcome: &Outcome,
    platform_fee_bps: u32,
) -> PayoutResult {
    let (winning_pool, losing_pool) = match winning_outcome {
        Outcome::Yes => (total_pool_yes, total_pool_no),
        Outcome::No => (total_pool_no, total_pool_yes),
    };

    // If the losing pool is empty, it's a no-contest.
    // Winners get their stake back exactly, no fee.
    if losing_pool.is_zero() {
        log!("No-contest detected: losing pool is empty. Refunding stakes at par.");
        return PayoutResult {
            amount: winning_pool,
            outcome: winning_outcome.clone(),
        };
    }

    // Standard payout calculation with fee
    let total_pot = winning_pool + losing_pool;
    
    // Calculate fee amount
    let fee_amount = total_pot.checked_mul(platform_fee_bps as u128)
        .expect("Fee calculation overflow")
        .checked_div(10000)
        .expect("Fee division overflow");

    // Net pot after fee
    let net_pot = total_pot.checked_sub(fee_amount)
        .expect("Net pot calculation underflow");

    // The entire net pot goes to the winning side
    PayoutResult {
        amount: net_pot,
        outcome: winning_outcome.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one_sided_yes_poll_refund() {
        // Scenario: Yes wins, No pool is empty
        let yes_stake = NearToken::from_near(100);
        let no_stake = NearToken::from_near(0);
        let fee_bps = 250; // 2.5%

        let result = calculate_payout(yes_stake, no_stake, &Outcome::Yes, fee_bps);

        // Should receive exact stake back
        assert_eq!(result.amount, yes_stake);
        assert_eq!(result.outcome, Outcome::Yes);
    }

    #[test]
    fn test_one_sided_no_poll_refund() {
        // Scenario: No wins, Yes pool is empty
        let yes_stake = NearToken::from_near(0);
        let no_stake = NearToken::from_near(50);
        let fee_bps = 250; // 2.5%

        let result = calculate_payout(yes_stake, no_stake, &Outcome::No, fee_bps);

        // Should receive exact stake back
        assert_eq!(result.amount, no_stake);
        assert_eq!(result.outcome, Outcome::No);
    }

    #[test]
    fn test_normal_poll_with_fee() {
        // Scenario: Both pools have stakes, fee should be applied
        let yes_stake = NearToken::from_near(100);
        let no_stake = NearToken::from_near(100);
        let fee_bps = 250; // 2.5%

        let result = calculate_payout(yes_stake, no_stake, &Outcome::Yes, fee_bps);

        // Total pot = 200
        // Fee = 200 * 0.025 = 5
        // Net pot = 195
        let expected_net_pot = NearToken::from_near(195);
        
        assert_eq!(result.amount, expected_net_pot);
        assert_eq!(result.outcome, Outcome::Yes);
    }
}