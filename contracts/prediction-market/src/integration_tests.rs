//! Integration tests for PredictX platform.
//!
//! These tests exercise cross-contract interactions between PredictionMarket
//! and VotingOracle with real Stellar Asset Contract token transfers.
//!
//! **Current contract state:** The VotingOracle is a placeholder that stores
//! poll status via admin calls. Voting, claims, disputes, and treasury payouts
//! are not yet implemented. Tests exercise what IS implemented and document
//! what future scenarios will require.

extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env, String,
};

use predictx_shared::{PollCategory, PollStatus, PredictXError, StakeSide};

use crate::{voting_oracle, PredictionMarket, PredictionMarketClient};

// ── Constants ────────────────────────────────────────────────────────────────

/// 1 token = 10^7 base units (7 decimal places, Stellar standard).
const TOKEN: i128 = 10_000_000;

const PLATFORM_FEE_BPS: u32 = 500; // 5%
const BASE_TIME: u64 = 1_000_000;
const LOCK_TIME: u64 = BASE_TIME + 50_000;
const KICKOFF_TIME: u64 = BASE_TIME + 100_000;
const EMERGENCY_TIMEOUT: u64 = 7 * 24 * 60 * 60; // 604_800s = 7 days

// ── Test Platform ────────────────────────────────────────────────────────────

/// Deploys PredictionMarket (native) + VotingOracle (WASM) + SAC token.
/// Provides helper methods for common integration test operations.
struct TestPlatform {
    env: Env,
    admin: Address,
    #[allow(dead_code)]
    oracle_id: Address,
    contract_id: Address,
    token_addr: Address,
    treasury: Address,
    client: PredictionMarketClient<'static>,
    oracle_client: voting_oracle::Client<'static>,
}

/// Map `predictx_shared::PollStatus` → `voting_oracle::PollStatus` (WASM-imported type).
fn to_oracle_status(s: PollStatus) -> voting_oracle::PollStatus {
    match s {
        PollStatus::Active => voting_oracle::PollStatus::Active,
        PollStatus::Locked => voting_oracle::PollStatus::Locked,
        PollStatus::Voting => voting_oracle::PollStatus::Voting,
        PollStatus::AdminReview => voting_oracle::PollStatus::AdminReview,
        PollStatus::Disputed => voting_oracle::PollStatus::Disputed,
        PollStatus::Resolved => voting_oracle::PollStatus::Resolved,
        PollStatus::Cancelled => voting_oracle::PollStatus::Cancelled,
    }
}

impl TestPlatform {
    /// Deploy and initialise all contracts.
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        // ── Deploy voting oracle from WASM (cross-contract target) ───────
        let oracle_id = env.register(voting_oracle::WASM, ());
        let oracle_client = voting_oracle::Client::new(&env, &oracle_id);
        oracle_client.initialize(&admin);

        // ── Deploy real Stellar Asset Contract for token transfers ────────
        let token_admin = Address::generate(&env);
        let sac = env.register_stellar_asset_contract_v2(token_admin);
        let token_addr = sac.address();

        // Treasury is a plain address that receives token transfers.
        let treasury = Address::generate(&env);

        // ── Deploy prediction market ─────────────────────────────────────
        let contract_id = env.register(PredictionMarket, ());
        let client = PredictionMarketClient::new(&env, &contract_id);
        client.initialize(&admin, &oracle_id, &token_addr, &treasury, &PLATFORM_FEE_BPS);

        env.ledger().with_mut(|l| l.timestamp = BASE_TIME);

