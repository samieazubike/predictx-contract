use crate::{storage, DataKey, MAX_VOTERS};
use predictx_shared::{
    PollStatus, PredictXError, VoteChoice, VoteTally, AUTO_RESOLVE_THRESHOLD_BPS, BPS_DENOMINATOR,
    VOTING_WINDOW_SECS,
};
use soroban_sdk::{token, Address, Env, Symbol};

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
    voters.push_back(voter.clone());
    storage::write_voters(env, poll_id, &voters);
    storage::write_voted(env, poll_id, &voter);
    Ok(tally)
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

/// Claim a voter's share of a resolved poll's reserved reward pool.
///
/// Flow (Checks → Effects → Interactions):
/// 1. Authenticates the caller as the voter.
/// 2. Verifies the poll is resolved, else `PollNotLocked`.
/// 3. Rejects callers who did not vote, else `NotEligibleVoter`.
/// 4. Rejects a repeated claim, else `AlreadyClaimed`.
/// 5. Writes the `RewardClaimed` marker *before* transferring the tokens.
///
/// Eligibility is currently "cast a vote". Restricting rewards to voters who
/// backed the winning outcome is tracked separately.
pub fn claim_voter_reward(
    env: &Env,
    voter: Address,
    poll_id: u64,
) -> Result<i128, PredictXError> {
    voter.require_auth();

    // ── Checks ────────────────────────────────────────────────────────────────

    if !env
        .storage()
        .persistent()
        .has(&DataKey::PollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    // Rewards only unlock once the poll has settled.
    if crate::read_poll_status(env, poll_id) != PollStatus::Resolved {
        return Err(PredictXError::PollNotLocked);
    }

    let tally = storage::read_tally(env, poll_id).ok_or(PredictXError::PollNotFound)?;

    let voters = storage::read_voters(env, poll_id);
    if !voters.contains(voter.clone()) {
        return Err(PredictXError::NotEligibleVoter);
    }

    // A voter may only ever claim once. Checked here and re-armed below before
    // any tokens move.
    if storage::has_claimed_reward(env, poll_id, &voter) {
        return Err(PredictXError::AlreadyClaimed);
    }

    let amount = voter_reward_share(&tally, voters.len());
    if amount <= 0 {
        return Err(PredictXError::InsufficientBalance);
    }

    // ── Effects ───────────────────────────────────────────────────────────────

    // Set the marker before the transfer so a second claim cannot re-enter and
    // drain the reward pool.
    storage::write_reward_claimed(env, poll_id, &voter);

    // ── Interactions ──────────────────────────────────────────────────────────

    transfer_from_contract(env, &voter, amount)?;

    Ok(amount)
}

/// Share of the reserved voter reward pool owed to a single eligible voter.
///
/// The pool is split evenly across every eligible voter. Integer division
/// rounds down, leaving any remainder in the contract rather than overpaying.
/// Returns `0` when there is nothing to split.
pub(crate) fn voter_reward_share(tally: &VoteTally, eligible_voters: u32) -> i128 {
    if eligible_voters == 0 || tally.reward_pool <= 0 {
        return 0;
    }

    tally.reward_pool / i128::from(eligible_voters)
}

/// Read the configured voter-reward token, if one has been set.
fn reward_token(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::TokenAddress)
        .ok_or(PredictXError::NotInitialized)
}

/// Transfer `amount` of the reward token from this contract to `to`.
fn transfer_from_contract(env: &Env, to: &Address, amount: i128) -> Result<(), PredictXError> {
    let token_address = reward_token(env)?;
    let client = token::Client::new(env, &token_address);
    client.transfer(&env.current_contract_address(), to, &amount);
    Ok(())
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
        token, Address, Env, Vec,
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

    // ── claim_voter_reward / double-claim guard ───────────────────────────────

    /// Set up a resolved poll with `voter_count` voters, a reward token, and a
    /// reward pool backed by real minted tokens.
    ///
    /// No production path reserves the pool yet (tracked separately), so the
    /// tally is seeded directly here.
    fn setup_reward_poll(
        voter_count: u32,
        reward_pool: i128,
    ) -> (Env, VotingOracleClient<'static>, Address, Vec<Address>) {
        let (env, admin, client) = setup();

        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin);
        let token_address = token_contract.address();
        client.set_token_address(&admin, &token_address);

        let mut voters: Vec<Address> = Vec::new(&env);
        for _ in 0..voter_count {
            let v = voter(&env);
            client.cast_vote(&v, &1_u64, &VoteChoice::Yes);
            voters.push_back(v);
        }

        env.as_contract(&client.address, || {
            let mut stored = crate::storage::read_tally(&env, 1).unwrap();
            stored.reward_pool = reward_pool;
            crate::storage::write_tally(&env, &stored);
        });
        token::StellarAssetClient::new(&env, &token_address).mint(&client.address, &reward_pool);
        client.set_poll_status(&1_u64, &PollStatus::Resolved);

        (env, client, token_address, voters)
    }

    #[test]
    fn claim_voter_reward_pays_equal_share_and_marks_claimed() {
        let (env, client, token_address, voters) = setup_reward_poll(3, 300);
        let first = voters.get(0).unwrap();

        let paid = client.claim_voter_reward(&first, &1_u64);

        assert_eq!(paid, 100);
        assert_eq!(token::Client::new(&env, &token_address).balance(&first), 100);

        // The marker is persisted before the transfer completes.
        let claimed = env.as_contract(&client.address, || {
            crate::storage::has_claimed_reward(&env, 1, &first)
        });
        assert!(claimed);
    }

    #[test]
    fn second_claim_returns_already_claimed() {
        let (_env, client, _token_address, voters) = setup_reward_poll(2, 200);
        let first = voters.get(0).unwrap();

        client.claim_voter_reward(&first, &1_u64);

        let err = client
            .try_claim_voter_reward(&first, &1_u64)
            .expect_err("a second claim must be rejected");

        assert_eq!(err, Ok(PredictXError::AlreadyClaimed));
    }

    #[test]
    fn second_claim_does_not_change_contract_balance() {
        let (env, client, token_address, voters) = setup_reward_poll(2, 200);
        let first = voters.get(0).unwrap();
        client.claim_voter_reward(&first, &1_u64);

        let tokens = token::Client::new(&env, &token_address);
        let balance_after_first_claim = tokens.balance(&client.address);

        let _ = client
            .try_claim_voter_reward(&first, &1_u64)
            .expect_err("second claim must fail");

        assert_eq!(tokens.balance(&client.address), balance_after_first_claim);
    }
}
