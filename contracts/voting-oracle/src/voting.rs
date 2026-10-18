use crate::{storage, DataKey, MAX_VOTERS};
use predictx_shared::{
    PollStatus, PredictXError, VoteChoice, VoteTally, AUTO_RESOLVE_THRESHOLD_BPS, BPS_DENOMINATOR,
    VOTING_WINDOW_SECS,
};
use soroban_sdk::{Address, Env, Symbol};

/// Record a voter's choice on a poll.
///
/// Flow (Checks → Effects):
/// 1. Authenticates the caller as the voter.
/// 2. Verifies the poll is known to the oracle, else `PollNotFound`.
/// 3. Rejects a duplicate vote from the same voter, else `AlreadyVoted`.
/// 4. Loads the existing tally (or seeds a fresh one) and increments the
///    chosen outcome's counter plus the total voter count.
/// 5. Persists the updated tally and the per-voter dedup marker, and returns
///    the tally.
///
/// Votes are rejected once the voting window has closed, and — when a market
/// is registered via `set_market` — from anyone who staked on the poll.
pub fn cast_vote(
    env: &Env,
    voter: Address,
    poll_id: u64,
    choice: VoteChoice,
) -> Result<VoteTally, PredictXError> {
    voter.require_auth();

    // ── Checks ────────────────────────────────────────────────────────────────

    // Only accept votes on polls the oracle already knows about.
    if !env
        .storage()
        .persistent()
        .has(&DataKey::PollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    if crate::read_poll_status(env, poll_id) != PollStatus::Voting {
        return Err(PredictXError::VotingNotOpen);
    }

    let voting_end_time = crate::read_poll_status_updated_at(env, poll_id)
        .checked_add(VOTING_WINDOW_SECS)
        .unwrap_or(0);
    if env.ledger().timestamp() >= voting_end_time {
        return Err(PredictXError::VotingWindowExpired);
    }

    // Stakers have a financial interest in the outcome and may not vote on it.
    if let Some(market) = env
        .storage()
        .instance()
        .get::<DataKey, Address>(&DataKey::Market)
    {
        if crate::MarketClient::new(env, &market).has_user_staked(&poll_id, &voter) {
            return Err(PredictXError::VoterIsStaker);
        }
    }

    // Each address may vote at most once per poll.
    let mut voters = storage::read_voters(env, poll_id);
    if storage::has_voted(env, poll_id, &voter) || voters.contains(voter.clone()) {
        return Err(PredictXError::AlreadyVoted);
    }

    if voters.len() >= MAX_VOTERS {
        return Err(PredictXError::MaxVotersReached);
    }

    // ── Effects ───────────────────────────────────────────────────────────────

    // Load-or-create the tally, then record this vote.
    let mut tally = storage::read_tally(env, poll_id).unwrap_or(VoteTally {
        poll_id,
        yes_votes: 0,
        no_votes: 0,
        unclear_votes: 0,
        total_voters: 0,
        voting_end_time,
        reward_pool: 0,
    });

    match choice {
        VoteChoice::Yes => tally.yes_votes += 1,
        VoteChoice::No => tally.no_votes += 1,
        VoteChoice::Unclear => tally.unclear_votes += 1,
    }
    tally.total_voters += 1;

    storage::write_tally(env, &tally);
    voters.push_back(voter.clone());
    storage::write_voters(env, poll_id, &voters);
    storage::write_voted(env, poll_id, &voter);
    // Persist the choice itself (not just the count) so reward eligibility can
    // be checked against the resolved outcome later.
    storage::write_vote_choice(env, poll_id, &voter, choice);
    Ok(tally)
}

/// Set (fund) the voter reward reserve for `poll_id`.
///
/// Admin-only. The reserve *size* policy is out of scope; this just records the
/// amount that eligible voters will share.
pub fn set_reward_pool(
    env: &Env,
    caller: Address,
    poll_id: u64,
    amount: i128,
) -> Result<(), PredictXError> {
    storage::require_admin(env, &caller)?;
    caller.require_auth();

    if amount < 0 {
        return Err(PredictXError::InvalidRewardAmount);
    }
    if !env
        .storage()
        .persistent()
        .has(&DataKey::PollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    storage::write_reward_pool(env, poll_id, amount);
    Ok(())
}

/// Claim the caller's voter reward for a resolved poll.
///
/// Only voters who backed the winning outcome are eligible: a voter on the
/// losing side, an `Unclear` voter, or a non-voter is rejected with
/// [`PredictXError::VoterNotEligible`]. The reward reserve is split evenly
/// across the *eligible* voters (never the total voter count), and each voter
/// may claim at most once.
pub fn claim_reward(env: &Env, voter: Address, poll_id: u64) -> Result<i128, PredictXError> {
    voter.require_auth();

    if !env
        .storage()
        .persistent()
        .has(&DataKey::PollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    // Rewards only open once the poll has a resolved, decisive outcome.
    let outcome: VoteChoice = env
        .storage()
        .persistent()
        .get(&DataKey::PollOutcome(poll_id))
        .ok_or(PredictXError::OutcomeNotAvailable)?;
    if outcome == VoteChoice::Unclear {
        return Err(PredictXError::OutcomeNotAvailable);
    }

    if storage::has_claimed_reward(env, poll_id, &voter) {
        return Err(PredictXError::AlreadyClaimed);
    }

    // The claimant must have voted, and must have backed the winning side.
    let choice =
        storage::read_vote_choice(env, poll_id, &voter).ok_or(PredictXError::VoterNotEligible)?;
    if choice != outcome {
        return Err(PredictXError::VoterNotEligible);
    }

    let eligible = eligible_voter_count(env, poll_id, outcome);
    if eligible == 0 {
        return Err(PredictXError::VoterNotEligible);
    }

    let pool = storage::read_reward_pool(env, poll_id);
    let share = if pool > 0 {
        pool / i128::from(eligible)
    } else {
        0
    };

    storage::write_reward_claimed(env, poll_id, &voter);
    env.storage()
        .persistent()
        .set(&DataKey::VoterReward(poll_id, voter.clone()), &share);

    env.events()
        .publish((Symbol::new(env, "RewardClaimed"), poll_id, voter), share);

    Ok(share)
}

/// Count the voters on `poll_id` whose recorded choice equals `outcome`.
///
/// Bounded by `MAX_VOTERS`, so a full roster scan is cheap.
fn eligible_voter_count(env: &Env, poll_id: u64, outcome: VoteChoice) -> u32 {
    let voters = storage::read_voters(env, poll_id);
    let mut count = 0_u32;
    for i in 0..voters.len() {
        let voter = voters.get(i).unwrap();
        if storage::read_vote_choice(env, poll_id, &voter) == Some(outcome) {
            count += 1;
        }
    }
    count
}

/// Resolve a voting poll when the winning outcome reaches the automatic
/// resolution threshold after the voting window closes.
pub fn auto_resolve(env: &Env, poll_id: u64) -> Result<VoteChoice, PredictXError> {
    if !env
        .storage()
        .persistent()
        .has(&DataKey::PollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    if crate::read_poll_status(env, poll_id) != PollStatus::Voting {
        return Err(PredictXError::VotingNotOpen);
    }

    let tally = storage::read_tally(env, poll_id).ok_or(PredictXError::PollNotFound)?;
    if env.ledger().timestamp() < tally.voting_end_time {
        return Err(PredictXError::VotingNotOpen);
    }

    let (outcome, winning_votes) =
        if tally.yes_votes >= tally.no_votes && tally.yes_votes >= tally.unclear_votes {
            (VoteChoice::Yes, tally.yes_votes)
        } else if tally.no_votes >= tally.unclear_votes {
            (VoteChoice::No, tally.no_votes)
        } else {
            (VoteChoice::Unclear, tally.unclear_votes)
        };

    if tally.total_voters == 0 {
        return Err(PredictXError::ConsensusNotReached);
    }

    let consensus_bps = (u64::from(winning_votes) * u64::from(BPS_DENOMINATOR)
        / u64::from(tally.total_voters)) as u32;
    if consensus_bps < AUTO_RESOLVE_THRESHOLD_BPS {
        return Err(PredictXError::ConsensusNotReached);
    }

    let now = env.ledger().timestamp();
    let stored_status = crate::StoredPollStatus {
        status: PollStatus::Resolved,
        updated_at: now,
    };
    env.storage()
        .persistent()
        .set(&DataKey::PollStatus(poll_id), &stored_status);
    env.storage()
        .persistent()
        .set(&DataKey::PollOutcome(poll_id), &outcome);

    env.events().publish(
        (Symbol::new(env, "AutoResolved"), poll_id, outcome),
        consensus_bps,
    );

    Ok(outcome)
}

/// Share of the decisive (Yes/No) votes held by the leading outcome.
///
/// Returns `(leading_is_yes, share_bps)`, where `share_bps` is rounded down
/// to whole basis points out of [`BPS_DENOMINATOR`].
///
/// - `Unclear` votes are excluded from the denominator: they signal "cannot
///   judge", not a preference.
/// - A Yes/No tie resolves to Yes (`true`) at 5000 bps, so the result is
///   deterministic.
/// - A tally with no decisive votes (e.g. all `Unclear`) returns `(false, 0)`
///   instead of dividing by zero.
///
/// Pure and side-effect free so the routing thresholds can be unit-tested
/// against it directly.
#[allow(dead_code)] // consumed by the upcoming threshold-routing issues
pub(crate) fn consensus_bps(tally: &VoteTally) -> (bool, u32) {
    let decisive = u64::from(tally.yes_votes) + u64::from(tally.no_votes);
    if decisive == 0 {
        return (false, 0);
    }

    let leading_is_yes = tally.yes_votes >= tally.no_votes;
    let leading_votes = if leading_is_yes {
        tally.yes_votes
    } else {
        tally.no_votes
    };

    let share_bps = (u64::from(leading_votes) * u64::from(BPS_DENOMINATOR) / decisive) as u32;
    (leading_is_yes, share_bps)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use predictx_shared::{PollStatus, PredictXError, VoteChoice, VOTING_WINDOW_SECS};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };

    use crate::{VotingOracle, VotingOracleClient, MAX_VOTERS};

    fn setup() -> (Env, Address, VotingOracleClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &cid);
        let admin = Address::generate(&env);

        client.initialize(&admin);
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        // Register poll 1 as a known poll. `initiate_voting` (#80) will later
        // be the real production path for this transition.
        client.set_poll_status(&1_u64, &PollStatus::Voting);

        (env, admin, client)
    }

    fn voter(env: &Env) -> Address {
        Address::generate(env)
    }

    #[test]
    fn cast_vote_records_choice_and_counts_voters() {
        let (env, _admin, client) = setup();

        let tally = client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);

        assert_eq!(tally.poll_id, 1);
        assert_eq!(tally.yes_votes, 1);
        assert_eq!(tally.no_votes, 0);
        assert_eq!(tally.unclear_votes, 0);
        assert_eq!(tally.total_voters, 1);
    }

    #[test]
    fn cast_vote_records_distinct_voters_in_persistent_roster() {
        let (env, _admin, client) = setup();
        let first = voter(&env);
        let second = voter(&env);

        client.cast_vote(&first, &1_u64, &VoteChoice::Yes);
        client.cast_vote(&second, &1_u64, &VoteChoice::No);

        let voters = client.get_voters(&1_u64);
        assert_eq!(voters.len(), 2);
        assert_eq!(voters.get(0).unwrap(), first);
        assert_eq!(voters.get(1).unwrap(), second);
    }

    #[test]
    fn duplicate_vote_does_not_duplicate_voter_roster_entry() {
        let (env, _admin, client) = setup();
        let voter = voter(&env);

        client.cast_vote(&voter, &1_u64, &VoteChoice::Yes);
        let err = client
            .try_cast_vote(&voter, &1_u64, &VoteChoice::No)
            .expect_err("duplicate vote must be rejected");

        assert_eq!(err, Ok(PredictXError::AlreadyVoted));
        assert_eq!(client.get_voters(&1_u64).len(), 1);
    }

    #[test]
    fn cast_vote_rejects_voter_roster_over_cap() {
        let (env, _admin, client) = setup();

        for _ in 0..MAX_VOTERS {
            client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        }

        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes)
            .expect_err("voter roster cap must be enforced");

        assert_eq!(err, Ok(PredictXError::MaxVotersReached));
        assert_eq!(client.get_voters(&1_u64).len(), MAX_VOTERS);
    }

    #[test]
    fn cast_vote_updates_persisted_tally() {
        let (env, _admin, client) = setup();

        client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);

        let tally = client.cast_vote(&voter(&env), &1_u64, &VoteChoice::No);

        assert_eq!(tally.yes_votes, 1);
        assert_eq!(tally.no_votes, 1);
        assert_eq!(tally.total_voters, 2);
    }

    #[test]
    fn cast_vote_accumulates_all_three_choices() {
        let (env, _admin, client) = setup();

        client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        client.cast_vote(&voter(&env), &1_u64, &VoteChoice::No);
        let tally = client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Unclear);

        assert_eq!(tally.yes_votes, 1);
        assert_eq!(tally.no_votes, 1);
        assert_eq!(tally.unclear_votes, 1);
        assert_eq!(tally.total_voters, 3);
    }

    #[test]
    fn cast_vote_rejects_unknown_poll() {
        let (env, _admin, client) = setup();

        let err = client
            .try_cast_vote(&voter(&env), &999_u64, &VoteChoice::Yes)
            .expect_err("unknown poll must be rejected");

        assert_eq!(err, Ok(PredictXError::PollNotFound));
    }

    #[test]
    fn cast_vote_rejects_active_poll() {
        let (env, _admin, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Active);

        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes)
            .expect_err("active poll must reject voting");

        assert_eq!(err, Ok(PredictXError::VotingNotOpen));
    }

    #[test]
    fn cast_vote_rejects_resolved_poll() {
        let (env, _admin, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Resolved);

        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes)
            .expect_err("resolved poll must reject voting");

        assert_eq!(err, Ok(PredictXError::VotingNotOpen));
    }

    #[test]
    fn cast_vote_rejects_duplicate_vote_from_same_voter() {
        let (env, _admin, client) = setup();
        let v = voter(&env);

        client.cast_vote(&v, &1_u64, &VoteChoice::Yes);

        let err = client
            .try_cast_vote(&v, &1_u64, &VoteChoice::No)
            .expect_err("a second vote from the same voter must be rejected");

        assert_eq!(err, Ok(PredictXError::AlreadyVoted));
    }

    #[test]
    fn rejected_duplicate_vote_leaves_tally_unchanged() {
        let (env, _admin, client) = setup();
        let v = voter(&env);

        client.cast_vote(&v, &1_u64, &VoteChoice::Yes);
        let rejected = client
            .try_cast_vote(&v, &1_u64, &VoteChoice::No)
            .expect_err("second vote must be rejected");
        assert_eq!(rejected, Ok(PredictXError::AlreadyVoted));

        // A fresh voter's tally proves the rejected vote added nothing.
        let tally = client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Unclear);

        assert_eq!(tally.yes_votes, 1);
        assert_eq!(tally.no_votes, 0);
        assert_eq!(tally.unclear_votes, 1);
        assert_eq!(tally.total_voters, 2);
    }

    #[test]
    fn two_different_voters_can_vote_on_the_same_poll() {
        let (env, _admin, client) = setup();

        client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        let tally = client.cast_vote(&voter(&env), &1_u64, &VoteChoice::No);

        assert_eq!(tally.yes_votes, 1);
        assert_eq!(tally.no_votes, 1);
        assert_eq!(tally.total_voters, 2);
    }

    #[test]
    fn same_voter_can_vote_on_two_different_polls() {
        let (env, _admin, client) = setup();
        let v = voter(&env);

        client.cast_vote(&v, &1_u64, &VoteChoice::Yes);
        client.set_poll_status(&2_u64, &PollStatus::Voting);

        let tally = client.cast_vote(&v, &2_u64, &VoteChoice::Yes);

        assert_eq!(tally.poll_id, 2);
        assert_eq!(tally.yes_votes, 1);
        assert_eq!(tally.total_voters, 1);
    }

    fn cast_votes(env: &Env, client: &VotingOracleClient, yes_votes: u32, no_votes: u32) {
        for _ in 0..yes_votes {
            client.cast_vote(&voter(env), &1_u64, &VoteChoice::Yes);
        }
        for _ in 0..no_votes {
            client.cast_vote(&voter(env), &1_u64, &VoteChoice::No);
        }
    }

    #[test]
    fn auto_resolves_at_or_above_threshold_and_emits_event() {
        use soroban_sdk::{testutils::Events, TryIntoVal};

        let (env, _admin, client) = setup();
        cast_votes(&env, &client, 24, 1);
        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);

        let outcome = client.auto_resolve(&1_u64);
        let events = env.events().all();

        assert_eq!(outcome, VoteChoice::Yes);
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Resolved);
        assert_eq!(client.get_poll_outcome(&1_u64), VoteChoice::Yes);

        assert_eq!(events.len(), 1);
        let (_, topics, data) = events.get(0).unwrap();
        let name: soroban_sdk::Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
        let event_outcome: VoteChoice = topics.get(2).unwrap().try_into_val(&env).unwrap();
        let consensus_bps: u32 = data.try_into_val(&env).unwrap();
        assert_eq!(name, soroban_sdk::Symbol::new(&env, "AutoResolved"));
        assert_eq!(event_outcome, VoteChoice::Yes);
        assert_eq!(consensus_bps, 9_600);
    }

    #[test]
    fn auto_resolve_rejects_consensus_below_threshold() {
        let (env, _admin, client) = setup();
        cast_votes(&env, &client, 54, 10);
        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);

        let err = client
            .try_auto_resolve(&1_u64)
            .expect_err("84.9% consensus must not auto-resolve");

        assert_eq!(err, Ok(PredictXError::ConsensusNotReached));
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Voting);
    }

    #[test]
    fn auto_resolve_rejects_open_voting_window() {
        let (env, _admin, client) = setup();
        cast_votes(&env, &client, 24, 1);

        let err = client
            .try_auto_resolve(&1_u64)
            .expect_err("resolution must wait for the voting window to close");

        assert_eq!(err, Ok(PredictXError::VotingNotOpen));
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Voting);
    }

    // ── consensus_bps ─────────────────────────────────────────────────────────

    fn tally(yes_votes: u32, no_votes: u32, unclear_votes: u32) -> predictx_shared::VoteTally {
        predictx_shared::VoteTally {
            poll_id: 1,
            yes_votes,
            no_votes,
            unclear_votes,
            total_voters: yes_votes + no_votes + unclear_votes,
            voting_end_time: 0,
            reward_pool: 0,
        }
    }

    #[test]
    fn consensus_bps_matches_spec_worked_example() {
        assert_eq!(super::consensus_bps(&tally(45, 2, 0)), (true, 9_574));
        assert_eq!(super::consensus_bps(&tally(2, 45, 0)), (false, 9_574));
    }

    #[test]
    fn consensus_bps_ignores_unclear_votes() {
        assert_eq!(
            super::consensus_bps(&tally(45, 2, 30)),
            super::consensus_bps(&tally(45, 2, 0))
        );
    }

    #[test]
    fn consensus_bps_all_unclear_returns_zero() {
        assert_eq!(super::consensus_bps(&tally(0, 0, 7)), (false, 0));
        assert_eq!(super::consensus_bps(&tally(0, 0, 0)), (false, 0));
    }

    #[test]
    fn consensus_bps_tie_favours_yes() {
        assert_eq!(super::consensus_bps(&tally(10, 10, 3)), (true, 5_000));
    }

    // ── Voter rewards (#105) ──────────────────────────────────────────────────

    /// Fund poll 1 and resolve it to `Yes` (30 Yes / 5 No == 85.7% consensus).
    fn fund_and_resolve_yes(env: &Env, client: &VotingOracleClient, admin: &Address, pool: i128) {
        client.set_reward_pool(admin, &1_u64, &pool);
        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);
        assert_eq!(client.auto_resolve(&1_u64), VoteChoice::Yes);
    }

    #[test]
    fn majority_voter_claims_share_divided_by_eligible_count() {
        let (env, admin, client) = setup();
        let winner = voter(&env);
        client.cast_vote(&winner, &1_u64, &VoteChoice::Yes);
        for _ in 0..29 {
            client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        }
        for _ in 0..5 {
            client.cast_vote(&voter(&env), &1_u64, &VoteChoice::No);
        }
        fund_and_resolve_yes(&env, &client, &admin, 300);

        // 300 shared across the 30 eligible (winning) voters, not 300/35.
        assert_eq!(client.claim_reward(&winner, &1_u64), 10);
        assert!(client.has_claimed_reward(&1_u64, &winner));
    }

    #[test]
    fn minority_voter_cannot_claim() {
        let (env, admin, client) = setup();
        let loser = voter(&env);
        client.cast_vote(&loser, &1_u64, &VoteChoice::No);
        for _ in 0..30 {
            client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        }
        fund_and_resolve_yes(&env, &client, &admin, 300);

        let err = client
            .try_claim_reward(&loser, &1_u64)
            .expect_err("a losing-side voter must not be paid");

        assert_eq!(err, Ok(PredictXError::VoterNotEligible));
        assert!(!client.has_claimed_reward(&1_u64, &loser));
    }

    #[test]
    fn unclear_voter_cannot_claim() {
        let (env, admin, client) = setup();
        let unclear = voter(&env);
        client.cast_vote(&unclear, &1_u64, &VoteChoice::Unclear);
        for _ in 0..30 {
            client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        }
        fund_and_resolve_yes(&env, &client, &admin, 300);

        let err = client
            .try_claim_reward(&unclear, &1_u64)
            .expect_err("an Unclear voter must not be paid");

        assert_eq!(err, Ok(PredictXError::VoterNotEligible));
    }

    #[test]
    fn claim_rejected_before_resolution_and_on_double_claim() {
        let (env, admin, client) = setup();
        let winner = voter(&env);
        client.cast_vote(&winner, &1_u64, &VoteChoice::Yes);
        for _ in 0..29 {
            client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        }
        client.set_reward_pool(&admin, &1_u64, &300);

        // No resolved outcome yet.
        let early = client
            .try_claim_reward(&winner, &1_u64)
            .expect_err("claims must wait for a resolved outcome");
        assert_eq!(early, Ok(PredictXError::OutcomeNotAvailable));

        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);
        client.auto_resolve(&1_u64);
        client.claim_reward(&winner, &1_u64);

        let twice = client
            .try_claim_reward(&winner, &1_u64)
            .expect_err("a second claim must be rejected");
        assert_eq!(twice, Ok(PredictXError::AlreadyClaimed));
    }

    // ── Security regressions ─────────────────────────────────────────────────

    #[test]
    fn cast_vote_rejects_after_voting_window_closes() {
        let (env, _, client) = setup();
        env.ledger()
            .with_mut(|l| l.timestamp += VOTING_WINDOW_SECS);
        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, PredictXError::VotingWindowExpired);
    }

    #[test]
    fn late_vote_cannot_reset_tally_and_decide_alone() {
        // Before the fix a stalled tally could be re-seeded by a late voter
        // whose single vote then cleared the auto-resolve threshold.
        let (env, _, client) = setup();
        for _ in 0..5 {
            client.cast_vote(&voter(&env), &1_u64, &VoteChoice::No);
        }
        env.ledger()
            .with_mut(|l| l.timestamp += VOTING_WINDOW_SECS + 1);
        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, PredictXError::VotingWindowExpired);
        assert_eq!(client.auto_resolve(&1_u64), VoteChoice::No);
    }

    #[test]
    fn tally_survives_past_temporary_storage_ttl() {
        let (env, _, client) = setup();
        client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);

        // Advance far beyond the minimum temporary-entry TTL while staying
        // inside the voting window.
        env.ledger().with_mut(|l| {
            l.sequence_number += 2_000;
            l.timestamp += 600;
        });

        let tally = client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        assert_eq!(tally.total_voters, 2);
        assert_eq!(tally.yes_votes, 2);
    }

    #[test]
    fn set_market_is_admin_only() {
        let (env, admin, client) = setup();
        let market = Address::generate(&env);
        let err = client
            .try_set_market(&voter(&env), &market)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, PredictXError::Unauthorized);
        client.set_market(&admin, &market);
        assert_eq!(client.market(), Some(market));
    }
}
