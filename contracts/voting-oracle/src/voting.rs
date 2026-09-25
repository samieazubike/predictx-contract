use crate::{storage, DataKey};
use predictx_shared::{
    PollStatus, PredictXError, VoteChoice, VoteTally, AUTO_RESOLVE_THRESHOLD_BPS, BPS_DENOMINATOR,
    VOTER_REWARD_BPS, VOTING_WINDOW_SECS,
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
/// Out of scope for this change (tracked in separate issues): excluding stakers.
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

    // Each address may vote at most once per poll.
    if storage::has_voted(env, poll_id, &voter) {
        return Err(PredictXError::AlreadyVoted);
    }

    // ── Effects ───────────────────────────────────────────────────────────────

    // Load-or-create the tally, then record this vote.
    let mut tally = storage::read_tally(env, poll_id).unwrap_or(VoteTally {
        poll_id,
        yes_votes: 0,
        no_votes: 0,
        unclear_votes: 0,
        total_voters: 0,
        voting_end_time: crate::read_poll_status_updated_at(env, poll_id)
            .checked_add(VOTING_WINDOW_SECS)
            .unwrap_or(0),
        reward_pool: 0,
    });

    match choice {
        VoteChoice::Yes => tally.yes_votes += 1,
        VoteChoice::No => tally.no_votes += 1,
        VoteChoice::Unclear => tally.unclear_votes += 1,
    }
    tally.total_voters += 1;

    storage::write_tally(env, &tally);
    storage::write_voted(env, poll_id, &voter);
    Ok(tally)
}

/// Resolve a voting poll when the winning outcome reaches the automatic
/// resolution threshold after the voting window closes.
///
/// `total_pool` is the staking pool the caller (eventually the
/// `PredictionMarket` contract) settles against; `VOTER_REWARD_BPS` of it is
/// reserved for eligible voters and written to the tally's `reward_pool`.
/// The reserve is computed once at resolution: once the poll status moves to
/// `Resolved`, every later resolution attempt is rejected, so it is never
/// recomputed.
pub fn auto_resolve(
    env: &Env,
    poll_id: u64,
    total_pool: i128,
) -> Result<VoteChoice, PredictXError> {
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

    let mut tally = storage::read_tally(env, poll_id).ok_or(PredictXError::PollNotFound)?;
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

    // ── Effects ───────────────────────────────────────────────────────────────

    // Reserve the voters' share of the pool once, at resolution time. Reaching
    // this point guarantees the poll settles exactly once: every later attempt
    // is rejected by the `PollStatus` guard above, so the reserve is never
    // recomputed.
    tally.reward_pool = voter_reward_reserve(total_pool);
    storage::write_tally(env, &tally);

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

/// Share of the total pool reserved for eligible voters at resolution time.
///
/// Computes `total_pool * VOTER_REWARD_BPS / 10_000`, rounded down. Pure so
/// it can be unit-tested directly: non-positive pools reserve nothing, and
/// the default `VOTER_REWARD_BPS` of 100 yields exactly 1% of the pool.
pub(crate) fn voter_reward_reserve(total_pool: i128) -> i128 {
    if total_pool <= 0 {
        return 0;
    }
    total_pool * i128::from(VOTER_REWARD_BPS) / i128::from(BPS_DENOMINATOR)
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

    use predictx_shared::{
        PollStatus, PredictXError, VoteChoice, VoteTally, VOTER_REWARD_BPS, VOTING_WINDOW_SECS,
    };
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env,
    };

    use crate::{storage, VotingOracle, VotingOracleClient};

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

    /// Read a poll's tally from the contract's own storage (tests run outside
    /// the contract, so direct storage access must be wrapped).
    fn stored_tally(env: &Env, client: &VotingOracleClient, poll_id: u64) -> Option<VoteTally> {
        env.as_contract(&client.address, || storage::read_tally(env, poll_id))
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

        let outcome = client.auto_resolve(&1_u64, &1_000_000_000_000i128);
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
        cast_votes(&env, &client, 849, 151);
        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);

        let err = client
            .try_auto_resolve(&1_u64, &0i128)
            .expect_err("84.9% consensus must not auto-resolve");

        assert_eq!(err, Ok(PredictXError::ConsensusNotReached));
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Voting);
    }

    #[test]
    fn auto_resolve_rejects_open_voting_window() {
        let (env, _admin, client) = setup();
        cast_votes(&env, &client, 24, 1);

        let err = client
            .try_auto_resolve(&1_u64, &0i128)
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

    // ── voter_reward_reserve / auto_resolve reward pool ───────────────────────

    #[test]
    fn voter_reward_reserve_is_one_percent_at_default_bps() {
        assert_eq!(
            super::voter_reward_reserve(1_000_000_000_000i128),
            10_000_000_000i128
        );
        assert_eq!(super::voter_reward_reserve(1_000i128), 10i128);
        // Rounded down to whole token units.
        assert_eq!(super::voter_reward_reserve(55i128), 0i128);
    }

    #[test]
    fn auto_resolve_with_no_voters_reserves_zero() {
        let (env, _admin, client) = setup();
        // Seed a tally with zero voters so resolution reaches the voter
        // check with a stored tally in place.
        env.as_contract(&client.address, || {
            storage::write_tally(&env, &tally(0, 0, 0))
        });
        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);

        let err = client
            .try_auto_resolve(&1_u64, &1_000_000_000_000i128)
            .expect_err("a poll with no voters must not resolve");

        assert_eq!(err, Ok(PredictXError::ConsensusNotReached));
        let stored = stored_tally(&env, &client, 1_u64).expect("seeded tally must persist");
        assert_eq!(stored.reward_pool, 0, "no voters means nothing is reserved");
    }

    #[test]
    fn auto_resolve_reserves_reward_pool_from_total_pool() {
        let (env, _admin, client) = setup();
        cast_votes(&env, &client, 24, 1);
        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);

        let total_pool = 1_000_000_000_000i128;
        client.auto_resolve(&1_u64, &total_pool);

        let tally = stored_tally(&env, &client, 1_u64).expect("tally must exist after resolution");
        assert_eq!(
            tally.reward_pool,
            total_pool * i128::from(VOTER_REWARD_BPS) / 10_000,
            "reserve must be VOTER_REWARD_BPS of the settled pool"
        );
    }

    #[test]
    fn auto_resolve_recomputes_nothing_once_resolved() {
        let (env, _admin, client) = setup();
        cast_votes(&env, &client, 24, 1);
        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);

        client.auto_resolve(&1_u64, &1_000_000_000_000i128);
        let tally = stored_tally(&env, &client, 1_u64).expect("tally must exist after resolution");
        assert_eq!(tally.reward_pool, 10_000_000_000i128);

        // A later attempt (e.g. a different pool figure) must not touch the
        // stored reserve — resolution is idempotent-by-rejection.
        let err = client
            .try_auto_resolve(&1_u64, &500_000_000_000i128)
            .expect_err("a resolved poll must not resolve again");
        assert_eq!(err, Ok(PredictXError::VotingNotOpen));

        let tally_after = stored_tally(&env, &client, 1_u64).expect("tally must persist");
        assert_eq!(
            tally_after.reward_pool, 10_000_000_000i128,
            "reserve must be computed exactly once"
        );
    }
}
