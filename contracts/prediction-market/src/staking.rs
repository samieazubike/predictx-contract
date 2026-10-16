use soroban_sdk::{Address, Env, Symbol, Vec};
use crate::{PoolInfo, get_platform_stats, set_platform_stats, ensure_not_paused, token_utils};
use predictx_shared::{
    Poll, PollStatus, Stake, StakeSide, PredictXError,
    MIN_STAKE_AMOUNT, BPS_DENOMINATOR, MAX_USER_STAKES_PAGE_SIZE,
    DataKey, Poll, PollStatus, Stake, StakeSide, PredictXError,
    Poll, PollStatus, Stake, StakeSide, UserStats, PredictXError,
    Poll, PollStatus, Stake, StakeSide, PredictXError, UserStats,
    MIN_STAKE_AMOUNT, BPS_DENOMINATOR,
    MIN_STAKE_AMOUNT, BPS_DENOMINATOR, MAX_CUMULATIVE_STAKE_PER_POLL,
};
use crate::{
    ensure_not_paused, get_platform_stats, set_platform_stats, token_utils, DataKey, PoolInfo,
};
use predictx_shared::{
    Poll, PollStatus, PredictXError, Stake, StakeSide, BPS_DENOMINATOR, MAX_STAKE_AMOUNT,
    MAX_USER_STAKES_PAGE_SIZE, MIN_STAKE_AMOUNT,
};
use crate::{
    ensure_not_paused, get_platform_stats, set_platform_stats, token_utils, DataKey, PoolInfo,
};
use predictx_shared::{
    Poll, PollStatus, PredictXError, Stake, StakeSide, BPS_DENOMINATOR, MIN_STAKE_AMOUNT,
};
use soroban_sdk::{Address, Env, Symbol, Vec};
use crate::{DataKey, PoolInfo, get_platform_stats, set_platform_stats, ensure_not_paused, token_utils, load_poll, store_poll, store_stake};

// ── User statistics ──────────────────────────────────────────────────────────

/// Load a user's aggregate stats, creating a zeroed record for first-time users.
pub(crate) fn get_user_stats(env: &Env, user: &Address) -> UserStats {
    env.storage()
        .persistent()
        .get(&DataKey::UserStats(user.clone()))
        .unwrap_or(UserStats {
            total_staked: 0,
            total_won: 0,
            total_lost: 0,
            polls_participated: 0,
            polls_won: 0,
            polls_lost: 0,
            votes_cast: 0,
            voting_rewards_earned: 0,
        })
}

/// Record a successful stake in the user's stats.
///
/// One storage read + one write per stake — no scans.
/// `polls_participated` counts *distinct* polls: a user can only stake once
/// per poll (enforced by `HasStaked`), so every successful stake is a new
/// poll participation by construction.
fn record_stake_in_user_stats(env: &Env, staker: &Address, amount: i128) {
    let mut user_stats = get_user_stats(env, staker);
    user_stats.total_staked += amount;
    user_stats.polls_participated += 1;
    env.storage()
        .persistent()
        .set(&DataKey::UserStats(staker.clone()), &user_stats);
}
use crate::{DataKey, PoolInfo, get_platform_stats, set_platform_stats, ensure_not_paused, token_utils, escrow};
use crate::{PoolInfo, get_platform_stats, set_platform_stats, ensure_not_paused, token_utils};
use predictx_shared::DataKey;
use crate::{
    ensure_not_paused, get_platform_stats, set_platform_stats, token_utils, DataKey, PoolInfo,
};
use predictx_shared::{
    Poll, PollStatus, PredictXError, Stake, StakeSide, BPS_DENOMINATOR, MIN_STAKE_AMOUNT,
};
use soroban_sdk::{Address, Env, Symbol, Vec};

// ── Stake placement ───────────────────────────────────────────────────────────

