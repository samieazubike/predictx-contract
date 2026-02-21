#![no_std]

pub mod constants;
pub mod errors;
pub mod matches;
pub mod storage;
pub mod types;

pub use constants::*;
pub use errors::PredictXError;
pub use storage::DataKey;
pub use types::{
    Dispute, Match, PlatformStats, Poll, PollCategory, PollStatus, Stake, StakeSide,
    UserStats, VoteChoice, VoteTally,
};

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};

#[contract]
pub struct PredictXContract;

#[contractimpl]
impl PredictXContract {
    /// Bootstrap the contract. Can only be called once.
    pub fn initialize(env: Env, admin: Address) -> Result<(), PredictXError> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(PredictXError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextMatchId, &1u64);
        env.storage().instance().set(&DataKey::NextPollId, &1u64);
        env.storage().instance().set(&DataKey::Initialized, &true);
        Ok(())
    }

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

    pub fn ping(_env: Env) -> bool { true }
}