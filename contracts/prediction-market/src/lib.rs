#![no_std]
// `update_match` mirrors the Match fields as optional arguments; the lint also
// fires on the client code `#[contractimpl]` generates for it.
#![allow(clippy::too_many_arguments)]

mod matches;
mod payouts;
mod staking;
pub(crate) mod token_utils;

use predictx_shared::{
    Match, PlatformStats, Poll, PollCategory, PollStatus, PredictXError, Stake, StakeSide,
    BPS_DENOMINATOR, EMERGENCY_TIMEOUT_SECS, MAX_POLLS_PER_MATCH, MAX_QUESTION_LENGTH,
};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Symbol, Vec};

mod voting_oracle {
    soroban_sdk::contractimport!(file = "wasm/voting_oracle.wasm");
}

fn map_oracle_poll_status(status: voting_oracle::PollStatus) -> PollStatus {
    match status {
        voting_oracle::PollStatus::Active      => PollStatus::Active,
        voting_oracle::PollStatus::Locked      => PollStatus::Locked,
        voting_oracle::PollStatus::Voting      => PollStatus::Voting,
        voting_oracle::PollStatus::AdminReview => PollStatus::AdminReview,
        voting_oracle::PollStatus::Disputed    => PollStatus::Disputed,
        voting_oracle::PollStatus::Resolved    => PollStatus::Resolved,
        voting_oracle::PollStatus::Cancelled   => PollStatus::Cancelled,
    }
}

#[contract]
pub struct PredictionMarket;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    // ── oracle / admin keys ───────────────────────────────────────────────────
    Admin,
    VotingOracle,
    Paused,
    TokenAddress,
    TreasuryAddress,
    PlatformFeeBps,
    Stake(u64, Address),
    EmergencyClaimed(u64, Address),
    PlatformStats,
    // ── match management keys ─────────────────────────────────────────────────
    Initialized,
    NextMatchId,
    NextPollId,
    Match(u64),
    MatchPolls(u64),
    // ── poll & staking keys ───────────────────────────────────────────────────
    Poll(u64),
    UserStakes(Address),
    HasStaked(u64, Address),
}

/// Pool state returned by `get_pool_info`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolInfo {
    pub yes_pool: i128,
    pub no_pool: i128,
    pub yes_count: u32,
    pub no_count: u32,
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage().instance().get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)
}

fn get_oracle(env: &Env) -> Result<Address, PredictXError> {
    env.storage().instance().get(&DataKey::VotingOracle)
        .ok_or(PredictXError::NotInitialized)
}

fn is_paused(env: &Env) -> bool {
    env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
}

pub(crate) fn ensure_not_paused(env: &Env) -> Result<(), PredictXError> {
    if is_paused(env) {
        return Err(PredictXError::ContractPaused);
    }
    Ok(())
}

pub(crate) fn get_platform_stats(env: &Env) -> PlatformStats {
    env.storage().instance().get(&DataKey::PlatformStats)
        .unwrap_or(PlatformStats {
            total_value_locked: 0,
            total_polls_created: 0,
            total_stakes_placed: 0,
            total_payouts: 0,
            total_users: 0,
        })
}

pub(crate) fn set_platform_stats(env: &Env, stats: &PlatformStats) {
    env.storage().instance().set(&DataKey::PlatformStats, stats);
}

fn load_stake(env: &Env, poll_id: u64, user: &Address) -> Option<Stake> {
    env.storage().persistent().get(&DataKey::Stake(poll_id, user.clone()))
}

fn has_emergency_claimed(env: &Env, poll_id: u64, user: &Address) -> bool {
    env.storage().persistent()
        .get(&DataKey::EmergencyClaimed(poll_id, user.clone()))
        .unwrap_or(false)
}

fn set_emergency_claimed(env: &Env, poll_id: u64, user: &Address) {
    env.storage().persistent()
        .set(&DataKey::EmergencyClaimed(poll_id, user.clone()), &true);
}

fn load_poll(env: &Env, poll_id: u64) -> Option<Poll> {
    env.storage().persistent().get(&DataKey::Poll(poll_id))
}

/// Whether stakes on `poll_id` may be refunded through `emergency_withdraw`.
///
/// The market's own poll record is authoritative: a poll the market has
/// resolved is settled through `claim_winnings` only (otherwise winners could
/// claim *and* refund, and losers could refund a lost stake), and a poll the
/// market has cancelled is always refundable. Otherwise the oracle status is
/// consulted: `Cancelled`, or `Disputed`/`Locked` for longer than
/// `EMERGENCY_TIMEOUT_SECS`.
fn emergency_eligible(env: &Env, poll_id: u64) -> Result<bool, PredictXError> {
    if let Some(poll) = load_poll(env, poll_id) {
        match poll.status {
            PollStatus::Resolved => return Ok(false),
            PollStatus::Cancelled => return Ok(true),
            _ => {}
        }
    }
    let client = voting_oracle::Client::new(env, &get_oracle(env)?);
    let eligible = match map_oracle_poll_status(client.get_poll_status(&poll_id)) {
        PollStatus::Cancelled => true,
        PollStatus::Disputed | PollStatus::Locked => {
            let updated_at = client.get_poll_status_updated_at(&poll_id);
            updated_at != 0
                && env.ledger().timestamp().saturating_sub(updated_at) >= EMERGENCY_TIMEOUT_SECS
        }
        _ => false,
    };
    Ok(eligible)
}