/// Place a stake on a poll outcome. Follows Checks-Effects-Interactions pattern.
///
/// 1. Validates all preconditions (checks)
/// 2. Transfers tokens from staker to contract (interactions — first because
///    Soroban token transfers are safe against re-entrancy)
/// 3. Records state changes (effects)
///
/// A single stake must sit within `[MIN_STAKE_AMOUNT, MAX_STAKE_AMOUNT]`; both
/// bounds are inclusive. The ceiling is per stake, not per user — a user may
/// still back a poll with several accounts.
pub fn stake(
    env: &Env,
    staker: Address,
    poll_id: u64,
    amount: i128,
    side: StakeSide,
) -> Result<Stake, PredictXError> {
    staker.require_auth();
    ensure_not_paused(env)?;

    // ── Checks ────────────────────────────────────────────────────────────────

    if amount <= 0 {
        return Err(PredictXError::StakeAmountZero);
    }
    if amount < MIN_STAKE_AMOUNT {
        return Err(PredictXError::StakeBelowMinimum);
    }
    if amount > MAX_STAKE_AMOUNT {
        return Err(PredictXError::StakeAboveMaximum);
    }

    let mut poll: Poll = load_poll(env, poll_id)
        .ok_or(PredictXError::PollNotFound)?;

    if poll.outcome.is_some() || poll.outcome_source.is_some() {
        return Err(PredictXError::PollAlreadyResolved);
    }

    if poll.status != PollStatus::Active {
        return Err(PredictXError::PollNotActive);
    }

    if env.ledger().timestamp() >= poll.lock_time {
        // Lazily persist the lock: the poll is past its lock time, so the
        // stored status must self-correct to `Locked` before we reject the
        // stake. A single extra storage write, only on this path.
        if poll.status != PollStatus::Locked {
            poll.status = PollStatus::Locked;
            env.storage()
                .persistent()
                .set(&DataKey::Poll(poll_id), &poll);
        }
        return Err(PredictXError::PollLocked);
    }

    if env
        .storage()
        .persistent()
        .has(&DataKey::HasStaked(poll_id, staker.clone()))
    {
        return Err(PredictXError::AlreadyStaked);
    }

    // Enforce the maximum cumulative stake per user per poll.  A per-call cap
    // is trivially bypassed by splitting across several calls, so we track the
    // running total for this (poll, user) pair and reject any stake that would
    // push it over the limit.
    let current_total: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::UserPollStakeTotal(poll_id, staker.clone()))
        .unwrap_or(0);
    let new_total = current_total
        .checked_add(amount)
        .ok_or(PredictXError::StakeAmountZero)?;
    if new_total > MAX_CUMULATIVE_STAKE_PER_POLL {
        return Err(PredictXError::CumulativeStakeLimitExceeded);
    }

    // ── Interactions ──────────────────────────────────────────────────────────

    token_utils::transfer_to_contract(env, &staker, amount)?;

    // ── Effects ───────────────────────────────────────────────────────────────

    let stake_record = Stake {
        user: staker.clone(),
        poll_id,
        amount,
        side,
        claimed: false,
        staked_at: env.ledger().timestamp(),
    };

    // Store stake record + flag
    store_stake(env, poll_id, &stake_record);
    env.storage()
        .persistent()
        .set(&DataKey::HasStaked(poll_id, staker.clone()), &true);

    let match_has_staked_key = DataKey::HasMatchStaked(poll.match_id, staker.clone());
    if !env.storage().persistent().has(&match_has_staked_key) {
        env.storage().persistent().set(&match_has_staked_key, &true);
        let distinct_stakers: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::MatchStakerCount(poll.match_id))
            .unwrap_or(0);
        env.storage().persistent().set(
            &DataKey::MatchStakerCount(poll.match_id),
            &(distinct_stakers + 1),
        );
    }
    // Persist the updated cumulative total for this (poll, user) pair.
    env.storage()
        .persistent()
        .set(&DataKey::UserPollStakeTotal(poll_id, staker.clone()), &new_total);

    // Update pool totals
    match side {
        StakeSide::Yes => {
            poll.yes_pool += amount;
            poll.yes_count += 1;
        }
        StakeSide::No => {
            poll.no_pool += amount;
            poll.no_count += 1;
        }
    }
    store_poll(env, &poll);

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

    // Update platform stats
    let mut stats = get_platform_stats(env);
    stats.total_value_locked += amount;
    stats.total_stakes_placed += 1;
    set_platform_stats(env, &stats);

    // Update per-user stats (issue #149)
    record_stake_in_user_stats(env, &staker, amount);
    // Track per-poll escrow liability
    escrow::add_liability(env, poll_id, amount);

    // Emit event
    env.events().publish(
        (Symbol::new(env, "StakePlaced"), poll_id, staker),
        (amount, side),
    );

    Ok(stake_record)
}

// ── Treasury accounting events ────────────────────────────────────────────────

/// Emit `FeesDeposited(from, amount)` after a successful fee deposit.
pub fn emit_fees_deposited(env: &Env, from: Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "FeesDeposited"), from),
        amount,
    );
}

/// Emit `FeesWithdrawn(to, amount)` after a successful fee withdrawal.
pub fn emit_fees_withdrawn(env: &Env, to: Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "FeesWithdrawn"), to),
        amount,
    );
}

/// Emit `RewardsFunded(poll_id, amount)` after rewards are funded.
pub fn emit_rewards_funded(env: &Env, poll_id: u64, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "RewardsFunded"), poll_id),
        amount,
    );
}

// ── View functions ────────────────────────────────────────────────────────────

/// Retrieve a user's stake record for a poll.
pub fn get_stake_info(env: &Env, poll_id: u64, user: &Address) -> Result<Stake, PredictXError> {
    use crate::load_stake;
    load_stake(env, poll_id, user)
        .ok_or(PredictXError::NotStaker)
}

/// List all poll IDs a user has staked on.
///
/// # Deprecated
///
/// This function returns an unbounded `Vec` and will eventually fail once a
/// user's stake history grows past the ledger entry / resource limit. Prefer
/// [`get_user_stakes_paged`] for any new call sites. Kept for backwards
/// compatibility.
pub fn get_user_stakes(env: &Env, user: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::UserStakes(user.clone()))
        .unwrap_or(Vec::new(env))
}

