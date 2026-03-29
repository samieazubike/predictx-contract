#![no_std]

use predictx_shared::{
    add_admin as shared_add_admin, get_admins as shared_get_admins,
    get_super_admin as shared_get_super_admin, is_admin as shared_is_admin,
    remove_admin as shared_remove_admin,
    DataKey as SharedDataKey, PredictXError, Poll, PollCategory, PollStatus,
};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[contract]
pub struct PollFactory;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    NextPollId,
    Poll(u64),
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    shared_get_super_admin(env)
}

fn next_poll_id(env: &Env) -> u64 {
    env.storage().instance().get(&DataKey::NextPollId).unwrap_or(1)
}

fn bump_poll_id(env: &Env) -> u64 {
    let id = next_poll_id(env);
    env.storage().instance().set(&DataKey::NextPollId, &(id + 1));
    id
}

#[contractimpl]
impl PollFactory {
    pub fn initialize(env: Env, admin: Address) -> Result<(), PredictXError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&SharedDataKey::SuperAdmin, &admin);
        env.storage().instance().set(&SharedDataKey::AdminList, &soroban_sdk::Vec::<Address>::new(&env));
        env.storage().instance().set(&DataKey::NextPollId, &1_u64);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, PredictXError> {
        get_admin(&env)
    }

    pub fn get_super_admin(env: Env) -> Result<Address, PredictXError> {
        shared_get_super_admin(&env)
    }

    pub fn get_admins(env: Env) -> soroban_sdk::Vec<Address> {
        shared_get_admins(&env)
    }

    pub fn is_admin(env: Env, address: Address) -> Result<bool, PredictXError> {
        shared_is_admin(&env, &address)
    }

    pub fn add_admin(env: Env, super_admin: Address, new_admin: Address) -> Result<(), PredictXError> {
        shared_add_admin(&env, &super_admin, new_admin)
    }

    pub fn remove_admin(env: Env, super_admin: Address, admin_to_remove: Address) -> Result<(), PredictXError> {
        shared_remove_admin(&env, &super_admin, admin_to_remove)
    }

    pub fn create_poll(
        env: Env,
        creator: Address,
        question: String,
        lock_timestamp: u64,
    ) -> Result<u64, PredictXError> {
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::NotInitialized);
        }
        creator.require_auth();

        let poll_id = bump_poll_id(&env);
        let poll = Poll {
            poll_id,
    			match_id: 0,                          // polls not linked to matches yet
    			creator: creator.clone(),
				question,
				category: PollCategory::Other,
				lock_time: lock_timestamp,
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
        Ok(poll_id)
    }

    pub fn get_poll(env: Env, poll_id: u64) -> Result<Poll, PredictXError> {
        env.storage()
            .persistent()
            .get(&DataKey::Poll(poll_id))
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
    fn create_and_get_poll() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(PollFactory, ());
        let client = PollFactoryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let creator = Address::generate(&env);
        let question = String::from_str(&env, "Will Palmer score?");

        let poll_id = client.create_poll(&creator, &question, &123_u64);
        let poll = client.get_poll(&poll_id);

        assert_eq!(poll.poll_id, poll_id);
        assert_eq!(poll.creator, creator);
        assert_eq!(poll.question, question);
        assert_eq!(poll.status, PollStatus::Active);
        assert_eq!(poll.lock_time, 123_u64);
    }

    #[test]
    fn only_super_admin_can_manage_admins() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PollFactory, ());
        let client = PollFactoryClient::new(&env, &contract_id);

        let super_admin = Address::generate(&env);
        client.initialize(&super_admin);
        let attacker = Address::generate(&env);
        let admin = Address::generate(&env);

        let err = client
            .try_add_admin(&attacker, &admin)
            .expect_err("only super admin can add");
        assert_eq!(err, Ok(PredictXError::Unauthorized));
        client.add_admin(&super_admin, &admin);
        assert!(client.is_admin(&admin));
    }
}
