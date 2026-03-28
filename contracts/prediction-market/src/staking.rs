use soroban_sdk::{Address, Env, Symbol, Vec};
use predictx_shared::{
    Poll, PollStatus, Stake, StakeSide, PredictXError,
    MIN_STAKE_AMOUNT, MAX_STAKE_AMOUNT, BPS_DENOMINATOR, LOCK_TIME_BUFFER_SECS,
    safe_add, safe_sub, safe_proportional, validate_stake_amount,
    is_before_lock_time, verify_solvency, 
    ReentrancyGuard, ReentrancyGuardFunction,
};
use crate::{DataKey, PoolInfo, get_platform_stats, set_platform_stats, ensure_not_paused, token_utils};

// ── Stake placement ───────────────────────────────────────────────────────────

/// Place a stake on a poll outcome. Follows Checks-Effects-Interactions pattern
/// with reentrancy protection.
///
/// Security measures:
/// 1. Reentrancy guard prevents recursive calls
/// 2. Input validation for amount (positive, min/max bounds)
/// 3. Lock time validation with buffer to prevent frontrunning
/// 4. Double-stake prevention
/// 5. State changes before external calls (CEI pattern)
pub fn stake(
    env: &Env,
    staker: Address,
    poll_id: u64,
    amount: i128,
    side: StakeSide,
) -> Result<Stake, PredictXError> {
    // ── Reentrancy protection ──────────────────────────────────────────────
    let _guard = ReentrancyGuard::new(env, ReentrancyGuardFunction::Stake);

    staker.require_auth();
    ensure_not_paused(env)?;

    // ── Checks ────────────────────────────────────────────────────────────────

    // Validate stake amount bounds (prevents overflow attacks)
    validate_stake_amount(amount, MIN_STAKE_AMOUNT, MAX_STAKE_AMOUNT)?;

    let mut poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;

    // Poll must be active
    if poll.status != PollStatus::Active {
        return Err(PredictXError::PollNotActive);
    }

    // Check lock time with buffer to prevent frontrunning
    // Users cannot stake too close to lock time (frontrunning prevention)
    let current_time = env.ledger().timestamp();
    if !is_before_lock_time(current_time, poll.lock_time, LOCK_TIME_BUFFER_SECS) {
        return Err(PredictXError::PollLocked);
    }

    // Check if user has already staked
    if env
        .storage()
        .persistent()
        .has(&DataKey::HasStaked(poll_id, staker.clone()))
    {
        return Err(PredictXError::AlreadyStaked);
    }

    // ── Effects ───────────────────────────────────────────────────────────────
    // State changes happen BEFORE external token transfer (CEI pattern)

    let stake_record = Stake {
        user: staker.clone(),
        poll_id,
        amount,
        side,
        claimed: false,
        staked_at: current_time,
    };

    // Store stake record + flag
    env.storage()
        .persistent()
        .set(&DataKey::Stake(poll_id, staker.clone()), &stake_record);
    env.storage()
        .persistent()
        .set(&DataKey::HasStaked(poll_id, staker.clone()), &true);

    // Update pool totals using safe arithmetic
    match side {
        StakeSide::Yes => {
            poll.yes_pool = safe_add(poll.yes_pool, amount)?;
            poll.yes_count += 1;
        }
        StakeSide::No => {
            poll.no_pool = safe_add(poll.no_pool, amount)?;
            poll.no_count += 1;
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::Poll(poll_id), &poll);

    // Track user's staked polls
    let mut user_stakes: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::UserStakes(staker.clone()))
        .unwrap_or(Vec::new(env));
    user_stakes.push_back(poll_id);
    env.storage()
        .persistent()
        .set(&DataKey::UserStakes(staker.clone()), &user_stakes);

    // Update platform stats using safe arithmetic
    let mut stats = get_platform_stats(env);
    stats.total_value_locked = safe_add(stats.total_value_locked, amount)?;
    stats.total_stakes_placed += 1;
    set_platform_stats(env, &stats);

    // ── Interactions ──────────────────────────────────────────────────────────
    // External token transfer happens LAST after all state changes

    token_utils::transfer_to_contract(env, &staker, amount)?;

    // Emit event
    env.events().publish(
        (Symbol::new(env, "StakePlaced"), poll_id, staker),
        (amount, side),
    );

    Ok(stake_record)
}

