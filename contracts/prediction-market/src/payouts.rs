use predictx_shared::{
    Poll, PollStatus, PredictXError, Stake, StakeSide, BPS_DENOMINATOR,
};
use soroban_sdk::{Address, Env, Symbol};

use crate::{
    get_platform_stats, set_platform_stats, token_utils, DataKey,
};

pub fn resolve_poll(
    env: &Env,
    admin: Address,
    poll_id: u64,
    outcome: bool,
) -> Result<(), PredictXError> {
    admin.require_auth();
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)?;
    if admin != stored_admin {
        return Err(PredictXError::Unauthorized);
    }

    let mut poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;

    if poll.status == PollStatus::Resolved {
        return Err(PredictXError::PollAlreadyResolved);
    }

    poll.status = PollStatus::Resolved;
    poll.outcome = Some(outcome);
    poll.resolution_time = env.ledger().timestamp();
    env.storage()
        .persistent()
        .set(&DataKey::Poll(poll_id), &poll);

    let total_pool = poll.yes_pool + poll.no_pool;
    let fee = total_pool * token_utils::get_platform_fee_bps(env) as i128
        / BPS_DENOMINATOR as i128;

    env.events().publish(
        (Symbol::new(env, "PollResolved"), poll_id),
        (outcome, total_pool, fee),
    );

    Ok(())
}

/// Whether the platform fee for `poll_id` has already been sent to the
/// treasury. The marker is per poll, not per claim, so the second winner to
/// claim cannot pay the fee twice.
fn has_fee_paid(env: &Env, poll_id: u64) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::FeePaid(poll_id))
        .unwrap_or(false)
}

fn set_fee_paid(env: &Env, poll_id: u64) {
    env.storage()
        .persistent()
        .set(&DataKey::FeePaid(poll_id), &true);
}

/// Move the platform fee for `poll_id` to the treasury on the first claim and
/// remember that it happened. Returns the fee so callers can report the same
/// distributable pot on every later claim.
pub fn ensure_platform_fee_routed(
    env: &Env,
    poll_id: u64,
    total_pool: i128,
) -> Result<i128, PredictXError> {
    let fee = total_pool * token_utils::get_platform_fee_bps(env) as i128
        / BPS_DENOMINATOR as i128;

    if has_fee_paid(env, poll_id) {
        return Ok(fee);
    }

    if fee > 0 {
        token_utils::transfer_to_treasury(env, fee)?;
    }
    set_fee_paid(env, poll_id);
    Ok(fee)
}

pub fn claim_winnings(
    env: &Env,
    user: Address,
    poll_id: u64,
) -> Result<i128, PredictXError> {
    user.require_auth();

    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;
    if poll.status != PollStatus::Resolved {
        return Err(PredictXError::PollNotLocked);
    }

    let mut stake: Stake = env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, user.clone()))
        .ok_or(PredictXError::NotStaker)?;
    if stake.claimed {
        return Err(PredictXError::AlreadyClaimed);
    }

    let amount = calculate_winnings_for(&poll, &stake, token_utils::get_platform_fee_bps(env))?;
    if amount <= 0 {
        return Err(PredictXError::NotOnWinningSide);
    }

    // Skim the platform fee before paying anyone out: `calculate_winnings_for`
    // already excludes it from `amount`, so without this transfer the fee would
    // simply stay stranded in the contract.
    ensure_platform_fee_routed(env, poll_id, poll.yes_pool + poll.no_pool)?;

    stake.claimed = true;
    env.storage()
        .persistent()
        .set(&DataKey::Stake(poll_id, user.clone()), &stake);

    token_utils::transfer_from_contract(env, &user, amount)?;

    let mut stats = get_platform_stats(env);
    stats.total_value_locked -= amount;
    stats.total_payouts += amount;
    set_platform_stats(env, &stats);

    env.events().publish(
        (Symbol::new(env, "WinningsClaimed"), poll_id, user),
        amount,
    );

    Ok(amount)
}

pub fn calculate_winnings(
    env: &Env,
    poll_id: u64,
    user: Address,
) -> Result<i128, PredictXError> {
    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;
    let stake: Stake = env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, user))
        .ok_or(PredictXError::NotStaker)?;

    calculate_winnings_for(&poll, &stake, token_utils::get_platform_fee_bps(env))
}

