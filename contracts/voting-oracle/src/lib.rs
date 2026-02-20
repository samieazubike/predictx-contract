#![no_std]

use predictx_shared::{Error, PollStatus};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contract]
pub struct VotingOracle;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    PollStatus(u64),
}

fn get_admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
}

#[contractimpl]
impl VotingOracle {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, Error> {
        get_admin(&env)
    }

    /// Placeholder oracle state setter.
    ///
    /// This exists only to validate cross-contract invocation patterns during
    /// Phase 1 scaffolding.
    pub fn set_poll_status(env: Env, poll_id: u64, status: PollStatus) -> Result<(), Error> {
        let admin = get_admin(&env)?;
        admin.require_auth();

        env.storage()
            .persistent()
            .set(&DataKey::PollStatus(poll_id), &status);
        Ok(())
    }

    /// Placeholder oracle query used by `PredictionMarket`.
    pub fn get_poll_status(env: Env, poll_id: u64) -> PollStatus {
        env.storage()
            .persistent()
            .get(&DataKey::PollStatus(poll_id))
            .unwrap_or(PollStatus::Open)
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
