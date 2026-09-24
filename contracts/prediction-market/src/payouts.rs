//! Payout & claim engine — platform fee routing and winnings claims.
//!
//! On the **first** successful claim for a resolved poll the configured
//! platform fee is deducted from the combined pool and transferred to the
//! treasury exactly once (`DataKey::FeePaid`). Subsequent claims reuse the
//! same fee figure so the distributable pot stays consistent.

use soroban_sdk::{Address, Env};
use predictx_shared::{
    Poll, PollStatus, Stake, StakeSide, PredictXError, BPS_DENOMINATOR, PLATFORM_FEE_BPS,
};
use crate::{DataKey, get_platform_stats, set_platform_stats, token_utils};

// ── Pure helpers ──────────────────────────────────────────────────────────────

/// Return the winning-side pool for a resolved poll.
pub fn winning_pool(poll: &Poll) -> Result<i128, PredictXError> {
    match poll.outcome {
        Some(true) => Ok(poll.yes_pool),
        Some(false) => Ok(poll.no_pool),
        None => Err(PredictXError::InvalidOutcome),
    }
}

/// Return the losing-side pool for a resolved poll.
pub fn losing_pool(poll: &Poll) -> Result<i128, PredictXError> {
    match poll.outcome {
        Some(true) => Ok(poll.no_pool),
        Some(false) => Ok(poll.yes_pool),
        None => Err(PredictXError::InvalidOutcome),
    }
}

/// `fee = total * fee_bps / BPS_DENOMINATOR` (integer division).
pub fn fee_amount(total: i128, fee_bps: u32) -> i128 {
    total * (fee_bps as i128) / (BPS_DENOMINATOR as i128)
}

/// Proportional share of the distributable pot.
/// `share = user_stake * distributable / winning_pool`.
pub fn payout_share(
    user_stake: i128,
    winning_pool_amt: i128,
    distributable: i128,
) -> Result<i128, PredictXError> {
    if winning_pool_amt == 0 {
        return Err(PredictXError::InvalidOutcome);
    }
    Ok(user_stake * distributable / winning_pool_amt)
}

fn has_fee_paid(env: &Env, poll_id: u64) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::FeePaid(poll_id))
        .unwrap_or(false)
}

fn set_fee_paid(env: &Env, poll_id: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::FeePaid(poll_id), &true);
}

/// Ensure the platform fee for `poll_id` has been transferred to the treasury
/// exactly once. Returns the fee amount (recomputed deterministically so later
/// claimants see the same distributable pot).
pub fn ensure_platform_fee_routed(
    env: &Env,
    poll_id: u64,
    total_pool: i128,
) -> Result<i128, PredictXError> {
    let fee_bps = token_utils::get_platform_fee_bps(env);
    let fee = fee_amount(total_pool, fee_bps);

    if has_fee_paid(env, poll_id) {
        return Ok(fee);
    }

    if fee > 0 {
        token_utils::transfer_to_treasury(env, fee)?;
    }
    set_fee_paid(env, poll_id);
    Ok(fee)
}