        TestPlatform {
            env,
            admin,
            oracle_id,
            contract_id,
            token_addr,
            treasury,
            client,
            oracle_client,
        }
    }

    /// Create a Soroban `String`.
    fn s(&self, text: &str) -> String {
        String::from_str(&self.env, text)
    }

    /// Generate a new address and optionally mint tokens to it.
    fn create_user(&self, tokens: i128) -> Address {
        let user = Address::generate(&self.env);
        if tokens > 0 {
            let sac = token::StellarAssetClient::new(&self.env, &self.token_addr);
            sac.mint(&user, &tokens);
        }
        user
    }

    /// Create a match with default teams; kickoff at `KICKOFF_TIME`.
    fn create_match(&self) -> u64 {
        self.client.create_match(
            &self.admin,
            &self.s("Chelsea"),
            &self.s("Man United"),
            &self.s("Premier League"),
            &self.s("Stamford Bridge"),
            &KICKOFF_TIME,
        )
    }

    /// Create a poll under `match_id` with a custom question; locks at `LOCK_TIME`.
    fn create_poll(&self, match_id: u64, question: &str) -> u64 {
        let creator = self.create_user(0);
        self.client.create_poll(
            &creator,
            &match_id,
            &self.s(question),
            &PollCategory::PlayerEvent,
            &LOCK_TIME,
        )
    }

    /// Set the oracle poll status (converts shared → oracle type).
    fn set_oracle_status(&self, poll_id: u64, status: PollStatus) {
        self.oracle_client
            .set_poll_status(&poll_id, &to_oracle_status(status));
    }

    /// Token balance of any address.
    fn balance_of(&self, addr: &Address) -> i128 {
        token::Client::new(&self.env, &self.token_addr).balance(addr)
    }

    /// Token balance held by the prediction-market contract.
    fn contract_balance(&self) -> i128 {
        self.balance_of(&self.contract_id)
    }

    /// Set the ledger timestamp to an absolute value.
    fn set_time(&self, ts: u64) {
        self.env.ledger().with_mut(|l| l.timestamp = ts);
    }

    /// Advance the ledger timestamp by `secs` seconds.
    fn advance_time(&self, secs: u64) {
        let now = self.env.ledger().timestamp();
        self.env.ledger().with_mut(|l| l.timestamp = now + secs);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Scenario 1: Happy Path — Complete Prediction Lifecycle
// ══════════════════════════════════════════════════════════════════════════════
//
// Exercises: deploy → init → create match → create poll → 4 users stake →
// lock time passes → match finishes → oracle status transitions → verify
// pools, balances, and platform stats.
//
// NOTE: Claims, voting rewards, and treasury fee distribution are not yet
// implemented. Those steps are documented but not asserted.

#[test]
fn test_scenario_1_happy_path_complete_lifecycle() {
    let p = TestPlatform::new();

    // 1–3. Admin creates match, user creates poll
    let match_id = p.create_match();
    let m = p.client.get_match(&match_id);
    assert_eq!(m.home_team, p.s("Chelsea"), "Home team should be Chelsea");
    assert_eq!(m.away_team, p.s("Man United"), "Away team should be Man United");
    assert!(!m.is_finished, "Match should not be finished yet");

    let poll_id = p.create_poll(match_id, "Will Palmer score?");
    let poll = p.client.get_poll(&poll_id);
    assert_eq!(poll.status, PollStatus::Active, "New poll should be Active");
    assert_eq!(poll.match_id, match_id, "Poll should be linked to match");

    // 4–8. Four users stake: B=700 Yes, C=300 No, D=1000 Yes, E=200 No
    let user_b = p.create_user(700 * TOKEN);
    let user_c = p.create_user(300 * TOKEN);
    let user_d = p.create_user(1000 * TOKEN);
    let user_e = p.create_user(200 * TOKEN);

    p.client
        .stake(&user_b, &poll_id, &(700 * TOKEN), &StakeSide::Yes);
    p.client
        .stake(&user_c, &poll_id, &(300 * TOKEN), &StakeSide::No);
    p.client
        .stake(&user_d, &poll_id, &(1000 * TOKEN), &StakeSide::Yes);
    p.client
        .stake(&user_e, &poll_id, &(200 * TOKEN), &StakeSide::No);

    // Verify pool totals
    let pool = p.client.get_pool_info(&poll_id);
    let total_pool = 2200 * TOKEN;
    assert_eq!(
        pool.yes_pool,
        1700 * TOKEN,
        "Yes pool: B(700) + D(1000) = 1700"
    );
    assert_eq!(
        pool.no_pool,
        500 * TOKEN,
        "No pool: C(300) + E(200) = 500"
    );
    assert_eq!(pool.yes_count, 2, "Should have 2 Yes stakers");
    assert_eq!(pool.no_count, 2, "Should have 2 No stakers");

    // Contract should hold all staked tokens
    assert_eq!(
        p.contract_balance(),
        total_pool,
        "Contract should hold the entire pool"
    );

    // All user balances should be zero after staking
    assert_eq!(p.balance_of(&user_b), 0, "User B balance after stake");
    assert_eq!(p.balance_of(&user_c), 0, "User C balance after stake");
    assert_eq!(p.balance_of(&user_d), 0, "User D balance after stake");
    assert_eq!(p.balance_of(&user_e), 0, "User E balance after stake");

    // 9. Time advances past lock time — no more stakes allowed
    p.set_time(LOCK_TIME + 1);

    let late_user = p.create_user(100 * TOKEN);
    let err = p
        .client
        .try_stake(&late_user, &poll_id, &(100 * TOKEN), &StakeSide::Yes)
        .expect_err("Should reject stake after lock time");
    assert_eq!(err, Ok(PredictXError::PollLocked));

    // 10. Admin marks match as finished
    p.client.finish_match(&p.admin, &match_id);
    assert!(
        p.client.get_match(&match_id).is_finished,
        "Match should be finished"
    );

    // 11–15. Simulate voting via oracle status transitions
    p.set_oracle_status(poll_id, PollStatus::Voting);
    assert_eq!(
        p.client.oracle_poll_status(&poll_id),
        PollStatus::Voting,
        "Cross-contract: oracle should report Voting"
    );

    // Simulate >85% consensus → auto-resolve as Yes
    p.set_oracle_status(poll_id, PollStatus::Resolved);
    assert_eq!(
        p.client.oracle_poll_status(&poll_id),
        PollStatus::Resolved,
        "Cross-contract: oracle should report Resolved"
    );

    // 16–22. Verify platform stats
    let stats = p.client.get_platform_stats();
    assert_eq!(stats.total_polls_created, 1, "Should have 1 poll");
    assert_eq!(stats.total_stakes_placed, 4, "Should have 4 stakes");
    assert_eq!(stats.total_value_locked, total_pool, "TVL should equal total pool");

    // Contract still holds all tokens — claims not yet implemented
    assert_eq!(
        p.contract_balance(),
        total_pool,
        "Contract should hold full pool until claims are implemented"
    );

    // Verify each user's stake record
    let stake_b = p.client.get_stake_info(&poll_id, &user_b);
    assert_eq!(stake_b.amount, 700 * TOKEN);
    assert_eq!(stake_b.side, StakeSide::Yes);
    assert!(!stake_b.claimed);

    let stake_d = p.client.get_stake_info(&poll_id, &user_d);
    assert_eq!(stake_d.amount, 1000 * TOKEN);
    assert_eq!(stake_d.side, StakeSide::Yes);

    // NOTE: Steps 17–22 from the issue (claim winnings, voting rewards,
    // treasury fees, financial invariant) require claim/payout functions
    // that are not yet implemented. They will be added in future PRs.
}

// ══════════════════════════════════════════════════════════════════════════════
// Scenario 2: Admin Review Path
// ══════════════════════════════════════════════════════════════════════════════
//
// Simulates: vote consensus between 60–85% → AdminReview → admin verifies →
// Resolved. Exercises cross-contract status transitions.

#[test]
fn test_scenario_2_admin_review_path() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Will Chelsea win?");

    // Users stake
    let user_a = p.create_user(500 * TOKEN);
    let user_b = p.create_user(500 * TOKEN);
    p.client
        .stake(&user_a, &poll_id, &(500 * TOKEN), &StakeSide::Yes);
    p.client
        .stake(&user_b, &poll_id, &(500 * TOKEN), &StakeSide::No);

    // Lock time passes, match finishes
    p.set_time(LOCK_TIME + 1);
    p.client.finish_match(&p.admin, &match_id);

    // Simulate voting: 70% Yes, 30% No → between 60–85% → AdminReview
    p.set_oracle_status(poll_id, PollStatus::Voting);
    assert_eq!(p.client.oracle_poll_status(&poll_id), PollStatus::Voting);

    p.set_oracle_status(poll_id, PollStatus::AdminReview);
    assert_eq!(
        p.client.oracle_poll_status(&poll_id),
        PollStatus::AdminReview,
        "Poll should be in AdminReview after ambiguous vote"
    );

    // Admin verifies with evidence → Resolved
    p.set_oracle_status(poll_id, PollStatus::Resolved);
    assert_eq!(
        p.client.oracle_poll_status(&poll_id),
        PollStatus::Resolved,
        "Admin should have resolved the poll"
    );

    // Contract still holds all staked tokens
    assert_eq!(p.contract_balance(), 1000 * TOKEN);

    // NOTE: Claims after admin review resolution require claim functions
    // that are not yet implemented.
}

