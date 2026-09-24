#![no_std]

use predictx_shared::{PredictXError, PollStatus};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

pub mod voting;

#[contract]
pub struct VotingOracle;

#[contracttype]
#[derive(Clone)]
pub(crate) struct StoredPollStatus {
    pub status: PollStatus,
    pub updated_at: u64,
    pub provisional_outcome: Option<bool>,
}

#[contracttype]
#[derive(Clone)]
pub(crate) enum DataKey {
    Admin,
    PollStatus(u64),
    Evidence(u64),
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
    pub fn set_poll_status(env: Env, poll_id: u64, status: PollStatus) -> Result<(), PredictXError> {
        let admin = get_admin(&env)?;
        admin.require_auth();

        let stored = StoredPollStatus {
            status,
            updated_at: env.ledger().timestamp(),
            provisional_outcome: None,
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

    /// Stores the evidence hash for a poll (e.g. an IPFS CID or stats-API reference).
    /// Reject empty evidence strings. This is meant to be called during `initiate_voting`
    /// or as a helper until it's merged.
    pub fn set_evidence(
        env: Env,
        poll_id: u64,
        evidence_hash: soroban_sdk::String,
    ) -> Result<(), PredictXError> {
        let admin = get_admin(&env)?;
        admin.require_auth();

        if evidence_hash.len() == 0 {
            return Err(PredictXError::InvalidEvidence);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Evidence(poll_id), &evidence_hash);
        Ok(())
    }

    /// Retrieves the evidence hash for a given poll.
    /// Returns `PollNotFound` if the evidence does not exist (meaning no vote opened or no evidence).
    pub fn get_evidence(env: Env, poll_id: u64) -> Result<soroban_sdk::String, PredictXError> {
        env.storage()
            .persistent()
            .get(&DataKey::Evidence(poll_id))
            .ok_or(PredictXError::PollNotFound)
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
    fn set_and_get_evidence() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let poll_id = 1_u64;
        let evidence_hash = soroban_sdk::String::from_str(&env, "ipfs://Qm123");

        // Set evidence
        client.set_evidence(&poll_id, &evidence_hash);

        // Get evidence, should round-trip unchanged
        let retrieved = client.get_evidence(&poll_id);
        assert_eq!(retrieved, evidence_hash);
    }

    #[test]
    fn get_evidence_not_found() {
        let env = Env::default();
        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        // Get evidence for a non-existent poll should return PollNotFound
        let res = client.try_get_evidence(&99_u64);
        assert_eq!(res, Err(Ok(PredictXError::PollNotFound)));
    }

    #[test]
    fn set_evidence_empty_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let poll_id = 2_u64;
        let empty_evidence = soroban_sdk::String::from_str(&env, "");

        // Set empty evidence should fail with InvalidEvidence
        let res = client.try_set_evidence(&poll_id, &empty_evidence);
        assert_eq!(res, Err(Ok(PredictXError::InvalidEvidence)));
    }
}
