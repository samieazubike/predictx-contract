use crate::DataKey;
use predictx_shared::{Poll, PollStatus, PredictXError, Stake, StakeSide};
use soroban_sdk::{token, Address, Env, Symbol, Vec};

const MIN_STAKE_AMOUNT: i128 = 10;
const PLATFORM_FEE_BPS: i128 = 500; // 5%
const BPS_DENOMINATOR: i128 = 10_000;

// ── Internal helpers ──────────────────────────────────────────────────────────

fn get_token(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::TokenAddress)
        .unwrap()
}

fn get_poll(env: &Env, poll_id: u64) -> Result<Poll, PredictXError> {
    env.storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)
}

fn set_poll(env: &Env, poll: &Poll) {
    env.storage()
        .persistent()
        .set(&DataKey::Poll(poll.poll_id), poll);
}

// ── Staking ───────────────────────────────────────────────────────────────────

/// Place a stake on a poll outcome. Transfers tokens from staker to contract.
/// Follows Checks-Effects-Interactions pattern.
pub fn stake(
    env: &Env,
    staker: Address,
    poll_id: u64,
    amount: i128,
    side: StakeSide,
) -> Result<Stake, PredictXError> {
    staker.require_auth();

    // ── Checks ────────────────────────────────────────────────────────────────
    if amount < MIN_STAKE_AMOUNT {
        return Err(PredictXError::StakeAmountZero);
    }

    let mut poll = get_poll(env, poll_id)?;

    if poll.status != PollStatus::Active {
        return Err(PredictXError::PollNotActive);
    }

    let now = env.ledger().timestamp();
    if now >= poll.lock_time {
        return Err(PredictXError::PollLocked);
    }

    let already_staked: bool = env
        .storage()
        .persistent()
        .get(&DataKey::HasStaked(poll_id, staker.clone()))
        .unwrap_or(false);
    if already_staked {
        return Err(PredictXError::AlreadyStaked);
    }

    // ── Effects ───────────────────────────────────────────────────────────────
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
    set_poll(env, &poll);

    let new_stake = Stake {
        user: staker.clone(),
        poll_id,
        amount,
        side: side.clone(),
        claimed: false,
        staked_at: now,
    };
    env.storage()
        .persistent()
        .set(&DataKey::StakeRecord(poll_id, staker.clone()), &new_stake);

    env.storage()
        .persistent()
        .set(&DataKey::HasStaked(poll_id, staker.clone()), &true);

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
    let mut stats = crate::get_platform_stats(env);
    stats.total_value_locked += amount;
    stats.total_stakes_placed += 1;
    crate::set_platform_stats(env, &stats);

    // ── Interactions ──────────────────────────────────────────────────────────
    let token_addr = get_token(env);
    let contract_addr = env.current_contract_address();
    token::Client::new(env, &token_addr).transfer(&staker, &contract_addr, &amount);

    env.events().publish(
        (Symbol::new(env, "StakePlaced"), poll_id, staker.clone()),
        (amount, side),
    );

    Ok(new_stake)
}

// ── View functions ────────────────────────────────────────────────────────────

pub fn get_stake(env: &Env, poll_id: u64, user: Address) -> Result<Stake, PredictXError> {
    env.storage()
        .persistent()
        .get(&DataKey::StakeRecord(poll_id, user))
        .ok_or(PredictXError::NotStaker)
}

pub fn get_user_stakes(env: &Env, user: Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::UserStakes(user))
        .unwrap_or(Vec::new(env))
}

pub fn has_staked(env: &Env, poll_id: u64, user: Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::HasStaked(poll_id, user))
        .unwrap_or(false)
}

/// Read-only winnings preview used by the frontend staking calculator.
/// Formula: (amount / (side_pool + amount)) * total_pool_after * (1 - fee_bps)
pub fn calculate_potential_winnings(
    env: &Env,
    poll_id: u64,
    side: StakeSide,
    amount: i128,
) -> Result<i128, PredictXError> {
    let poll = get_poll(env, poll_id)?;

    let side_pool = match side {
        StakeSide::Yes => poll.yes_pool,
        StakeSide::No => poll.no_pool,
    };

    let total_pool_after = poll.yes_pool + poll.no_pool + amount;
    let side_pool_after = side_pool + amount;

    if side_pool_after == 0 {
        return Ok(0);
    }

    // Proportional share of total pool, then apply (1 - fee) using BPS
    let gross = (amount * total_pool_after) / side_pool_after;
    let net = gross * (BPS_DENOMINATOR - PLATFORM_FEE_BPS) / BPS_DENOMINATOR;

    Ok(net)
}

