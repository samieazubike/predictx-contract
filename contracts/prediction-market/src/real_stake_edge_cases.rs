//! Real stake-path edge-case tests for issue #163.
//!
//! Existing cancel/emergency tests inject `DataKey::Stake` via
//! `env.as_contract` storage writes. These tests instead drive the public
//! `stake` entrypoint (SAC token transfer + stake record) and then exercise
//! `cancel_poll` / `emergency_withdraw` against that real stake.

use super::*;
use predictx_shared::{PollCategory, StakeSide};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::token;

const TEST_FEE_BPS: u32 = 500;
const STAKE_AMOUNT: i128 = 50_000_000; // 50 tokens (above MIN_STAKE_AMOUNT)

struct RealStakeEnv {
    env: Env,
    admin: Address,
    oracle_id: Address,
    oracle_client: voting_oracle::Client<'static>,
    token_addr: Address,
    contract_id: Address,
    client: PredictionMarketClient<'static>,
    poll_id: u64,
    staker: Address,
}

/// Full setup that creates a real SAC token, registers the market against it,
/// creates a poll through the public API, and places a stake via `client.stake`.
fn setup_real_stake() -> RealStakeEnv {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1_000_000);

    let admin = Address::generate(&env);
    let oracle_id = env.register(voting_oracle::WASM, ());
    let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
    oracle_client.initialize(&admin, &Address::generate(&env));

    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = token_contract.address();

    let contract_id = env.register(PredictionMarket, ());
    let client = PredictionMarketClient::new(&env, &contract_id);
    let treasury = Address::generate(&env);
    client.initialize(&admin, &oracle_id, &token_addr, &treasury, &TEST_FEE_BPS);

    // Allow the market contract to push oracle status updates on cancel.
    oracle_client.add_admin(&admin, &contract_id);

    let match_id = client.create_match(
        &admin,
        &String::from_str(&env, "Home"),
        &String::from_str(&env, "Away"),
        &String::from_str(&env, "League"),
        &String::from_str(&env, "Venue"),
        &2_000_u64,
    );
    let poll_id = client.create_poll(
        &admin,
        &match_id,
        &String::from_str(&env, "Will the home team win?"),
        &PollCategory::TeamEvent,
        &2_000_u64, // lock_time after current timestamp 1_000_000
    );

    let staker = Address::generate(&env);
    let sac = token::StellarAssetClient::new(&env, &token_addr);
    sac.mint(&staker, &(STAKE_AMOUNT * 3));
    sac.mint(&contract_id, &STAKE_AMOUNT); // pre-fund contract path edge cases

    // Place the stake through the real public entrypoint.
    client.stake(&staker, &poll_id, &STAKE_AMOUNT, &StakeSide::Yes);

    RealStakeEnv {
        env,
        admin,
        oracle_id,
        oracle_client,
        token_addr,
        contract_id,
        client,
        poll_id,
        staker,
    }
}

fn token_balance(env: &Env, token: &Address, who: &Address) -> i128 {
    token::Client::new(env, token).balance(who)
}

