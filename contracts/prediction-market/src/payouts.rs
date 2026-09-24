use soroban_sdk::{Address, Env};
use predictx_shared::{PollStatus, Stake, StakeSide, BPS_DENOMINATOR};
use crate::{DataKey, token_utils};

// ── Claimable amount view ─────────────────────────────────────────────────────

/// Return the exact token amount a user would receive from `claim_winnings`
/// on the given poll, **without mutating any state**.
///
/// Returns `0` for every ineligible case rather than erroring, so the frontend
/// can call this safely for any (poll, user) pair:
///
/// | Case                              | Returns |
/// |-----------------------------------|---------|
/// | Poll not found                    | `0`     |
/// | Poll not yet resolved             | `0`     |
/// | User never staked                 | `0`     |
/// | User staked on the losing side    | `0`     |
/// | User already claimed their reward | `0`     |
/// | User is a winner with open claim  | exact payout |
///
/// # Payout formula
///
/// Winners receive their stake back **plus** a proportional share of the losing
/// pool after the platform fee is deducted:
///
/// ```text
/// net_losing_pool = losing_pool * (BPS_DENOMINATOR - fee_bps) / BPS_DENOMINATOR
/// payout = stake + stake * net_losing_pool / winning_pool
/// ```
///
/// Integer division truncates toward zero — any dust stays in the contract.
pub fn get_claimable_amount(env: &Env, poll_id: u64, user: &Address) -> i128 {
    // ── 1. Load the poll — return 0 if it doesn't exist ──────────────────────
    let poll = match env
        .storage()
        .persistent()
        .get::<_, predictx_shared::Poll>(&DataKey::Poll(poll_id))
    {
        Some(p) => p,
        None => return 0,
    };

    // ── 2. Only resolved polls have claimable amounts ─────────────────────────
    if poll.status != PollStatus::Resolved {
        return 0;
    }

    // ── 3. An outcome must be set ──────────────────────────────────────────────
    let yes_won = match poll.outcome {
        Some(v) => v,
        None => return 0,
    };

    // ── 4. Load the user's stake — return 0 if they never staked ─────────────
    let stake: Stake = match env
        .storage()
        .persistent()
        .get(&DataKey::Stake(poll_id, user.clone()))
    {
        Some(s) => s,
        None => return 0,
    };

    // ── 5. Already claimed → nothing left to collect ──────────────────────────
    if stake.claimed {
        return 0;
    }

    // ── 6. Must be on the winning side ────────────────────────────────────────
    let staker_won = match stake.side {
        StakeSide::Yes => yes_won,
        StakeSide::No => !yes_won,
    };
    if !staker_won {
        return 0;
    }

    // ── 7. Compute payout ─────────────────────────────────────────────────────
    let (winning_pool, losing_pool) = if yes_won {
        (poll.yes_pool, poll.no_pool)
    } else {
        (poll.no_pool, poll.yes_pool)
    };

    // Guard against a degenerate pool (no losers → only return stake).
    if winning_pool == 0 {
        return stake.amount;
    }

    let fee_bps = token_utils::get_platform_fee_bps(env);
    let fee_factor = (BPS_DENOMINATOR - fee_bps) as i128;
    let bps = BPS_DENOMINATOR as i128;

    // net_losing_pool = losing_pool * (1 - fee%)
    // share_of_losers = stake * net_losing_pool / winning_pool
    // payout          = stake + share_of_losers
    let net_losing_pool = losing_pool * fee_factor / bps;
    let share_of_losers = stake.amount * net_losing_pool / winning_pool;

    stake.amount + share_of_losers
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use soroban_sdk::{testutils::Address as _, token, Address, Env, String};
    use predictx_shared::{Poll, PollCategory, PollStatus, Stake, StakeSide};
    use crate::{DataKey, PredictionMarket, PredictionMarketClient};

    // ── Test helpers ──────────────────────────────────────────────────────────

    struct TestSetup<'a> {
        env: Env,
        admin: Address,
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

        TestSetup {
            env,
            admin,
            oracle_id,
            token_addr,
            contract_id,
            client,
        }
    }

    fn mint_tokens(s: &TestSetup, to: &Address, amount: i128) {
        token::StellarAssetClient::new(&s.env, &s.token_addr).mint(to, &amount);
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
            &String::from_str(&s.env, "PL"),
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

    /// Plant a resolved poll with given pool sizes and winning side directly in
    /// storage, bypassing the staking flow (which would require a live token
    /// transfer to the contract that has already happened).
    fn plant_resolved_poll(
        s: &TestSetup,
        poll_id: u64,
        yes_pool: i128,
        no_pool: i128,
        yes_won: bool,
    ) {
        s.env.as_contract(&s.contract_id, || {
            let mut poll: Poll = s
                .env
                .storage()
                .persistent()
                .get(&DataKey::Poll(poll_id))
                .unwrap();
            poll.yes_pool = yes_pool;
            poll.no_pool = no_pool;
            poll.status = PollStatus::Resolved;
            poll.outcome = Some(yes_won);
            s.env.storage().persistent().set(&DataKey::Poll(poll_id), &poll);
        });
    }

    /// Plant a stake record directly into storage (stake amount is assumed to
    /// already be held by the contract — we fund the contract separately).
    fn plant_stake(
        s: &TestSetup,
        poll_id: u64,
        user: &Address,
        amount: i128,
        side: StakeSide,
        claimed: bool,
    ) {
        s.env.as_contract(&s.contract_id, || {
            let stake = Stake {
                user: user.clone(),
                poll_id,
                amount,
                side,
                claimed,
                staked_at: s.env.ledger().timestamp(),
            };
            s.env.storage().persistent().set(&DataKey::Stake(poll_id, user.clone()), &stake);
        });
    }

    // ── Test 1: view output matches actual claim payout ───────────────────────
    //
    // This is the primary acceptance-criteria test: get_claimable_amount must
    // return the same value that claim_winnings would transfer to the winner.
    // Because claim_winnings hasn't been implemented yet we verify against the
    // hand-computed expected payout instead, using the same formula that both
    // functions will use.
    //
    // Setup:
    //   Yes pool = 300 tokens, No pool = 200 tokens, fee = 5 %
    //   Winner is Yes (yes_won = true)
    //   User staked 100 tokens on Yes
    //
    // Expected payout:
    //   net_losing_pool = 200 * 9500 / 10000 = 190
    //   share_of_losers = 100 * 190 / 300    =  63  (truncated)
    //   payout          = 100 + 63           = 163
    #[test]
    fn view_output_equals_expected_claim_payout() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);

        let yes_pool: i128 = 300_000_000;
        let no_pool: i128 = 200_000_000;
        let user_stake: i128 = 100_000_000;

        plant_resolved_poll(&s, poll_id, yes_pool, no_pool, true);
        let user = Address::generate(&s.env);
        plant_stake(&s, poll_id, &user, user_stake, StakeSide::Yes, false);

        // Fund contract so it notionally holds the total pool.
        mint_tokens(&s, &s.contract_id, yes_pool + no_pool);

        let claimable = s.client.get_claimable_amount(&poll_id, &user);

        // Hand-compute the expected value.
        let fee_bps: i128 = 500;
        let bps: i128 = 10_000;
        let net_losing = no_pool * (bps - fee_bps) / bps;
        let expected = user_stake + user_stake * net_losing / yes_pool;

        assert_eq!(claimable, expected);
        assert!(claimable > 0);
    }

    // ── Test 2: returns 0 for every ineligible case ───────────────────────────
    //
    // Covers: unresolved poll, non-staker, losing staker, already-claimed stake.
    #[test]
    fn returns_zero_for_all_ineligible_cases() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);

        let yes_pool: i128 = 200_000_000;
        let no_pool: i128 = 100_000_000;

        // ── a) poll still active (not resolved) ───────────────────────────────
        let user = Address::generate(&s.env);
        plant_stake(&s, poll_id, &user, 50_000_000, StakeSide::Yes, false);
        assert_eq!(
            s.client.get_claimable_amount(&poll_id, &user),
            0,
            "active poll should return 0"
        );

        // ── b) non-existent poll ID ───────────────────────────────────────────
        assert_eq!(
            s.client.get_claimable_amount(&9999_u64, &user),
            0,
            "non-existent poll should return 0"
        );

        // Resolve the poll with Yes winning.
        plant_resolved_poll(&s, poll_id, yes_pool, no_pool, true);

        // ── c) user who never staked ──────────────────────────────────────────
        let never_staked = Address::generate(&s.env);
        assert_eq!(
            s.client.get_claimable_amount(&poll_id, &never_staked),
            0,
            "non-staker should return 0"
        );

        // ── d) losing staker (staked No, Yes won) ─────────────────────────────
        let loser = Address::generate(&s.env);
        plant_stake(&s, poll_id, &loser, 60_000_000, StakeSide::No, false);
        assert_eq!(
            s.client.get_claimable_amount(&poll_id, &loser),
            0,
            "losing staker should return 0"
        );

        // ── e) already-claimed winner ─────────────────────────────────────────
        let already_claimed = Address::generate(&s.env);
        plant_stake(&s, poll_id, &already_claimed, 50_000_000, StakeSide::Yes, true);
        assert_eq!(
            s.client.get_claimable_amount(&poll_id, &already_claimed),
            0,
            "already-claimed stake should return 0"
        );
    }

    // ── Test 3: proportional payouts across multiple winners ──────────────────
    //
    // When two users stake different amounts on the winning side their
    // claimable amounts must be proportional to their stakes.
    //
    // Setup:
    //   Yes pool = winner_a (100) + winner_b (200) = 300 tokens
    //   No pool  = 150 tokens, fee = 5 %
    //
    // For winner_a (100-token stake):
    //   net_losing = 150 * 9500 / 10000 = 142 (truncated)
    //   share      = 100 * 142 / 300    =  47 (truncated)
    //   payout     = 100 + 47           = 147
    //
    // For winner_b (200-token stake):
    //   share      = 200 * 142 / 300    =  94 (truncated)
    //   payout     = 200 + 94           = 294
    #[test]
    fn proportional_payouts_for_multiple_winners() {
        let s = setup();
        let poll_id = create_poll(&s, 2_000_000);

        let stake_a: i128 = 100_000_000;
        let stake_b: i128 = 200_000_000;
        let no_pool: i128  = 150_000_000;
        let yes_pool = stake_a + stake_b; // 300_000_000

        plant_resolved_poll(&s, poll_id, yes_pool, no_pool, true);

        let winner_a = Address::generate(&s.env);
        let winner_b = Address::generate(&s.env);
        plant_stake(&s, poll_id, &winner_a, stake_a, StakeSide::Yes, false);
        plant_stake(&s, poll_id, &winner_b, stake_b, StakeSide::Yes, false);

        let claimable_a = s.client.get_claimable_amount(&poll_id, &winner_a);
        let claimable_b = s.client.get_claimable_amount(&poll_id, &winner_b);

        // Hand-compute expected values.
        let fee_bps: i128 = 500;
        let bps: i128 = 10_000;
        let net_losing = no_pool * (bps - fee_bps) / bps;
        let expected_a = stake_a + stake_a * net_losing / yes_pool;
        let expected_b = stake_b + stake_b * net_losing / yes_pool;

        assert_eq!(claimable_a, expected_a);
        assert_eq!(claimable_b, expected_b);

        // winner_b staked 2× winner_a so the combined claims should respect
        // that ratio (within integer rounding of ±1 token unit).
        assert!(
            (claimable_b - 2 * claimable_a).abs() <= 1,
            "claimable_b ({claimable_b}) should be ~2× claimable_a ({claimable_a})"
        );
    }
}