/// Record a final outcome on a market poll. Shared by the oracle-gated
/// `resolve_poll` and the permissionless `sync_resolution`.
fn apply_resolution(env: &Env, poll_id: u64, outcome: bool) -> Result<(), PredictXError> {
    let mut poll = load_poll(env, poll_id).ok_or(PredictXError::PollNotFound)?;

    if poll.status == PollStatus::Resolved || poll.outcome.is_some() {
        return Err(PredictXError::PollAlreadyResolved);
    }
    if poll.status == PollStatus::Cancelled {
        return Err(PredictXError::PollNotActive);
    }

    poll.outcome = Some(outcome);
    poll.resolution_time = env.ledger().timestamp();
    poll.status = PollStatus::Resolved;

    env.storage()
        .persistent()
        .set(&DataKey::Poll(poll_id), &poll);

    let total_pool = poll.yes_pool + poll.no_pool;
    let fee = total_pool * token_utils::get_platform_fee_bps(env) as i128
        / BPS_DENOMINATOR as i128;
    env.events().publish(
        (Symbol::new(env, "PollResolved"), poll_id),
        (outcome, total_pool, fee),
    );
    Ok(())
}


#[contractimpl]
impl PredictionMarket {
    pub fn initialize(
        env: Env,
        admin: Address,
        voting_oracle: Address,
        token_address: Address,
        treasury_address: Address,
        platform_fee_bps: u32,
    ) -> Result<(), PredictXError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::AlreadyInitialized);
        }
        admin.require_auth();
        if platform_fee_bps > BPS_DENOMINATOR {
            return Err(PredictXError::InvalidFeeBps);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::VotingOracle, &voting_oracle);
        env.storage().instance().set(&DataKey::TokenAddress, &token_address);
        env.storage().instance().set(&DataKey::TreasuryAddress, &treasury_address);
        env.storage().instance().set(&DataKey::PlatformFeeBps, &platform_fee_bps);
        env.storage().instance().set(&DataKey::NextMatchId, &1u64);
        env.storage().instance().set(&DataKey::NextPollId, &1u64);
        env.storage().instance().set(&DataKey::Initialized, &true);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, PredictXError> { get_admin(&env) }
    pub fn oracle(env: Env) -> Result<Address, PredictXError> { get_oracle(&env) }

    pub fn set_oracle(env: Env, voting_oracle: Address) -> Result<(), PredictXError> {
        ensure_not_paused(&env)?;
        let admin = get_admin(&env)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::VotingOracle, &voting_oracle);
        Ok(())
    }

    pub fn pause(env: Env, admin: Address) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin { return Err(PredictXError::Unauthorized); }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((Symbol::new(&env, "ContractPaused"),), true);
        Ok(())
    }

    pub fn unpause(env: Env, admin: Address) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin { return Err(PredictXError::Unauthorized); }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events().publish((Symbol::new(&env, "ContractUnpaused"),), true);
        Ok(())
    }

    pub fn is_paused(env: Env) -> bool { is_paused(&env) }

    pub fn oracle_poll_status(env: Env, poll_id: u64) -> Result<PollStatus, PredictXError> {
        let oracle_id = get_oracle(&env)?;
        let client = voting_oracle::Client::new(&env, &oracle_id);
        Ok(map_oracle_poll_status(client.get_poll_status(&poll_id)))
    }

    /// Cancel a poll so every staker can recover their stake.
    ///
    /// Deliberately allowed while paused: cancelling is the emergency remedy.
    pub fn cancel_poll(env: Env, admin: Address, poll_id: u64) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin { return Err(PredictXError::Unauthorized); }
        admin.require_auth();

        let mut poll = load_poll(&env, poll_id).ok_or(PredictXError::PollNotFound)?;
        if poll.status == PollStatus::Resolved {
            return Err(PredictXError::PollAlreadyResolved);
        }
        poll.status = PollStatus::Cancelled;
        env.storage().persistent().set(&DataKey::Poll(poll_id), &poll);

        let oracle_id = get_oracle(&env)?;
        let client = voting_oracle::Client::new(&env, &oracle_id);
        client.set_poll_status(&poll_id, &voting_oracle::PollStatus::Cancelled);
        env.events().publish((Symbol::new(&env, "PollCancelled"),), poll_id);
        Ok(())
    }

    pub fn check_emergency_eligible(env: Env, poll_id: u64) -> bool {
        emergency_eligible(&env, poll_id).unwrap_or(false)
    }

    /// Refund a stake on a cancelled or stalled poll.
    ///
    /// Deliberately allowed while paused so users always have an exit.
    pub fn emergency_withdraw(env: Env, user: Address, poll_id: u64) -> Result<i128, PredictXError> {
        user.require_auth();
        if has_emergency_claimed(&env, poll_id, &user) {
            return Err(PredictXError::AlreadyClaimed);
        }
        if !emergency_eligible(&env, poll_id)? {
            return Err(PredictXError::EmergencyWithdrawNotAllowed);
        }
        let mut stake = load_stake(&env, poll_id, &user).ok_or(PredictXError::NotStaker)?;
        if stake.claimed {
            return Err(PredictXError::AlreadyClaimed);
        }

        // Effects before the token transfer.
        set_emergency_claimed(&env, poll_id, &user);
        stake.claimed = true;
        env.storage()
            .persistent()
            .set(&DataKey::Stake(poll_id, user.clone()), &stake);

        token_utils::transfer_from_contract(&env, &user, stake.amount)?;

        let mut stats = get_platform_stats(&env);
        stats.total_value_locked = stats.total_value_locked.saturating_sub(stake.amount);
        set_platform_stats(&env, &stats);
        env.events().publish((Symbol::new(&env, "EmergencyWithdrawal"), poll_id, user.clone()), stake.amount);
        Ok(stake.amount)
    }

    // ── Poll management ──────────────────────────────────────────────────────

    pub fn create_poll(
        env: Env,
        creator: Address,
        match_id: u64,
        question: String,
        category: PollCategory,
        lock_time: u64,
    ) -> Result<u64, PredictXError> {
        ensure_not_paused(&env)?;
        creator.require_auth();

        // Validate match exists and has not finished: a poll on a finished
        // match has a publicly known outcome.
        let m: Match = env
            .storage()
            .persistent()
            .get(&DataKey::Match(match_id))
            .ok_or(PredictXError::MatchNotFound)?;
        if m.is_finished {
            return Err(PredictXError::MatchAlreadyStarted);
        }

        if question.is_empty() || question.len() > MAX_QUESTION_LENGTH {
            return Err(PredictXError::PollQuestionTooLong);
        }

        // Validate lock_time is in the future
        if lock_time <= env.ledger().timestamp() {
            return Err(PredictXError::InvalidLockTime);
        }

        // Check max polls per match
        let mut match_polls: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::MatchPolls(match_id))
            .unwrap_or(Vec::new(&env));
        if match_polls.len() >= MAX_POLLS_PER_MATCH {
            return Err(PredictXError::MaxPollsPerMatchReached);
        }

        let poll_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextPollId)
            .unwrap_or(1);

        let poll = Poll {
            poll_id,
            match_id,
            creator: creator.clone(),
            question,
            category,
            lock_time,
            yes_pool: 0,
            no_pool: 0,
            yes_count: 0,
            no_count: 0,
            status: PollStatus::Active,
            outcome: None,
            resolution_time: 0,
            created_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Poll(poll_id), &poll);

        match_polls.push_back(poll_id);
        env.storage()
            .persistent()
            .set(&DataKey::MatchPolls(match_id), &match_polls);

        env.storage()
            .instance()
            .set(&DataKey::NextPollId, &(poll_id + 1));

        let mut stats = get_platform_stats(&env);
        stats.total_polls_created += 1;
        set_platform_stats(&env, &stats);

        env.events()
            .publish((Symbol::new(&env, "PollCreated"), poll_id), ());

        Ok(poll_id)
    }


    /// Resolve a poll with a boolean outcome. Callable only by the registered oracle.
    pub fn resolve_poll(
        env: Env,
        caller: Address,
        poll_id: u64,
        outcome: bool,
    ) -> Result<(), PredictXError> {
        ensure_not_paused(&env)?;
        caller.require_auth();
        let oracle = get_oracle(&env)?;
        if caller != oracle {
            return Err(PredictXError::Unauthorized);
        }
        apply_resolution(&env, poll_id, outcome)
    }

    /// Permissionlessly copy a final outcome from the voting oracle.
    ///
    /// The oracle contract never calls back into the market, so without this
    /// a poll resolved by community vote could never open for claims. Only a
    /// decisive (`Yes`/`No`) outcome on a poll the oracle marks `Resolved` is
    /// accepted.
    pub fn sync_resolution(env: Env, poll_id: u64) -> Result<bool, PredictXError> {
        ensure_not_paused(&env)?;
        let client = voting_oracle::Client::new(&env, &get_oracle(&env)?);
        if map_oracle_poll_status(client.get_poll_status(&poll_id)) != PollStatus::Resolved {
            return Err(PredictXError::OutcomeNotAvailable);
        }
        let outcome = match client.try_get_poll_outcome(&poll_id) {
            Ok(Ok(voting_oracle::VoteChoice::Yes)) => true,
            Ok(Ok(voting_oracle::VoteChoice::No)) => false,
            _ => return Err(PredictXError::OutcomeNotAvailable),
        };
        apply_resolution(&env, poll_id, outcome)?;
        Ok(outcome)
    }

    pub fn get_poll(env: Env, poll_id: u64) -> Result<Poll, PredictXError> {
        env.storage()
            .persistent()
            .get(&DataKey::Poll(poll_id))
            .ok_or(PredictXError::PollNotFound)
    }

    // ── Staking ───────────────────────────────────────────────────────────────

    pub fn stake(
        env: Env,
        staker: Address,
        poll_id: u64,
        amount: i128,
        side: StakeSide,
    ) -> Result<Stake, PredictXError> {
        staking::stake(&env, staker, poll_id, amount, side)
    }

    pub fn get_stake_info(env: Env, poll_id: u64, user: Address) -> Result<Stake, PredictXError> {
        staking::get_stake_info(&env, poll_id, &user)
    }

    pub fn get_user_stakes(env: Env, user: Address) -> Vec<u64> {
        staking::get_user_stakes(&env, &user)
    }

    pub fn has_user_staked(env: Env, poll_id: u64, user: Address) -> bool {
        staking::has_user_staked(&env, poll_id, &user)
    }

    pub fn calculate_potential_winnings(
        env: Env,
        poll_id: u64,
        side: StakeSide,
        amount: i128,
    ) -> Result<i128, PredictXError> {
        staking::calculate_potential_winnings(&env, poll_id, side, amount)
    }

    pub fn get_pool_info(env: Env, poll_id: u64) -> Result<PoolInfo, PredictXError> {
        staking::get_pool_info(&env, poll_id)
    }

    pub fn get_platform_stats(env: Env) -> PlatformStats {
        get_platform_stats(&env)
    }

    // ── Token view functions ──────────────────────────────────────────────────

    pub fn get_token_address(env: Env) -> Result<Address, PredictXError> {
        token_utils::get_token_address(&env)
    }

    pub fn get_treasury_address(env: Env) -> Result<Address, PredictXError> {
        token_utils::get_treasury_address(&env)
    }

    pub fn get_platform_fee_bps(env: Env) -> u32 {
        token_utils::get_platform_fee_bps(&env)
    }

    pub fn get_contract_balance(env: Env) -> Result<i128, PredictXError> {
        token_utils::get_balance(&env)
    }

    // ── Match management ──────────────────────────────────────────────────────

    pub fn create_match(
        env: Env, admin: Address,
        home_team: String, away_team: String,
        league: String, venue: String,
        kickoff_time: u64,
    ) -> Result<u64, PredictXError> {
        matches::create_match(&env, admin, home_team, away_team, league, venue, kickoff_time)
    }

    pub fn update_match(
        env: Env, admin: Address, match_id: u64,
        home_team: Option<String>, away_team: Option<String>,
        league: Option<String>, venue: Option<String>,
        kickoff_time: Option<u64>,
    ) -> Result<Match, PredictXError> {
        matches::update_match(&env, admin, match_id, home_team, away_team, league, venue, kickoff_time)
    }

    pub fn finish_match(env: Env, admin: Address, match_id: u64) -> Result<(), PredictXError> {
        matches::finish_match(&env, admin, match_id)
    }

    pub fn get_match(env: Env, match_id: u64) -> Result<Match, PredictXError> {
        matches::get_match(&env, match_id)
    }

    pub fn get_match_polls(env: Env, match_id: u64) -> Result<Vec<u64>, PredictXError> {
        matches::get_match_polls(&env, match_id)
    }

    pub fn get_match_count(env: Env) -> u64 {
        matches::get_match_count(&env)
    }

    // ── Payouts ───────────────────────────────────────────────────────────────

    /// Claim winnings after a resolved poll.
    ///
    /// If the winning pool is empty (every staker was on the losing side),
    /// any staker may recover their original stake fee-free.  See
    /// [`payouts::claim_winnings`] for the full description.
    pub fn claim_winnings(
        env: Env,
        claimant: Address,
        poll_id: u64,
    ) -> Result<i128, PredictXError> {
        ensure_not_paused(&env)?;
        payouts::claim_winnings(&env, claimant, poll_id)
    }

    pub fn calculate_winnings(
        env: Env,
        poll_id: u64,
        user: Address,
    ) -> Result<i128, PredictXError> {
        payouts::calculate_winnings(&env, poll_id, user)
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test {
    use super::*;
    use predictx_shared::{PollCategory, StakeSide};
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::token;

    /// Default platform fee BPS for tests (5%).
    const TEST_FEE_BPS: u32 = 500;

    #[test]
    fn initialize_sets_admin_and_oracle() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle, &token, &treasury, &TEST_FEE_BPS);
        assert_eq!(client.admin(), admin);
        assert_eq!(client.oracle(), oracle);
    }

    #[test]
    fn initialize_stores_token_and_treasury() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let tok = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle, &tok, &treasury, &TEST_FEE_BPS);
        assert_eq!(client.get_token_address(), tok);
        assert_eq!(client.get_treasury_address(), treasury);
        assert_eq!(client.get_platform_fee_bps(), TEST_FEE_BPS);
    }

    #[test]
    fn initialize_is_one_time_only() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle, &token, &treasury, &TEST_FEE_BPS);
        let err = client.try_initialize(&admin, &oracle, &token, &treasury, &TEST_FEE_BPS).expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::AlreadyInitialized));
    }

    #[test]
    fn cross_contract_oracle_call_works() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);
        oracle_client.set_poll_status(&7_u64, &voting_oracle::PollStatus::Resolved);
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let tok = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle_id, &tok, &treasury, &TEST_FEE_BPS);
        let status = client.oracle_poll_status(&7_u64);
        assert_eq!(status, PollStatus::Resolved);
    }

    #[test]
    fn pause_and_unpause_toggle_contract_state() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let tok = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle, &tok, &treasury, &TEST_FEE_BPS);
        assert!(!client.is_paused());
        client.pause(&admin);
        assert!(client.is_paused());
        let err = client.try_set_oracle(&oracle).expect_err("should be blocked");
        assert_eq!(err, Ok(PredictXError::ContractPaused));
        client.unpause(&admin);
        assert!(!client.is_paused());
    }

    #[test]
    fn cancel_poll_sets_cancelled_status_and_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let tok = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle_id, &tok, &treasury, &TEST_FEE_BPS);
        seed_active_poll(&env, &contract_id, 1, &admin);
        client.cancel_poll(&admin, &1_u64);
        assert_eq!(oracle_client.get_poll_status(&1_u64), voting_oracle::PollStatus::Cancelled);
        assert_eq!(client.get_poll(&1_u64).status, PollStatus::Cancelled);
    }

    // Helper to set up a real-token environment for emergency withdrawal tests
    fn setup_emergency_env() -> (Env, Address, Address, Address, PredictionMarketClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);

        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        // Real token for transfers
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_addr = token_contract.address();
        let treasury = Address::generate(&env);

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        client.initialize(&admin, &oracle_id, &token_addr, &treasury, &TEST_FEE_BPS);

        (env, admin, oracle_id, contract_id, client)
    }

    /// Mint tokens to an address using the real SAC
    fn mint_to(env: &Env, token_addr: &Address, to: &Address, amount: i128) {
        let sac = token::StellarAssetClient::new(env, token_addr);
        sac.mint(to, &amount);
    }

    #[test]
    fn emergency_withdraw_on_cancelled_poll_refunds_stake() {
        let (env, admin, _oracle_id, contract_id, client) = setup_emergency_env();
        let token_addr: Address = env.as_contract(&contract_id, || {
            env.storage().instance().get(&DataKey::TokenAddress).unwrap()
        });

        let user = Address::generate(&env);
        let amount: i128 = 50;
        seed_active_poll(&env, &contract_id, 10, &admin);

        // Fund the contract so it can transfer back
        mint_to(&env, &token_addr, &contract_id, amount);

        let stake = Stake { user: user.clone(), poll_id: 10, amount, side: StakeSide::Yes, claimed: false, staked_at: env.ledger().timestamp() };
        env.as_contract(&contract_id, || {
            env.storage().persistent().set(&DataKey::Stake(10, user.clone()), &stake);
        });
        client.cancel_poll(&admin, &10_u64);
        let refunded = client.emergency_withdraw(&user, &10_u64);
        assert_eq!(refunded, amount);

        // Verify token was transferred
        let tok = token::Client::new(&env, &token_addr);
        assert_eq!(tok.balance(&user), amount);
        assert_eq!(tok.balance(&contract_id), 0);
    }

    #[test]
    fn emergency_withdraw_after_dispute_timeout() {
        let (env, _admin, oracle_id, contract_id, client) = setup_emergency_env();
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        let token_addr: Address = env.as_contract(&contract_id, || {
            env.storage().instance().get(&DataKey::TokenAddress).unwrap()
        });

        env.ledger().set_timestamp(100);
        oracle_client.set_poll_status(&5_u64, &voting_oracle::PollStatus::Disputed);

        let user = Address::generate(&env);
        let amount: i128 = 25;
        mint_to(&env, &token_addr, &contract_id, amount);

        let stake = Stake { user: user.clone(), poll_id: 5, amount, side: StakeSide::No, claimed: false, staked_at: env.ledger().timestamp() };
        env.as_contract(&contract_id, || {
            env.storage().persistent().set(&DataKey::Stake(5, user.clone()), &stake);
        });
        env.ledger().set_timestamp(100 + EMERGENCY_TIMEOUT_SECS + 1);
        assert!(client.check_emergency_eligible(&5_u64));
        let refunded = client.emergency_withdraw(&user, &5_u64);
        assert_eq!(refunded, amount);
    }

    #[test]
    fn emergency_withdraw_rejected_before_timeout() {
        let (env, _admin, oracle_id, contract_id, client) = setup_emergency_env();
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);

        env.ledger().set_timestamp(200);
        oracle_client.set_poll_status(&2_u64, &voting_oracle::PollStatus::Locked);

        let user = Address::generate(&env);
        let stake = Stake { user: user.clone(), poll_id: 2, amount: 30, side: StakeSide::Yes, claimed: false, staked_at: env.ledger().timestamp() };
        env.as_contract(&contract_id, || {
            env.storage().persistent().set(&DataKey::Stake(2, user.clone()), &stake);
        });
        env.ledger().set_timestamp(200 + EMERGENCY_TIMEOUT_SECS - 1);
        assert!(!client.check_emergency_eligible(&2_u64));
        let err = client.try_emergency_withdraw(&user, &2_u64).expect_err("should reject");
        assert_eq!(err, Ok(PredictXError::EmergencyWithdrawNotAllowed));
    }

    #[test]
    fn emergency_withdraw_prevents_double_withdrawal() {
        let (env, _admin, oracle_id, contract_id, client) = setup_emergency_env();
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        let token_addr: Address = env.as_contract(&contract_id, || {
            env.storage().instance().get(&DataKey::TokenAddress).unwrap()
        });

        env.ledger().set_timestamp(300);
        oracle_client.set_poll_status(&3_u64, &voting_oracle::PollStatus::Disputed);

        let user = Address::generate(&env);
        let amount: i128 = 40;
        mint_to(&env, &token_addr, &contract_id, amount);

        let stake = Stake { user: user.clone(), poll_id: 3, amount, side: StakeSide::No, claimed: false, staked_at: env.ledger().timestamp() };
        env.as_contract(&contract_id, || {
            env.storage().persistent().set(&DataKey::Stake(3, user.clone()), &stake);
        });
        env.ledger().set_timestamp(300 + EMERGENCY_TIMEOUT_SECS + 1);
        let refunded = client.emergency_withdraw(&user, &3_u64);
        assert_eq!(refunded, amount);
        let err = client.try_emergency_withdraw(&user, &3_u64).expect_err("double withdrawal should fail");
        assert_eq!(err, Ok(PredictXError::AlreadyClaimed));
    }

    fn seed_active_poll(env: &Env, contract_id: &Address, poll_id: u64, creator: &Address) {
        let poll = Poll {
            poll_id,
            match_id: 1,
            creator: creator.clone(),
            question: String::from_str(env, "Will the home team win?"),
            category: PollCategory::TeamEvent,
            lock_time: env.ledger().timestamp() + 3600,
            yes_pool: 0,
            no_pool: 0,
            yes_count: 0,
            no_count: 0,
            status: PollStatus::Active,
            outcome: None,
            resolution_time: 0,
            created_at: env.ledger().timestamp(),
        };
        env.as_contract(contract_id, || {
            env.storage().persistent().set(&DataKey::Poll(poll_id), &poll);
        });
    }

    #[test]
    fn resolve_poll_rejects_non_oracle() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let stranger = Address::generate(&env);
        let tok = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle, &tok, &treasury, &TEST_FEE_BPS);
        seed_active_poll(&env, &contract_id, 1, &admin);
        let err = client.try_resolve_poll(&stranger, &1_u64, &true).expect_err("non-oracle");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
    }

    #[test]
    fn resolve_poll_rejects_unknown_poll() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let tok = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle, &tok, &treasury, &TEST_FEE_BPS);
        let err = client.try_resolve_poll(&oracle, &99_u64, &false).expect_err("missing");
        assert_eq!(err, Ok(PredictXError::PollNotFound));
    }

    #[test]
    fn resolve_poll_sets_outcome_and_resolution_time() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(1_700_000_000);
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let tok = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle, &tok, &treasury, &TEST_FEE_BPS);
        seed_active_poll(&env, &contract_id, 7, &admin);
        client.resolve_poll(&oracle, &7_u64, &true);
        let poll = client.get_poll(&7_u64);
        assert_eq!(poll.outcome, Some(true));
        assert_eq!(poll.resolution_time, 1_700_000_000);
        assert_eq!(poll.status, PollStatus::Resolved);
    }

    #[test]
    fn resolve_poll_rejects_already_resolved() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        let tok = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle, &tok, &treasury, &TEST_FEE_BPS);
        seed_active_poll(&env, &contract_id, 3, &admin);
        client.resolve_poll(&oracle, &3_u64, &false);
        let err = client.try_resolve_poll(&oracle, &3_u64, &true).expect_err("already");
        assert_eq!(err, Ok(PredictXError::PollAlreadyResolved));
    }


    // ── Security regressions ─────────────────────────────────────────────────

    struct Sec {
        env: Env,
        admin: Address,
        oracle_id: Address,
        token_addr: Address,
        contract_id: Address,
        client: PredictionMarketClient<'static>,
    }

    const LOCK: u64 = 2_000_000;

    fn sec_setup() -> Sec {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let oracle_id = env.register(voting_oracle::WASM, ());
        voting_oracle::Client::new(&env, &oracle_id).initialize(&admin);
        let token_addr = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle_id, &token_addr, &treasury, &TEST_FEE_BPS);
        env.ledger().set_timestamp(1_000_000);
        Sec { env, admin, oracle_id, token_addr, contract_id, client }
    }

    fn sec_poll(s: &Sec) -> u64 {
        let match_id = s.client.create_match(
            &s.admin,
            &String::from_str(&s.env, "Arsenal"),
            &String::from_str(&s.env, "Chelsea"),
            &String::from_str(&s.env, "Premier League"),
            &String::from_str(&s.env, "Emirates"),
            &(LOCK + 3_600),
        );
        s.client.create_poll(
            &s.admin,
            &match_id,
            &String::from_str(&s.env, "Will Palmer score?"),
            &PollCategory::PlayerEvent,
            &LOCK,
        )
    }

    fn sec_stake(s: &Sec, poll_id: u64, side: StakeSide, amount: i128) -> Address {
        let user = Address::generate(&s.env);
        mint_to(&s.env, &s.token_addr, &user, amount);
        s.client.stake(&user, &poll_id, &amount, &side);
        user
    }

    fn balance(s: &Sec, who: &Address) -> i128 {
        token::Client::new(&s.env, &s.token_addr).balance(who)
    }

    const AMT: i128 = 100_000_000;

    #[test]
    fn resolved_poll_cannot_also_be_emergency_withdrawn() {
        // Oracle left at Locked while the market resolves; after the timeout
        // the old code let winners claim *and* refund, and losers refund.
        let s = sec_setup();
        let oracle = voting_oracle::Client::new(&s.env, &s.oracle_id);
        let poll_id = sec_poll(&s);
        let winner = sec_stake(&s, poll_id, StakeSide::Yes, AMT);
        let loser = sec_stake(&s, poll_id, StakeSide::No, AMT);
        oracle.set_poll_status(&poll_id, &voting_oracle::PollStatus::Locked);

        s.client.resolve_poll(&s.oracle_id, &poll_id, &true);
        s.env.ledger().set_timestamp(1_000_000 + EMERGENCY_TIMEOUT_SECS + 1);
        assert!(!s.client.check_emergency_eligible(&poll_id));

        let payout = s.client.claim_winnings(&winner, &poll_id);
        assert_eq!(payout, 190_000_000);
        for user in [&winner, &loser] {
            let err = s.client.try_emergency_withdraw(user, &poll_id).unwrap_err();
            assert_eq!(err, Ok(PredictXError::EmergencyWithdrawNotAllowed));
        }
        assert_eq!(balance(&s, &s.contract_id), 0);
    }

    #[test]
    fn emergency_refund_marks_stake_claimed() {
        let s = sec_setup();
        let poll_id = sec_poll(&s);
        let user = sec_stake(&s, poll_id, StakeSide::Yes, AMT);
        s.client.cancel_poll(&s.admin, &poll_id);
        assert_eq!(s.client.emergency_withdraw(&user, &poll_id), AMT);
        assert!(s.client.get_stake_info(&poll_id, &user).claimed);
        let err = s.client.try_emergency_withdraw(&user, &poll_id).unwrap_err();
        assert_eq!(err, Ok(PredictXError::AlreadyClaimed));
    }

    #[test]
    fn cancelled_poll_rejects_stakes_and_resolution() {
        let s = sec_setup();
        let poll_id = sec_poll(&s);
        s.client.cancel_poll(&s.admin, &poll_id);

        let user = Address::generate(&s.env);
        mint_to(&s.env, &s.token_addr, &user, AMT);
        let err = s.client.try_stake(&user, &poll_id, &AMT, &StakeSide::Yes).unwrap_err();
        assert_eq!(err, Ok(PredictXError::PollNotActive));

        let err = s.client.try_resolve_poll(&s.oracle_id, &poll_id, &true).unwrap_err();
        assert_eq!(err, Ok(PredictXError::PollNotActive));
    }

    #[test]
    fn cancel_rejects_resolved_and_unknown_polls() {
        let s = sec_setup();
        let poll_id = sec_poll(&s);
        s.client.resolve_poll(&s.oracle_id, &poll_id, &false);
        let err = s.client.try_cancel_poll(&s.admin, &poll_id).unwrap_err();
        assert_eq!(err, Ok(PredictXError::PollAlreadyResolved));
        let err = s.client.try_cancel_poll(&s.admin, &999_u64).unwrap_err();
        assert_eq!(err, Ok(PredictXError::PollNotFound));
    }

    #[test]
    fn stake_cancel_refund_returns_every_token() {
        let s = sec_setup();
        let poll_id = sec_poll(&s);
        let users = [
            sec_stake(&s, poll_id, StakeSide::Yes, AMT),
            sec_stake(&s, poll_id, StakeSide::No, 2 * AMT),
            sec_stake(&s, poll_id, StakeSide::Yes, 3 * AMT),
        ];
        s.client.cancel_poll(&s.admin, &poll_id);
        for (i, user) in users.iter().enumerate() {
            s.client.emergency_withdraw(user, &poll_id);
            assert_eq!(balance(&s, user), (i as i128 + 1) * AMT);
        }
        assert_eq!(balance(&s, &s.contract_id), 0);
        assert_eq!(s.client.get_platform_stats().total_value_locked, 0);
    }

    #[test]
    fn pause_blocks_value_flows_but_not_exits() {
        let s = sec_setup();
        let poll_id = sec_poll(&s);
        let user = sec_stake(&s, poll_id, StakeSide::Yes, AMT);
        let other = Address::generate(&s.env);
        mint_to(&s.env, &s.token_addr, &other, AMT);

        s.client.pause(&s.admin);
        let paused = Ok(PredictXError::ContractPaused);
        assert_eq!(s.client.try_stake(&other, &poll_id, &AMT, &StakeSide::No).unwrap_err(), paused);
        assert_eq!(
            s.client
                .try_create_poll(&s.admin, &1_u64, &String::from_str(&s.env, "Q?"), &PollCategory::Other, &LOCK)
                .unwrap_err(),
            paused
        );
        assert_eq!(s.client.try_resolve_poll(&s.oracle_id, &poll_id, &true).unwrap_err(), paused);
        assert_eq!(s.client.try_claim_winnings(&user, &poll_id).unwrap_err(), paused);
        assert_eq!(s.client.try_sync_resolution(&poll_id).unwrap_err(), paused);

        // The emergency exit stays open.
        s.client.cancel_poll(&s.admin, &poll_id);
        assert_eq!(s.client.emergency_withdraw(&user, &poll_id), AMT);
    }

    #[test]
    fn admin_entry_points_reject_non_admin() {
        let s = sec_setup();
        let poll_id = sec_poll(&s);
        let stranger = Address::generate(&s.env);
        let unauthorized = Ok(PredictXError::Unauthorized);
        assert_eq!(s.client.try_pause(&stranger).unwrap_err(), unauthorized);
        assert_eq!(s.client.try_unpause(&stranger).unwrap_err(), unauthorized);
        assert_eq!(s.client.try_cancel_poll(&stranger, &poll_id).unwrap_err(), unauthorized);
        assert_eq!(s.client.try_resolve_poll(&stranger, &poll_id, &true).unwrap_err(), unauthorized);
    }

    #[test]
    fn stake_requires_staker_auth() {
        let s = sec_setup();
        let poll_id = sec_poll(&s);
        let user = Address::generate(&s.env);
        mint_to(&s.env, &s.token_addr, &user, AMT);
        s.env.set_auths(&[]);
        assert!(s.client.try_stake(&user, &poll_id, &AMT, &StakeSide::Yes).is_err());
        assert_eq!(balance(&s, &user), AMT);
    }

    #[test]
    fn initialize_rejects_fee_above_100_percent() {
        let env = Env::default();
        env.mock_all_auths();
        let client = PredictionMarketClient::new(&env, &env.register(PredictionMarket, ()));
        let a = || Address::generate(&env);
        let err = client.try_initialize(&a(), &a(), &a(), &a(), &10_001_u32).unwrap_err();
        assert_eq!(err, Ok(PredictXError::InvalidFeeBps));
    }

    #[test]
    fn create_poll_validates_question_and_match_state() {
        let s = sec_setup();
        let match_id = s.client.create_match(
            &s.admin,
            &String::from_str(&s.env, "A"),
            &String::from_str(&s.env, "B"),
            &String::from_str(&s.env, "L"),
            &String::from_str(&s.env, "V"),
            &(LOCK + 3_600),
        );
        let too_long = String::from_str(&s.env, &"x".repeat(MAX_QUESTION_LENGTH as usize + 1));
        for q in [String::from_str(&s.env, ""), too_long] {
            let err = s
                .client
                .try_create_poll(&s.admin, &match_id, &q, &PollCategory::Other, &LOCK)
                .unwrap_err();
            assert_eq!(err, Ok(PredictXError::PollQuestionTooLong));
        }

        s.client.finish_match(&s.admin, &match_id);
        let err = s
            .client
            .try_create_poll(&s.admin, &match_id, &String::from_str(&s.env, "Known?"), &PollCategory::Other, &LOCK)
            .unwrap_err();
        assert_eq!(err, Ok(PredictXError::MatchAlreadyStarted));
    }

    #[test]
    fn potential_winnings_overflow_returns_error() {
        let s = sec_setup();
        let poll_id = sec_poll(&s);
        s.env.as_contract(&s.contract_id, || {
            let mut poll: Poll = s.env.storage().persistent().get(&DataKey::Poll(poll_id)).unwrap();
            poll.yes_pool = i128::MAX / 4;
            poll.no_pool = i128::MAX / 4;
            s.env.storage().persistent().set(&DataKey::Poll(poll_id), &poll);
        });
        let err = s
            .client
            .try_calculate_potential_winnings(&poll_id, &StakeSide::Yes, &(i128::MAX / 4))
            .unwrap_err();
        assert_eq!(err, Ok(PredictXError::ArithmeticOverflow));
    }

    #[test]
    fn community_vote_resolution_flows_through_to_claims() {
        let s = sec_setup();
        let oracle = voting_oracle::Client::new(&s.env, &s.oracle_id);
        oracle.set_market(&s.admin, &s.contract_id);
        let poll_id = sec_poll(&s);
        let winner = sec_stake(&s, poll_id, StakeSide::Yes, AMT);
        let loser = sec_stake(&s, poll_id, StakeSide::No, AMT);

        // Not resolved on the oracle yet.
        assert_eq!(
            s.client.try_sync_resolution(&poll_id).unwrap_err(),
            Ok(PredictXError::OutcomeNotAvailable)
        );

        s.env.ledger().set_timestamp(LOCK + 7_200);
        oracle.set_poll_status(&poll_id, &voting_oracle::PollStatus::Voting);

        // Stakers have a stake in the outcome and may not vote.
        let err = oracle.try_cast_vote(&winner, &poll_id, &voting_oracle::VoteChoice::Yes);
        assert!(err.is_err());

        for _ in 0..9 {
            oracle.cast_vote(&Address::generate(&s.env), &poll_id, &voting_oracle::VoteChoice::Yes);
        }
        oracle.cast_vote(&Address::generate(&s.env), &poll_id, &voting_oracle::VoteChoice::No);
        s.env.ledger().set_timestamp(LOCK + 7_200 + predictx_shared::VOTING_WINDOW_SECS);
        oracle.auto_resolve(&poll_id);

        assert!(s.client.sync_resolution(&poll_id));
        assert_eq!(
            s.client.try_sync_resolution(&poll_id).unwrap_err(),
            Ok(PredictXError::PollAlreadyResolved)
        );
        assert_eq!(s.client.calculate_winnings(&poll_id, &winner), 190_000_000);
        assert_eq!(s.client.claim_winnings(&winner, &poll_id), 190_000_000);
        assert_eq!(
            s.client.try_claim_winnings(&loser, &poll_id).unwrap_err(),
            Ok(PredictXError::NotOnWinningSide)
        );
        let treasury = s.client.get_treasury_address();
        assert_eq!(balance(&s, &treasury), 10_000_000);
        assert_eq!(balance(&s, &s.contract_id), 0);
    }
}
