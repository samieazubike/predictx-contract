use crate::{PredictionMarket, PredictionMarketClient};
use predictx_shared::{PollCategory, PollStatus, PredictXError, MAX_POLLS_PER_MATCH};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, String};

const TEST_FEE_BPS: u32 = 500;

fn s(env: &Env, value: &str) -> String {
    String::from_str(env, value)
}

fn setup() -> (Env, PredictionMarketClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_000_000);

    let admin = Address::generate(&env);
    let oracle = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    let contract_id = env.register(PredictionMarket, ());
    let client = PredictionMarketClient::new(&env, &contract_id);
    client.initialize(&admin, &oracle, &token, &treasury, &TEST_FEE_BPS);

    (env, client, admin)
}

fn create_match(client: &PredictionMarketClient<'_>, env: &Env, admin: &Address) -> u64 {
    client.create_match(
        admin,
        &s(env, "Arsenal"),
        &s(env, "Chelsea"),
        &s(env, "Premier League"),
        &s(env, "Emirates"),
        &1_300_000,
    )
}

#[test]
fn test_create_poll_supports_all_categories() {
    let (env, client, admin) = setup();
    let match_id = create_match(&client, &env, &admin);
    let categories = [
        PollCategory::PlayerEvent,
        PollCategory::TeamEvent,
        PollCategory::ScorePrediction,
        PollCategory::Other,
    ];

    for (index, category) in categories.into_iter().enumerate() {
        let poll_id = client.create_poll(
            &admin,
            &match_id,
            &s(&env, "Question"),
            &category,
            &(1_100_000 + index as u64),
        );
        assert_eq!(client.get_poll(&poll_id).category, category);
    }
}

#[test]
fn test_create_poll_rejects_past_lock_time() {
    let (env, client, admin) = setup();
    let match_id = create_match(&client, &env, &admin);

    assert_eq!(
        client
            .try_create_poll(
                &admin,
                &match_id,
                &s(&env, "Late poll"),
                &PollCategory::Other,
                &999_999_u64
            )
            .expect_err("past lock time should fail"),
        Ok(PredictXError::InvalidLockTime)
    );
}

#[test]
fn test_create_poll_rejects_missing_match() {
    let (env, client, admin) = setup();

    assert_eq!(
        client
            .try_create_poll(
                &admin,
                &999_u64,
                &s(&env, "Missing match"),
                &PollCategory::Other,
                &1_100_000_u64
            )
            .expect_err("missing match should fail"),
        Ok(PredictXError::MatchNotFound)
    );
}

#[test]
fn test_create_poll_enforces_max_per_match_limit() {
    let (env, client, admin) = setup();
    let match_id = create_match(&client, &env, &admin);

    for offset in 0..MAX_POLLS_PER_MATCH {
        client.create_poll(
            &admin,
            &match_id,
            &s(&env, "Question"),
            &PollCategory::Other,
            &(1_100_000 + offset as u64),
        );
    }

    assert_eq!(
        client
            .try_create_poll(
                &admin,
                &match_id,
                &s(&env, "Overflow"),
                &PollCategory::Other,
                &1_200_000_u64
            )
            .expect_err("extra poll should fail"),
        Ok(PredictXError::MaxPollsPerMatchReached)
    );
}

#[test]
fn test_get_poll_rejects_missing_id() {
    let (_env, client, _admin) = setup();
    assert_eq!(
        client.try_get_poll(&404_u64).expect_err("missing poll should fail"),
        Ok(PredictXError::PollNotFound)
    );
}

#[test]
fn test_new_poll_starts_active_and_tracks_match_membership() {
    let (env, client, admin) = setup();
    let match_id = create_match(&client, &env, &admin);
    let poll_id = client.create_poll(
        &admin,
        &match_id,
        &s(&env, "Will the match exceed 2.5 goals?"),
        &PollCategory::ScorePrediction,
        &1_100_000,
    );

    let poll = client.get_poll(&poll_id);
    let match_polls = client.get_match_polls(&match_id);

    assert_eq!(poll.status, PollStatus::Active);
    assert_eq!(poll.match_id, match_id);
    assert_eq!(match_polls.len(), 1);
    assert_eq!(match_polls.get(0).unwrap(), poll_id);
}
