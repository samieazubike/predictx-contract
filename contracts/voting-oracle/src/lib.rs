#![no_std]

use predictx_shared::{PredictXError, PollStatus, VoteTally};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

mod voting;

#[contract]
pub struct VotingOracle;

#[contracttype]
#[derive(Clone)]
pub(crate) struct StoredPollStatus {
    pub(crate) status: PollStatus,
    pub(crate) updated_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub(crate) enum DataKey {
    Admin,
    PollStatus(u64),
    /// `poll_id` → vote tally. (Temporary — only needed during the voting window)
    VoteTally(u64),
    /// `(poll_id, voter)` → `bool` — has this voter cast a vote? (Temporary)
    HasVoted(u64, Address),
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)
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
    pub fn set_poll_status(
        env: Env,
        poll_id: u64,
        status: PollStatus,
    ) -> Result<(), PredictXError> {
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

    pub fn process_tally(env: Env, tally: VoteTally) -> Result<(), PredictXError> {
        let admin = get_admin(&env)?;
        admin.require_auth();

        voting::process_tally(&env, &tally);
        Ok(())
    /// Record a voter's choice on a poll.
    pub fn cast_vote(
        env: Env,
        voter: Address,
        poll_id: u64,
        choice: VoteChoice,
    ) -> Result<VoteTally, PredictXError> {
        voting::cast_vote(&env, voter, poll_id, choice)
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
}
