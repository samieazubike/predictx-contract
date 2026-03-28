use crate::{Treasury, TreasuryClient};
use predictx_shared::PredictXError;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

#[test]
fn test_admin_requires_initialization() {
    let env = Env::default();
    let contract_id = env.register(Treasury, ());
    let client = TreasuryClient::new(&env, &contract_id);

    assert_eq!(
        client.try_admin().expect_err("admin should require init"),
        Ok(PredictXError::NotInitialized)
    );
}

#[test]
fn test_deposit_requires_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Treasury, ());
    let client = TreasuryClient::new(&env, &contract_id);
    let user = Address::generate(&env);

    assert_eq!(
        client
            .try_deposit(&user, &10_i128)
            .expect_err("deposit should require init"),
        Ok(PredictXError::NotInitialized)
    );
}

#[test]
fn test_deposit_rejects_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Treasury, ());
    let client = TreasuryClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(
        client
            .try_deposit(&user, &0_i128)
            .expect_err("zero deposit should fail"),
        Ok(PredictXError::StakeAmountZero)
    );
}

#[test]
fn test_balance_requires_initialization() {
    let env = Env::default();
    let contract_id = env.register(Treasury, ());
    let client = TreasuryClient::new(&env, &contract_id);
    let user = Address::generate(&env);

    assert_eq!(
        client
            .try_balance(&user)
            .expect_err("balance should require init"),
        Ok(PredictXError::NotInitialized)
    );
}