// ══════════════════════════════════════════════════════════════════════════════
// Scenario 3: Dispute Path — Emergency Withdrawal after Dispute Timeout
// ══════════════════════════════════════════════════════════════════════════════
//
// Exercises: stake → dispute status → 7-day timeout → emergency withdrawal
// with full refund. Verifies cross-contract oracle check and token transfer.

#[test]
fn test_scenario_3_dispute_path_emergency_withdrawal() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Will Palmer score?");

    // Users stake
    let user_a = p.create_user(800 * TOKEN);
    let user_b = p.create_user(400 * TOKEN);
    p.client
        .stake(&user_a, &poll_id, &(800 * TOKEN), &StakeSide::Yes);
    p.client
        .stake(&user_b, &poll_id, &(400 * TOKEN), &StakeSide::No);
    let total = 1200 * TOKEN;
    assert_eq!(p.contract_balance(), total);

    // Set poll to Disputed via oracle
    p.set_oracle_status(poll_id, PollStatus::Disputed);

    // Before timeout — emergency withdrawal should be rejected
    assert!(
        !p.client.check_emergency_eligible(&poll_id),
        "Should not be eligible before timeout"
    );
    let err = p
        .client
        .try_emergency_withdraw(&user_a, &poll_id)
        .expect_err("Should reject before timeout");
    assert_eq!(err, Ok(PredictXError::EmergencyWithdrawNotAllowed));

    // Advance past 7-day timeout
    p.advance_time(EMERGENCY_TIMEOUT + 1);

    assert!(
        p.client.check_emergency_eligible(&poll_id),
        "Should be eligible after timeout"
    );

    // Both users withdraw — full refund, no platform fee
    let refund_a = p.client.emergency_withdraw(&user_a, &poll_id);
    assert_eq!(refund_a, 800 * TOKEN, "User A should get full 800 token refund");
    assert_eq!(p.balance_of(&user_a), 800 * TOKEN);

    let refund_b = p.client.emergency_withdraw(&user_b, &poll_id);
    assert_eq!(refund_b, 400 * TOKEN, "User B should get full 400 token refund");
    assert_eq!(p.balance_of(&user_b), 400 * TOKEN);

    // Contract should be empty
    assert_eq!(
        p.contract_balance(),
        0,
        "Contract should hold 0 tokens after all emergency withdrawals"
    );

    // Treasury should NOT have received any fee
    assert_eq!(p.balance_of(&p.treasury), 0, "No platform fee on emergency withdrawal");

    // Platform stats TVL should have decreased
    let stats = p.client.get_platform_stats();
    assert_eq!(
        stats.total_value_locked, 0,
        "TVL should be 0 after all emergency withdrawals"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Scenario 4: Emergency Withdrawal — Cancelled Poll
// ══════════════════════════════════════════════════════════════════════════════
//
// Exercises the cancel_poll cross-contract call (PredictionMarket → Oracle)
// followed by immediate emergency withdrawal (no timeout needed).

#[test]
fn test_scenario_4_cancelled_poll_emergency_withdrawal() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Will Palmer score?");

    // Users stake
    let user_a = p.create_user(600 * TOKEN);
    let user_b = p.create_user(300 * TOKEN);
    let user_c = p.create_user(100 * TOKEN);
    p.client
        .stake(&user_a, &poll_id, &(600 * TOKEN), &StakeSide::Yes);
    p.client
        .stake(&user_b, &poll_id, &(300 * TOKEN), &StakeSide::No);
    p.client
        .stake(&user_c, &poll_id, &(100 * TOKEN), &StakeSide::Yes);
    let total = 1000 * TOKEN;
    assert_eq!(p.contract_balance(), total);

    // Admin cancels poll — cross-contract call to oracle
    p.client.cancel_poll(&p.admin, &poll_id);

    // Verify oracle received the cancellation via cross-contract read
    assert_eq!(
        p.client.oracle_poll_status(&poll_id),
        PollStatus::Cancelled,
        "Market → Oracle round-trip should show Cancelled"
    );

    // Emergency withdrawal is immediately eligible (no timeout)
    assert!(p.client.check_emergency_eligible(&poll_id));

    // All three users withdraw
    let refund_a = p.client.emergency_withdraw(&user_a, &poll_id);
    let refund_b = p.client.emergency_withdraw(&user_b, &poll_id);
    let refund_c = p.client.emergency_withdraw(&user_c, &poll_id);

    assert_eq!(refund_a, 600 * TOKEN, "User A full refund");
    assert_eq!(refund_b, 300 * TOKEN, "User B full refund");
    assert_eq!(refund_c, 100 * TOKEN, "User C full refund");

    // Verify final balances
    assert_eq!(p.balance_of(&user_a), 600 * TOKEN);
    assert_eq!(p.balance_of(&user_b), 300 * TOKEN);
    assert_eq!(p.balance_of(&user_c), 100 * TOKEN);
    assert_eq!(p.contract_balance(), 0, "Contract should be empty");
    assert_eq!(p.balance_of(&p.treasury), 0, "No fee on cancellation");
}

// ══════════════════════════════════════════════════════════════════════════════
// Scenario 5: Multi-Poll Match
// ══════════════════════════════════════════════════════════════════════════════
//
// 5 polls under a single match, multiple users staking across different polls.
// Verifies per-poll isolation and aggregate contract balance.