fn calculate_winnings_for(
    poll: &Poll,
    stake: &Stake,
    fee_bps: u32,
) -> Result<i128, PredictXError> {
    if poll.status != PollStatus::Resolved {
        return Err(PredictXError::PollNotLocked);
    }

    let outcome = poll.outcome.ok_or(PredictXError::InvalidOutcome)?;
    let winning_side = if outcome { StakeSide::Yes } else { StakeSide::No };
    if stake.side != winning_side {
        return Ok(0);
    }

    let winning_pool = if outcome { poll.yes_pool } else { poll.no_pool };
    if winning_pool <= 0 {
        return Ok(0);
    }

    let total_pool = poll.yes_pool + poll.no_pool;
    let payout_pool = total_pool * (BPS_DENOMINATOR - fee_bps) as i128
        / BPS_DENOMINATOR as i128;

    Ok(stake.amount * payout_pool / winning_pool)
}

#[cfg(test)]
mod test {
    extern crate std;

    use predictx_shared::{PollCategory, PredictXError, StakeSide};
    use soroban_sdk::{
        testutils::{Address as _, Events, Ledger},
        token, Address, Env, String, Symbol, TryIntoVal,
    };

    use crate::{PredictionMarket, PredictionMarketClient};

    struct TestSetup<'a> {
        env: Env,
        admin: Address,
        token_addr: Address,
        client: PredictionMarketClient<'a>,
    }

    fn setup() -> TestSetup<'static> {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let oracle_id = env.register(crate::voting_oracle::WASM, ());
        let oracle_client = crate::voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin);
        let token_addr = token_contract.address();

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle_id, &token_addr, &treasury, &500_u32);
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        TestSetup { env, admin, token_addr, client }
    }

    fn create_poll(s: &TestSetup, lock_time: u64) -> u64 {
        let match_id = s.client.create_match(
            &s.admin,
            &String::from_str(&s.env, "Arsenal"),
            &String::from_str(&s.env, "Chelsea"),
            &String::from_str(&s.env, "Premier League"),
            &String::from_str(&s.env, "Emirates"),
            &(lock_time + 3600),
        );
        s.client.create_poll(
            &s.admin,
            &match_id,
            &String::from_str(&s.env, "Will Palmer score?"),
            &PollCategory::PlayerEvent,
            &lock_time,
        )
    }

    fn mint_tokens(s: &TestSetup, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&s.env, &s.token_addr).mint(to, &amount);
    }

    fn stake_user(
        s: &TestSetup,
        poll_id: u64,
        side: StakeSide,
        amount: i128,
    ) -> Address {
        let user = Address::generate(&s.env);
        mint_tokens(s, &user, amount);
        s.client.stake(&user, &poll_id, &amount, &side);
        user
    }

    #[test]
    fn resolve_poll_emits_poll_resolved_event() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        stake_user(&s, poll_id, StakeSide::Yes, 100_000_000);
        stake_user(&s, poll_id, StakeSide::No, 300_000_000);

        s.client.resolve_poll(&s.admin, &poll_id, &true);

        let events = s.env.events().all();
        let (_, topics, data) = events.get(events.len() - 1).unwrap();
        let name: Symbol = topics.get(0).unwrap().try_into_val(&s.env).unwrap();
        let topic_poll_id: u64 = topics.get(1).unwrap().try_into_val(&s.env).unwrap();
        let payload: (bool, i128, i128) = data.try_into_val(&s.env).unwrap();

        assert_eq!(name, Symbol::new(&s.env, "PollResolved"));
        assert_eq!(topic_poll_id, poll_id);
        assert_eq!(payload, (true, 400_000_000, 20_000_000));
    }

    #[test]
    fn claim_winnings_emits_event_after_successful_transfer() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let winner = stake_user(&s, poll_id, StakeSide::Yes, 100_000_000);
        stake_user(&s, poll_id, StakeSide::No, 300_000_000);
        s.client.resolve_poll(&s.admin, &poll_id, &true);

        let claimed = s.client.claim_winnings(&winner, &poll_id);

        let events = s.env.events().all();
        let (_, topics, data) = events.get(events.len() - 1).unwrap();
        let name: Symbol = topics.get(0).unwrap().try_into_val(&s.env).unwrap();
        let topic_poll_id: u64 = topics.get(1).unwrap().try_into_val(&s.env).unwrap();
        let topic_user: Address = topics.get(2).unwrap().try_into_val(&s.env).unwrap();
        let amount: i128 = data.try_into_val(&s.env).unwrap();

        assert_eq!(claimed, 380_000_000);
        assert_eq!(name, Symbol::new(&s.env, "WinningsClaimed"));
        assert_eq!(topic_poll_id, poll_id);
        assert_eq!(topic_user, winner);
        assert_eq!(amount, claimed);

        let token_client = token::Client::new(&s.env, &s.token_addr);
        assert_eq!(token_client.balance(&winner), claimed);
    }

    #[test]
    fn successful_claim_marks_stake_as_claimed() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let winner = stake_user(&s, poll_id, StakeSide::Yes, 100_000_000);
        stake_user(&s, poll_id, StakeSide::No, 300_000_000);
        s.client.resolve_poll(&s.admin, &poll_id, &true);

        s.client.claim_winnings(&winner, &poll_id);

        let stake = s.client.get_stake_info(&poll_id, &winner);
        assert!(stake.claimed);
    }

    #[test]
    fn second_claim_returns_already_claimed() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let winner = stake_user(&s, poll_id, StakeSide::Yes, 100_000_000);
        stake_user(&s, poll_id, StakeSide::No, 300_000_000);
        s.client.resolve_poll(&s.admin, &poll_id, &true);
        s.client.claim_winnings(&winner, &poll_id);

        let err = s
            .client
            .try_claim_winnings(&winner, &poll_id)
            .expect_err("second claim should fail")
            .unwrap();

        assert_eq!(err, PredictXError::AlreadyClaimed);
    }

    #[test]
    fn second_claim_does_not_change_contract_balance() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let winner = stake_user(&s, poll_id, StakeSide::Yes, 100_000_000);
        stake_user(&s, poll_id, StakeSide::No, 300_000_000);
        s.client.resolve_poll(&s.admin, &poll_id, &true);
        s.client.claim_winnings(&winner, &poll_id);

        let balance_after_first_claim = s.client.get_contract_balance();
        let _ = s
            .client
            .try_claim_winnings(&winner, &poll_id)
            .expect_err("second claim should fail");

        assert_eq!(s.client.get_contract_balance(), balance_after_first_claim);
    }

    #[test]
    fn failed_zero_value_claim_emits_no_event() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        stake_user(&s, poll_id, StakeSide::Yes, 100_000_000);
        let loser = stake_user(&s, poll_id, StakeSide::No, 300_000_000);
        s.client.resolve_poll(&s.admin, &poll_id, &true);

        let err = s
            .client
            .try_claim_winnings(&loser, &poll_id)
            .expect_err("losing claim should fail")
            .unwrap();

        assert_eq!(err, PredictXError::NotOnWinningSide);

        let events = s.env.events().all();
        for i in 0..events.len() {
            let (_, topics, _) = events.get(i).unwrap();
            let name: Symbol = topics.get(0).unwrap().try_into_val(&s.env).unwrap();
            assert_ne!(name, Symbol::new(&s.env, "WinningsClaimed"));
        }
    }

    #[test]
    fn first_claim_routes_the_platform_fee_to_the_treasury() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let winner = stake_user(&s, poll_id, StakeSide::Yes, 100_000_000);
        stake_user(&s, poll_id, StakeSide::No, 300_000_000);
        s.client.resolve_poll(&s.admin, &poll_id, &true);

        let token_client = token::Client::new(&s.env, &s.token_addr);
        let treasury = s.client.get_treasury_address();
        assert_eq!(token_client.balance(&treasury), 0);

        s.client.claim_winnings(&winner, &poll_id);

        // 5% of the 400_000_000 combined pool.
        assert_eq!(token_client.balance(&treasury), 20_000_000);
    }

    #[test]
    fn platform_fee_is_routed_only_once_across_claims() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);
        let alice = stake_user(&s, poll_id, StakeSide::Yes, 60_000_000);
        let bob = stake_user(&s, poll_id, StakeSide::Yes, 40_000_000);
        stake_user(&s, poll_id, StakeSide::No, 300_000_000);
        s.client.resolve_poll(&s.admin, &poll_id, &true);

        s.client.claim_winnings(&alice, &poll_id);
        s.client.claim_winnings(&bob, &poll_id);

        let token_client = token::Client::new(&s.env, &s.token_addr);
        let treasury = s.client.get_treasury_address();
        assert_eq!(token_client.balance(&treasury), 20_000_000);

        // 380_000_000 distributable, split 60/40 across the winning pool.
        assert_eq!(token_client.balance(&alice), 228_000_000);
        assert_eq!(token_client.balance(&bob), 152_000_000);
    }
}