// ── View functions ────────────────────────────────────────────────────────────

/// Retrieve a user's stake record for a poll.
pub fn get_stake_info(env: &Env, poll_id: u64, user: &Address) -> Result<Stake, PredictXError> {
    env.storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, user.clone()))
        .ok_or(PredictXError::NotStaker)
}

/// List all poll IDs a user has staked on.
pub fn get_user_stakes(env: &Env, user: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::UserStakes(user.clone()))
        .unwrap_or(Vec::new(env))
}

/// Check whether a user has already staked on a given poll.
pub fn has_user_staked(env: &Env, poll_id: u64, user: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::HasStaked(poll_id, user.clone()))
}

/// Calculate potential winnings **before** a stake is placed (read-only UI preview).
///
/// Formula (safe integer arithmetic, all in base token units):
/// ```text
/// pool_on_side_after  = pool_on_side + amount
/// total_pool_after    = yes_pool + no_pool + amount
/// winnings = amount * total_pool_after * (BPS_DENOMINATOR - PLATFORM_FEE_BPS)
///            / (pool_on_side_after * BPS_DENOMINATOR)
/// ```
/// Uses checked arithmetic to prevent overflow. Integer division rounds down
/// — dust stays in contract (benefits the platform).
pub fn calculate_potential_winnings(
    env: &Env,
    poll_id: u64,
    side: StakeSide,
    amount: i128,
) -> Result<i128, PredictXError> {
    if amount <= 0 {
        return Err(PredictXError::StakeAmountZero);
    }

    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;

    let pool_on_side = match side {
        StakeSide::Yes => poll.yes_pool,
        StakeSide::No => poll.no_pool,
    };

    // Use safe arithmetic to prevent overflow
    let pool_on_side_after = safe_add(pool_on_side, amount)?;
    let total_pool_after = safe_add(safe_add(poll.yes_pool, poll.no_pool)?, amount)?;

    let fee_bps = token_utils::get_platform_fee_bps(env);
    let fee_factor = (BPS_DENOMINATOR - fee_bps) as i128;
    let bps = BPS_DENOMINATOR as i128;

    // winnings = (amount / pool_on_side_after) * total_pool_after * (1 - fee%)
    // Use safe_proportional to prevent overflow
    let winnings = safe_proportional(
        safe_mul(amount, total_pool_after)?,
        fee_factor,
        safe_mul(pool_on_side_after, bps)?,
    )?;

    Ok(winnings)
}