#[test]
fn test_scenario_5_multi_poll_match() {
    let p = TestPlatform::new();

    // 1. Admin creates match
    let match_id = p.create_match();

    // 2. Create 5 polls for the same match
    let questions = [
        "Will Palmer score?",
        "Will Chelsea win?",
        "Over 2.5 goals?",
        "Red card in match?",
        "First goal before 20min?",
    ];
    let mut poll_ids = [0u64; 5];
    for (i, q) in questions.iter().enumerate() {
        poll_ids[i] = p.create_poll(match_id, q);
    }

    // Verify match has 5 polls
    let match_polls = p.client.get_match_polls(&match_id);
    assert_eq!(match_polls.len(), 5, "Match should have 5 polls");

    // 3. Multiple users stake on different polls
    let mut total_staked: i128 = 0;
    let stake_amount = 100 * TOKEN;

    for i in 0..5u32 {
        let user_yes = p.create_user(stake_amount);
        let user_no = p.create_user(stake_amount);
        let pid = poll_ids[i as usize];

        p.client
            .stake(&user_yes, &pid, &stake_amount, &StakeSide::Yes);
        p.client
            .stake(&user_no, &pid, &stake_amount, &StakeSide::No);
        total_staked += stake_amount * 2;

        // Verify per-poll pool
        let pool = p.client.get_pool_info(&pid);
        assert_eq!(pool.yes_pool, stake_amount, "Poll {} yes_pool", i);
        assert_eq!(pool.no_pool, stake_amount, "Poll {} no_pool", i);
        assert_eq!(pool.yes_count, 1);
        assert_eq!(pool.no_count, 1);
    }

    // 4. Verify total contract balance equals all stakes across all polls
    assert_eq!(
        p.contract_balance(),
        total_staked,
        "Contract balance should equal sum of all stakes across 5 polls"
    );

    // 5. Time advances, match finishes
    p.set_time(LOCK_TIME + 1);
    p.client.finish_match(&p.admin, &match_id);

    // 6. Some polls auto-resolve, some go to admin review
    p.set_oracle_status(poll_ids[0], PollStatus::Resolved);
    p.set_oracle_status(poll_ids[1], PollStatus::Resolved);
    p.set_oracle_status(poll_ids[2], PollStatus::AdminReview);
    p.set_oracle_status(poll_ids[3], PollStatus::AdminReview);
    p.set_oracle_status(poll_ids[4], PollStatus::Resolved);

    // Verify cross-contract status reads
    assert_eq!(
        p.client.oracle_poll_status(&poll_ids[0]),
        PollStatus::Resolved
    );
    assert_eq!(
        p.client.oracle_poll_status(&poll_ids[2]),
        PollStatus::AdminReview
    );

    // Admin reviews resolve remaining polls
    p.set_oracle_status(poll_ids[2], PollStatus::Resolved);
    p.set_oracle_status(poll_ids[3], PollStatus::Resolved);

    // All 5 polls resolved
    for &pid in &poll_ids {
        assert_eq!(
            p.client.oracle_poll_status(&pid),
            PollStatus::Resolved,
            "All polls should be Resolved"
        );
    }

    // Platform stats
    let stats = p.client.get_platform_stats();
    assert_eq!(stats.total_polls_created, 5, "Should have 5 polls");
    assert_eq!(
        stats.total_stakes_placed, 10,
        "Should have 10 stakes (2 per poll)"
    );
    assert_eq!(stats.total_value_locked, total_staked, "TVL matches total staked");

    // Contract still holds all tokens (claims not implemented)
    assert_eq!(p.contract_balance(), total_staked);
}

// ══════════════════════════════════════════════════════════════════════════════
// Scenario 6: Concurrent Users — 10 Users on Same Poll
// ══════════════════════════════════════════════════════════════════════════════
//
// Verifies pool totals are accurate with many concurrent stakers and that
// no accounting drift occurs.

