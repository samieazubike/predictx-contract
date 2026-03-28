use crate::{PollFactory, PollFactoryClient};
use predictx_shared::{PollStatus, PredictXError};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, String};

#[test]
fn test_create_poll_requires_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PollFactory, ());
    let client = PollFactoryClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let question = String::from_str(&env, "Will Arsenal win?");

    assert_eq!(
        client
            .try_create_poll(&creator, &question, &123_u64)
            .expect_err("create_poll should require init"),
        Ok(PredictXError::NotInitialized)
    );
}

#[test]
fn test_get_poll_rejects_missing_id() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PollFactory, ());
    let client = PollFactoryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(
        client.try_get_poll(&99_u64).expect_err("missing poll should fail"),
        Ok(PredictXError::PollNotFound)
    );
}

#[test]
fn test_create_poll_increments_identifiers() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(222);
    let contract_id = env.register(PollFactory, ());
    let client = PollFactoryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let creator = Address::generate(&env);
    client.initialize(&admin);

    let first = client.create_poll(&creator, &String::from_str(&env, "One"), &500_u64);
    let second = client.create_poll(&creator, &String::from_str(&env, "Two"), &600_u64);

    assert_eq!(first, 1);
    assert_eq!(second, 2);
    assert_eq!(client.get_poll(&second).status, PollStatus::Active);
    assert_eq!(client.get_poll(&second).created_at, 222);
}

#[test]
fn test_initialize_rejects_double_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PollFactory, ());
    let client = PollFactoryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(
        client
            .try_initialize(&admin)
            .expect_err("double initialize should fail"),
        Ok(PredictXError::AlreadyInitialized)
    );
}