/// Claim winnings after a poll resolves.
///
/// On the first claim for the poll the platform fee is skimmed from the
/// combined pool and sent to the treasury (`FeePaid` marker). The caller's
/// payout is their share of `total_pool - fee`.
pub fn claim_winnings(
    env: &Env,
    claimant: Address,
    poll_id: u64,
) -> Result<i128, PredictXError> {
    claimant.require_auth();

    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;

    if poll.status != PollStatus::Resolved {
        return Err(PredictXError::PollNotActive);
    }

    let outcome_yes = poll.outcome.ok_or(PredictXError::InvalidOutcome)?;

    let mut stake: Stake = env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, claimant.clone()))
        .ok_or(PredictXError::NotStaker)?;

    if stake.claimed {
        return Err(PredictXError::AlreadyClaimed);
    }

    let win_pool = winning_pool(&poll)?;
    if win_pool == 0 {
        return Err(PredictXError::InvalidOutcome);
    }

    let on_winning_side = match stake.side {
        StakeSide::Yes => outcome_yes,
        StakeSide::No => !outcome_yes,
    };
    if !on_winning_side {
        return Err(PredictXError::NotOnWinningSide);
    }

    let total_pool = poll.yes_pool + poll.no_pool;
    let fee = ensure_platform_fee_routed(env, poll_id, total_pool)?;
    let distributable = total_pool - fee;
    let payout = payout_share(stake.amount, win_pool, distributable)?;

    // Checks-effects-interactions: persist claimed before transfer.
    stake.claimed = true;
    env.storage()
        .persistent()
        .set(&DataKey::Stake(poll_id, claimant.clone()), &stake);

    token_utils::transfer_from_contract(env, &claimant, payout)?;

    let mut stats = get_platform_stats(env);
    stats.total_value_locked = stats.total_value_locked.saturating_sub(payout);
    stats.total_payouts += payout;
    set_platform_stats(env, &stats);

    Ok(payout)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Address, Env, String,
    };
    use predictx_shared::{Poll, PollCategory, PollStatus, Stake, StakeSide};
    use crate::{DataKey, PredictionMarket, PredictionMarketClient};

    struct TestSetup<'a> {
        env: Env,
        admin: Address,
        #[allow(dead_code)]
        oracle_id: Address,
        token_addr: Address,
        contract_id: Address,
        client: PredictionMarketClient<'a>,
        treasury: Address,
    }

    fn setup() -> TestSetup<'static> {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let oracle_id = env.register(crate::voting_oracle::WASM, ());
        let oracle_client = crate::voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_addr = token_contract.address();

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle_id, &token_addr, &treasury, &PLATFORM_FEE_BPS);

        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        TestSetup {
            env,
            admin,
            oracle_id,
            token_addr,
            contract_id,
            client,
            treasury,
        }
    }

    fn mint_tokens(s: &TestSetup, to: &Address, amount: i128) {
        let sac = token::StellarAssetClient::new(&s.env, &s.token_addr);
        sac.mint(to, &amount);
    }

    fn token_balance(s: &TestSetup, addr: &Address) -> i128 {
        token::Client::new(&s.env, &s.token_addr).balance(addr)
    }

    fn inject_resolved_poll(
        s: &TestSetup,
        poll_id: u64,
        outcome_yes: bool,
        yes_pool: i128,
        no_pool: i128,
    ) {
        s.env.as_contract(&s.contract_id, || {
            let poll = Poll {
                poll_id,
                match_id: 1,
                creator: s.admin.clone(),
                question: String::from_str(&s.env, "fee test"),
                category: PollCategory::PlayerEvent,
                lock_time: 500_000,
                yes_pool,
                no_pool,
                yes_count: if yes_pool > 0 { 1 } else { 0 },
                no_count: if no_pool > 0 { 1 } else { 0 },
                status: PollStatus::Resolved,
                outcome: Some(outcome_yes),
                resolution_time: 1_000_000,
                created_at: 900_000,
            };
            s.env.storage().persistent().set(&DataKey::Poll(poll_id), &poll);
        });
    }

    fn inject_stake(s: &TestSetup, poll_id: u64, user: &Address, amount: i128, side: StakeSide) {
        s.env.as_contract(&s.contract_id, || {
            let stake = Stake {
                user: user.clone(),
                poll_id,
                amount,
                side,
                claimed: false,
                staked_at: 900_000,
            };
            s.env
                .storage()
                .persistent()
                .set(&DataKey::Stake(poll_id, user.clone()), &stake);
        });
    }

    /// Fee equals 5% of the combined pool at the default `PLATFORM_FEE_BPS`.
    #[test]
    fn fee_equals_five_percent_of_combined_pool() {
        let yes_pool: i128 = 450_000_000;
        let no_pool: i128 = 300_000_000;
        let total = yes_pool + no_pool;
        let fee = fee_amount(total, PLATFORM_FEE_BPS);
        assert_eq!(PLATFORM_FEE_BPS, 500);
        assert_eq!(fee, total * 500 / 10_000);
        assert_eq!(fee, 37_500_000);

        let distributable = total - fee;
        assert_eq!(distributable, 712_500_000);

        let share = payout_share(100_000_000, yes_pool, distributable).unwrap();
        assert_eq!(share, 100_000_000 * 712_500_000 / 450_000_000);
    }

    /// Treasury balance increases by exactly the fee on the first claim.
    #[test]
    fn treasury_balance_increases_by_exactly_the_fee() {
        let s = setup();
        let poll_id: u64 = 72;
        let yes_amt: i128 = 400_000_000;
        let no_amt: i128 = 600_000_000;
        let total = yes_amt + no_amt;
        let expected_fee = fee_amount(total, PLATFORM_FEE_BPS);
        assert_eq!(expected_fee, 50_000_000); // 5% of 1B

        let winner = Address::generate(&s.env);
        mint_tokens(&s, &s.contract_id, total);
        inject_resolved_poll(&s, poll_id, true, yes_amt, no_amt);
        inject_stake(&s, poll_id, &winner, yes_amt, StakeSide::Yes);

        assert_eq!(token_balance(&s, &s.treasury), 0);
        let payout = s.client.claim_winnings(&winner, &poll_id);

        assert_eq!(token_balance(&s, &s.treasury), expected_fee);
        let expected_payout = payout_share(yes_amt, yes_amt, total - expected_fee).unwrap();
        assert_eq!(payout, expected_payout);
        assert_eq!(payout, 950_000_000); // sole winner gets full distributable
    }

    /// Fee is transferred exactly once no matter how many stakers claim.
    #[test]
    fn fee_transferred_exactly_once_across_multiple_claims() {
        let s = setup();
        let poll_id: u64 = 73;
        let yes_a: i128 = 300_000_000;
        let yes_b: i128 = 200_000_000;
        let no_amt: i128 = 500_000_000;
        let yes_pool = yes_a + yes_b;
        let total = yes_pool + no_amt;
        let expected_fee = fee_amount(total, PLATFORM_FEE_BPS);
        assert_eq!(expected_fee, 50_000_000);

        let user_a = Address::generate(&s.env);
        let user_b = Address::generate(&s.env);
        mint_tokens(&s, &s.contract_id, total);
        inject_resolved_poll(&s, poll_id, true, yes_pool, no_amt);
        inject_stake(&s, poll_id, &user_a, yes_a, StakeSide::Yes);
        inject_stake(&s, poll_id, &user_b, yes_b, StakeSide::Yes);

        assert_eq!(token_balance(&s, &s.treasury), 0);

        let payout_a = s.client.claim_winnings(&user_a, &poll_id);
        assert_eq!(token_balance(&s, &s.treasury), expected_fee);

        let payout_b = s.client.claim_winnings(&user_b, &poll_id);
        // Second claim must NOT skim another fee
        assert_eq!(token_balance(&s, &s.treasury), expected_fee);

        let distributable = total - expected_fee;
        assert_eq!(
            payout_a,
            payout_share(yes_a, yes_pool, distributable).unwrap()
        );
        assert_eq!(
            payout_b,
            payout_share(yes_b, yes_pool, distributable).unwrap()
        );
        assert_eq!(payout_a + payout_b, distributable);
    }
}