#[test]
fn test_scenario_6_concurrent_users() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Will Palmer score?");

    // 10 users stake with varying amounts
    let amounts: [i128; 10] = [
        50 * TOKEN,
        75 * TOKEN,
        100 * TOKEN,
        150 * TOKEN,
        200 * TOKEN,
        120 * TOKEN,
        80 * TOKEN,
        300 * TOKEN,
        60 * TOKEN,
        250 * TOKEN,
    ];

    let mut users: [Option<Address>; 10] = Default::default();
    let mut total_yes: i128 = 0;
    let mut total_no: i128 = 0;
    let mut yes_count: u32 = 0;
    let mut no_count: u32 = 0;

    for i in 0..10 {
        let user = p.create_user(amounts[i]);
        // Even-index → Yes, Odd-index → No
        let side = if i % 2 == 0 {
            StakeSide::Yes
        } else {
            StakeSide::No
        };

        p.client.stake(&user, &poll_id, &amounts[i], &side);

        match side {
            StakeSide::Yes => {
                total_yes += amounts[i];
                yes_count += 1;
            }
            StakeSide::No => {
                total_no += amounts[i];
                no_count += 1;
            }
        }
        users[i] = Some(user);
    }

    // Verify pool totals
    let pool = p.client.get_pool_info(&poll_id);
    assert_eq!(pool.yes_pool, total_yes, "Yes pool mismatch");
    assert_eq!(pool.no_pool, total_no, "No pool mismatch");
    assert_eq!(pool.yes_count, yes_count, "Yes count mismatch");
    assert_eq!(pool.no_count, no_count, "No count mismatch");

    let total_staked = total_yes + total_no;

    // Contract balance matches total
    assert_eq!(
        p.contract_balance(),
        total_staked,
        "Contract should hold exactly the total staked amount"
    );

    // Each user's balance should be zero
    for (i, user_opt) in users.iter().enumerate() {
        let user = user_opt.as_ref().unwrap();
        assert_eq!(
            p.balance_of(user),
            0,
            "User {} balance should be 0 after staking",
            i
        );
    }

    // Platform stats consistency
    let stats = p.client.get_platform_stats();
    assert_eq!(stats.total_stakes_placed, 10);
    assert_eq!(stats.total_value_locked, total_staked);

    // Verify each user has_staked flag and stake record
    for (i, user_opt) in users.iter().enumerate() {
        let user = user_opt.as_ref().unwrap();
        assert!(
            p.client.has_user_staked(&poll_id, user),
            "User {} should be marked as having staked",
            i
        );
        let stake = p.client.get_stake_info(&poll_id, user);
        assert_eq!(stake.amount, amounts[i], "User {} stake amount", i);
        assert!(!stake.claimed, "User {} should not be claimed", i);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Scenario 7: Full User Journey (Dashboard Simulation)
// ══════════════════════════════════════════════════════════════════════════════
//
// Simulates the user journey from the Web PRD:
// See platform stats → browse matches → select poll → calculate winnings →
// stake → view dashboard → match ends → oracle resolves.

#[test]
fn test_scenario_7_full_user_journey() {
    let p = TestPlatform::new();

    // 1. User lands — sees platform stats (initially zero)
    let stats = p.client.get_platform_stats();
    assert_eq!(stats.total_polls_created, 0);
    assert_eq!(stats.total_stakes_placed, 0);
    assert_eq!(stats.total_value_locked, 0);

    // 2. Admin creates match
    let match_id = p.create_match();
    assert_eq!(p.client.get_match_count(), 1);
    let m = p.client.get_match(&match_id);
    assert_eq!(m.home_team, p.s("Chelsea"));

    // 3. User browses polls — sees "Will Palmer score?"
    let poll_id = p.create_poll(match_id, "Will Palmer score?");
    let poll = p.client.get_poll(&poll_id);
    assert_eq!(poll.question, p.s("Will Palmer score?"));

    // Build some existing pool: another user stakes first
    let early_bird = p.create_user(500 * TOKEN);
    p.client
        .stake(&early_bird, &poll_id, &(500 * TOKEN), &StakeSide::No);

    // 4. User calculates potential winnings for 200 tokens on Yes
    let potential = p
        .client
        .calculate_potential_winnings(&poll_id, &StakeSide::Yes, &(200 * TOKEN));
    // With 500 No in pool + 200 Yes:
    // pool_on_side_after = 200, total_pool_after = 700
    // winnings = 200 * 700 * 9500 / (200 * 10000) = 665 tokens (integer math)
    assert!(
        potential > 200 * TOKEN,
        "Potential winnings ({}) should exceed stake ({})",
        potential,
        200 * TOKEN
    );
    assert!(
        potential < 700 * TOKEN,
        "Potential winnings should be less than total pool"
    );

    // 5. User stakes 200 tokens on Yes
    let user = p.create_user(200 * TOKEN);
    p.client
        .stake(&user, &poll_id, &(200 * TOKEN), &StakeSide::Yes);

    // 6. Dashboard shows active stake
    let user_stakes = p.client.get_user_stakes(&user);
    assert_eq!(user_stakes.len(), 1, "User should have 1 active stake");
    assert_eq!(user_stakes.get(0).unwrap(), poll_id);

    let stake_info = p.client.get_stake_info(&poll_id, &user);
    assert_eq!(stake_info.amount, 200 * TOKEN);
    assert_eq!(stake_info.side, StakeSide::Yes);
    assert!(!stake_info.claimed);

    // Platform stats updated
    let stats = p.client.get_platform_stats();
    assert_eq!(stats.total_polls_created, 1);
    assert_eq!(stats.total_stakes_placed, 2);
    assert_eq!(stats.total_value_locked, 700 * TOKEN);

    // 7. Match ends → voting opens → resolves
    p.set_time(LOCK_TIME + 1);
    p.client.finish_match(&p.admin, &match_id);

    p.set_oracle_status(poll_id, PollStatus::Voting);
    p.set_oracle_status(poll_id, PollStatus::Resolved);

    // User checks poll status via market → oracle cross-contract call
    assert_eq!(p.client.oracle_poll_status(&poll_id), PollStatus::Resolved);

    // Contract holds all funds
    assert_eq!(p.contract_balance(), 700 * TOKEN);

    // NOTE: Claim winnings, dashboard profit/ROI display require claim
    // functions that are not yet implemented.
}

// ══════════════════════════════════════════════════════════════════════════════
// Cross-Contract Tests
// ══════════════════════════════════════════════════════════════════════════════

/// Verifies that cancel_poll performs a real cross-contract call to the oracle.
#[test]
fn test_cross_contract_cancel_propagates_to_oracle() {
    let p = TestPlatform::new();

    // Initial status should be Active (oracle default)
    assert_eq!(
        p.client.oracle_poll_status(&42),
        PollStatus::Active,
        "Default oracle status is Active"
    );

    // Cancel via prediction market → cross-contract → oracle
    p.client.cancel_poll(&p.admin, &42);

    // Reading back through the market's cross-contract query
    assert_eq!(
        p.client.oracle_poll_status(&42),
        PollStatus::Cancelled,
        "Market → Oracle round-trip should return Cancelled"
    );
}

/// Verifies that oracle_poll_status reads through the cross-contract boundary
/// for each possible PollStatus variant.
#[test]
fn test_cross_contract_all_status_variants() {
    let p = TestPlatform::new();

    let statuses = [
        PollStatus::Active,
        PollStatus::Locked,
        PollStatus::Voting,
        PollStatus::AdminReview,
        PollStatus::Disputed,
        PollStatus::Resolved,
        PollStatus::Cancelled,
    ];

    for (i, &status) in statuses.iter().enumerate() {
        let pid = (i + 1) as u64;
        p.set_oracle_status(pid, status);
        assert_eq!(
            p.client.oracle_poll_status(&pid),
            status,
            "Cross-contract read should return correct status for poll {}",
            pid
        );
    }
}

/// Verifies that emergency_withdraw reads oracle status cross-contract.
#[test]
fn test_cross_contract_emergency_checks_oracle() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Test poll");

    let user = p.create_user(100 * TOKEN);
    p.client
        .stake(&user, &poll_id, &(100 * TOKEN), &StakeSide::Yes);

    // Poll is Active in oracle → not eligible
    assert!(
        !p.client.check_emergency_eligible(&poll_id),
        "Active poll should not be eligible"
    );

    // Set to Resolved → not eligible (resolved polls have normal claims, not emergency)
    p.set_oracle_status(poll_id, PollStatus::Resolved);
    assert!(
        !p.client.check_emergency_eligible(&poll_id),
        "Resolved poll should not be eligible for emergency withdrawal"
    );

    // Set to Voting → not eligible
    p.set_oracle_status(poll_id, PollStatus::Voting);
    assert!(!p.client.check_emergency_eligible(&poll_id));

    // Set to Cancelled → immediately eligible
    p.set_oracle_status(poll_id, PollStatus::Cancelled);
    assert!(
        p.client.check_emergency_eligible(&poll_id),
        "Cancelled poll should be immediately eligible"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Financial Invariant Tests
// ══════════════════════════════════════════════════════════════════════════════

/// After staking, contract token balance must exactly equal the sum of all
/// pool totals (yes_pool + no_pool) across all polls.
#[test]
fn test_financial_invariant_balance_equals_pools() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_1 = p.create_poll(match_id, "Poll 1");
    let poll_2 = p.create_poll(match_id, "Poll 2");

    let mut expected_total: i128 = 0;

    // Stake on poll 1
    for &amount in &[100 * TOKEN, 200 * TOKEN, 150 * TOKEN] {
        let user = p.create_user(amount);
        let side = if amount > 150 * TOKEN {
            StakeSide::No
        } else {
            StakeSide::Yes
        };
        p.client.stake(&user, &poll_1, &amount, &side);
        expected_total += amount;

        // Invariant holds after every stake
        assert_eq!(p.contract_balance(), expected_total);
    }

    // Stake on poll 2
    for &amount in &[300 * TOKEN, 400 * TOKEN] {
        let user = p.create_user(amount);
        p.client.stake(&user, &poll_2, &amount, &StakeSide::Yes);
        expected_total += amount;
        assert_eq!(p.contract_balance(), expected_total);
    }

    // Verify pool totals match
    let p1 = p.client.get_pool_info(&poll_1);
    let p2 = p.client.get_pool_info(&poll_2);
    let pool_sum = p1.yes_pool + p1.no_pool + p2.yes_pool + p2.no_pool;
    assert_eq!(
        p.contract_balance(),
        pool_sum,
        "Contract balance must equal sum of all pool totals"
    );
}

/// After emergency withdrawal of all stakers, contract balance must be zero.
#[test]
fn test_financial_invariant_zero_balance_after_full_emergency() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Test poll");

    // 5 users stake varying amounts
    let amounts = [50 * TOKEN, 100 * TOKEN, 200 * TOKEN, 75 * TOKEN, 150 * TOKEN];
    let mut users: [Option<Address>; 5] = Default::default();
    let mut total: i128 = 0;

    for (i, &amt) in amounts.iter().enumerate() {
        let user = p.create_user(amt);
        let side = if i % 2 == 0 {
            StakeSide::Yes
        } else {
            StakeSide::No
        };
        p.client.stake(&user, &poll_id, &amt, &side);
        users[i] = Some(user);
        total += amt;
    }
    assert_eq!(p.contract_balance(), total);

    // Cancel poll → all can emergency withdraw
    p.client.cancel_poll(&p.admin, &poll_id);

    let mut withdrawn: i128 = 0;
    for (i, user_opt) in users.iter().enumerate() {
        let user = user_opt.as_ref().unwrap();
        let refund = p.client.emergency_withdraw(user, &poll_id);
        assert_eq!(refund, amounts[i]);
        withdrawn += refund;
    }

    assert_eq!(withdrawn, total, "Total withdrawn should equal total staked");
    assert_eq!(
        p.contract_balance(),
        0,
        "Contract must hold exactly 0 after all emergency withdrawals"
    );
}

