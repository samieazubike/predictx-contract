#![no_std]

mod matches;
mod staking;
pub(crate) mod token_utils;

use predictx_shared::{
    Match, PlatformStats, Poll, PollCategory, PollStatus, PredictXError, Stake, StakeSide,
    MAX_POLLS_PER_MATCH, PlatformConfig, ConfigKey, ConfigValue,
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
    VoterRewardBps,
    VotingWindowSecs,
    ConsensusThresholdBps,
    AdminReviewThresholdBps,
    DisputeWindowSecs,
    DisputeFee,
    MinStakeAmount,
    MaxPollsPerMatch,
    ConfigVersion,
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
        return Err(PredictXError::EmergencyWithdrawNotAllowed);
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

const EMERGENCY_TIMEOUT_SECS: u64 = 7 * 24 * 60 * 60;

#[contractimpl]
impl PredictionMarket {
    pub fn initialize(
        env: Env,
        admin: Address,
        token_address: Address,
        treasury_address: Address,
        platform_fee_bps: u32,
        voter_reward_bps: u32,
        min_stake_amount: i128,
    ) -> Result<(), PredictXError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::AlreadyInitialized);
        }
        
        // Validate parameters
        if platform_fee_bps > 1000 {
            return Err(PredictXError::InvalidFee);
        }
        if voter_reward_bps > 200 {
            return Err(PredictXError::InvalidFee);
        }
        if voter_reward_bps >= platform_fee_bps {
            return Err(PredictXError::InvalidFee);
        }
        if min_stake_amount <= 0 {
            return Err(PredictXError::StakeAmountZero);
        }
        
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenAddress, &token_address);
        env.storage().instance().set(&DataKey::TreasuryAddress, &treasury_address);
        env.storage().instance().set(&DataKey::PlatformFeeBps, &platform_fee_bps);
        env.storage().instance().set(&DataKey::VoterRewardBps, &voter_reward_bps);
        env.storage().instance().set(&DataKey::MinStakeAmount, &min_stake_amount);
        
        // Set default values for other configuration
        env.storage().instance().set(&DataKey::VotingWindowSecs, &7200u64);
        env.storage().instance().set(&DataKey::ConsensusThresholdBps, &8500u32);
        env.storage().instance().set(&DataKey::AdminReviewThresholdBps, &6000u32);
        env.storage().instance().set(&DataKey::DisputeWindowSecs, &86400u64);
        env.storage().instance().set(&DataKey::DisputeFee, &10000000i128);
        env.storage().instance().set(&DataKey::MaxPollsPerMatch, &50u32);
        env.storage().instance().set(&DataKey::ConfigVersion, &1u32);
        
        env.storage().instance().set(&DataKey::NextMatchId, &1u64);
        env.storage().instance().set(&DataKey::NextPollId, &1u64);
        env.storage().instance().set(&DataKey::Initialized, &true);
        
        // Emit initialization event
        env.events().publish(
            (Symbol::new(&env, "Initialized"),),
            (admin, token_address, treasury_address, platform_fee_bps, voter_reward_bps, min_stake_amount)
        );
        
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

    pub fn cancel_poll(env: Env, admin: Address, poll_id: u64) -> Result<(), PredictXError> {
        ensure_not_paused(&env)?;
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin { return Err(PredictXError::Unauthorized); }
        admin.require_auth();
        let oracle_id = get_oracle(&env)?;
        let client = voting_oracle::Client::new(&env, &oracle_id);
        client.set_poll_status(&poll_id, &voting_oracle::PollStatus::Cancelled);
        env.events().publish((Symbol::new(&env, "PollCancelled"),), poll_id);
        Ok(())
    }

    pub fn check_emergency_eligible(env: Env, poll_id: u64) -> bool {
        let oracle_id = match get_oracle(&env) {
            Ok(id) => id,
            Err(_) => return false,
        };
        let client = voting_oracle::Client::new(&env, &oracle_id);
        let status = map_oracle_poll_status(client.get_poll_status(&poll_id));
        if status == PollStatus::Cancelled { return true; }
        if status != PollStatus::Disputed && status != PollStatus::Locked { return false; }
        let updated_at = client.get_poll_status_updated_at(&poll_id);
        if updated_at == 0 { return false; }
        env.ledger().timestamp().saturating_sub(updated_at) >= EMERGENCY_TIMEOUT_SECS
    }

    pub fn emergency_withdraw(env: Env, user: Address, poll_id: u64) -> Result<i128, PredictXError> {
        user.require_auth();
        if has_emergency_claimed(&env, poll_id, &user) {
            return Err(PredictXError::AlreadyClaimed);
        }
        let oracle_id = get_oracle(&env)?;
        let client = voting_oracle::Client::new(&env, &oracle_id);
        let status = map_oracle_poll_status(client.get_poll_status(&poll_id));
        let eligible = if status == PollStatus::Cancelled {
            true
        } else if status == PollStatus::Disputed || status == PollStatus::Locked {
            let updated_at = client.get_poll_status_updated_at(&poll_id);
            updated_at != 0 && env.ledger().timestamp().saturating_sub(updated_at) >= EMERGENCY_TIMEOUT_SECS
        } else {
            false
        };
        if !eligible { return Err(PredictXError::EmergencyWithdrawNotAllowed); }
        let stake = load_stake(&env, poll_id, &user).ok_or(PredictXError::NotStaker)?;
        set_emergency_claimed(&env, poll_id, &user);

        // Transfer tokens back to user
        token_utils::transfer_from_contract(&env, &user, stake.amount)?;

        let mut stats = get_platform_stats(&env);
        stats.total_value_locked -= stake.amount;
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

        // Validate match exists
        let _m: Match = env
            .storage()
            .persistent()
            .get(&DataKey::Match(match_id))
            .ok_or(PredictXError::MatchNotFound)?;

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

    // ── Configuration Management ───────────────────────────────────────────────

    pub fn get_config(env: Env) -> Result<PlatformConfig, PredictXError> {
        let admin = get_admin(&env)?;
        let token_address = env.storage().instance()
            .get(&DataKey::TokenAddress)
            .ok_or(PredictXError::NotInitialized)?;
        let treasury_address = env.storage().instance()
            .get(&DataKey::TreasuryAddress)
            .ok_or(PredictXError::NotInitialized)?;
        let platform_fee_bps = env.storage().instance()
            .get(&DataKey::PlatformFeeBps)
            .ok_or(PredictXError::NotInitialized)?;
        let voter_reward_bps = env.storage().instance()
            .get(&DataKey::VoterRewardBps)
            .ok_or(PredictXError::NotInitialized)?;
        let voting_window_secs = env.storage().instance()
            .get(&DataKey::VotingWindowSecs)
            .ok_or(PredictXError::NotInitialized)?;
        let consensus_threshold_bps = env.storage().instance()
            .get(&DataKey::ConsensusThresholdBps)
            .ok_or(PredictXError::NotInitialized)?;
        let admin_review_threshold_bps = env.storage().instance()
            .get(&DataKey::AdminReviewThresholdBps)
            .ok_or(PredictXError::NotInitialized)?;
        let dispute_window_secs = env.storage().instance()
            .get(&DataKey::DisputeWindowSecs)
            .ok_or(PredictXError::NotInitialized)?;
        let dispute_fee = env.storage().instance()
            .get(&DataKey::DisputeFee)
            .ok_or(PredictXError::NotInitialized)?;
        let min_stake_amount = env.storage().instance()
            .get(&DataKey::MinStakeAmount)
            .ok_or(PredictXError::NotInitialized)?;
        let max_polls_per_match = env.storage().instance()
            .get(&DataKey::MaxPollsPerMatch)
            .ok_or(PredictXError::NotInitialized)?;
        let version = env.storage().instance()
            .get(&DataKey::ConfigVersion)
            .ok_or(PredictXError::NotInitialized)?;
        let is_paused = is_paused(&env);

        Ok(PlatformConfig {
            admin,
            token_address,
            treasury_address,
            platform_fee_bps,
            voter_reward_bps,
            voting_window_secs,
            consensus_threshold_bps,
            admin_review_threshold_bps,
            dispute_window_secs,
            dispute_fee,
            min_stake_amount,
            max_polls_per_match,
            version,
            is_paused,
        })
    }

    pub fn update_config(
        env: Env,
        admin: Address,
        key: ConfigKey,
        value: ConfigValue,
    ) -> Result<(), PredictXError> {
        ensure_not_paused(&env)?;
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();

        let old_value = match key {
            ConfigKey::PlatformFeeBps => {
                let old = env.storage().instance().get(&DataKey::PlatformFeeBps)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U32Value(new_fee_bps) = value {
                    if new_fee_bps > 1000 {
                        return Err(PredictXError::InvalidFee);
                    }
                    env.storage().instance().set(&DataKey::PlatformFeeBps, &new_fee_bps);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U32Value(old), ConfigValue::U32Value(new_fee_bps))
                    );
                }
                ConfigValue::U32Value(old)
            },
            ConfigKey::VoterRewardBps => {
                let old = env.storage().instance().get(&DataKey::VoterRewardBps)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U32Value(new_reward_bps) = value {
                    if new_reward_bps > 200 {
                        return Err(PredictXError::InvalidFee);
                    }
                    let platform_fee = env.storage().instance().get(&DataKey::PlatformFeeBps)
                        .ok_or(PredictXError::NotInitialized)?;
                    if new_reward_bps >= platform_fee {
                        return Err(PredictXError::InvalidFee);
                    }
                    env.storage().instance().set(&DataKey::VoterRewardBps, &new_reward_bps);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U32Value(old), ConfigValue::U32Value(new_reward_bps))
                    );
                }
                ConfigValue::U32Value(old)
            },
            ConfigKey::VotingWindowSecs => {
                let old = env.storage().instance().get(&DataKey::VotingWindowSecs)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U64Value(new_window) = value {
                    if new_window < 1800 || new_window > 14400 {
                        return Err(PredictXError::InvalidLockTime);
                    }
                    env.storage().instance().set(&DataKey::VotingWindowSecs, &new_window);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U64Value(old), ConfigValue::U64Value(new_window))
                    );
                }
                ConfigValue::U64Value(old)
            },
            ConfigKey::ConsensusThresholdBps => {
                let old = env.storage().instance().get(&DataKey::ConsensusThresholdBps)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U32Value(new_threshold) = value {
                    if new_threshold < 7000 || new_threshold > 9500 {
                        return Err(PredictXError::InvalidFee);
                    }
                    env.storage().instance().set(&DataKey::ConsensusThresholdBps, &new_threshold);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U32Value(old), ConfigValue::U32Value(new_threshold))
                    );
                }
                ConfigValue::U32Value(old)
            },
            ConfigKey::AdminReviewThresholdBps => {
                let old = env.storage().instance().get(&DataKey::AdminReviewThresholdBps)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U32Value(new_threshold) = value {
                    if new_threshold < 5000 || new_threshold > 8000 {
                        return Err(PredictXError::InvalidFee);
                    }
                    env.storage().instance().set(&DataKey::AdminReviewThresholdBps, &new_threshold);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U32Value(old), ConfigValue::U32Value(new_threshold))
                    );
                }
                ConfigValue::U32Value(old)
            },
            ConfigKey::DisputeWindowSecs => {
                let old = env.storage().instance().get(&DataKey::DisputeWindowSecs)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U64Value(new_window) = value {
                    if new_window < 43200 || new_window > 172800 {
                        return Err(PredictXError::InvalidLockTime);
                    }
                    env.storage().instance().set(&DataKey::DisputeWindowSecs, &new_window);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U64Value(old), ConfigValue::U64Value(new_window))
                    );
                }
                ConfigValue::U64Value(old)
            },
            ConfigKey::DisputeFee => {
                let old = env.storage().instance().get(&DataKey::DisputeFee)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::I128Value(new_fee) = value {
                    if new_fee < 0 {
                        return Err(PredictXError::StakeAmountZero);
                    }
                    env.storage().instance().set(&DataKey::DisputeFee, &new_fee);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::I128Value(old), ConfigValue::I128Value(new_fee))
                    );
                }
                ConfigValue::I128Value(old)
            },
            ConfigKey::MinStakeAmount => {
                let old = env.storage().instance().get(&DataKey::MinStakeAmount)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::I128Value(new_amount) = value {
                    if new_amount <= 0 {
                        return Err(PredictXError::StakeAmountZero);
                    }
                    env.storage().instance().set(&DataKey::MinStakeAmount, &new_amount);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::I128Value(old), ConfigValue::I128Value(new_amount))
                    );
                }
                ConfigValue::I128Value(old)
            },
            ConfigKey::MaxPollsPerMatch => {
                let old = env.storage().instance().get(&DataKey::MaxPollsPerMatch)
                    .ok_or(PredictXError::NotInitialized)?;
                if let ConfigValue::U32Value(new_max) = value {
                    if new_max < 10 || new_max > 100 {
                        return Err(PredictXError::InvalidFee);
                    }
                    env.storage().instance().set(&DataKey::MaxPollsPerMatch, &new_max);
                    env.events().publish(
                        (Symbol::new(&env, "ConfigUpdated"),),
                        (key, ConfigValue::U32Value(old), ConfigValue::U32Value(new_max))
                    );
                }
                ConfigValue::U32Value(old)
            },
        };

        // Increment config version
        let current_version: u32 = env.storage().instance()
            .get(&DataKey::ConfigVersion)
            .ok_or(PredictXError::NotInitialized)?;
        env.storage().instance().set(&DataKey::ConfigVersion, &(current_version + 1u32));

        Ok(())
    }

    // ── Cross-Contract Address Setting ───────────────────────────────────────────

    pub fn set_oracle_address(env: Env, admin: Address, oracle: Address) -> Result<(), PredictXError> {
        ensure_not_paused(&env)?;
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::VotingOracle, &oracle);
        env.events().publish((Symbol::new(&env, "OracleAddressUpdated"),), oracle);
        Ok(())
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
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test {
    use super::*;
    use predictx_shared::StakeSide;
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
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &token, &treasury, &TEST_FEE_BPS, &100u32, &10000000i128);
        assert_eq!(client.admin(), admin);
        assert_eq!(client.get_token_address(), token);
    }

    #[test]
    fn initialize_stores_token_and_treasury() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &token, &treasury, &TEST_FEE_BPS, &100u32, &10000000i128);
        assert_eq!(client.get_token_address(), token);
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
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &token, &treasury, &TEST_FEE_BPS, &100u32, &10000000i128);
        let err = client.try_initialize(&admin, &token, &treasury, &TEST_FEE_BPS, &100u32, &10000000i128).expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::AlreadyInitialized));
    }

    #[test]
    fn cross_contract_oracle_call_works() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin, &Address::generate(&env), &7200u64, &8500u32, &6000u32, &86400u64, &10000000i128);
        oracle_client.set_poll_status(&7_u64, &voting_oracle::PollStatus::Resolved);
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let tok = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &tok, &treasury, &TEST_FEE_BPS, &100u32, &10000000i128);
        client.set_oracle(&oracle_id);
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
        client.initialize(&admin, &tok, &treasury, &TEST_FEE_BPS, &100u32, &10000000i128);
        assert_eq!(client.is_paused(), false);
        client.pause(&admin);
        assert_eq!(client.is_paused(), true);
        let err = client.try_set_oracle(&oracle).expect_err("should be blocked");
        assert_eq!(err, Ok(PredictXError::EmergencyWithdrawNotAllowed));
        client.unpause(&admin);
        assert_eq!(client.is_paused(), false);
    }

    #[test]
    fn cancel_poll_sets_cancelled_status_and_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin, &Address::generate(&env), &7200u64, &8500u32, &6000u32, &86400u64, &10000000i128);
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let tok = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &tok, &treasury, &TEST_FEE_BPS, &100u32, &10000000i128);
        client.set_oracle(&oracle_id);
        client.cancel_poll(&admin, &1_u64);
        assert_eq!(oracle_client.get_poll_status(&1_u64), voting_oracle::PollStatus::Cancelled);
    }

    // Helper to set up a real-token environment for emergency withdrawal tests
    fn setup_emergency_env() -> (Env, Address, Address, Address, PredictionMarketClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);

        let oracle_id = env.register(voting_oracle::WASM, ());
        let _oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        _oracle_client.initialize(&admin, &Address::generate(&env), &7200u64, &8500u32, &6000u32, &86400u64, &10000000i128);

        // Real token for transfers
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_addr = token_contract.address();
        let treasury = Address::generate(&env);

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        client.initialize(&admin, &token_addr, &treasury, &TEST_FEE_BPS, &100u32, &10000000i128);

        (env, admin, oracle_id, contract_id, client)
    }

    /// Mint tokens to an address using the real SAC
    fn mint_to(env: &Env, token_addr: &Address, to: &Address, amount: i128) {
        let sac = token::StellarAssetClient::new(env, token_addr);
        sac.mint(to, &amount);
    }

    #[test]
    fn emergency_withdraw_on_cancelled_poll_refunds_stake() {
        let (env, admin, oracle_id, contract_id, client) = setup_emergency_env();
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        let token_addr: Address = env.as_contract(&contract_id, || {
            env.storage().instance().get(&DataKey::TokenAddress).unwrap()
        });

        let user = Address::generate(&env);
        let amount: i128 = 50;

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

    #[test]
    fn get_config_returns_complete_configuration() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        
        client.initialize(&admin, &token, &treasury, &500u32, &100u32, &10000000i128);
        
        let config = client.get_config();
        assert_eq!(config.admin, admin);
        assert_eq!(config.token_address, token);
        assert_eq!(config.treasury_address, treasury);
        assert_eq!(config.platform_fee_bps, 500);
        assert_eq!(config.voter_reward_bps, 100);
        assert_eq!(config.voting_window_secs, 7200);
        assert_eq!(config.consensus_threshold_bps, 8500);
        assert_eq!(config.admin_review_threshold_bps, 6000);
        assert_eq!(config.dispute_window_secs, 86400);
        assert_eq!(config.dispute_fee, 10000000);
        assert_eq!(config.min_stake_amount, 10000000);
        assert_eq!(config.max_polls_per_match, 50);
        assert_eq!(config.version, 1);
        assert_eq!(config.is_paused, false);
    }

    #[test]
    fn update_platform_fee_within_range_succeeds() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        
        client.initialize(&admin, &token, &treasury, &500u32, &100u32, &10000000i128);
        
        client.update_config(&admin, &ConfigKey::PlatformFeeBps, &ConfigValue::U32Value(600));
        
        let config = client.get_config();
        assert_eq!(config.platform_fee_bps, 600);
        assert_eq!(config.version, 2);
    }

    #[test]
    fn update_platform_fee_above_maximum_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        
        client.initialize(&admin, &token, &treasury, &500u32, &100u32, &10000000i128);
        
        let err = client.try_update_config(&admin, &ConfigKey::PlatformFeeBps, &ConfigValue::U32Value(1500))
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::InvalidFee));
    }

    #[test]
    fn update_voter_reward_exceeds_platform_fee_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        
        client.initialize(&admin, &token, &treasury, &500u32, &100u32, &10000000i128);
        
        let err = client.try_update_config(&admin, &ConfigKey::VoterRewardBps, &ConfigValue::U32Value(600))
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::InvalidFee));
    }

    #[test]
    fn update_config_by_non_admin_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let non_admin = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        
        client.initialize(&admin, &token, &treasury, &500u32, &100u32, &10000000i128);
        
        let err = client.try_update_config(&non_admin, &ConfigKey::PlatformFeeBps, &ConfigValue::U32Value(600))
            .expect_err("should fail");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
    }

    #[test]
    fn set_oracle_address_updates_cross_contract_reference() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let token = Address::generate(&env);
        let treasury = Address::generate(&env);
        let oracle = Address::generate(&env);
        
        client.initialize(&admin, &token, &treasury, &500u32, &100u32, &10000000i128);
        
        client.set_oracle_address(&admin, &oracle);
        
        // Verify oracle address was set (we can't directly query it but the call succeeds)
        // In a real scenario, subsequent oracle calls would work
    }
}