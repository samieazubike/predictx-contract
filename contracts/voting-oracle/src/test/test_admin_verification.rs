use crate::{VotingOracle, VotingOracleClient};
use predictx_shared::PredictXError;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

#[test]
fn test_admin_requires_initialization() {
    let env = Env::default();
    let contract_id = env.register(VotingOracle, ());
    let client = VotingOracleClient::new(&env, &contract_id);

    assert_eq!(
        client.try_admin().expect_err("admin should require init"),
        Ok(PredictXError::NotInitialized)
    );
}

#[test]
fn test_initialize_rejects_double_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(VotingOracle, ());
    let client = VotingOracleClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    assert_eq!(
        client
            .try_initialize(&admin)
            .expect_err("double initialize should fail"),
        Ok(PredictXError::AlreadyInitialized)
    );
}