/// TVL in platform stats tracks correctly through stake + emergency withdrawal.
#[test]
fn test_financial_invariant_tvl_tracking() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "TVL test");

    // Initially zero
    assert_eq!(p.client.get_platform_stats().total_value_locked, 0);

    // Stake 300 tokens
    let user_a = p.create_user(300 * TOKEN);
    p.client
        .stake(&user_a, &poll_id, &(300 * TOKEN), &StakeSide::Yes);
    assert_eq!(
        p.client.get_platform_stats().total_value_locked,
        300 * TOKEN
    );

    // Stake 200 more
    let user_b = p.create_user(200 * TOKEN);
    p.client
        .stake(&user_b, &poll_id, &(200 * TOKEN), &StakeSide::No);
    assert_eq!(
        p.client.get_platform_stats().total_value_locked,
        500 * TOKEN
    );

    // Emergency withdrawal reduces TVL
    p.client.cancel_poll(&p.admin, &poll_id);
    p.client.emergency_withdraw(&user_a, &poll_id);
    assert_eq!(
        p.client.get_platform_stats().total_value_locked,
        200 * TOKEN,
        "TVL should decrease by withdrawn amount"
    );

    p.client.emergency_withdraw(&user_b, &poll_id);
    assert_eq!(
        p.client.get_platform_stats().total_value_locked,
        0,
        "TVL should be 0 after all withdrawals"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Time Manipulation Tests
// ══════════════════════════════════════════════════════════════════════════════

/// Staking is rejected exactly at lock_time and after, accepted before.
#[test]
fn test_time_lock_enforcement_boundary() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Lock time test");

    // Just before lock time — staking should work
    p.set_time(LOCK_TIME - 1);
    let user_ok = p.create_user(50 * TOKEN);
    p.client
        .stake(&user_ok, &poll_id, &(50 * TOKEN), &StakeSide::Yes);

    // Exactly at lock time — staking should be rejected
    p.set_time(LOCK_TIME);
    let user_at = p.create_user(50 * TOKEN);
    let err = p
        .client
        .try_stake(&user_at, &poll_id, &(50 * TOKEN), &StakeSide::Yes)
        .expect_err("Should reject at exact lock time");
    assert_eq!(err, Ok(PredictXError::PollLocked));

    // After lock time — staking should be rejected
    p.set_time(LOCK_TIME + 1000);
    let user_late = p.create_user(50 * TOKEN);
    let err = p
        .client
        .try_stake(&user_late, &poll_id, &(50 * TOKEN), &StakeSide::Yes)
        .expect_err("Should reject after lock time");
    assert_eq!(err, Ok(PredictXError::PollLocked));
}

/// Emergency withdrawal timing: Disputed poll requires exact timeout.
#[test]
fn test_time_emergency_timeout_boundary() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Timeout test");

    let user = p.create_user(100 * TOKEN);
    p.client
        .stake(&user, &poll_id, &(100 * TOKEN), &StakeSide::Yes);

    // Set disputed at current time (BASE_TIME)
    p.set_oracle_status(poll_id, PollStatus::Disputed);

    // At BASE_TIME + EMERGENCY_TIMEOUT - 1 → not eligible
    p.set_time(BASE_TIME + EMERGENCY_TIMEOUT - 1);
    assert!(!p.client.check_emergency_eligible(&poll_id));

    // At BASE_TIME + EMERGENCY_TIMEOUT → eligible (>=)
    p.set_time(BASE_TIME + EMERGENCY_TIMEOUT);
    assert!(
        p.client.check_emergency_eligible(&poll_id),
        "Should be eligible at exactly EMERGENCY_TIMEOUT"
    );

    // Withdraw succeeds
    let refund = p.client.emergency_withdraw(&user, &poll_id);
    assert_eq!(refund, 100 * TOKEN);
}

/// Match creation rejects kickoff in the past.
#[test]
fn test_time_match_kickoff_must_be_future() {
    let p = TestPlatform::new();

    // Kickoff in the past
    let err = p
        .client
        .try_create_match(
            &p.admin,
            &p.s("A"),
            &p.s("B"),
            &p.s("L"),
            &p.s("V"),
            &(BASE_TIME - 1),
        )
        .expect_err("Should reject past kickoff");
    assert_eq!(err, Ok(PredictXError::InvalidLockTime));

    // Kickoff at exactly now
    let err = p
        .client
        .try_create_match(
            &p.admin,
            &p.s("A"),
            &p.s("B"),
            &p.s("L"),
            &p.s("V"),
            &BASE_TIME,
        )
        .expect_err("Should reject kickoff at current time");
    assert_eq!(err, Ok(PredictXError::InvalidLockTime));
}

// ══════════════════════════════════════════════════════════════════════════════
// Error & Edge Case Tests
// ══════════════════════════════════════════════════════════════════════════════

/// Double staking on the same poll is rejected.
#[test]
fn test_error_double_stake_rejected() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Double stake test");

    let user = p.create_user(200 * TOKEN);
    p.client
        .stake(&user, &poll_id, &(100 * TOKEN), &StakeSide::Yes);

    let err = p
        .client
        .try_stake(&user, &poll_id, &(100 * TOKEN), &StakeSide::No)
        .expect_err("Should reject double stake");
    assert_eq!(err, Ok(PredictXError::AlreadyStaked));
}