/// Paginated view over a user's staked poll IDs.
///
/// Returns up to `limit` poll IDs starting at `start`, in the same order as
/// [`get_user_stakes`]. `limit` is capped at [`MAX_USER_STAKES_PAGE_SIZE`];
/// a `limit` of `0` yields an empty vector. Out-of-range `start` yields an
/// empty vector.
pub fn get_user_stakes_paged(env: &Env, user: &Address, start: u32, limit: u32) -> Vec<u64> {
    let all: Vec<u64> = env
        .storage()
        .persistent()
        .get(&DataKey::UserStakes(user.clone()))
        .unwrap_or(Vec::new(env));

    let capped = if limit > MAX_USER_STAKES_PAGE_SIZE {
        MAX_USER_STAKES_PAGE_SIZE
    } else {
        limit
    };

    let mut page: Vec<u64> = Vec::new(env);
    let len = all.len();
    if start >= len || capped == 0 {
        return page;
    }

    let end = core::cmp::min(start.saturating_add(capped), len);
    let mut i = start;
    while i < end {
        if let Some(id) = all.get(i) {
            page.push_back(id);
        }
        i += 1;
    }
    page
}

/// Check whether a user has already staked on a given poll.
pub fn has_user_staked(env: &Env, poll_id: u64, user: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::HasStaked(poll_id, user.clone()))
}

/// Return the cumulative amount a user has staked on a given poll.
///
/// This is the running total used to enforce `MAX_CUMULATIVE_STAKE_PER_POLL`.
/// Returns `0` if the user has never staked on the poll.
pub fn get_user_poll_stake_total(env: &Env, poll_id: u64, user: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::UserPollStakeTotal(poll_id, user.clone()))
        .unwrap_or(0)
}

/// Calculate potential winnings **before** a stake is placed (read-only UI preview).
///
/// Formula (integer arithmetic, all in base token units):
/// ```text
/// pool_on_side_after  = pool_on_side + amount
/// total_pool_after    = yes_pool + no_pool + amount
/// winnings = amount * total_pool_after * (BPS_DENOMINATOR - PLATFORM_FEE_BPS)
///            / (pool_on_side_after * BPS_DENOMINATOR)
/// ```
/// Integer division rounds down — dust stays in contract (benefits the platform).
pub fn calculate_potential_winnings(
    env: &Env,
    poll_id: u64,
    side: StakeSide,
    amount: i128,
) -> Result<i128, PredictXError> {
    if amount <= 0 {
        return Err(PredictXError::StakeAmountZero);
    }

    let poll: Poll = load_poll(env, poll_id)
        .ok_or(PredictXError::PollNotFound)?;

    let pool_on_side = match side {
        StakeSide::Yes => poll.yes_pool,
        StakeSide::No => poll.no_pool,
    };

    let pool_on_side_after = pool_on_side + amount;
    let total_pool_after = poll.yes_pool + poll.no_pool + amount;

    let fee_bps = token_utils::get_platform_fee_bps(env);
    let fee_factor = (BPS_DENOMINATOR - fee_bps) as i128;
    let bps = BPS_DENOMINATOR as i128;

    // winnings = (amount / pool_on_side_after) * total_pool_after * (1 - fee%)
    let winnings = amount * total_pool_after * fee_factor / (pool_on_side_after * bps);

    Ok(winnings)
}

/// Return the per-poll escrow liability tracked by the contract.
pub fn get_poll_escrow(env: &Env, poll_id: u64) -> i128 {
    escrow::get_liability(env, poll_id)
}

/// Return pool state for a poll.
pub fn get_pool_info(env: &Env, poll_id: u64) -> Result<PoolInfo, PredictXError> {
    let poll: Poll = load_poll(env, poll_id)
        .ok_or(PredictXError::PollNotFound)?;

    Ok(PoolInfo {
        yes_pool: poll.yes_pool,
        no_pool: poll.no_pool,
        yes_count: poll.yes_count,
        no_count: poll.no_count,
    })
}

// ── User stats helpers ────────────────────────────────────────────────────────

