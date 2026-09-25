use soroban_sdk::{Address, Env, Symbol};
use predictx_shared::{
    BPS_DENOMINATOR, Poll, PollStatus, PredictXError, Stake, StakeSide,
};

use crate::{
    token_utils, DataKey, get_platform_stats, set_platform_stats,
};

// ── Payout math ───────────────────────────────────────────────────────────────

/// Compute the proportional payout for a winning stake.
///
/// Formula (matching the product spec):
/// ```text
/// payout = (user_stake / winning_pool) * (total_pool - fee)
/// ```
///
/// Integer arithmetic preserves precision by multiplying the numerator first:
/// `stake_amount * distributable / winning_pool`.
///
/// The fee amount is `total_pool * fee_bps / BPS_DENOMINATOR`; it is deducted
/// from the distributable pool but **not** transferred to the treasury in this
/// issue (fee transfer is a separate concern).
pub(crate) fn compute_payout(
    env: &Env,
    stake_amount: i128,
    winning_pool: i128,
    total_pool: i128,
) -> Result<i128, PredictXError> {
    if winning_pool == 0 {
        // Out of scope to handle gracefully, but guard against div-by-zero.
        return Err(PredictXError::TransferFailed);
    }

    let fee_bps = token_utils::get_platform_fee_bps(env);
    let fee_amount = total_pool * (fee_bps as i128) / (BPS_DENOMINATOR as i128);
    let distributable = total_pool - fee_amount;

    Ok(stake_amount * distributable / winning_pool)
}

/// Resolve the winning `StakeSide` from a poll's outcome.
///
/// `Poll.outcome` is `Some(true)` when Yes wins, `Some(false)` when No wins.
fn winning_side(outcome: bool) -> StakeSide {
    if outcome {
        StakeSide::Yes
    } else {
        StakeSide::No
    }
}

// ── Claim ─────────────────────────────────────────────────────────────────────

