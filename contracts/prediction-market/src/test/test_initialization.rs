use crate::{voting_oracle, PredictionMarket, PredictionMarketClient};
use predictx_shared::PredictXError;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

const TEST_FEE_BPS: u32 = 500;

fn setup_client(env: &Env) -> (Address, PredictionMarketClient<'_>, Address, Address, Address, Address) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let oracle_id = env.register(voting_oracle::WASM, ());
    let oracle_client = voting_oracle::Client::new(env, &oracle_id);
    oracle_client.initialize(&admin);
    let token_admin = Address::generate(env);
    let token = env.register_stellar_asset_contract_v2(token_admin);
    let treasury = Address::generate(env);
    let contract_id = env.register(PredictionMarket, ());
    let client = PredictionMarketClient::new(env, &contract_id);
    (contract_id, client, admin, oracle_id, token.address(), treasury)
}

#[test]
fn test_admin_and_oracle_require_initialization() {
    let env = Env::default();
    let contract_id = env.register(PredictionMarket, ());
    let client = PredictionMarketClient::new(&env, &contract_id);

    assert_eq!(
        client.try_admin().expect_err("admin should require init"),
        Ok(PredictXError::NotInitialized)
    );
    assert_eq!(
        client.try_oracle().expect_err("oracle should require init"),
        Ok(PredictXError::NotInitialized)
    );
}

#[test]
fn test_initialize_sets_token_and_fee_configuration() {
    let env = Env::default();
    let (_contract_id, client, admin, oracle_id, token, treasury) = setup_client(&env);

    client.initialize(&admin, &oracle_id, &token, &treasury, &TEST_FEE_BPS);

    assert_eq!(client.admin(), admin);
    assert_eq!(client.oracle(), oracle_id);
    assert_eq!(client.get_token_address(), token);
    assert_eq!(client.get_treasury_address(), treasury);
    assert_eq!(client.get_platform_fee_bps(), TEST_FEE_BPS);
}

#[test]
fn test_initialize_rejects_double_initialization() {
    let env = Env::default();
    let (_contract_id, client, admin, oracle_id, token, treasury) = setup_client(&env);

    client.initialize(&admin, &oracle_id, &token, &treasury, &TEST_FEE_BPS);

    assert_eq!(
        client
            .try_initialize(&admin, &oracle_id, &token, &treasury, &TEST_FEE_BPS)
            .expect_err("second initialize should fail"),
        Ok(PredictXError::AlreadyInitialized)
    );
}

#[test]
fn test_token_views_fail_before_initialization() {
    let env = Env::default();
    let contract_id = env.register(PredictionMarket, ());
    let client = PredictionMarketClient::new(&env, &contract_id);

    assert_eq!(
        client
            .try_get_token_address()
            .expect_err("token should require init"),
        Ok(PredictXError::NotInitialized)
    );
    assert_eq!(
        client
            .try_get_treasury_address()
            .expect_err("treasury should require init"),
        Ok(PredictXError::NotInitialized)
    );
    assert_eq!(
        client
            .try_get_contract_balance()
            .expect_err("balance should require init"),
        Ok(PredictXError::NotInitialized)
    );
}