pub fn get_pool_info(env: &Env, poll_id: u64) -> Result<(i128, i128, u32, u32), PredictXError> {
    let poll = get_poll(env, poll_id)?;
    Ok((poll.yes_pool, poll.no_pool, poll.yes_count, poll.no_count))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use crate::{DataKey, PredictionMarket, PredictionMarketClient};
    use predictx_shared::{Poll, PollCategory, PollStatus, PredictXError, StakeSide};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token::StellarAssetClient,
        Address, Env,
    };

    const LOCK_TIME: u64 = 2_000_000;

    fn setup() -> (Env, Address, PredictionMarketClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();

        // Deploy a real Stellar asset contract for token transfers
        let token_admin = Address::generate(&env);
        let token_addr = env
            .register_stellar_asset_contract_v2(token_admin.clone())
            .address();

        // Register prediction-market
        let cid = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &cid);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);

        client.initialize(&admin, &oracle);
        client.set_token(&token_addr);

        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        (env, admin, client, token_addr)
    }

    fn fund_user(env: &Env, token_addr: &Address, _token_admin: &Address, user: &Address, amount: i128) {
        StellarAssetClient::new(env, token_addr).mint(user, &amount);
    }

    fn make_poll(env: &Env, client: &PredictionMarketClient, admin: &Address, poll_id: u64) {
        let cid = client.address.clone();
        let poll = Poll {
            poll_id,
            match_id: 1,
            creator: admin.clone(),
            question: soroban_sdk::String::from_str(env, "Will Arsenal win?"),
            category: PollCategory::TeamEvent,
            lock_time: LOCK_TIME,
            yes_pool: 0,
            no_pool: 0,
            yes_count: 0,
            no_count: 0,
            status: PollStatus::Active,
            outcome: None,
            resolution_time: 0,
            created_at: env.ledger().timestamp(),
        };
        env.as_contract(&cid, || {
            env.storage()
                .persistent()
                .set(&DataKey::Poll(poll_id), &poll);
        });
    }

    // ── stake: happy paths ────────────────────────────────────────────────────

    #[test]
    fn test_stake_yes_updates_pool() {
        let (env, admin, client, token_addr) = setup();
        let token_admin = Address::generate(&env);
        make_poll(&env, &client, &admin, 1);

        let user = Address::generate(&env);
        fund_user(&env, &token_addr, &token_admin, &user, 1_000);

        let s = client.stake(&user, &1u64, &500, &StakeSide::Yes);
        assert_eq!(s.amount, 500);
        assert_eq!(s.side, StakeSide::Yes);

        let (yes, no, yc, nc) = client.get_pool_info(&1u64);
        assert_eq!(yes, 500);
        assert_eq!(no, 0);
        assert_eq!(yc, 1);
        assert_eq!(nc, 0);
    }

    #[test]
    fn test_stake_no_updates_pool() {
        let (env, admin, client, token_addr) = setup();
        let token_admin = Address::generate(&env);
        make_poll(&env, &client, &admin, 1);

        let user = Address::generate(&env);
        fund_user(&env, &token_addr, &token_admin, &user, 1_000);

        client.stake(&user, &1u64, &300, &StakeSide::No);
        let (yes, no, _, nc) = client.get_pool_info(&1u64);
        assert_eq!(yes, 0);
        assert_eq!(no, 300);
        assert_eq!(nc, 1);
    }

    #[test]
    fn test_multiple_stakers_both_sides() {
        let (env, admin, client, token_addr) = setup();
        let token_admin = Address::generate(&env);
        make_poll(&env, &client, &admin, 1);

        let u1 = Address::generate(&env);
        let u2 = Address::generate(&env);
        fund_user(&env, &token_addr, &token_admin, &u1, 1_000);
        fund_user(&env, &token_addr, &token_admin, &u2, 1_000);

        client.stake(&u1, &1u64, &700, &StakeSide::Yes);
        client.stake(&u2, &1u64, &300, &StakeSide::No);

        let (yes, no, yc, nc) = client.get_pool_info(&1u64);
        assert_eq!(yes, 700);
        assert_eq!(no, 300);
        assert_eq!(yc, 1);
        assert_eq!(nc, 1);
    }

    #[test]
    fn test_user_stakes_history_tracked() {
        let (env, admin, client, token_addr) = setup();
        let token_admin = Address::generate(&env);
        make_poll(&env, &client, &admin, 1);
        make_poll(&env, &client, &admin, 2);

        let user = Address::generate(&env);
        fund_user(&env, &token_addr, &token_admin, &user, 2_000);

        client.stake(&user, &1u64, &100, &StakeSide::Yes);
        client.stake(&user, &2u64, &200, &StakeSide::No);

        let history = client.get_user_stakes(&user);
        assert_eq!(history.len(), 2);
    }

    // ── stake: rejection cases ────────────────────────────────────────────────

    #[test]
    fn test_stake_rejects_zero_amount() {
        let (env, admin, client, _) = setup();
        make_poll(&env, &client, &admin, 1);
        let user = Address::generate(&env);
        let err = client
            .try_stake(&user, &1u64, &0, &StakeSide::Yes)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, PredictXError::StakeAmountZero);
    }

    #[test]
    fn test_stake_rejects_below_minimum() {
        let (env, admin, client, _) = setup();
        make_poll(&env, &client, &admin, 1);
        let user = Address::generate(&env);
        let err = client
            .try_stake(&user, &1u64, &5, &StakeSide::Yes)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, PredictXError::StakeAmountZero);
    }

    #[test]
    fn test_stake_rejects_locked_poll() {
        let (env, admin, client, token_addr) = setup();
        let token_admin = Address::generate(&env);
        make_poll(&env, &client, &admin, 1);
        env.ledger().with_mut(|l| l.timestamp = LOCK_TIME + 1);
        let user = Address::generate(&env);
        fund_user(&env, &token_addr, &token_admin, &user, 1_000);
        let err = client
            .try_stake(&user, &1u64, &100, &StakeSide::Yes)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, PredictXError::PollLocked);
    }

    #[test]
    fn test_stake_rejects_inactive_poll() {
        let (env, admin, client, token_addr) = setup();
        let token_admin = Address::generate(&env);

        // Create a resolved poll
        let cid = client.address.clone();
        let poll = Poll {
            poll_id: 1,
            match_id: 1,
            creator: admin.clone(),
            question: soroban_sdk::String::from_str(&env, "Q?"),
            category: PollCategory::Other,
            lock_time: LOCK_TIME,
            yes_pool: 0,
            no_pool: 0,
            yes_count: 0,
            no_count: 0,
            status: PollStatus::Resolved,
            outcome: Some(true),
            resolution_time: 0,
            created_at: 0,
        };
        env.as_contract(&cid, || {
            env.storage().persistent().set(&DataKey::Poll(1u64), &poll);
        });

        let user = Address::generate(&env);
        fund_user(&env, &token_addr, &token_admin, &user, 1_000);
        let err = client
            .try_stake(&user, &1u64, &100, &StakeSide::Yes)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, PredictXError::PollNotActive);
    }

    #[test]
    fn test_stake_rejects_double_stake() {
        let (env, admin, client, token_addr) = setup();
        let token_admin = Address::generate(&env);
        make_poll(&env, &client, &admin, 1);
        let user = Address::generate(&env);
        fund_user(&env, &token_addr, &token_admin, &user, 2_000);
        client.stake(&user, &1u64, &100, &StakeSide::Yes);
        let err = client
            .try_stake(&user, &1u64, &100, &StakeSide::No)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, PredictXError::AlreadyStaked);
    }

    #[test]
    fn test_stake_nonexistent_poll() {
        let (env, _, client, _) = setup();
        let user = Address::generate(&env);
        let err = client
            .try_stake(&user, &999u64, &100, &StakeSide::Yes)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, PredictXError::PollNotFound);
    }

    // ── has_staked ────────────────────────────────────────────────────────────

    #[test]
    fn test_has_staked_returns_correct_value() {
        let (env, admin, client, token_addr) = setup();
        let token_admin = Address::generate(&env);
        make_poll(&env, &client, &admin, 1);
        let user = Address::generate(&env);
        fund_user(&env, &token_addr, &token_admin, &user, 1_000);

        assert!(!client.has_staked(&1u64, &user));
        client.stake(&user, &1u64, &100, &StakeSide::Yes);
        assert!(client.has_staked(&1u64, &user));
    }

    // ── calculate_potential_winnings ──────────────────────────────────────────

    #[test]
    fn test_potential_winnings_accurate() {
        let (env, admin, client, token_addr) = setup();
        let token_admin = Address::generate(&env);
        make_poll(&env, &client, &admin, 1);

        let u1 = Address::generate(&env);
        fund_user(&env, &token_addr, &token_admin, &u1, 10_000);
        client.stake(&u1, &1u64, &7_000, &StakeSide::Yes);

        let u2 = Address::generate(&env);
        fund_user(&env, &token_addr, &token_admin, &u2, 10_000);
        client.stake(&u2, &1u64, &3_000, &StakeSide::No);

        // New user considering staking 700 on Yes
        // yes_pool=7000, no_pool=3000, total=10000
        // after stake: side_pool_after=7700, total_after=10700
        // gross = 700 * 10700 / 7700 = 972 (integer div)
        // net   = 972 * 9500 / 10000 = 923
        let winnings = client.calculate_potential_winnings(&1u64, &StakeSide::Yes, &700);
        assert_eq!(winnings, 923);
    }

    #[test]
    fn test_potential_winnings_empty_pool() {
        let (env, admin, client, _) = setup();
        make_poll(&env, &client, &admin, 1);
        // First staker — no existing pool
        // side_pool_after = 0 + 500 = 500, total_after = 500
        // gross = 500 * 500 / 500 = 500, net = 500 * 9500 / 10000 = 475
        let winnings = client.calculate_potential_winnings(&1u64, &StakeSide::Yes, &500);
        assert_eq!(winnings, 475);
    }
}