/// Claim winnings for a stake on a resolved poll.
///
/// Flow:
/// 1. Authenticates the caller (`user.require_auth`).
/// 2. Requires the poll to exist and be in `Resolved` status — otherwise
///    returns [`PredictXError::PollNotActive`].
/// 3. Requires the caller to have an existing stake on the poll — otherwise
///    returns [`PredictXError::NotStaker`].
/// 4. Requires the stake to be on the winning side — otherwise returns
///    [`PredictXError::NotOnWinningSide`].
/// 5. Computes the proportional share:
///    `stake / winning_pool * (total_pool - fee)`.
/// 6. Transfers the payout from the contract to the user.
///
/// Returns the amount paid out.
///
/// **Out of scope** (separate issues): the claimed-flag guard (double-claim
/// prevention), fee-to-treasury transfer, and empty-pool edge cases.
pub fn claim_winnings(env: &Env, user: Address, poll_id: u64) -> Result<i128, PredictXError> {
    // ── Auth ────────────────────────────────────────────────────────────────
    user.require_auth();

    // ── Checks ──────────────────────────────────────────────────────────────

    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;

    if poll.status != PollStatus::Resolved {
        return Err(PredictXError::PollNotActive);
    }

    let stake_record: Stake = env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, user.clone()))
        .ok_or(PredictXError::NotStaker)?;

    // Determine the winning side from the resolved outcome.
    let outcome = poll.outcome.ok_or(PredictXError::InvalidOutcome)?;
    if stake_record.side != winning_side(outcome) {
        return Err(PredictXError::NotOnWinningSide);
    }

    // ── Compute proportional payout ─────────────────────────────────────────

    let total_pool = poll.yes_pool + poll.no_pool;
    let winning_pool = if outcome { poll.yes_pool } else { poll.no_pool };

    let payout = compute_payout(env, stake_record.amount, winning_pool, total_pool)?;

    // ── Interaction ── transfer tokens from contract to user ────────────────

    token_utils::transfer_from_contract(env, &user, payout)?;

    // ── Effects ── update platform stats ─────────────────────────────────────

    let mut stats = get_platform_stats(env);
    stats.total_value_locked -= stake_record.amount;
    stats.total_payouts += payout;
    set_platform_stats(env, &stats);

    env.events()
        .publish((Symbol::new(env, "WinningsClaimed"), poll_id, user.clone()), payout);

    Ok(payout)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Address, Env, String,
    };
    use predictx_shared::{PollCategory, PollStatus, PredictXError, StakeSide, Poll, Stake};
    use crate::{DataKey, PredictionMarket, PredictionMarketClient};

    // ── Helpers ───────────────────────────────────────────────────────────────

    struct TestSetup<'a> {
        env: Env,
        admin: Address,
        oracle_id: Address,
        token_addr: Address,
        contract_id: Address,
        client: PredictionMarketClient<'a>,
    }

    fn setup() -> TestSetup<'static> {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        // Register voting oracle
        let oracle_id = env.register(crate::voting_oracle::WASM, ());
        let oracle_client = crate::voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        // Register a Stellar-asset token for staking
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_addr = token_contract.address();

        // Register prediction market and initialize
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle_id, &token_addr, &treasury, &500_u32);

        // Set ledger timestamp
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        TestSetup {
            env,
            admin,
            oracle_id,
            token_addr,
            contract_id,
            client,
        }
    }

    /// Create a test match + poll. Returns poll_id.
    fn create_test_poll(s: &TestSetup, lock_time: u64) -> u64 {
        let match_id = s.client.create_match(
            &s.admin,
            &String::from_str(&s.env, "Arsenal"),
            &String::from_str(&s.env, "Chelsea"),
            &String::from_str(&s.env, "Premier League"),
            &String::from_str(&s.env, "Emirates"),
            &(lock_time + 3600),
        );
        s.client.create_poll(
            &s.admin,
            &match_id,
            &String::from_str(&s.env, "Will Palmer score?"),
            &PollCategory::PlayerEvent,
            &lock_time,
        )
    }

    fn mint_tokens(s: &TestSetup, to: &Address, amount: i128) {
        let sac = token::StellarAssetClient::new(&s.env, &s.token_addr);
        sac.mint(to, &amount);
    }

    fn token_balance(s: &TestSetup, addr: &Address) -> i128 {
        token::Client::new(&s.env, &s.token_addr).balance(addr)
    }

    /// Resolve a poll in the prediction-market's local storage so that
    /// `claim_winnings` sees status = Resolved and a known outcome.
    fn resolve_poll(s: &TestSetup, poll_id: u64, outcome: bool) {
        s.env.as_contract(&s.contract_id, || {
            let mut poll: Poll =
                s.env.storage().persistent().get(&DataKey::Poll(poll_id)).unwrap();
            poll.status = PollStatus::Resolved;
            poll.outcome = Some(outcome);
            s.env.storage().persistent().set(&DataKey::Poll(poll_id), &poll);
        });
    }

    // ── Success cases ────────────────────────────────────────────────────────

    #[test]
    fn winning_staker_receives_proportional_payout_yes_wins() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        // Two stakers: 100M on Yes, 200M on No
        let yes_user = Address::generate(&s.env);
        let no_user = Address::generate(&s.env);
        let yes_amount: i128 = 100_000_000;
        let no_amount: i128 = 200_000_000;
        mint_tokens(&s, &yes_user, yes_amount);
        mint_tokens(&s, &no_user, no_amount);

        s.client.stake(&yes_user, &poll_id, &yes_amount, &StakeSide::Yes);
        s.client.stake(&no_user, &poll_id, &no_amount, &StakeSide::No);

        // Contract now holds yes_amount + no_amount = 300M
        assert_eq!(token_balance(&s, &s.contract_id), yes_amount + no_amount);

        // Resolve: Yes wins
        resolve_poll(&s, poll_id, true);

        // --- expected payout ---
        // total_pool   = 100M + 200M = 300M
        // fee (5%)     = 300M * 500 / 10_000 = 15M
        // distributable = 300M - 15M = 285M
        // winning_pool (Yes) = 100M
        // payout = 100M * 285M / 100M = 285M
        let total_pool = yes_amount + no_amount;
        let fee_bps = 500_u32;
        let fee_amount = total_pool * (fee_bps as i128) / (10_000_i128);
        let distributable = total_pool - fee_amount;
        let expected_payout = yes_amount * distributable / yes_amount; // = 285_000_000

        let paid = s.client.claim_winnings(&yes_user, &poll_id);
        assert_eq!(paid, expected_payout);

        // User received the payout
        assert_eq!(token_balance(&s, &yes_user), expected_payout);
        // Contract balance decreased by the payout (fee remains in contract)
        assert_eq!(token_balance(&s, &s.contract_id), (yes_amount + no_amount) - expected_payout);
    }

    #[test]
    fn winning_staker_receives_proportional_payout_no_wins() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        let yes_user = Address::generate(&s.env);
        let no_user = Address::generate(&s.env);
        let yes_amount: i128 = 150_000_000;
        let no_amount: i128 = 50_000_000;
        mint_tokens(&s, &yes_user, yes_amount);
        mint_tokens(&s, &no_user, no_amount);

        s.client.stake(&yes_user, &poll_id, &yes_amount, &StakeSide::Yes);
        s.client.stake(&no_user, &poll_id, &no_amount, &StakeSide::No);

        // Resolve: No wins
        resolve_poll(&s, poll_id, false);

        // total_pool = 200M, fee 5% = 10M, distributable = 190M
        // winning_pool (No) = 50M
        // payout = 50M * 190M / 50M = 190M
        let total_pool = yes_amount + no_amount;
        let fee_amount = total_pool * 500 / 10_000;
        let distributable = total_pool - fee_amount;
        let expected_payout = no_amount * distributable / no_amount; // = 190_000_000

        let paid = s.client.claim_winnings(&no_user, &poll_id);
        assert_eq!(paid, expected_payout);

        assert_eq!(token_balance(&s, &no_user), expected_payout);
    }

    #[test]
    fn winning_staker_share_is_proportional_to_stake() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        // Three yes stakers with different amounts
        let user_a = Address::generate(&s.env);
        let user_b = Address::generate(&s.env);
        let user_c = Address::generate(&s.env);
        let no_user = Address::generate(&s.env);

        let stake_a: i128 = 100_000_000;
        let stake_b: i128 = 200_000_000;
        let stake_c: i128 = 300_000_000;
        let stake_no: i128 = 600_000_000;

        mint_tokens(&s, &user_a, stake_a);
        mint_tokens(&s, &user_b, stake_b);
        mint_tokens(&s, &user_c, stake_c);
        mint_tokens(&s, &no_user, stake_no);

        s.client.stake(&user_a, &poll_id, &stake_a, &StakeSide::Yes);
        s.client.stake(&user_b, &poll_id, &stake_b, &StakeSide::Yes);
        s.client.stake(&user_c, &poll_id, &stake_c, &StakeSide::Yes);
        s.client.stake(&no_user, &poll_id, &stake_no, &StakeSide::No);

        resolve_poll(&s, poll_id, true);

        let total_pool = stake_a + stake_b + stake_c + stake_no; // = 1.2B
        let winning_pool = stake_a + stake_b + stake_c; // = 600M
        let fee_amount = total_pool * 500 / 10_000; // = 60M
        let distributable = total_pool - fee_amount; // = 1.14B

        let expected_a = stake_a * distributable / winning_pool;
        let expected_b = stake_b * distributable / winning_pool;
        let expected_c = stake_c * distributable / winning_pool;

        let paid_a = s.client.claim_winnings(&user_a, &poll_id);
        let paid_b = s.client.claim_winnings(&user_b, &poll_id);
        let paid_c = s.client.claim_winnings(&user_c, &poll_id);

        assert_eq!(paid_a, expected_a);
        assert_eq!(paid_b, expected_b);
        assert_eq!(paid_c, expected_c);

        // Sum of payouts + fee should equal total pool
        assert_eq!(
            paid_a + paid_b + paid_c + fee_amount,
            total_pool
        );
    }

    // ── Rejection cases ──────────────────────────────────────────────────────

    #[test]
    fn losing_staker_gets_not_on_winning_side() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        let yes_user = Address::generate(&s.env);
        let no_user = Address::generate(&s.env);
        let yes_amount: i128 = 100_000_000;
        let no_amount: i128 = 100_000_000;
        mint_tokens(&s, &yes_user, yes_amount);
        mint_tokens(&s, &no_user, no_amount);

        s.client.stake(&yes_user, &poll_id, &yes_amount, &StakeSide::Yes);
        s.client.stake(&no_user, &poll_id, &no_amount, &StakeSide::No);

        resolve_poll(&s, poll_id, true); // Yes wins

        let err = s
            .client
            .try_claim_winnings(&no_user, &poll_id)
            .expect_err("should reject losing staker");
        assert_eq!(err, Ok(PredictXError::NotOnWinningSide));
    }

    #[test]
    fn non_staker_gets_not_staker() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        // Only one person stakes
        let user = Address::generate(&s.env);
        let amount: i128 = 100_000_000;
        mint_tokens(&s, &user, amount);
        s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);

        resolve_poll(&s, poll_id, true);

        let non_staker = Address::generate(&s.env);

        let err = s
            .client
            .try_claim_winnings(&non_staker, &poll_id)
            .expect_err("should reject non-staker");
        assert_eq!(err, Ok(PredictXError::NotStaker));
    }

    #[test]
    fn unresolved_poll_gets_poll_not_active() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        let user = Address::generate(&s.env);
        let amount: i128 = 100_000_000;
        mint_tokens(&s, &user, amount);
        s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);

        // Poll is still Active — not resolved
        let err = s
            .client
            .try_claim_winnings(&user, &poll_id)
            .expect_err("should reject unresolved poll");
        assert_eq!(err, Ok(PredictXError::PollNotActive));
    }
}