#[test]
fn cancel_poll_with_real_stake_marks_stake_claimable_via_emergency() {
    let s = setup_real_stake();
    let user_balance_before = token_balance(&s.env, &s.token_addr, &s.staker);
    let stake = s.client.get_stake_info(&s.poll_id, &s.staker);
    assert_eq!(stake.amount, STAKE_AMOUNT);
    assert!(!stake.claimed);
    assert_eq!(
        s.client.get_poll(&s.poll_id).status,
        PollStatus::Active
    );

    // Cancel through the public admin entrypoint (real state transition).
    s.client.cancel_poll(&s.admin, &s.poll_id);
    assert_eq!(
        s.client.get_poll(&s.poll_id).status,
        PollStatus::Cancelled
    );
    assert_eq!(
        s.oracle_client.get_poll_status(&s.poll_id),
        voting_oracle::PollStatus::Cancelled
    );

    // Stake record still exists and is unclaimed until emergency withdraw.
    let stake = s.client.get_stake_info(&s.poll_id, &s.staker);
    assert_eq!(stake.amount, STAKE_AMOUNT);
    assert!(!stake.claimed);

    // Emergency withdraw against the REAL stake (not a storage-injected one).
    let refunded = s.client.emergency_withdraw(&s.staker, &s.poll_id);
    assert_eq!(refunded, STAKE_AMOUNT);

    let stake = s.client.get_stake_info(&s.poll_id, &s.staker);
    assert!(stake.claimed, "emergency withdraw must mark the real stake claimed");

    let user_balance_after = token_balance(&s.env, &s.token_addr, &s.staker);
    assert_eq!(
        user_balance_after,
        user_balance_before + STAKE_AMOUNT,
        "tokens must return to the staker via the real transfer path"
    );
}

#[test]
fn emergency_withdraw_real_stake_rejects_double_claim() {
    let s = setup_real_stake();
    s.client.cancel_poll(&s.admin, &s.poll_id);

    let first = s.client.emergency_withdraw(&s.staker, &s.poll_id);
    assert_eq!(first, STAKE_AMOUNT);

    let err = s
        .client
        .try_emergency_withdraw(&s.staker, &s.poll_id)
        .expect_err("second emergency withdraw must fail");
    assert_eq!(err, Ok(PredictXError::AlreadyClaimed));
}

#[test]
fn emergency_withdraw_real_stake_rejects_when_poll_still_active() {
    let s = setup_real_stake();

    // Poll is still Active — emergency withdraw is not allowed yet.
    let err = s
        .client
        .try_emergency_withdraw(&s.staker, &s.poll_id)
        .expect_err("active poll must block emergency withdraw");
    assert_eq!(err, Ok(PredictXError::EmergencyWithdrawNotAllowed));

    // Real stake is untouched.
    let stake = s.client.get_stake_info(&s.poll_id, &s.staker);
    assert!(!stake.claimed);
    assert_eq!(stake.amount, STAKE_AMOUNT);
}

#[test]
fn emergency_withdraw_real_stake_rejects_non_staker() {
    let s = setup_real_stake();
    s.client.cancel_poll(&s.admin, &s.poll_id);

    let stranger = Address::generate(&s.env);
    let err = s
        .client
        .try_emergency_withdraw(&stranger, &s.poll_id)
        .expect_err("non-staker cannot emergency-withdraw");
    assert_eq!(err, Ok(PredictXError::NotStaker));
}

#[test]
fn cancel_poll_with_real_stake_rejects_non_admin() {
    let s = setup_real_stake();
    let stranger = Address::generate(&s.env);
    let err = s
        .client
        .try_cancel_poll(&stranger, &s.poll_id)
        .expect_err("non-admin cannot cancel");
    assert_eq!(err, Ok(PredictXError::Unauthorized));

    // Stake still Active and unclaimed.
    assert_eq!(
        s.client.get_poll(&s.poll_id).status,
        PollStatus::Active
    );
    let stake = s.client.get_stake_info(&s.poll_id, &s.staker);
    assert!(!stake.claimed);
}

#[test]
fn real_stake_rejects_second_stake_on_same_poll() {
    let s = setup_real_stake();
    let err = s
        .client
        .try_stake(&s.staker, &s.poll_id, &STAKE_AMOUNT, &StakeSide::No)
        .expect_err("duplicate stake on same poll must fail");
    assert_eq!(err, Ok(PredictXError::AlreadyStaked));
}

#[test]
fn real_stake_rejects_amount_below_minimum() {
    let s = setup_real_stake();
    let err = s
        .client
        .try_stake(&s.staker, &s.poll_id, &1_i128, &StakeSide::Yes)
        .expect_err("dust stake must fail");
    assert_eq!(err, Ok(PredictXError::StakeBelowMinimum));
}