/// Record the outcome of a claim in the user's `UserStats`.
///
/// `total_won` is tracked as **net profit** (payout minus the original stake),
/// not gross payout.  This keeps win-rate and ROI numbers meaningful: a user
/// who breaks even on a large stake should not appear to have "won" that stake.
///
/// `outcome` semantics:
/// - `Some(true)`  → winning claim: increments `polls_won`, adds net profit to `total_won`
/// - `Some(false)` → losing position: increments `polls_lost`, adds stake to `total_lost`
/// - `None`        → refund on a cancelled poll: counts as neither
pub fn record_claim_outcome(
    env: &Env,
    user: &Address,
    outcome: Option<bool>,
    stake_amount: i128,
    payout: i128,
) {
    let mut stats: UserStats = env
        .storage()
        .persistent()
        .get(&DataKey::UserStats(user.clone()))
        .unwrap_or(UserStats {
            total_won: 0,
            total_lost: 0,
            polls_won: 0,
            polls_lost: 0,
        });

    match outcome {
        Some(true) => {
            stats.polls_won += 1;
            stats.total_won += payout - stake_amount;
        }
        Some(false) => {
            stats.polls_lost += 1;
            stats.total_lost += stake_amount;
        }
        None => {}
    }

    env.storage()
        .persistent()
        .set(&DataKey::UserStats(user.clone()), &stats);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use crate::{DataKey, PredictionMarket, PredictionMarketClient};
    use predictx_shared::{
        Poll, PollCategory, PollStatus, PredictXError, StakeSide, MAX_STAKE_AMOUNT,
        MIN_STAKE_AMOUNT,
    };
    use predictx_shared::{Poll, PollCategory, PollStatus, PredictXError, StakeSide};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Address, Env, String,
    };
    use predictx_shared::{DataKey, PollCategory, PollStatus, PredictXError, StakeSide, Poll};
    use crate::{PredictionMarket, PredictionMarketClient};
    use predictx_shared::{PollCategory, PollStatus, PredictXError, StakeSide, Poll, UserStats};
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

        // Register a Stellar-asset token for staking
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_addr = token_contract.address();

        oracle_client.initialize(&admin, &token_addr);

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
            let mut poll: Poll = s
                .env
                .storage()
                .persistent()
                .get(&DataKey::Poll(poll_id))
                .unwrap();
            poll.status = PollStatus::Locked;
            s.env
                .storage()
                .persistent()
                .set(&DataKey::Poll(poll_id), &poll);
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

    // ── Lazy auto-lock on stake (issue #125) ─────────────────────────────

    /// Staking past lock_time must both reject the stake AND persist the
    /// `Locked` status so the stored poll state self-corrects.
    #[test]
    fn stake_on_expired_poll_persists_locked_status() {
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

        // The rejection itself reverts with the host (failed invocations roll
        // back storage), so the persisted lock lands on the next successful
        // access: `get_poll` self-corrects and persists `Locked`.
        let poll = s.client.get_poll(&poll_id);
        assert_eq!(poll.status, PollStatus::Locked);

        // The raw storage entry now really reads Locked.
        let poll: Poll = s.env.as_contract(&s.contract_id, || {
            s.env
                .storage()
                .persistent()
                .get(&DataKey::Poll(poll_id))
                .unwrap()
        });
        assert_eq!(poll.status, PollStatus::Locked);
    }

    /// A second attempt after the lazy lock must be rejected with
    /// `PollNotActive` — proving the status really was persisted, not
    /// merely recomputed on each call.
    #[test]
    fn second_stake_after_lazy_lock_sees_locked_status() {
        let s = setup();
        let lock_time = 1_500_000;
        let poll_id = create_test_poll(&s, lock_time);

        s.env.ledger().with_mut(|l| l.timestamp = lock_time + 1);

        let user = Address::generate(&s.env);
        mint_tokens(&s, &user, 100_000_000);

        let err = s
            .client
            .try_stake(&user, &poll_id, &50_000_000_i128, &StakeSide::Yes)
            .expect_err("first attempt should lock the poll");
        assert_eq!(err, Ok(PredictXError::PollLocked));

        // A successful read persists the correction (`Locked`), so the
        // status check now governs the second attempt.
        let _ = s.client.get_poll(&poll_id);

        // Status check now fires before the lock-time check.
        let err = s
            .client
            .try_stake(&user, &poll_id, &50_000_000_i128, &StakeSide::No)
            .expect_err("second attempt should fail on status");
        assert_eq!(err, Ok(PredictXError::PollNotActive));
    }

    /// An already-`Locked` poll is left untouched by a rejected stake:
    /// no pool mutations, no stake records.
    #[test]
    fn staking_locked_poll_does_not_mutate_pool_totals() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        // Pre-lock the poll without advancing time past its lock_time.
        s.env.as_contract(&s.contract_id, || {
            let mut poll: Poll = s
                .env
                .storage()
                .persistent()
                .get(&DataKey::Poll(poll_id))
                .unwrap();
            poll.status = PollStatus::Locked;
            s.env
                .storage()
                .persistent()
                .set(&DataKey::Poll(poll_id), &poll);
        });

        let user = Address::generate(&s.env);
        mint_tokens(&s, &user, 50_000_000);

        let err = s
            .client
            .try_stake(&user, &poll_id, &50_000_000_i128, &StakeSide::Yes)
            .expect_err("should reject");
        assert_eq!(err, Ok(PredictXError::PollNotActive));

        let pool = s.client.get_pool_info(&poll_id);
        assert_eq!(pool.yes_pool, 0);
        assert_eq!(pool.no_pool, 0);
        assert!(!s.client.has_user_staked(&poll_id, &user));
    }

    #[test]
    fn stake_rejects_double_stake() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        mint_tokens(&s, &user, 100_000_000);

        s.client
            .stake(&user, &poll_id, &50_000_000_i128, &StakeSide::Yes);

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

    // ── Maximum stake cap ─────────────────────────────────────────────────────

    #[test]
    fn stake_accepts_exactly_the_maximum() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        mint_tokens(&s, &user, MAX_STAKE_AMOUNT);

        let stake = s
            .client
            .stake(&user, &poll_id, &MAX_STAKE_AMOUNT, &StakeSide::Yes);

        assert_eq!(stake.amount, MAX_STAKE_AMOUNT);
        let pool = s.client.get_pool_info(&poll_id);
        assert_eq!(pool.yes_pool, MAX_STAKE_AMOUNT);
    }

    #[test]
    fn stake_rejects_one_base_unit_above_the_maximum() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let too_large: i128 = MAX_STAKE_AMOUNT + 1;
        mint_tokens(&s, &user, too_large);

        let err = s
            .client
            .try_stake(&user, &poll_id, &too_large, &StakeSide::Yes)
            .expect_err("should reject");

        assert_eq!(err, Ok(PredictXError::StakeAboveMaximum));
        // The rejected stake must leave the pool untouched.
        let pool = s.client.get_pool_info(&poll_id);
        assert_eq!(pool.yes_pool, 0);
        assert!(!s.client.has_user_staked(&poll_id, &user));
    }

    #[test]
    fn stake_rejects_far_above_the_maximum() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        // A whale-sized bet: 10_000 tokens at 7dp.
        let whale: i128 = 1_000_000_000;
        mint_tokens(&s, &user, whale);

        let err = s
            .client
            .try_stake(&user, &poll_id, &whale, &StakeSide::No)
            .expect_err("should reject");

        assert_eq!(err, Ok(PredictXError::StakeAboveMaximum));
        assert_eq!(token_balance(&s, &user), whale, "no tokens may move");
    // ── Cumulative stake limit ────────────────────────────────────────────────

    #[test]
    fn two_stakes_summing_under_limit_both_succeed() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);

        // Two stakes that together stay strictly below the cumulative cap.
        let first: i128 = MAX_CUMULATIVE_STAKE_PER_POLL / 4;
        let second: i128 = MAX_CUMULATIVE_STAKE_PER_POLL / 4;
        mint_tokens(&s, &user, first + second);

        s.client.stake(&user, &poll_id, &first, &StakeSide::Yes);
        s.client.stake(&user, &poll_id, &second, &StakeSide::No);

        assert_eq!(
            s.client.get_user_poll_stake_total(&poll_id, &user),
            first + second
        );
    }

    #[test]
    fn stake_crossing_cumulative_limit_is_rejected() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);

        // First stake fills most of the cap; the second would cross it.
        let first: i128 = MAX_CUMULATIVE_STAKE_PER_POLL - 1;
        let second: i128 = 2;
        mint_tokens(&s, &user, first + second);

        s.client.stake(&user, &poll_id, &first, &StakeSide::Yes);

        let err = s
            .client
            .try_stake(&user, &poll_id, &second, &StakeSide::No)
            .expect_err("should reject stake crossing cumulative limit");
        assert_eq!(err, Ok(PredictXError::CumulativeStakeLimitExceeded));

        // The running total must be unchanged after the rejected stake.
        assert_eq!(s.client.get_user_poll_stake_total(&poll_id, &user), first);
    }

    #[test]
    fn running_total_is_readable_and_starts_at_zero() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let amount: i128 = 50_000_000;
        mint_tokens(&s, &user, amount);

        assert_eq!(s.client.get_user_poll_stake_total(&poll_id, &user), 0);

        s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);

        assert_eq!(s.client.get_user_poll_stake_total(&poll_id, &user), amount);
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
        let amounts: [i128; 3] = [20_000_000, 40_000_000, 30_000_000];

        for i in 0..3u32 {
            mint_tokens(&s, &users.get(i).unwrap(), amounts[i as usize]);
        }

        // Two yes stakers, one no staker
        s.client.stake(
            &users.get(0).unwrap(),
            &poll_id,
            &amounts[0],
            &StakeSide::Yes,
        );
        s.client.stake(
            &users.get(1).unwrap(),
            &poll_id,
            &amounts[1],
            &StakeSide::No,
        );
        s.client.stake(
            &users.get(2).unwrap(),
            &poll_id,
            &amounts[2],
            &StakeSide::Yes,
        );

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
        // base * 5 stayers must stay inside MAX_STAKE_AMOUNT.
        let base: i128 = 10_000_000;

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
    fn get_user_stakes_paged_matches_unbounded_order() {
        let s = setup();
        let user = Address::generate(&s.env);
        let amount: i128 = 50_000_000;

        let mut expected: soroban_sdk::Vec<u64> = soroban_sdk::Vec::new(&s.env);
        for _ in 0..5 {
            let poll_id = create_test_poll(&s, 2_000_000);
            mint_tokens(&s, &user, amount);
            s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);
            expected.push_back(poll_id);
        }

        let all = s.client.get_user_stakes(&user);
        assert_eq!(all.len(), 5);

        let page = s.client.get_user_stakes_paged(&user, &0_u32, &10_u32);
        assert_eq!(page.len(), 5);
        for i in 0..5_u32 {
            assert_eq!(page.get(i).unwrap(), all.get(i).unwrap());
            assert_eq!(page.get(i).unwrap(), expected.get(i).unwrap());
        }
    }

    #[test]
    fn get_user_stakes_paged_caps_page_size() {
        let s = setup();
        let user = Address::generate(&s.env);
        let amount: i128 = 50_000_000;

        // Request far more than the cap; only the cap should be returned.
        s.env.budget().reset_unlimited();
        for _ in 0..(crate::staking::MAX_USER_STAKES_PAGE_SIZE + 5) {
            let poll_id = create_test_poll(&s, 2_000_000);
            mint_tokens(&s, &user, amount);
            s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);
        }

        // The view itself must still fit inside a normal invocation budget.
        s.env.budget().reset_default();
        let page = s.client.get_user_stakes_paged(&user, &0_u32, &u32::MAX);
        assert_eq!(page.len(), crate::staking::MAX_USER_STAKES_PAGE_SIZE);
    }

    #[test]
    fn get_user_stakes_paged_empty_for_user_with_no_stakes() {
        let s = setup();
        let user = Address::generate(&s.env);

        let page = s.client.get_user_stakes_paged(&user, &0_u32, &10_u32);
        assert_eq!(page.len(), 0);

        // Out-of-range start also yields an empty page.
        let page2 = s.client.get_user_stakes_paged(&user, &100_u32, &10_u32);
        assert_eq!(page2.len(), 0);
    }

    #[test]
    fn get_user_stakes_paged_stays_within_budget_for_100_plus_stakes() {
        let s = setup();
        let user = Address::generate(&s.env);
        let amount: i128 = 50_000_000;
        let total: u32 = 120;

        // Building 120 polls is what exhausts the default host budget, so
        // seed with an unlimited budget and measure only the view below.
        s.env.budget().reset_unlimited();
        let mut expected: soroban_sdk::Vec<u64> = soroban_sdk::Vec::new(&s.env);
        for _ in 0..total {
            let poll_id = create_test_poll(&s, 2_000_000);
            mint_tokens(&s, &user, amount);
            s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);
            expected.push_back(poll_id);
        }

        // Walk the full history in capped pages and confirm every id is
        // returned in the same order as the unbounded view, all within a
        // default-sized invocation budget.
        s.env.budget().reset_default();
        let page_size = crate::staking::MAX_USER_STAKES_PAGE_SIZE;
        let mut collected: soroban_sdk::Vec<u64> = soroban_sdk::Vec::new(&s.env);
        let mut start: u32 = 0;
        loop {
            let page = s.client.get_user_stakes_paged(&user, &start, &page_size);
            if page.len() == 0 {
                break;
            }
            // Each page must respect the cap.
            assert!(page.len() <= page_size);
            for i in 0..page.len() {
                collected.push_back(page.get(i).unwrap());
            }
            start = start.saturating_add(page.len());
        }

        assert_eq!(collected.len(), total);
        for i in 0..total {
            assert_eq!(collected.get(i).unwrap(), expected.get(i).unwrap());
        }
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
        let amount1: i128 = 30_000_000;
        let amount2: i128 = 40_000_000;
        mint_tokens(&s, &user1, amount1);
        mint_tokens(&s, &user2, amount2);

        s.client.stake(&user1, &poll_id, &amount1, &StakeSide::Yes);
        s.client.stake(&user2, &poll_id, &amount2, &StakeSide::No);

        let stats = s.client.get_platform_stats();
        assert_eq!(stats.total_value_locked, amount1 + amount2);
        assert_eq!(stats.total_stakes_placed, 2);
    }

    // ── User stats (issue #149) ───────────────────────────────────────────────

    /// First stake creates a zeroed-then-updated record rather than erroring.
    #[test]
    fn user_stats_created_on_first_stake() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);
        let amount: i128 = 50_000_000;
        mint_tokens(&s, &user, amount);

        // No record exists before the first stake
        s.env.as_contract(&s.contract_id, || {
            assert!(!s.env
                .storage()
                .persistent()
                .has(&DataKey::UserStats(user.clone())));
        });

        s.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);

        let stats = s.client.get_user_stats(&user);
        assert_eq!(stats.total_staked, amount);
        assert_eq!(stats.polls_participated, 1);
    }

    /// `total_staked` accumulates across polls for the same user.
    #[test]
    fn user_stats_total_staked_accumulates_across_polls() {
        let s = setup();
        let poll_id1 = create_test_poll(&s, 2_000_000);
        let poll_id2 = create_test_poll(&s, 2_000_000);
        let poll_id3 = create_test_poll(&s, 2_000_000);

        let user = Address::generate(&s.env);
        let amounts: [i128; 3] = [50_000_000, 120_000_000, 30_000_000];
        mint_tokens(&s, &user, amounts[0] + amounts[1] + amounts[2]);

        s.client.stake(&user, &poll_id1, &amounts[0], &StakeSide::Yes);
        s.client.stake(&user, &poll_id2, &amounts[1], &StakeSide::No);
        s.client.stake(&user, &poll_id3, &amounts[2], &StakeSide::Yes);

        let stats = s.client.get_user_stats(&user);
        assert_eq!(stats.total_staked, amounts[0] + amounts[1] + amounts[2]);
        assert_eq!(stats.polls_participated, 3);
    }

    /// `polls_participated` counts distinct polls, not stakes.
    ///
    /// A user can place at most one stake per poll (`AlreadyStaked` guard),
    /// so the equivalent of "two stakes on one poll" is: two users on one
    /// poll, each with their own counter — plus a rejected double-stake on
    /// the same poll must not touch the failed staker's stats.
    #[test]
    fn user_stats_polls_participated_counts_distinct_polls_not_stakes() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let poll_id2 = create_test_poll(&s, 2_000_000);

        let user_a = Address::generate(&s.env);
        let user_b = Address::generate(&s.env);
        let amount: i128 = 60_000_000;
        mint_tokens(&s, &user_a, amount * 2);
        mint_tokens(&s, &user_b, amount);

        // user_a stakes on two distinct polls → polls_participated == 2
        s.client.stake(&user_a, &poll_id, &amount, &StakeSide::Yes);
        s.client.stake(&user_a, &poll_id2, &amount, &StakeSide::No);

        // user_b is rejected by the AlreadyStaked-equivalent guard (same poll
        // as user_a is fine, but a second stake on the SAME poll must fail);
        // user_b's single successful stake on poll_id is their only participation.
        let err = s
            .client
            .try_stake(&user_a, &poll_id, &amount, &StakeSide::No)
            .expect_err("double stake on the same poll should fail");
        assert_eq!(err, Ok(PredictXError::AlreadyStaked));

        s.client.stake(&user_b, &poll_id, &amount, &StakeSide::No);

        let stats_a = s.client.get_user_stats(&user_a);
        assert_eq!(stats_a.polls_participated, 2);
        assert_eq!(stats_a.total_staked, amount * 2);

        let stats_b = s.client.get_user_stats(&user_b);
        assert_eq!(stats_b.polls_participated, 1);
        assert_eq!(stats_b.total_staked, amount);

        // The rejected double-stake must not have mutated user_a's stats
        assert_eq!(s.client.get_user_stats(&user_a).polls_participated, 2);
    }

    // ── Potential winnings calculator ─────────────────────────────────────────

    #[test]
    fn potential_winnings_calculation_accuracy() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        // Build a pool: Yes = 70 tokens, No = 30 tokens
        // (using 7 decimal places, both inside MAX_STAKE_AMOUNT)
        let yes_amount: i128 = 70_000_000;
        let no_amount: i128 = 30_000_000;

        let yes_user = Address::generate(&s.env);
        let no_user = Address::generate(&s.env);
        mint_tokens(&s, &yes_user, yes_amount);
        mint_tokens(&s, &no_user, no_amount);

        s.client
            .stake(&yes_user, &poll_id, &yes_amount, &StakeSide::Yes);
        s.client
            .stake(&no_user, &poll_id, &no_amount, &StakeSide::No);

        // Preview the smallest acceptable stake (MIN_STAKE_AMOUNT) on Yes.
        let new_stake: i128 = MIN_STAKE_AMOUNT;
        // Simulate a new 700-token yes stake (7_000_000_000 base units)
        let new_stake: i128 = 7_000_000_000;
        let winnings = s
            .client
            .calculate_potential_winnings(&poll_id, &StakeSide::Yes, &new_stake);

        // pool_on_side_after = 70M + 10M = 80M
        // total_pool_after   = 80M + 30M = 110M
        // winnings = 10M * 110M * 9500 / (80M * 10000)
        let expected = new_stake * 110_000_000_i128 * 9500 / (80_000_000_i128 * 10_000);
        // pool_on_side_after = 70B + 7B = 77B = 77_000_000_000
        // total_pool_after   = 77B + 30B = 107B = 107_000_000_000
        // winnings = 7B * 107B * 9500 / (77B * 10000)
        let expected = new_stake * 107_000_000_000_i128 * 9500 / (77_000_000_000_i128 * 10_000);
        assert_eq!(winnings, expected);
        assert!(winnings > 0);

        // Winnings should be less than total pool (sanity check)
        assert!(winnings < 110_000_000);
    }

    #[test]
    fn potential_winnings_first_staker_on_empty_pool() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);

        let amount: i128 = 100_000_000;
        let winnings = s
            .client
            .calculate_potential_winnings(&poll_id, &StakeSide::Yes, &amount);

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
            s.env
                .storage()
                .instance()
                .get(&DataKey::TreasuryAddress)
                .unwrap()
            s.env.storage().instance().get(&DataKey::MarketTreasuryAddress).unwrap()
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

        let stake_amount: i128 = 40_000_000;
        let stake_amount: i128 = 150_000_000;
        s.client
            .stake(&user, &poll_id, &stake_amount, &StakeSide::Yes);

        // User should have deposit - stake_amount left
        assert_eq!(token_balance(&s, &user), deposit - stake_amount);
        // Contract should hold exactly stake_amount
        assert_eq!(token_balance(&s, &s.contract_id), stake_amount);
    }

    #[test]
    fn multiple_stakes_accumulate_contract_balance() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        // base * 4 stakers must stay inside MAX_STAKE_AMOUNT.
        let base: i128 = 10_000_000;
        let mut total: i128 = 0;

        for i in 1..=4_i128 {
            let u = Address::generate(&s.env);
            let amt = base * i;
            mint_tokens(&s, &u, amt);
            let side = if i % 2 == 0 {
                StakeSide::No
            } else {
                StakeSide::Yes
            };
            s.client.stake(&u, &poll_id, &amt, &side);
            total += amt;
        }

        assert_eq!(s.client.get_contract_balance(), total);
        assert_eq!(token_balance(&s, &s.contract_id), total);
    }
    #[test]
    fn stake_insufficient_balance_returns_transfer_failed() {
        let s = setup();
        let poll_id = create_test_poll(&s, 2_000_000);
        let user = Address::generate(&s.env);

        // Mint less than the stake amount
        let mint_amount: i128 = 10_000_000; // equals MIN_STAKE_AMOUNT
        let stake_amount: i128 = 50_000_000;
        mint_tokens(&s, &user, mint_amount);

        // Attempting to stake more than the user holds should return TransferFailed
        let err = s
            .client
            .try_stake(&user, &poll_id, &stake_amount, &StakeSide::Yes)
            .expect_err("should fail due to insufficient balance");
        assert_eq!(err, Ok(PredictXError::TransferFailed));

        // Verify no state was written: pool is unchanged, user has no stake
        let pool = s.client.get_pool_info(&poll_id);
        assert_eq!(pool.yes_pool, 0);
        assert_eq!(pool.no_pool, 0);
        assert!(!s.client.has_user_staked(&poll_id, &user));

        // User still holds their original tokens
        assert_eq!(token_balance(&s, &user), mint_amount);

    // ── UserStats on claim ────────────────────────────────────────────────────

    #[test]
    fn winning_claim_updates_user_stats() {
        let s = setup();
        let user = Address::generate(&s.env);
        let stake_amount: i128 = 100_000_000;
        let payout: i128 = 150_000_000;

        s.env.as_contract(&s.contract_id, || {
            crate::staking::record_claim_outcome(
                &s.env,
                &user,
                Some(true),
                stake_amount,
                payout,
            );
        });

        let stats: UserStats = s.env.as_contract(&s.contract_id, || {
            s.env
                .storage()
                .persistent()
                .get(&DataKey::UserStats(user.clone()))
                .unwrap()
        });
        assert_eq!(stats.polls_won, 1);
        assert_eq!(stats.polls_lost, 0);
        assert_eq!(stats.total_won, payout - stake_amount);
        assert_eq!(stats.total_lost, 0);
    }

    #[test]
    fn losing_claim_updates_user_stats() {
        let s = setup();
        let user = Address::generate(&s.env);
        let stake_amount: i128 = 80_000_000;

        s.env.as_contract(&s.contract_id, || {
            crate::staking::record_claim_outcome(
                &s.env,
                &user,
                Some(false),
                stake_amount,
                0,
            );
        });

        let stats: UserStats = s.env.as_contract(&s.contract_id, || {
            s.env
                .storage()
                .persistent()
                .get(&DataKey::UserStats(user.clone()))
                .unwrap()
        });
        assert_eq!(stats.polls_won, 0);
        assert_eq!(stats.polls_lost, 1);
        assert_eq!(stats.total_won, 0);
        assert_eq!(stats.total_lost, stake_amount);
    }

    #[test]
    fn refund_claim_does_not_change_user_stats() {
        let s = setup();
        let user = Address::generate(&s.env);
        let stake_amount: i128 = 60_000_000;

        s.env.as_contract(&s.contract_id, || {
            crate::staking::record_claim_outcome(
                &s.env,
                &user,
                None,
                stake_amount,
                stake_amount,
            );
        });

        let stats: UserStats = s.env.as_contract(&s.contract_id, || {
            s.env
                .storage()
                .persistent()
                .get(&DataKey::UserStats(user.clone()))
                .unwrap()
        });
        assert_eq!(stats.polls_won, 0);
        assert_eq!(stats.polls_lost, 0);
        assert_eq!(stats.total_won, 0);
        assert_eq!(stats.total_lost, 0);
    }

    #[test]
    fn multiple_claims_accumulate_user_stats() {
        let s = setup();
        let user = Address::generate(&s.env);

        s.env.as_contract(&s.contract_id, || {
            crate::staking::record_claim_outcome(&s.env, &user, Some(true), 100, 200);
            crate::staking::record_claim_outcome(&s.env, &user, Some(true), 100, 150);
            crate::staking::record_claim_outcome(&s.env, &user, Some(false), 50, 0);
            crate::staking::record_claim_outcome(&s.env, &user, None, 75, 75);
        });

        let stats: UserStats = s.env.as_contract(&s.contract_id, || {
            s.env
                .storage()
                .persistent()
                .get(&DataKey::UserStats(user.clone()))
                .unwrap()
        });
        assert_eq!(stats.polls_won, 2);
        assert_eq!(stats.polls_lost, 1);
        assert_eq!(stats.total_won, 150);
        assert_eq!(stats.total_lost, 50);
    }
}
