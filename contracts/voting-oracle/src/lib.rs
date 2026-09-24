#![no_std]

use predictx_shared::{PollStatus, PredictXError};
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, Env, String};

#[contract]
pub struct VotingOracle;

#[contractevent]
pub struct AdminVerified {
    poll_id: u64,
    admin: Address,
    outcome: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredPollStatus {
    status: PollStatus,
    updated_at: u64,
    outcome: Option<bool>,
    reasoning: String,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    PollStatus(u64),
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)
}

fn empty_poll_status(env: &Env) -> StoredPollStatus {
    StoredPollStatus {
        status: PollStatus::Active,
        updated_at: 0,
        outcome: None,
        reasoning: String::from_str(env, ""),
    }
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

    pub fn set_poll_status(env: Env, poll_id: u64, status: PollStatus) -> Result<(), PredictXError> {
        let admin = get_admin(&env)?;
        admin.require_auth();

        let stored = StoredPollStatus {
            status,
            updated_at: env.ledger().timestamp(),
            outcome: None,
            reasoning: String::from_str(&env, ""),
        };

        env.storage().persistent().set(&DataKey::PollStatus(poll_id), &stored);
        Ok(())
    }

    pub fn admin_verify(
        env: Env,
        admin: Address,
        poll_id: u64,
        outcome: bool,
        reasoning: String,
    ) -> Result<(), PredictXError> {
        let stored_admin = get_admin(&env)?;
        if admin != stored_admin {
            return Err(PredictXError::Unauthorized);
        }
        admin.require_auth();

        let mut stored: StoredPollStatus = env
            .storage()
            .persistent()
            .get(&DataKey::PollStatus(poll_id))
            .unwrap_or_else(|| empty_poll_status(&env));

        if stored.status != PollStatus::AdminReview {
            return Err(PredictXError::ConsensusNotReached);
        }

        stored.status = PollStatus::Resolved;
        stored.updated_at = env.ledger().timestamp();
        stored.outcome = Some(outcome);
        stored.reasoning = reasoning.clone();

        env.storage().persistent().set(&DataKey::PollStatus(poll_id), &stored);

        AdminVerified {
            poll_id,
            admin: admin.clone(),
            outcome,
        }
        .publish(&env);

        Ok(())
    }

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

    pub fn get_poll_outcome(env: Env, poll_id: u64) -> Option<bool> {
        let stored: Option<StoredPollStatus> = env
            .storage()
            .persistent()
            .get(&DataKey::PollStatus(poll_id));

        stored.and_then(|s| s.outcome)
    }

    pub fn get_poll_reasoning(env: Env, poll_id: u64) -> String {
        let stored: Option<StoredPollStatus> = env
            .storage()
            .persistent()
            .get(&DataKey::PollStatus(poll_id));

        stored
            .map(|s| s.reasoning)
            .unwrap_or_else(|| String::from_str(&env, ""))
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
    fn admin_verify_resolves_review_and_persists_reasoning() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);
        client.set_poll_status(&7_u64, &PollStatus::AdminReview);

        let reasoning = String::from_str(&env, "Community vote was inconclusive, but the evidence supports Yes.");
        client.admin_verify(&admin, &7_u64, &true, &reasoning);

        assert_eq!(client.get_poll_status(&7_u64), PollStatus::Resolved);
        assert_eq!(client.get_poll_outcome(&7_u64), Some(true));
        assert_eq!(client.get_poll_reasoning(&7_u64), reasoning);
    }

    #[test]
    fn admin_verify_rejects_non_admin() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let attacker = Address::generate(&env);
        client.initialize(&admin);
        client.set_poll_status(&11_u64, &PollStatus::AdminReview);

        let reasoning = String::from_str(&env, "Not allowed");
        let err = client
            .try_admin_verify(&attacker, &11_u64, &false, &reasoning)
            .unwrap_err();

        assert_eq!(err, Err(soroban_sdk::InvokeError::Abort), "non-admin verification should be rejected");
    }

    #[test]
    fn admin_verify_rejects_non_admin_review_poll() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);
        client.set_poll_status(&18_u64, &PollStatus::Resolved);

        let reasoning = String::from_str(&env, "This poll already resolved.");
        let err = client
            .try_admin_verify(&admin, &18_u64, &false, &reasoning)
            .unwrap_err();

        assert_eq!(err, Err(soroban_sdk::InvokeError::Abort), "non-admin-review polls should fail");
    }

    #[test]
    fn admin_verify_cannot_override_high_consensus_resolution() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);
        client.set_poll_status(&24_u64, &PollStatus::Resolved);

        let reasoning = String::from_str(&env, "Attempted override of completed consensus poll.");
        let err = client
            .try_admin_verify(&admin, &24_u64, &true, &reasoning)
            .unwrap_err();

        assert_eq!(err, Err(soroban_sdk::InvokeError::Abort), "admin cannot override already-resolved consensus");
    }
}
