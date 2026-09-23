#![no_std]

use predictx_shared::{PollStatus, PredictXError, VoteTally};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contract]
pub struct VotingOracle;

#[contracttype]
#[derive(Clone)]
struct StoredPollStatus {
    status: PollStatus,
    updated_at: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    PollStatus(u64),
    Tally(u64),
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)
}

/// Read the stored vote tally for `poll_id`, if any.
fn read_tally(env: &Env, poll_id: u64) -> Option<VoteTally> {
    env.storage().persistent().get(&DataKey::Tally(poll_id))
}

/// Persist `tally` under its poll ID.
fn write_tally(env: &Env, tally: &VoteTally) {
    env.storage()
        .persistent()
        .set(&DataKey::Tally(tally.poll_id), tally);
}

#[contractimpl]
impl VotingOracle {
    pub fn initialize(env: Env, admin: Address) -> Result<(), PredictXError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::AlreadyInitialized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, PredictXError> {
        get_admin(&env)
    }

    /// Placeholder oracle state setter.
    ///
    /// This exists only to validate cross-contract invocation patterns during
    /// Phase 1 scaffolding.
    pub fn set_poll_status(env: Env, poll_id: u64, status: PollStatus) -> Result<(), PredictXError> {
        let admin = get_admin(&env)?;
        admin.require_auth();

        let stored = StoredPollStatus {
            status,
            updated_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::PollStatus(poll_id), &stored);
        Ok(())
    }

    /// Placeholder oracle query used by `PredictionMarket`.
    pub fn get_poll_status(env: Env, poll_id: u64) -> PollStatus {
        let stored: Option<StoredPollStatus> = env
            .storage()
            .persistent()
            .get(&DataKey::PollStatus(poll_id));

        stored.map(|s| s.status).unwrap_or(PollStatus::Active)
    }

    pub fn get_poll_status_updated_at(env: Env, poll_id: u64) -> u64 {
        let stored: Option<StoredPollStatus> = env
            .storage()
            .persistent()
            .get(&DataKey::PollStatus(poll_id));

        stored.map(|s| s.updated_at).unwrap_or(0)
    }

    /// Read the aggregated community vote tally for `poll_id`.
    ///
    /// Returns [`PredictXError::PollNotFound`] when no tally has been recorded
    /// for the poll yet.
    pub fn get_vote_tally(env: Env, poll_id: u64) -> Result<VoteTally, PredictXError> {
        read_tally(&env, poll_id).ok_or(PredictXError::PollNotFound)
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn set_and_get_status() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        client.set_poll_status(&42_u64, &PollStatus::Resolved);
        assert_eq!(client.get_poll_status(&42_u64), PollStatus::Resolved);
    }

    #[test]
    fn get_vote_tally_returns_poll_not_found_for_unknown_poll() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let err = client
            .try_get_vote_tally(&7_u64)
            .expect_err("unknown poll should error");
        assert_eq!(err, Ok(PredictXError::PollNotFound));
    }

    #[test]
    fn get_vote_tally_reads_back_stored_tally_field_for_field() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let tally = VoteTally {
            poll_id: 99,
            yes_votes: 12,
            no_votes: 5,
            unclear_votes: 3,
            total_voters: 20,
            voting_end_time: 1_700_000_000,
            reward_pool: 1_500_000,
        };

        env.as_contract(&contract_id, || {
            write_tally(&env, &tally);
        });

        let stored = client.get_vote_tally(&99_u64);
        assert_eq!(stored, tally);
        assert_eq!(stored.poll_id, 99);
        assert_eq!(stored.yes_votes, 12);
        assert_eq!(stored.no_votes, 5);
        assert_eq!(stored.unclear_votes, 3);
        assert_eq!(stored.total_voters, 20);
        assert_eq!(stored.voting_end_time, 1_700_000_000);
        assert_eq!(stored.reward_pool, 1_500_000);
    }

    #[test]
    fn write_tally_overwrites_previous_tally_for_same_poll() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let first = VoteTally {
            poll_id: 1,
            yes_votes: 1,
            no_votes: 0,
            unclear_votes: 0,
            total_voters: 1,
            voting_end_time: 100,
            reward_pool: 10,
        };
        let second = VoteTally {
            poll_id: 1,
            yes_votes: 4,
            no_votes: 2,
            unclear_votes: 1,
            total_voters: 7,
            voting_end_time: 200,
            reward_pool: 70,
        };

        env.as_contract(&contract_id, || {
            write_tally(&env, &first);
            write_tally(&env, &second);
        });

        assert_eq!(client.get_vote_tally(&1_u64), second);
    }
}
