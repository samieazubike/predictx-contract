#![no_std]

mod voting;

use predictx_shared::{PollStatus, PredictXError};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

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
    /// `poll_id` → `VoteTally`. (Temporary — only needed during voting window)
    VoteTally(u64),
    /// `poll_id` → IPFS evidence hash `String`. (Temporary — alongside tally)
    VotingEvidence(u64),
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

    /// Opens a two-hour community voting window for a finished poll.
    ///
    /// Delegates to [`voting::initiate_voting`].
    pub fn initiate_voting(
        env: Env,
        admin: Address,
        poll_id: u64,
        evidence_hash: String,
    ) -> Result<(), PredictXError> {
        voting::initiate_voting(env, admin, poll_id, evidence_hash)
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    fn setup_env() -> (soroban_sdk::Env, Address, VotingOracleClient<'static>) {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        // SAFETY: the Env outlives this test frame; the client borrows it.
        let client: VotingOracleClient<'static> = unsafe { core::mem::transmute(client) };
        (env, admin, client)
    }

    #[test]
    fn set_and_get_status() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        client.set_poll_status(&42_u64, &PollStatus::Resolved);
        assert_eq!(client.get_poll_status(&42_u64), PollStatus::Resolved);
    }

    // ── initiate_voting tests ─────────────────────────────────────────────────

    /// Happy path: tally is stored with the correct end-time and zeroed counts,
    /// and poll status transitions to `Voting`.
    #[test]
    fn test_initiate_voting_happy_path() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        env.ledger().set_timestamp(1_000_000);

        client.initiate_voting(
            &admin,
            &1_u64,
            &soroban_sdk::String::from_str(&env, "ipfs://QmEvidence1"),
        );

        // Tally must exist in temporary storage with correct fields.
        let tally: predictx_shared::VoteTally = env
            .storage()
            .temporary()
            .get(&DataKey::VoteTally(1))
            .expect("VoteTally should be stored");

        assert_eq!(
            tally.voting_end_time,
            1_000_000 + predictx_shared::VOTING_WINDOW_SECS,
            "voting_end_time must be exactly now + VOTING_WINDOW_SECS"
        );
        assert_eq!(tally.yes_votes, 0);
        assert_eq!(tally.no_votes, 0);
        assert_eq!(tally.unclear_votes, 0);
        assert_eq!(tally.total_voters, 0);
        assert_eq!(tally.reward_pool, 0);

        // Poll status must be Voting.
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Voting);
    }

    /// `voting_end_time` must be exactly `start + 7200` (two hours).
    #[test]
    fn test_voting_end_time_is_exactly_two_hours() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let start_ts: u64 = 5_000_000;
        env.ledger().set_timestamp(start_ts);

        client.initiate_voting(
            &admin,
            &2_u64,
            &soroban_sdk::String::from_str(&env, "ipfs://evidence2"),
        );

        let tally: predictx_shared::VoteTally = env
            .storage()
            .temporary()
            .get(&DataKey::VoteTally(2))
            .unwrap();

        assert_eq!(
            tally.voting_end_time,
            start_ts + 7_200,
            "window must be exactly 7200 seconds (2 hours)"
        );
    }

    /// A non-admin caller must be rejected with `Unauthorized`.
    #[test]
    fn test_non_admin_gets_unauthorized() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        env.ledger().set_timestamp(1_000_000);

        let imposter = Address::generate(&env);

        let result = client.try_initiate_voting(
            &imposter,
            &3_u64,
            &soroban_sdk::String::from_str(&env, "fake"),
        );

        assert_eq!(
            result,
            Err(Ok(PredictXError::Unauthorized)),
            "non-admin must get Unauthorized"
        );
    }

    /// Calling `initiate_voting` twice for the same poll must return
    /// `PollAlreadyResolved` on the second call.
    #[test]
    fn test_double_initiate_gets_poll_already_resolved() {
        let env = soroban_sdk::Env::default();
        env.mock_all_auths();
        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        env.ledger().set_timestamp(1_000_000);

        // First call — must succeed.
        client.initiate_voting(
            &admin,
            &4_u64,
            &soroban_sdk::String::from_str(&env, "ipfs://first"),
        );

        // Second call for the same poll — must be rejected.
        let result = client.try_initiate_voting(
            &admin,
            &4_u64,
            &soroban_sdk::String::from_str(&env, "ipfs://second"),
        );

        assert_eq!(
            result,
            Err(Ok(PredictXError::PollAlreadyResolved)),
            "second initiate_voting for the same poll must get PollAlreadyResolved"
        );
    }
}
