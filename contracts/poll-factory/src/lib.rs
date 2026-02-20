#![no_std]

use predictx_shared::{Error, Poll, PollStatus};
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

fn get_admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
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
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextPollId, &1_u64);
        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, Error> {
        get_admin(&env)
    }

    pub fn create_poll(
        env: Env,
        creator: Address,
        question: String,
        lock_timestamp: u64,
    ) -> Result<u64, Error> {
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::NotInitialized);
        }
        creator.require_auth();

        let poll_id = bump_poll_id(&env);
        let poll = Poll {
            id: poll_id,
            creator,
            question,
            status: PollStatus::Open,
            lock_timestamp,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Poll(poll_id), &poll);
        Ok(poll_id)
    }

    pub fn get_poll(env: Env, poll_id: u64) -> Result<Poll, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Poll(poll_id))
            .ok_or(Error::PollNotFound)
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

        assert_eq!(poll.id, poll_id);
        assert_eq!(poll.creator, creator);
        assert_eq!(poll.question, question);
        assert_eq!(poll.status, PollStatus::Open);
        assert_eq!(poll.lock_timestamp, 123_u64);
    }
}
