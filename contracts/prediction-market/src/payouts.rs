use soroban_sdk::{Address, Env, Symbol};
use predictx_shared::{
    Poll, PollStatus, Stake, StakeSide, PredictXError,
    BPS_DENOMINATOR,
};
use crate::{DataKey, get_platform_stats, set_platform_stats, token_utils};

/// Resolve a poll using the configured admin and record its final outcome.
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

// ── Payout / claim engine ─────────────────────────────────────────────────────

/// Claim winnings (or a full stake refund) after a poll resolves.
///
/// ## Normal path
/// The caller must have staked on the winning side.  Their proportional share
/// of the total pool — minus the platform fee — is transferred to them and the
/// fee is sent to the treasury.
///
/// ## Empty winning-pool path (issue #74)
/// When a poll resolves Yes but *every* staker picked No (or vice versa) the
/// winning pool is zero.  There is nobody eligible to collect winnings, so the
/// entire pot would be stranded forever.  In this case we treat every staker —
/// regardless of side — as eligible for a full, fee-free refund of their
/// original stake.
///
/// **This is the one place `NotOnWinningSide` must NOT be returned.**
/// Returning it here would lock funds in the contract with no recovery path.
pub fn claim_winnings(
    env: &Env,
    claimant: Address,
    poll_id: u64,
) -> Result<i128, PredictXError> {
    claimant.require_auth();

    // ── Load & validate poll ──────────────────────────────────────────────────

    let poll: Poll = env
        .storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)?;

    if poll.status != PollStatus::Resolved {
        return Err(PredictXError::PollNotActive);
    }

    // outcome is always Some(_) for a Resolved poll
    let outcome_yes: bool = poll.outcome.ok_or(PredictXError::PollNotActive)?;

    // ── Load & validate stake ─────────────────────────────────────────────────

    let mut stake: Stake = env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, claimant.clone()))
        .ok_or(PredictXError::NotStaker)?;

    if stake.claimed {
        return Err(PredictXError::AlreadyClaimed);
    }

    // ── Determine winning pool and payout ─────────────────────────────────────

    let winning_pool: i128 = if outcome_yes { poll.yes_pool } else { poll.no_pool };
    let total_pool: i128 = poll.yes_pool + poll.no_pool;

    let payout: i128 = if winning_pool == 0 {
        // ── Empty winning-pool: full stake refund, no fee ─────────────────────
        //
        // Every staker — regardless of which side they chose — recovers their
        // original stake in full.  No platform fee is deducted because there
        // is no "winner's profit" to share.
        //
        // NOTE: we deliberately skip the `NotOnWinningSide` check here.
        // Returning that error would leave all funds permanently stranded.
        stake.amount
    } else {
        // ── Normal winning-side claim ─────────────────────────────────────────

        let staker_on_winning_side = match stake.side {
            StakeSide::Yes => outcome_yes,
            StakeSide::No => !outcome_yes,
        };

        if !staker_on_winning_side {
            return Err(PredictXError::NotOnWinningSide);
        }

        // Proportional share of total pool, after platform fee.
        //
        // payout = stake_amount * total_pool * (BPS_DENOMINATOR - fee_bps)
        //          / (winning_pool * BPS_DENOMINATOR)
        //
        // Integer division rounds down; any dust remains in the contract.
        let fee_bps = token_utils::get_platform_fee_bps(env);
        let fee_factor = (BPS_DENOMINATOR - fee_bps) as i128;
        let bps = BPS_DENOMINATOR as i128;

        let gross = stake.amount * total_pool / winning_pool;
        let net = gross * fee_factor / bps;
        let fee = gross - net;

        // Send platform fee to treasury
        if fee > 0 {
            token_utils::transfer_to_treasury(env, fee)?;
        }

        net
    };

    // ── Mark claimed & persist ────────────────────────────────────────────────

    stake.claimed = true;
    env.storage()
        .persistent()
        .set(&DataKey::Stake(poll_id, claimant.clone()), &stake);

    // ── Transfer payout to claimant ───────────────────────────────────────────

    token_utils::transfer_from_contract(env, &claimant, payout)?;

    // ── Update platform stats ─────────────────────────────────────────────────

    let mut stats = get_platform_stats(env);
    stats.total_value_locked = stats.total_value_locked.saturating_sub(payout);
    stats.total_payouts += payout;
    set_platform_stats(env, &stats);

    // ── Emit event ────────────────────────────────────────────────────────────

    env.events().publish(
        (Symbol::new(env, "WinningsClaimed"), poll_id, claimant),
        payout,
    );

    Ok(payout)
}