/// Return pool state for a poll.
pub fn get_pool_info(env: &Env, poll_id: u64) -> Result<PoolInfo, PredictXError> {
    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;

    Ok(PoolInfo {
        yes_pool: poll.yes_pool,
        no_pool: poll.no_pool,
        yes_count: poll.yes_count,
        no_count: poll.no_count,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Address, Env, String,
    };
    use predictx_shared::{PollCategory, PollStatus, PredictXError, StakeSide, Poll};
    use crate::{DataKey, PredictionMarket, PredictionMarketClient};

    // ── Helpers ───────────────────────────────────────────────────────────────

    struct TestSetup<'a> {
        env: Env,
        admin: Address,
        #[allow(dead_code)]
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

        TestSetup { env, admin, oracle_id, token_addr, contract_id, client }
    }

    /// Create a test match + poll with the given lock_time.  Returns poll_id.
    fn create_test_poll(s: &TestSetup, lock_time: u64) -> u64 {
        let match_id = s.client.create_match(
            &s.admin,
            &String::from_str(&s.env, "Arsenal"),
            &String::from_str(&s.env, "Chelsea"),
            &String::from_str(&s.env, "Premier League"),
            &String::from_str(&s.env, "Emirates"),
            &(lock_time + 3600), // kickoff after lock_time
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

    // ── Stake placement ───────────────────────────────────────────────────────

    #[test]
    fn stake_yes_side_succeeds() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let amount: i128 = 100_000_000;
        mint_tokens(&s, &user, amount);

        let stake = s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);

        assert_eq!(stake.user, user);
        assert_eq!(stake.poll_id, poll_id);
        assert_eq!(stake.amount, amount);
        assert_eq!(stake.side, StakeSide::Yes);
        assert!(!stake.claimed);
    }

    #[test]
    fn stake_no_side_succeeds() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let amount: i128 = 50_000_000;
        mint_tokens(&s, &user, amount);

        let stake = s.client.stake(&user, &poll_id, &amount, &StakeSide::No);

        assert_eq!(stake.side, StakeSide::No);
        assert_eq!(stake.amount, amount);
    }

    // ── Rejection tests ───────────────────────────────────────────────────────

    #[test]
    fn stake_rejects_on_non_active_poll() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        // Manually set poll status to Locked
        s.env.as_contract(&s.contract_id, || {
            let mut poll: Poll =
                s.env.storage().persistent().get(&DataKey::Poll(poll_id)).unwrap();
            poll.status = PollStatus::Locked;
            s.env.storage().persistent().set(&DataKey::Poll(poll_id), &poll);
        });

        let user = Address::generate(&s.env);
        mint_tokens(&s, &user, 50_000_000);

        let err = s
            .client
            .try_stake(&user, &poll_id, &50_000_000_i128, &StakeSide::Yes)
            .expect_err("should reject");
        assert_eq!(err, Ok(PredictXError::PollNotActive));
    }

    #[test]
    fn stake_rejects_after_lock_time() {
        let s = setup();
        let lock_time = 1_500_000;
        let poll_id = create_test_poll(&s, lock_time);

        // Advance time past lock_time
        s.env.ledger().with_mut(|l| l.timestamp = lock_time + 1);

        let user = Address::generate(&s.env);
        mint_tokens(&s, &user, 50_000_000);

        let err = s
            .client
            .try_stake(&user, &poll_id, &50_000_000_i128, &StakeSide::Yes)
            .expect_err("should reject");
        assert_eq!(err, Ok(PredictXError::PollLocked));
    }

    #[test]
    fn stake_rejects_double_stake() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        mint_tokens(&s, &user, 100_000_000);

        s.client.stake(&user, &poll_id, &50_000_000_i128, &StakeSide::Yes);

        let err = s
            .client
            .try_stake(&user, &poll_id, &50_000_000_i128, &StakeSide::No)
            .expect_err("should reject double-stake");
        assert_eq!(err, Ok(PredictXError::AlreadyStaked));
    }

    #[test]
    fn stake_rejects_zero_amount() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);

        let err = s
            .client
            .try_stake(&user, &poll_id, &0_i128, &StakeSide::Yes)
            .expect_err("should reject");
        assert_eq!(err, Ok(PredictXError::StakeAmountZero));
    }

    #[test]
    fn stake_rejects_negative_amount() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);

        let err = s
            .client
            .try_stake(&user, &poll_id, &(-100_i128), &StakeSide::Yes)
            .expect_err("should reject");
        assert_eq!(err, Ok(PredictXError::StakeAmountZero));
    }

    #[test]
    fn stake_rejects_below_minimum() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let small_amount: i128 = 1_000; // well below MIN_STAKE_AMOUNT
        mint_tokens(&s, &user, small_amount);

        let err = s
            .client
            .try_stake(&user, &poll_id, &small_amount, &StakeSide::Yes)
            .expect_err("should reject");
        assert_eq!(err, Ok(PredictXError::StakeBelowMinimum));
    }

    // ── Pool management ───────────────────────────────────────────────────────

    #[test]
    fn pool_totals_update_correctly_with_multiple_stakers() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        let users: soroban_sdk::Vec<Address> = {
            let mut v = soroban_sdk::Vec::new(&s.env);
            for _ in 0..3 {
                v.push_back(Address::generate(&s.env));
            }
            v
        };
        let amounts: [i128; 3] = [100_000_000, 200_000_000, 150_000_000];

        for i in 0..3u32 {
            mint_tokens(&s, &users.get(i).unwrap(), amounts[i as usize]);
        }

        // Two yes stakers, one no staker
        s.client.stake(&users.get(0).unwrap(), &poll_id, &amounts[0], &StakeSide::Yes);
        s.client.stake(&users.get(1).unwrap(), &poll_id, &amounts[1], &StakeSide::No);
        s.client.stake(&users.get(2).unwrap(), &poll_id, &amounts[2], &StakeSide::Yes);

        let pool = s.client.get_pool_info(&poll_id);
        assert_eq!(pool.yes_pool, amounts[0] + amounts[2]);
        assert_eq!(pool.no_pool, amounts[1]);
        assert_eq!(pool.yes_count, 2);
        assert_eq!(pool.no_count, 1);
    }

    #[test]
    fn token_transfer_occurs_correctly() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let amount: i128 = 100_000_000;
        mint_tokens(&s, &user, amount);

        assert_eq!(token_balance(&s, &user), amount);
        assert_eq!(token_balance(&s, &s.contract_id), 0);

        s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);

        assert_eq!(token_balance(&s, &user), 0);
        assert_eq!(token_balance(&s, &s.contract_id), amount);
    }

    #[test]
    fn concurrent_staking_on_both_sides() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let base: i128 = 50_000_000;

        let mut total_yes: i128 = 0;
        let mut total_no: i128 = 0;

        // 5 yes stakers
        for i in 1..=5_i128 {
            let u = Address::generate(&s.env);
            let amt = base * i;
            mint_tokens(&s, &u, amt);
            s.client.stake(&u, &poll_id, &amt, &StakeSide::Yes);
            total_yes += amt;
        }

        // 3 no stakers
        for i in 1..=3_i128 {
            let u = Address::generate(&s.env);
            let amt = base * (i + 1);
            mint_tokens(&s, &u, amt);
            s.client.stake(&u, &poll_id, &amt, &StakeSide::No);
            total_no += amt;
        }

        let pool = s.client.get_pool_info(&poll_id);
        assert_eq!(pool.yes_pool, total_yes);
        assert_eq!(pool.no_pool, total_no);
        assert_eq!(pool.yes_count, 5);
        assert_eq!(pool.no_count, 3);

        // Verify contract holds all tokens
        assert_eq!(token_balance(&s, &s.contract_id), total_yes + total_no);
    }

    // ── View function tests ───────────────────────────────────────────────────

    #[test]
    fn get_stake_returns_correct_record() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let amount: i128 = 75_000_000;
        mint_tokens(&s, &user, amount);

        s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);

        let stake = s.client.get_stake_info(&poll_id, &user);
        assert_eq!(stake.user, user);
        assert_eq!(stake.poll_id, poll_id);
        assert_eq!(stake.amount, amount);
        assert_eq!(stake.side, StakeSide::Yes);
        assert!(!stake.claimed);
    }

    #[test]
    fn get_user_stakes_tracks_poll_ids() {
        let s = setup();
        let poll_id1 = create_test_poll(&s, 2_000_000);
        let poll_id2 = create_test_poll(&s, 2_000_000);

        let user = Address::generate(&s.env);
        let amount: i128 = 50_000_000;
        mint_tokens(&s, &user, amount * 2);

        s.client.stake(&user, &poll_id1, &amount, &StakeSide::Yes);
        s.client.stake(&user, &poll_id2, &amount, &StakeSide::No);

        let stakes = s.client.get_user_stakes(&user);
        assert_eq!(stakes.len(), 2);
        assert_eq!(stakes.get(0).unwrap(), poll_id1);
        assert_eq!(stakes.get(1).unwrap(), poll_id2);
    }

    #[test]
    fn has_staked_returns_correct_value() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let amount: i128 = 50_000_000;
        mint_tokens(&s, &user, amount);

        assert!(!s.client.has_user_staked(&poll_id, &user));
        s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);
        assert!(s.client.has_user_staked(&poll_id, &user));
    }

    #[test]
    fn platform_stats_update_on_stake() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        let user1 = Address::generate(&s.env);
        let user2 = Address::generate(&s.env);
        let amount1: i128 = 100_000_000;
        let amount2: i128 = 200_000_000;
        mint_tokens(&s, &user1, amount1);
        mint_tokens(&s, &user2, amount2);

        s.client.stake(&user1, &poll_id, &amount1, &StakeSide::Yes);
        s.client.stake(&user2, &poll_id, &amount2, &StakeSide::No);

        let stats = s.client.get_platform_stats();
        assert_eq!(stats.total_value_locked, amount1 + amount2);
        assert_eq!(stats.total_stakes_placed, 2);
    }

    // ── Potential winnings calculator ─────────────────────────────────────────

    #[test]
    fn potential_winnings_calculation_accuracy() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        // Build a pool: Yes = 7_000 tokens, No = 3_000 tokens
        // (using 7 decimal places: 7_000 * 10^7 = 70_000_000_000)
        let yes_amount: i128 = 70_000_000_000;
        let no_amount: i128 = 30_000_000_000;

        let yes_user = Address::generate(&s.env);
        let no_user = Address::generate(&s.env);
        mint_tokens(&s, &yes_user, yes_amount);
        mint_tokens(&s, &no_user, no_amount);

        s.client.stake(&yes_user, &poll_id, &yes_amount, &StakeSide::Yes);
        s.client.stake(&no_user, &poll_id, &no_amount, &StakeSide::No);

        // Simulate a new 700-token yes stake (7_000_000_000 base units)
        let new_stake: i128 = 7_000_000_000;
        let winnings = s.client.calculate_potential_winnings(
            &poll_id,
            &StakeSide::Yes,
            &new_stake,
        );

        // pool_on_side_after = 70B + 7B = 77B = 77_000_000_000
        // total_pool_after   = 77B + 30B = 107B = 107_000_000_000
        // winnings = 7B * 107B * 9500 / (77B * 10000)
        let expected = new_stake * 107_000_000_000_i128 * 9500
            / (77_000_000_000_i128 * 10_000);
        assert_eq!(winnings, expected);
        assert!(winnings > 0);

        // Winnings should be less than total pool (sanity check)
        assert!(winnings < 107_000_000_000);
    }

    #[test]
    fn potential_winnings_first_staker_on_empty_pool() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        let amount: i128 = 100_000_000;
        let winnings = s.client.calculate_potential_winnings(
            &poll_id,
            &StakeSide::Yes,
            &amount,
        );

        // First staker: pool_on_side_after = amount, total_pool_after = amount
        // winnings = amount * amount * 9500 / (amount * 10000) = amount * 9500 / 10000
        let expected = amount * 9500 / 10_000;
        assert_eq!(winnings, expected);
    }

    #[test]
    fn potential_winnings_rejects_zero_amount() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        let err = s
            .client
            .try_calculate_potential_winnings(&poll_id, &StakeSide::Yes, &0_i128)
            .expect_err("should reject zero");
        assert_eq!(err, Ok(PredictXError::StakeAmountZero));
    }

    // ══ Token integration tests ═══════════════════════════════════════════════

    #[test]
    fn get_contract_balance_returns_zero_initially() {
        let s = setup();
        assert_eq!(s.client.get_contract_balance(), 0);
    }

    #[test]
    fn get_contract_balance_reflects_staked_tokens() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let amount: i128 = 100_000_000;
        mint_tokens(&s, &user, amount);

        s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);

        assert_eq!(s.client.get_contract_balance(), amount);
    }

    #[test]
    fn get_token_address_returns_stored_address() {
        let s = setup();
        assert_eq!(s.client.get_token_address(), s.token_addr);
    }

    #[test]
    fn get_treasury_address_returns_stored_address() {
        let s = setup();
        let treasury: Address = s.env.as_contract(&s.contract_id, || {
            s.env.storage().instance().get(&DataKey::TreasuryAddress).unwrap()
        });
        assert_eq!(s.client.get_treasury_address(), treasury);
    }

    #[test]
    fn get_platform_fee_bps_returns_configured_value() {
        let s = setup();
        assert_eq!(s.client.get_platform_fee_bps(), 500);
    }

    #[test]
    fn transfer_to_contract_moves_exact_amount() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let deposit: i128 = 200_000_000;
        mint_tokens(&s, &user, deposit);

        let stake_amount: i128 = 150_000_000;
        s.client.stake(&user, &poll_id, &stake_amount, &StakeSide::Yes);

        // User should have deposit - stake_amount left
        assert_eq!(token_balance(&s, &user), deposit - stake_amount);
        // Contract should hold exactly stake_amount
        assert_eq!(token_balance(&s, &s.contract_id), stake_amount);
    }

    #[test]
    fn multiple_stakes_accumulate_contract_balance() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let base: i128 = 50_000_000;
        let mut total: i128 = 0;

        for i in 1..=4_i128 {
            let u = Address::generate(&s.env);
            let amt = base * i;
            mint_tokens(&s, &u, amt);
            let side = if i % 2 == 0 { StakeSide::No } else { StakeSide::Yes };
            s.client.stake(&u, &poll_id, &amt, &side);
            total += amt;
        }

        assert_eq!(s.client.get_contract_balance(), total);
        assert_eq!(token_balance(&s, &s.contract_id), total);
    }
}
