use crate::{voting_oracle, PredictionMarket, PredictionMarketClient};
use predictx_shared::{PollCategory, PredictXError};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, String};

const TEST_FEE_BPS: u32 = 500;

fn s(env: &Env, value: &str) -> String {
    String::from_str(env, value)
}

fn setup() -> (Env, PredictionMarketClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let admin = Address::generate(&env);
    let oracle_id = env.register(voting_oracle::WASM, ());
    let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
    oracle_client.initialize(&admin);

    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin);
    let treasury = Address::generate(&env);

    let contract_id = env.register(PredictionMarket, ());
    let client = PredictionMarketClient::new(&env, &contract_id);
    client.initialize(&admin, &oracle_id, &token.address(), &treasury, &TEST_FEE_BPS);

    (env, client, admin, oracle_id)
}

fn create_match(client: &PredictionMarketClient<'_>, env: &Env, admin: &Address) -> u64 {
    client.create_match(
        admin,
        &s(env, "Arsenal"),
        &s(env, "Chelsea"),
        &s(env, "Premier League"),
        &s(env, "Emirates"),
        &1_200_000,
    )
}

#[test]
fn test_pause_rejects_non_admin() {
    let (env, client, _admin, _oracle_id) = setup();
    let attacker = Address::generate(&env);

    assert_eq!(
        client.try_pause(&attacker).expect_err("non-admin pause should fail"),
        Ok(PredictXError::Unauthorized)
    );
}

#[test]
fn test_cancel_poll_rejects_non_admin() {
    let (env, client, admin, _oracle_id) = setup();
    let match_id = create_match(&client, &env, &admin);
    let poll_id = client.create_poll(
        &admin,
        &match_id,
        &s(&env, "Will Arsenal win?"),
        &PollCategory::TeamEvent,
        &1_100_000,
    );
    let attacker = Address::generate(&env);

    assert_eq!(
        client
            .try_cancel_poll(&attacker, &poll_id)
            .expect_err("non-admin cancel should fail"),
        Ok(PredictXError::Unauthorized)
    );
}

#[test]
fn test_set_oracle_rejects_when_paused() {
    let (env, client, admin, _oracle_id) = setup();
    let new_oracle_admin = Address::generate(&env);
    let new_oracle = env.register(voting_oracle::WASM, ());
    let new_oracle_client = voting_oracle::Client::new(&env, &new_oracle);
    new_oracle_client.initialize(&new_oracle_admin);

    client.pause(&admin);

    assert_eq!(
        client
            .try_set_oracle(&new_oracle)
            .expect_err("set_oracle should fail while paused"),
        Ok(PredictXError::EmergencyWithdrawNotAllowed)
    );
}
