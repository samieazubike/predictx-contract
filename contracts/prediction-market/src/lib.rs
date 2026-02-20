#![no_std]

use predictx_shared::{Error, PollStatus};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

mod voting_oracle {
    soroban_sdk::contractimport!(file = "wasm/voting_oracle.wasm");
}

fn map_oracle_poll_status(status: voting_oracle::PollStatus) -> PollStatus {
    match status {
        voting_oracle::PollStatus::Open => PollStatus::Open,
        voting_oracle::PollStatus::Locked => PollStatus::Locked,
        voting_oracle::PollStatus::Resolved => PollStatus::Resolved,
        voting_oracle::PollStatus::Disputed => PollStatus::Disputed,
        voting_oracle::PollStatus::Cancelled => PollStatus::Cancelled,
    }
}

#[contract]
pub struct PredictionMarket;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    VotingOracle,
}

fn get_admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
}

fn get_oracle(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::VotingOracle)
        .ok_or(Error::NotInitialized)
}

#[contractimpl]
impl PredictionMarket {
    pub fn initialize(env: Env, admin: Address, voting_oracle: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::VotingOracle, &voting_oracle);

        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, Error> {
        get_admin(&env)
    }

    pub fn oracle(env: Env) -> Result<Address, Error> {
        get_oracle(&env)
    }

    pub fn set_oracle(env: Env, voting_oracle: Address) -> Result<(), Error> {
        let admin = get_admin(&env)?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::VotingOracle, &voting_oracle);
        Ok(())
    }

    /// Minimal cross-contract invocation example.
    ///
    /// Calls into the `VotingOracle` contract to fetch the current status for a poll.
    pub fn oracle_poll_status(env: Env, poll_id: u64) -> Result<PollStatus, Error> {
        let oracle_id = get_oracle(&env)?;
        let client = voting_oracle::Client::new(&env, &oracle_id);
        Ok(map_oracle_poll_status(client.get_poll_status(&poll_id)))
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn initialize_sets_admin_and_oracle() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);

        client.initialize(&admin, &oracle);
        assert_eq!(client.admin(), admin);
        assert_eq!(client.oracle(), oracle);
    }

    #[test]
    fn initialize_is_one_time_only() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);

        client.initialize(&admin, &oracle);
        let err = client
            .try_initialize(&admin, &oracle)
            .expect_err("should fail");
        assert_eq!(err, Ok(Error::AlreadyInitialized));
    }

    #[test]
    fn cross_contract_oracle_call_works() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        // Register VotingOracle from WASM (imported via `contractimport!`).
        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);
        oracle_client.set_poll_status(&7_u64, &voting_oracle::PollStatus::Resolved);

        // Register PredictionMarket natively and point it at the oracle contract.
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        client.initialize(&admin, &oracle_id);

        let status = client.oracle_poll_status(&7_u64);
        assert_eq!(status, PollStatus::Resolved);
    }
}
