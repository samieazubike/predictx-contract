#![no_std]

use predictx_shared::{
    accept_super_admin_transfer, get_oracle as shared_get_oracle,
    get_super_admin as shared_get_super_admin, propose_super_admin_transfer, require_oracle,
    set_oracle as shared_set_oracle, DataKey as SharedDataKey, PredictXError, PollStatus,
};
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
enum DataKey { PollStatus(u64) }

#[contractimpl]
impl VotingOracle {
    pub fn initialize(env: Env, super_admin: Address) -> Result<(), PredictXError> {
        if env.storage().instance().has(&SharedDataKey::SuperAdmin) {
            return Err(PredictXError::AlreadyInitialized);
        }
        super_admin.require_auth();
        env.storage().instance().set(&SharedDataKey::SuperAdmin, &super_admin);
        env.storage().instance().set(&SharedDataKey::OracleAddress, &super_admin);
        Ok(())
    }

    pub fn get_super_admin(env: Env) -> Result<Address, PredictXError> {
        shared_get_super_admin(&env)
    }

    pub fn oracle(env: Env) -> Result<Address, PredictXError> {
        shared_get_oracle(&env)
    }

    pub fn set_oracle(env: Env, super_admin: Address, oracle: Address) -> Result<(), PredictXError> {
        shared_set_oracle(&env, &super_admin, oracle)?;
        Ok(())
    }

    pub fn propose_super_admin_transfer(env: Env, super_admin: Address, new_super_admin: Address) -> Result<(), PredictXError> {
        propose_super_admin_transfer(&env, &super_admin, new_super_admin)
    }

    pub fn accept_super_admin_transfer(env: Env, pending_super_admin: Address) -> Result<(), PredictXError> {
        accept_super_admin_transfer(&env, &pending_super_admin)
    }

    /// Placeholder oracle state setter.
    ///
    /// This exists only to validate cross-contract invocation patterns during
    /// Phase 1 scaffolding.
    pub fn set_poll_status(env: Env, oracle: Address, poll_id: u64, status: PollStatus) -> Result<(), PredictXError> {
        require_oracle(&env, &oracle)?;

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

        client.set_poll_status(&admin, &42_u64, &PollStatus::Resolved);
        assert_eq!(client.get_poll_status(&42_u64), PollStatus::Resolved);
    }

    #[test]
    fn non_oracle_cannot_set_status() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);
        let stranger = Address::generate(&env);

        let err = client
            .try_set_poll_status(&stranger, &7_u64, &PollStatus::Cancelled)
            .expect_err("non-oracle should fail");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
    }

    #[test]
    fn two_step_super_admin_transfer_updates_control() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let super_admin = Address::generate(&env);
        let next_super_admin = Address::generate(&env);
        client.initialize(&super_admin);
        client.propose_super_admin_transfer(&super_admin, &next_super_admin);
        client.accept_super_admin_transfer(&next_super_admin);
        assert_eq!(client.get_super_admin(), next_super_admin);
    }
}
