use crate::{storage, DataKey, MAX_VOTERS};
use predictx_shared::{
    Dispute, PollStatus, PredictXError, VoteChoice, VoteTally, AUTO_RESOLVE_THRESHOLD_BPS,
    BPS_DENOMINATOR, DISPUTE_FEE, MULTI_SIG_REQUIRED, VOTING_WINDOW_SECS,
};
use soroban_sdk::{token, Address, Env, String, Symbol};

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

/// Initiate a dispute against a resolved poll.
///
/// Flow (Checks → Effects):
/// 1. Authenticates the caller as the initiator.
/// 2. Verifies the poll exists and is in `Resolved` status.
/// 3. Transfers the fixed dispute fee from the initiator to the contract.
/// 4. Constructs a `Dispute` record and persists it.
/// 5. Transitions the poll status to `Disputed`.
/// 6. Emits a `DisputeInitiated(poll_id, initiator)` event.
///
/// Out of scope: dispute window, duplicate guard, fee refund/forfeit.
pub fn initiate_dispute(
    env: &Env,
    initiator: Address,
    poll_id: u64,
    evidence_hash: String,
) -> Result<Dispute, PredictXError> {
    initiator.require_auth();

    // ── Checks ────────────────────────────────────────────────────────────────

    // Poll must be known to the oracle.
    if !env
        .storage()
        .persistent()
        .has(&DataKey::PollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    // Only resolved polls can be disputed.
    if crate::read_poll_status(env, poll_id) != PollStatus::Resolved {
        return Err(PredictXError::PollAlreadyResolved);
    }

    // ── Fee transfer ──────────────────────────────────────────────────────────

    let token_addr = crate::get_token_address(env)?;
    let token_client = token::Client::new(env, &token_addr);

    // Verify the initiator can provide the required dispute fee.
    if DISPUTE_FEE <= 0 || token_client.balance(&initiator) < DISPUTE_FEE {
        return Err(PredictXError::DisputeFeeRequired);
    }

    token_client.transfer(&initiator, &env.current_contract_address(), &DISPUTE_FEE);

    // ── Effects ───────────────────────────────────────────────────────────────

    let now = env.ledger().timestamp();

    let dispute = Dispute {
        poll_id,
        initiator: initiator.clone(),
        evidence_hash,
        dispute_fee: DISPUTE_FEE,
        admin_approvals: 0,
        required_approvals: MULTI_SIG_REQUIRED,
        resolved: false,
        initiated_at: now,
    };

    storage::write_dispute(env, &dispute);

    // Transition poll to Disputed.
    let stored_status = crate::StoredPollStatus {
        status: PollStatus::Disputed,
        updated_at: now,
    };
    env.storage()
        .persistent()
        .set(&DataKey::PollStatus(poll_id), &stored_status);

    env.events().publish(
        (Symbol::new(env, "DisputeInitiated"), poll_id),
        initiator,
    );

    Ok(dispute)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use predictx_shared::{
        Dispute, PollStatus, PredictXError, VoteChoice, DISPUTE_FEE, MULTI_SIG_REQUIRED,
        VOTING_WINDOW_SECS,
    };
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env, String,
    };

    use crate::{VotingOracle, VotingOracleClient, MAX_VOTERS};

    fn setup() -> (Env, Address, Address, VotingOracleClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &cid);
        let admin = Address::generate(&env);

        // Deploy a test token and fund the contract.
        let token_admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_addr = token_id.address();
        let token_client =
            soroban_sdk::token::StellarAssetClient::new(&env, &token_addr);

        client.initialize(&admin, &token_addr);
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);

        // Mint tokens that dispute tests can use.
        token_client.mint(&admin, &(DISPUTE_FEE * 10));

        // Register poll 1 as a known poll. `initiate_voting` (#80) will later
        // be the real production path for this transition.
        client.set_poll_status(&1_u64, &PollStatus::Voting);

        (env, admin, token_addr, client)
    }

    fn voter(env: &Env) -> Address {
        Address::generate(env)
    }

    #[test]
    fn cast_vote_records_choice_and_counts_voters() {
        let (env, _admin, _token, client) = setup();

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
        let (env, _admin, _token, client) = setup();

        client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);

        let tally = client.cast_vote(&voter(&env), &1_u64, &VoteChoice::No);

        assert_eq!(tally.yes_votes, 1);
        assert_eq!(tally.no_votes, 1);
        assert_eq!(tally.total_voters, 2);
    }

    #[test]
    fn cast_vote_accumulates_all_three_choices() {
        let (env, _admin, _token, client) = setup();

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
        let (env, _admin, _token, client) = setup();

        let err = client
            .try_cast_vote(&voter(&env), &999_u64, &VoteChoice::Yes)
            .expect_err("unknown poll must be rejected");

        assert_eq!(err, Ok(PredictXError::PollNotFound));
    }

    #[test]
    fn cast_vote_rejects_active_poll() {
        let (env, _admin, _token, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Active);

        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes)
            .expect_err("active poll must reject voting");

        assert_eq!(err, Ok(PredictXError::VotingNotOpen));
    }

    #[test]
    fn cast_vote_rejects_resolved_poll() {
        let (env, _admin, _token, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Resolved);

        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes)
            .expect_err("resolved poll must reject voting");

        assert_eq!(err, Ok(PredictXError::VotingNotOpen));
    }

    #[test]
    fn cast_vote_rejects_duplicate_vote_from_same_voter() {
        let (env, _admin, _token, client) = setup();
        let v = voter(&env);

        client.cast_vote(&v, &1_u64, &VoteChoice::Yes);

        let err = client
            .try_cast_vote(&v, &1_u64, &VoteChoice::No)
            .expect_err("a second vote from the same voter must be rejected");

        assert_eq!(err, Ok(PredictXError::AlreadyVoted));
    }

    #[test]
    fn rejected_duplicate_vote_leaves_tally_unchanged() {
        let (env, _admin, _token, client) = setup();
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
        let (env, _admin, _token, client) = setup();

        client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        let tally = client.cast_vote(&voter(&env), &1_u64, &VoteChoice::No);

        assert_eq!(tally.yes_votes, 1);
        assert_eq!(tally.no_votes, 1);
        assert_eq!(tally.total_voters, 2);
    }

    #[test]
    fn same_voter_can_vote_on_two_different_polls() {
        let (env, _admin, _token, client) = setup();
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

    // ── initiate_dispute ──────────────────────────────────────────────────────

    /// Helper: set up a resolved poll and a funded initiator for dispute tests.
    fn dispute_setup() -> (Env, Address, Address, VotingOracleClient<'static>) {
        let (env, admin, token_addr, client) = setup();

        // Set poll 1 to Resolved so it can be disputed.
        client.set_poll_status(&1_u64, &PollStatus::Resolved);

        // Create and fund the dispute initiator.
        let initiator = Address::generate(&env);
        let sac_client =
            soroban_sdk::token::StellarAssetClient::new(&env, &token_addr);
        // Mint directly to initiator (mock_all_auths bypasses admin check).
        sac_client.mint(&initiator, &(DISPUTE_FEE * 2));

        (env, admin, initiator, client)
    }

    #[test]
    fn initiate_dispute_persists_dispute_and_transitions_status() {
        let (env, _admin, initiator, client) = dispute_setup();
        let evidence = String::from_str(&env, "QmEvidence123");

        let dispute = client.initiate_dispute(&initiator, &1_u64, &evidence);

        // Dispute fields are correct.
        assert_eq!(dispute.poll_id, 1);
        assert_eq!(dispute.initiator, initiator);
        assert_eq!(dispute.dispute_fee, DISPUTE_FEE);
        assert_eq!(dispute.admin_approvals, 0);
        assert_eq!(dispute.required_approvals, MULTI_SIG_REQUIRED);
        assert!(!dispute.resolved);
        assert_eq!(dispute.initiated_at, 1_000_000);

        // Poll status transitions to Disputed.
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Disputed);

        // Dispute is retrievable via get_dispute.
        let stored = client.get_dispute(&1_u64);
        assert_eq!(stored.poll_id, 1);
        assert_eq!(stored.initiator, initiator);
    }

    #[test]
    fn initiate_dispute_emits_event() {
        use soroban_sdk::{testutils::Events, TryIntoVal};

        let (env, _admin, initiator, client) = dispute_setup();
        let evidence = String::from_str(&env, "QmEvidence123");

        client.initiate_dispute(&initiator, &1_u64, &evidence);

        let events = env.events().all();
        // Find the DisputeInitiated event (skip any token transfer events).
        let mut found = false;
        for i in 0..events.len() {
            let (_, topics, data) = events.get(i).unwrap();
            let name_result: Result<soroban_sdk::Symbol, _> =
                topics.get(0).unwrap().try_into_val(&env);
            if let Ok(name) = name_result {
                if name == soroban_sdk::Symbol::new(&env, "DisputeInitiated") {
                    let event_poll_id: u64 =
                        topics.get(1).unwrap().try_into_val(&env).unwrap();
                    let event_initiator: Address = data.try_into_val(&env).unwrap();
                    assert_eq!(event_poll_id, 1);
                    assert_eq!(event_initiator, initiator);
                    found = true;
                }
            }
        }
        assert!(found, "DisputeInitiated event must be emitted");
    }

    #[test]
    fn initiate_dispute_rejects_unfunded_initiator() {
        let (env, _admin, _token, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Resolved);

        let unfunded_initiator = Address::generate(&env);
        let evidence = String::from_str(&env, "QmEvidence123");

        let err = client
            .try_initiate_dispute(&unfunded_initiator, &1_u64, &evidence)
            .expect_err("disputing with no fee transferred must fail");

        assert_eq!(err, Ok(PredictXError::DisputeFeeRequired));
    }

    #[test]
    fn initiate_dispute_rejects_non_resolved_poll() {
        let (env, _admin, _token, client) = setup();
        let initiator = Address::generate(&env);
        let evidence = String::from_str(&env, "QmEvidence123");

        // Poll 1 is in Voting status (from setup).
        let err = client
            .try_initiate_dispute(&initiator, &1_u64, &evidence)
            .expect_err("disputing a non-resolved poll must fail");

        assert_eq!(err, Ok(PredictXError::PollAlreadyResolved));
    }

    #[test]
    fn initiate_dispute_rejects_unknown_poll() {
        let (env, _admin, _token, client) = setup();
        let initiator = Address::generate(&env);
        let evidence = String::from_str(&env, "QmEvidence123");

        let err = client
            .try_initiate_dispute(&initiator, &999_u64, &evidence)
            .expect_err("disputing an unknown poll must fail");

        assert_eq!(err, Ok(PredictXError::PollNotFound));
    }
}