/// Calculate a resolved poll's payout for a user without transferring tokens.
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
    if poll.status != PollStatus::Resolved {
        return Err(PredictXError::PollNotLocked);
    }

    let outcome = poll.outcome.ok_or(PredictXError::InvalidOutcome)?;
    let winning_pool = if outcome { poll.yes_pool } else { poll.no_pool };
    if winning_pool == 0 {
        return Ok(stake.amount);
    }
    if stake.side != if outcome { StakeSide::Yes } else { StakeSide::No } {
        return Ok(0);
    }

    let total_pool = poll.yes_pool + poll.no_pool;
    let payout_pool = total_pool
        * (BPS_DENOMINATOR - token_utils::get_platform_fee_bps(env)) as i128
        / BPS_DENOMINATOR as i128;
    Ok(stake.amount * payout_pool / winning_pool)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        token, Address, Env, String,
    };
    use predictx_shared::{
        Poll, PollCategory, PollStatus, PredictXError, Stake, StakeSide,
    };
    use crate::{DataKey, PredictionMarket, PredictionMarketClient};

    // ── Test helpers ──────────────────────────────────────────────────────────

    struct TestSetup<'a> {
        env: Env,
        admin: Address,
        #[allow(dead_code)]
        oracle_id: Address,
        token_addr: Address,
        contract_id: Address,
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
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_addr = token_contract.address();

        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        let treasury = Address::generate(&env);
        client.initialize(&admin, &oracle_id, &token_addr, &treasury, &500_u32);

        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        TestSetup { env, admin, oracle_id, token_addr, contract_id, client }
    }

    fn mint_tokens(s: &TestSetup, to: &Address, amount: i128) {
        let sac = token::StellarAssetClient::new(&s.env, &s.token_addr);
        sac.mint(to, &amount);
    }

    fn token_balance(s: &TestSetup, addr: &Address) -> i128 {
        token::Client::new(&s.env, &s.token_addr).balance(addr)
    }

    /// Create a match + poll and return the poll_id.
    fn create_poll(s: &TestSetup, lock_time: u64) -> u64 {
        let match_id = s.client.create_match(
            &s.admin,
            &String::from_str(&s.env, "Arsenal"),
            &String::from_str(&s.env, "Chelsea"),
            &String::from_str(&s.env, "Premier League"),
            &String::from_str(&s.env, "Emirates"),
            &(lock_time + 3_600),
        );
        s.client.create_poll(
            &s.admin,
            &match_id,
            &String::from_str(&s.env, "Will Palmer score?"),
            &PollCategory::PlayerEvent,
            &lock_time,
        )
    }

    /// Directly inject a resolved poll with a given outcome into storage.
    fn inject_resolved_poll(s: &TestSetup, poll_id: u64, outcome_yes: bool, yes_pool: i128, no_pool: i128) {
        s.env.as_contract(&s.contract_id, || {
            let poll = Poll {
                poll_id,
                match_id: 1,
                creator: s.admin.clone(),
                question: String::from_str(&s.env, "test"),
                category: PollCategory::PlayerEvent,
                lock_time: 500_000,
                yes_pool,
                no_pool,
                yes_count: if yes_pool > 0 { 1 } else { 0 },
                no_count: if no_pool > 0 { 1 } else { 0 },
                status: PollStatus::Resolved,
                outcome: Some(outcome_yes),
                resolution_time: 1_000_000,
                created_at: 900_000,
            };
            s.env.storage().persistent().set(&DataKey::Poll(poll_id), &poll);
        });
    }

    /// Inject a stake record directly (bypasses staking checks — used to set
    /// up state for claim tests without going through the full staking flow).
    fn inject_stake(s: &TestSetup, poll_id: u64, user: &Address, amount: i128, side: StakeSide) {
        s.env.as_contract(&s.contract_id, || {
            let stake = Stake {
                user: user.clone(),
                poll_id,
                amount,
                side,
                claimed: false,
                staked_at: 900_000,
            };
            s.env.storage().persistent().set(&DataKey::Stake(poll_id, user.clone()), &stake);
        });
    }

    // ── Tests: empty winning-pool path (issue #74) ────────────────────────────

    /// A losing staker can recover their original stake when the winning pool
    /// is empty (i.e. nobody staked on the winning side).
    #[test]
    fn empty_winning_pool_losing_staker_gets_full_refund() {
        let s = setup();

        // Poll resolves Yes, but only No stakers exist → yes_pool == 0
        let poll_id: u64 = 99;
        let no_stake_amount: i128 = 200_000_000;

        let no_user = Address::generate(&s.env);

        // Seed contract with the pool amount
        mint_tokens(&s, &s.contract_id, no_stake_amount);

        inject_resolved_poll(&s, poll_id, true, 0, no_stake_amount);
        inject_stake(&s, poll_id, &no_user, no_stake_amount, StakeSide::No);

        let refund = s.client.claim_winnings(&no_user, &poll_id);

        assert_eq!(refund, no_stake_amount, "should refund full stake");
        assert_eq!(
            token_balance(&s, &no_user),
            no_stake_amount,
            "user balance should equal refunded stake"
        );
    }

    /// No platform fee is taken in the empty-winning-pool case.
    #[test]
    fn empty_winning_pool_no_platform_fee_deducted() {
        let s = setup();

        // Poll resolves No, but only Yes stakers exist → no_pool == 0
        let poll_id: u64 = 100;
        let yes_stake_amount: i128 = 150_000_000;

        let yes_user = Address::generate(&s.env);
        mint_tokens(&s, &s.contract_id, yes_stake_amount);

        inject_resolved_poll(&s, poll_id, false, yes_stake_amount, 0);
        inject_stake(&s, poll_id, &yes_user, yes_stake_amount, StakeSide::Yes);

        let treasury_before = token_balance(&s, &s.client.get_treasury_address());
        let refund = s.client.claim_winnings(&yes_user, &poll_id);

        // Exact stake returned — no fee
        assert_eq!(refund, yes_stake_amount);
        // Treasury unchanged
        assert_eq!(
            token_balance(&s, &s.client.get_treasury_address()),
            treasury_before,
            "treasury must not receive any fee in empty-pool refund"
        );
    }

    /// Once all stakers have claimed in the empty-winning-pool scenario, the
    /// contract balance reaches exactly zero.
    #[test]
    fn empty_winning_pool_contract_balance_zero_after_all_claims() {
        let s = setup();

        // Poll resolves Yes, but all three stakers picked No
        let poll_id: u64 = 101;
        let amounts: [i128; 3] = [100_000_000, 200_000_000, 150_000_000];
        let total: i128 = amounts[0] + amounts[1] + amounts[2];

        let users: [Address; 3] = [
            Address::generate(&s.env),
            Address::generate(&s.env),
            Address::generate(&s.env),
        ];

        // Seed contract with the full pooled amount
        mint_tokens(&s, &s.contract_id, total);

        inject_resolved_poll(&s, poll_id, true, 0, total);

        for (i, user) in users.iter().enumerate() {
            inject_stake(&s, poll_id, user, amounts[i], StakeSide::No);
        }

        // All three stakers claim their refund
        for (i, user) in users.iter().enumerate() {
            let refund = s.client.claim_winnings(user, &poll_id);
            assert_eq!(refund, amounts[i]);
        }

        // Contract balance must be exactly zero — no stranded funds
        assert_eq!(
            token_balance(&s, &s.contract_id),
            0,
            "all funds should be returned; contract balance must be zero"
        );
    }

    #[test]
    fn successful_claim_marks_stake_as_claimed() {
        let s = setup();
        let poll_id: u64 = 102;
        let winner = Address::generate(&s.env);
        let winning_stake = 100_000_000;
        let losing_pool = 300_000_000;

        mint_tokens(&s, &s.contract_id, winning_stake + losing_pool);
        inject_resolved_poll(&s, poll_id, true, winning_stake, losing_pool);
        inject_stake(&s, poll_id, &winner, winning_stake, StakeSide::Yes);

        s.client.claim_winnings(&winner, &poll_id);

        let stake = s.client.get_stake_info(&poll_id, &winner);
        assert!(stake.claimed);
    }

    // ── Tests: normal claim path ──────────────────────────────────────────────

    /// A winner on the correct side receives their proportional payout.
    #[test]
    fn normal_winner_receives_proportional_payout() {
        let s = setup();

        let lock_time = 1_500_000;
        let poll_id = create_poll(&s, lock_time);

        let yes_user = Address::generate(&s.env);
        let no_user = Address::generate(&s.env);
        let yes_amount: i128 = 100_000_000;
        let no_amount: i128 = 100_000_000;

        mint_tokens(&s, &yes_user, yes_amount);
        mint_tokens(&s, &no_user, no_amount);

        s.client.stake(&yes_user, &poll_id, &yes_amount, &StakeSide::Yes);
        s.client.stake(&no_user, &poll_id, &no_amount, &StakeSide::No);

        // Resolve with Yes winning
        inject_resolved_poll(&s, poll_id, true, yes_amount, no_amount);

        let payout = s.client.claim_winnings(&yes_user, &poll_id);

        // gross = 100M * 200M / 100M = 200M; net = 200M * 9500 / 10000 = 190M
        let expected_net: i128 = 190_000_000;
        assert_eq!(payout, expected_net);
        assert!(token_balance(&s, &yes_user) >= expected_net);
    }

    /// A staker on the losing side is rejected with `NotOnWinningSide`.
    #[test]
    fn loser_cannot_claim_on_normal_resolution() {
        let s = setup();

        let lock_time = 1_500_000;
        let poll_id = create_poll(&s, lock_time);

        let yes_user = Address::generate(&s.env);
        let no_user = Address::generate(&s.env);
        let amount: i128 = 100_000_000;

        mint_tokens(&s, &yes_user, amount);
        mint_tokens(&s, &no_user, amount);

        s.client.stake(&yes_user, &poll_id, &amount, &StakeSide::Yes);
        s.client.stake(&no_user, &poll_id, &amount, &StakeSide::No);

        // Resolve with Yes winning — No user is the loser
        inject_resolved_poll(&s, poll_id, true, amount, amount);

        let err = s
            .client
            .try_claim_winnings(&no_user, &poll_id)
            .expect_err("loser should not be able to claim");
        assert_eq!(err, Ok(PredictXError::NotOnWinningSide));
    }

    /// Double-claiming is rejected with `AlreadyClaimed`.
    #[test]
    fn double_claim_is_rejected() {
        let s = setup();

        let poll_id: u64 = 200;
        let amount: i128 = 100_000_000;
        let user = Address::generate(&s.env);

        mint_tokens(&s, &s.contract_id, amount * 2);

        inject_resolved_poll(&s, poll_id, true, amount, amount);
        inject_stake(&s, poll_id, &user, amount, StakeSide::Yes);

        s.client.claim_winnings(&user, &poll_id);

        let err = s
            .client
            .try_claim_winnings(&user, &poll_id)
            .expect_err("second claim should fail");
        assert_eq!(err, Ok(PredictXError::AlreadyClaimed));
    }
}