/// Staking below MIN_STAKE_AMOUNT is rejected.
#[test]
fn test_error_stake_below_minimum() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Min stake test");

    let user = p.create_user(1_000_000); // less than MIN_STAKE_AMOUNT (10_000_000)
    let err = p
        .client
        .try_stake(&user, &poll_id, &1_000_000_i128, &StakeSide::Yes)
        .expect_err("Should reject below minimum");
    assert_eq!(err, Ok(PredictXError::StakeBelowMinimum));
}

/// Emergency double withdrawal is prevented.
#[test]
fn test_error_emergency_double_withdrawal() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Double withdraw test");

    let user = p.create_user(100 * TOKEN);
    p.client
        .stake(&user, &poll_id, &(100 * TOKEN), &StakeSide::Yes);

    p.client.cancel_poll(&p.admin, &poll_id);
    p.client.emergency_withdraw(&user, &poll_id);

    // Second withdrawal should fail
    let err = p
        .client
        .try_emergency_withdraw(&user, &poll_id)
        .expect_err("Should reject double emergency withdrawal");
    assert_eq!(err, Ok(PredictXError::AlreadyClaimed));
}

/// Non-staker cannot emergency withdraw.
#[test]
fn test_error_emergency_withdraw_non_staker() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Non-staker test");

    // Someone else stakes
    let staker = p.create_user(100 * TOKEN);
    p.client
        .stake(&staker, &poll_id, &(100 * TOKEN), &StakeSide::Yes);

    p.client.cancel_poll(&p.admin, &poll_id);

    // Non-staker tries to withdraw
    let non_staker = p.create_user(0);
    let err = p
        .client
        .try_emergency_withdraw(&non_staker, &poll_id)
        .expect_err("Non-staker should not be able to withdraw");
    assert_eq!(err, Ok(PredictXError::NotStaker));
}

/// Creating a poll on a nonexistent match fails.
#[test]
fn test_error_poll_on_nonexistent_match() {
    let p = TestPlatform::new();

    let creator = p.create_user(0);
    let err = p
        .client
        .try_create_poll(
            &creator,
            &999,
            &p.s("Bogus poll"),
            &PollCategory::Other,
            &LOCK_TIME,
        )
        .expect_err("Should reject poll on nonexistent match");
    assert_eq!(err, Ok(PredictXError::MatchNotFound));
}

/// Staking on a nonexistent poll fails.
#[test]
fn test_error_stake_on_nonexistent_poll() {
    let p = TestPlatform::new();

    let user = p.create_user(100 * TOKEN);
    let err = p
        .client
        .try_stake(&user, &999, &(100 * TOKEN), &StakeSide::Yes)
        .expect_err("Should reject stake on nonexistent poll");
    assert_eq!(err, Ok(PredictXError::PollNotFound));
}

/// Non-admin cannot create match, finish match, or cancel poll.
#[test]
fn test_error_unauthorized_admin_actions() {
    let p = TestPlatform::new();

    let impostor = p.create_user(0);

    // Cannot create match
    let err = p
        .client
        .try_create_match(
            &impostor,
            &p.s("A"),
            &p.s("B"),
            &p.s("L"),
            &p.s("V"),
            &KICKOFF_TIME,
        )
        .expect_err("Non-admin should not create match");
    assert_eq!(err, Ok(PredictXError::Unauthorized));

    // Create a real match to test other actions
    let match_id = p.create_match();

    // Cannot finish match
    let err = p
        .client
        .try_finish_match(&impostor, &match_id)
        .expect_err("Non-admin should not finish match");
    assert_eq!(err, Ok(PredictXError::Unauthorized));

    // Cannot cancel poll
    let err = p
        .client
        .try_cancel_poll(&impostor, &1)
        .expect_err("Non-admin should not cancel poll");
    assert_eq!(err, Ok(PredictXError::Unauthorized));
}

/// Paused contract blocks staking and poll creation.
#[test]
fn test_error_paused_contract_blocks_operations() {
    let p = TestPlatform::new();

    let match_id = p.create_match();

    // Pause the contract
    p.client.pause(&p.admin);
    assert!(p.client.is_paused());

    // Cannot create poll
    let creator = p.create_user(0);
    let err = p
        .client
        .try_create_poll(
            &creator,
            &match_id,
            &p.s("Paused poll"),
            &PollCategory::Other,
            &LOCK_TIME,
        )
        .expect_err("Should reject poll creation when paused");
    assert_eq!(err, Ok(PredictXError::EmergencyWithdrawNotAllowed));

    // Unpause restores operations
    p.client.unpause(&p.admin);
    assert!(!p.client.is_paused());

    let poll_id = p.create_poll(match_id, "After unpause");
    assert!(poll_id > 0, "Should succeed after unpause");
}

// ══════════════════════════════════════════════════════════════════════════════
// Locked Status Emergency Withdrawal
// ══════════════════════════════════════════════════════════════════════════════
//
// Tests the emergency path when a poll stays Locked for > 7 days
// (match never finishes, voting never starts).

#[test]
fn test_emergency_withdrawal_locked_timeout() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Locked timeout test");

    let user = p.create_user(250 * TOKEN);
    p.client
        .stake(&user, &poll_id, &(250 * TOKEN), &StakeSide::No);

    // Set oracle status to Locked (simulating lock time passed but match never finishes)
    p.set_time(LOCK_TIME + 1);
    p.set_oracle_status(poll_id, PollStatus::Locked);

    // Not eligible before timeout (oracle set at LOCK_TIME+1, need LOCK_TIME+1+EMERGENCY_TIMEOUT)
    p.set_time(LOCK_TIME + 1 + EMERGENCY_TIMEOUT - 1);
    assert!(!p.client.check_emergency_eligible(&poll_id));

    // Eligible after timeout from when status was set
    p.set_time(LOCK_TIME + 1 + EMERGENCY_TIMEOUT);
    assert!(p.client.check_emergency_eligible(&poll_id));

    let refund = p.client.emergency_withdraw(&user, &poll_id);
    assert_eq!(refund, 250 * TOKEN, "Should get full refund");
    assert_eq!(p.contract_balance(), 0);
}

// ══════════════════════════════════════════════════════════════════════════════
// Multi-User Multi-Poll Emergency Withdrawal
// ══════════════════════════════════════════════════════════════════════════════
//
// Several users across multiple polls all emergency-withdraw after cancellation.
// Verifies no cross-contamination between polls.

