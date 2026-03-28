use crate::{VotingOracle, VotingOracleClient};
use predictx_shared::{PollStatus, PredictXError};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env};

#[test]
fn test_get_poll_status_defaults_to_active() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VotingOracle, ());
    let client = VotingOracleClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(client.get_poll_status(&77_u64), PollStatus::Active);
    assert_eq!(client.get_poll_status_updated_at(&77_u64), 0);
}

#[test]
fn test_set_poll_status_updates_timestamp() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_234_567);

    let contract_id = env.register(VotingOracle, ());
    let client = VotingOracleClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    client.set_poll_status(&5_u64, &PollStatus::Resolved);

    assert_eq!(client.get_poll_status(&5_u64), PollStatus::Resolved);
    assert_eq!(client.get_poll_status_updated_at(&5_u64), 1_234_567);
}

#[test]
fn test_set_poll_status_requires_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VotingOracle, ());
    let client = VotingOracleClient::new(&env, &contract_id);

    assert_eq!(
        client
            .try_set_poll_status(&1_u64, &PollStatus::Locked)
            .expect_err("status set should require init"),
        Ok(PredictXError::NotInitialized)
    );
}
