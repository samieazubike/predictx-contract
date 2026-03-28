use crate::{voting_oracle, PredictionMarket, PredictionMarketClient};
use predictx_shared::{PredictXError, Stake, StakeSide};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, Address, Env};

const TEST_FEE_BPS: u32 = 500;
const EMERGENCY_TIMEOUT_SECS: u64 = 7 * 24 * 60 * 60;

fn setup() -> (Env, Address, Address, Address, PredictionMarketClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
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

    (env, admin, oracle_id, contract_id, client)
}

fn mint(env: &Env, token_addr: &Address, to: &Address, amount: i128) {
    let client = token::StellarAssetClient::new(env, token_addr);
    client.mint(to, &amount);
}

#[test]
fn test_check_emergency_eligible_returns_false_for_unknown_poll() {
    let (env, admin, oracle_id, _contract_id, client) = setup();
    let oracle_client = voting_oracle::Client::new(&env, &oracle_id);

    oracle_client.set_poll_status(&88_u64, &predictx_shared::PollStatus::Voting);
    assert!(!client.check_emergency_eligible(&88_u64));

    client.pause(&admin);
    assert!(!client.check_emergency_eligible(&999_u64));
}

#[test]
fn test_emergency_withdraw_rejects_user_without_stake() {
    let (env, _admin, oracle_id, _contract_id, client) = setup();
    let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
    let user = Address::generate(&env);

    env.ledger().set_timestamp(200);
    oracle_client.set_poll_status(&7_u64, &predictx_shared::PollStatus::Disputed);
    env.ledger().set_timestamp(200 + EMERGENCY_TIMEOUT_SECS + 1);

    assert_eq!(
        client
            .try_emergency_withdraw(&user, &7_u64)
            .expect_err("missing stake should fail"),
        Ok(PredictXError::NotStaker)
    );
}

#[test]
fn test_emergency_withdraw_reduces_contract_balance() {
    let (env, admin, oracle_id, contract_id, client) = setup();
    let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
    let token_addr = client.get_token_address();
    let user = Address::generate(&env);
    let amount = 25_000_000_i128;

    mint(&env, &token_addr, &contract_id, amount);

    let stake = Stake {
        user: user.clone(),
        poll_id: 11,
        amount,
        side: StakeSide::Yes,
        claimed: false,
        staked_at: 0,
    };

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&crate::DataKey::Stake(11, user.clone()), &stake);
    });

    client.cancel_poll(&admin, &11_u64);
    assert_eq!(client.get_contract_balance(), amount);
    assert_eq!(client.emergency_withdraw(&user, &11_u64), amount);
    assert_eq!(client.get_contract_balance(), 0);

    let token_client = token::Client::new(&env, &token_addr);
    assert_eq!(token_client.balance(&user), amount);
    assert_eq!(oracle_client.get_poll_status(&11_u64), predictx_shared::PollStatus::Cancelled);
}