#[test]
fn test_multi_poll_emergency_withdrawal_isolation() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_1 = p.create_poll(match_id, "Poll 1");
    let poll_2 = p.create_poll(match_id, "Poll 2");

    // Users on poll 1
    let u1 = p.create_user(100 * TOKEN);
    let u2 = p.create_user(200 * TOKEN);
    p.client
        .stake(&u1, &poll_1, &(100 * TOKEN), &StakeSide::Yes);
    p.client
        .stake(&u2, &poll_1, &(200 * TOKEN), &StakeSide::No);

    // Users on poll 2
    let u3 = p.create_user(300 * TOKEN);
    let u4 = p.create_user(400 * TOKEN);
    p.client
        .stake(&u3, &poll_2, &(300 * TOKEN), &StakeSide::Yes);
    p.client
        .stake(&u4, &poll_2, &(400 * TOKEN), &StakeSide::No);

    assert_eq!(p.contract_balance(), 1000 * TOKEN);

    // Cancel only poll 1
    p.client.cancel_poll(&p.admin, &poll_1);

    // Poll 1 users can withdraw
    p.client.emergency_withdraw(&u1, &poll_1);
    p.client.emergency_withdraw(&u2, &poll_1);
    assert_eq!(
        p.contract_balance(),
        700 * TOKEN,
        "After poll 1 withdrawals, poll 2 funds should remain"
    );

    // Poll 2 users cannot emergency withdraw (poll not cancelled or timed out)
    let err = p
        .client
        .try_emergency_withdraw(&u3, &poll_2)
        .expect_err("Poll 2 not cancelled");
    assert_eq!(err, Ok(PredictXError::EmergencyWithdrawNotAllowed));

    // Cancel poll 2 and withdraw
    p.client.cancel_poll(&p.admin, &poll_2);
    p.client.emergency_withdraw(&u3, &poll_2);
    p.client.emergency_withdraw(&u4, &poll_2);

    assert_eq!(
        p.contract_balance(),
        0,
        "Contract should be empty after all withdrawals"
    );

    // Verify each user got their exact stake back
    assert_eq!(p.balance_of(&u1), 100 * TOKEN);
    assert_eq!(p.balance_of(&u2), 200 * TOKEN);
    assert_eq!(p.balance_of(&u3), 300 * TOKEN);
    assert_eq!(p.balance_of(&u4), 400 * TOKEN);
}

// ══════════════════════════════════════════════════════════════════════════════
// User Staking Across Multiple Polls
// ══════════════════════════════════════════════════════════════════════════════

/// A single user can stake on multiple different polls.
#[test]
fn test_user_stakes_across_multiple_polls() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_1 = p.create_poll(match_id, "Poll A");
    let poll_2 = p.create_poll(match_id, "Poll B");
    let poll_3 = p.create_poll(match_id, "Poll C");

    let user = p.create_user(600 * TOKEN);

    p.client
        .stake(&user, &poll_1, &(100 * TOKEN), &StakeSide::Yes);
    p.client
        .stake(&user, &poll_2, &(200 * TOKEN), &StakeSide::No);
    p.client
        .stake(&user, &poll_3, &(300 * TOKEN), &StakeSide::Yes);

    // User should have 3 staked polls
    let user_stakes = p.client.get_user_stakes(&user);
    assert_eq!(user_stakes.len(), 3, "User should have stakes in 3 polls");

    // Remaining user balance
    assert_eq!(p.balance_of(&user), 0, "User staked all 600 tokens");

    // Contract holds 600
    assert_eq!(p.contract_balance(), 600 * TOKEN);

    // Verify individual stakes
    assert_eq!(
        p.client.get_stake_info(&poll_1, &user).amount,
        100 * TOKEN
    );
    assert_eq!(
        p.client.get_stake_info(&poll_2, &user).amount,
        200 * TOKEN
    );
    assert_eq!(
        p.client.get_stake_info(&poll_3, &user).amount,
        300 * TOKEN
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Potential Winnings Accuracy
// ══════════════════════════════════════════════════════════════════════════════

/// Potential winnings calculation is consistent with pool state and fee.
#[test]
fn test_potential_winnings_with_existing_pool() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "Winnings calc test");

    // Build pool: 700 Yes, 300 No
    let u1 = p.create_user(700 * TOKEN);
    let u2 = p.create_user(300 * TOKEN);
    p.client
        .stake(&u1, &poll_id, &(700 * TOKEN), &StakeSide::Yes);
    p.client
        .stake(&u2, &poll_id, &(300 * TOKEN), &StakeSide::No);

    // Calculate potential winnings for 100 tokens on Yes
    let new_stake = 100 * TOKEN;
    let winnings = p
        .client
        .calculate_potential_winnings(&poll_id, &StakeSide::Yes, &new_stake);

    // Manual calculation:
    // pool_on_side_after = 700 + 100 = 800 tokens
    // total_pool_after   = 700 + 300 + 100 = 1100 tokens
    // fee_factor = 10000 - 500 = 9500
    // winnings = 100 * 1100 * 9500 / (800 * 10000)
    let expected = new_stake * (1100 * TOKEN) * 9500 / ((800 * TOKEN) * 10_000);
    assert_eq!(winnings, expected, "Winnings calculation mismatch");
    assert!(winnings > new_stake, "Winner should profit");
    assert!(winnings < 1100 * TOKEN, "Winnings < total pool");

    // No side should yield higher winnings (minority bonus)
    let no_winnings = p
        .client
        .calculate_potential_winnings(&poll_id, &StakeSide::No, &new_stake);
    let no_expected = new_stake * (1100 * TOKEN) * 9500 / ((400 * TOKEN) * 10_000);
    assert_eq!(no_winnings, no_expected);
    assert!(
        no_winnings > winnings,
        "Minority side (No) should have higher potential winnings"
    );
}

/// First staker on empty pool gets (amount * 0.95) — just their own stake minus fee.
#[test]
fn test_potential_winnings_first_staker() {
    let p = TestPlatform::new();

    let match_id = p.create_match();
    let poll_id = p.create_poll(match_id, "First staker test");

    let amount = 100 * TOKEN;
    let winnings = p
        .client
        .calculate_potential_winnings(&poll_id, &StakeSide::Yes, &amount);

    // First staker: pool_on_side_after = amount, total_pool_after = amount
    // winnings = amount * amount * 9500 / (amount * 10000) = amount * 9500 / 10000
    let expected = amount * 9500 / 10_000;
    assert_eq!(
        winnings, expected,
        "First staker should get stake * (1 - 5% fee)"
    );
}
