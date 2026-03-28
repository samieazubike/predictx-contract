use crate::{PredictionMarket, PredictionMarketClient};
use predictx_shared::{PollCategory, PredictXError, StakeSide, MIN_STAKE_AMOUNT};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, Env, String};

const TEST_FEE_BPS: u32 = 500;

fn s(env: &Env, value: &str) -> String {
    String::from_str(env, value)
}

struct Setup<'a> {
    env: Env,
    token: Address,
    contract_id: Address,
    admin: Address,
    client: PredictionMarketClient<'a>,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin);
    let oracle = Address::generate(&env);
    let treasury = Address::generate(&env);
    let contract_id = env.register(PredictionMarket, ());
    let client = PredictionMarketClient::new(&env, &contract_id);
    client.initialize(&admin, &oracle, &token.address(), &treasury, &TEST_FEE_BPS);

    Setup {
        env,
        token: token.address(),
        contract_id,
        admin,
        client,
    }
}

fn create_poll(setup: &Setup<'_>) -> u64 {
    let match_id = setup.client.create_match(
        &setup.admin,
        &s(&setup.env, "Arsenal"),
        &s(&setup.env, "Chelsea"),
        &s(&setup.env, "Premier League"),
        &s(&setup.env, "Emirates"),
        &1_300_000,
    );
    setup.client.create_poll(
        &setup.admin,
        &match_id,
        &s(&setup.env, "Will Arsenal score first?"),
        &PollCategory::TeamEvent,
        &1_100_000,
    )
}

fn mint(env: &Env, token: &Address, to: &Address, amount: i128) {
    let client = token::StellarAssetClient::new(env, token);
    client.mint(to, &amount);
}

#[test]
fn test_get_stake_info_rejects_non_staker() {
    let setup = setup();
    let poll_id = create_poll(&setup);
    let user = Address::generate(&setup.env);

    assert_eq!(
        setup
            .client
            .try_get_stake_info(&poll_id, &user)
            .expect_err("missing stake should fail"),
        Ok(PredictXError::NotStaker)
    );
}

#[test]
fn test_get_pool_info_rejects_missing_poll() {
    let setup = setup();

    assert_eq!(
        setup
            .client
            .try_get_pool_info(&999_u64)
            .expect_err("missing poll should fail"),
        Ok(PredictXError::PollNotFound)
    );
}

#[test]
fn test_stake_records_history_and_balance_changes() {
    let setup = setup();
    let poll_id = create_poll(&setup);
    let user = Address::generate(&setup.env);
    let amount = MIN_STAKE_AMOUNT * 3;
    mint(&setup.env, &setup.token, &user, amount);

    setup.client.stake(&user, &poll_id, &amount, &StakeSide::Yes);

    let user_stakes = setup.client.get_user_stakes(&user);
    let balance = token::Client::new(&setup.env, &setup.token).balance(&setup.contract_id);
    let pool = setup.client.get_pool_info(&poll_id);

    assert_eq!(user_stakes.len(), 1);
    assert_eq!(user_stakes.get(0).unwrap(), poll_id);
    assert_eq!(balance, amount);
    assert_eq!(pool.yes_pool, amount);
    assert_eq!(pool.yes_count, 1);
}

#[test]
fn test_calculate_potential_winnings_rejects_missing_poll() {
    let setup = setup();
    assert_eq!(
        setup
            .client
            .try_calculate_potential_winnings(&999_u64, &StakeSide::Yes, &MIN_STAKE_AMOUNT)
            .expect_err("missing poll should fail"),
        Ok(PredictXError::PollNotFound)
    );
}
