use predictx_shared::constants::{ADMIN_REVIEW_THRESHOLD_BPS, AUTO_RESOLVE_THRESHOLD_BPS, BPS_DENOMINATOR};
use predictx_shared::PollStatus;
use soroban_sdk::{Env, symbol_short};
use crate::{DataKey, StoredPollStatus};

pub fn determine_consensus_status(yes_votes: u32, no_votes: u32) -> PollStatus {
    let total = yes_votes + no_votes;
    if total == 0 {
        return PollStatus::AdminReview;
    }

    let max_votes = if yes_votes > no_votes { yes_votes } else { no_votes };
    let consensus_bps = (max_votes as u64 * BPS_DENOMINATOR as u64) / (total as u64);

    if consensus_bps >= AUTO_RESOLVE_THRESHOLD_BPS as u64 {
        PollStatus::Resolved
    } else if consensus_bps >= ADMIN_REVIEW_THRESHOLD_BPS as u64 {
        PollStatus::AdminReview
    } else {
        PollStatus::Disputed
    }
}

pub fn update_poll_status(env: &Env, poll_id: u64, yes_votes: u32, no_votes: u32) -> PollStatus {
    let status = determine_consensus_status(yes_votes, no_votes);
    let provisional_outcome = if yes_votes > no_votes {
        Some(true)
    } else if no_votes > yes_votes {
        Some(false)
    } else {
        None
    };
    
    let stored = StoredPollStatus {
        status,
        updated_at: env.ledger().timestamp(),
        provisional_outcome,
    };

    env.storage()
        .persistent()
        .set(&DataKey::PollStatus(poll_id), &stored);

    // emit status event
    let topics = (symbol_short!("status"), poll_id);
    env.events().publish(topics, status as u32);
    
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_review_boundaries() {
        // EXACTLY 60% parks in AdminReview
        // 60 votes vs 40 votes => 60% max => AdminReview
        assert_eq!(determine_consensus_status(60, 40), PollStatus::AdminReview);

        // EXACTLY 85% auto-resolves
        // 85 votes vs 15 votes => 85% max => Resolved
        assert_eq!(determine_consensus_status(85, 15), PollStatus::Resolved);

        // 70% consensus parks in AdminReview
        // 70 votes vs 30 votes => 70% max => AdminReview
        assert_eq!(determine_consensus_status(70, 30), PollStatus::AdminReview);

        // 59% disputes
        // 59 votes vs 41 votes => 59% max => Disputed
        assert_eq!(determine_consensus_status(59, 41), PollStatus::Disputed);
//! Voting lifecycle functions for the VotingOracle contract.

use predictx_shared::{PollStatus, PredictXError, VoteTally, VOTING_WINDOW_SECS};
use soroban_sdk::{Address, Env, String};

use crate::{DataKey, StoredPollStatus};

/// Opens a two-hour community voting window for a finished poll.
///
/// # Arguments
/// * `env`           — Soroban environment
/// * `admin`         — The admin address; must match the stored admin
/// * `poll_id`       — ID of the poll whose match has concluded
/// * `evidence_hash` — IPFS/hash of evidence supporting the outcome
///
/// # Errors
/// * [`PredictXError::Unauthorized`]        — caller is not the stored admin
/// * [`PredictXError::PollAlreadyResolved`] — voting has already been opened
///                                            for this poll (status is already
///                                            `PollStatus::Voting`)
///
/// # Storage written
/// * `DataKey::VoteTally(poll_id)`      — zeroed tally with
///   `voting_end_time = now + VOTING_WINDOW_SECS`, written to **temporary**
///   storage (only needed for the duration of the voting window)
/// * `DataKey::VotingEvidence(poll_id)` — the IPFS evidence hash, written to
///   **temporary** storage alongside the tally
/// * `DataKey::PollStatus(poll_id)`     — set to `PollStatus::Voting` via the
///   same `StoredPollStatus` pattern used by `set_poll_status`
pub fn initiate_voting(
    env: Env,
    admin: Address,
    poll_id: u64,
    evidence_hash: String,
) -> Result<(), PredictXError> {
    // 1. Require admin authentication.
    admin.require_auth();

    // 2. Verify the caller is the stored admin.
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)?;

    if admin != stored_admin {
        return Err(PredictXError::Unauthorized);
    }

    // 3. Guard against double-opening: if the poll is already in the Voting
    //    state we treat it the same as "already resolved" — the window is open.
    let current_status: PollStatus = env
        .storage()
        .persistent()
        .get::<DataKey, StoredPollStatus>(&DataKey::PollStatus(poll_id))
        .map(|s| s.status)
        .unwrap_or(PollStatus::Active);

    if current_status == PollStatus::Voting {
        return Err(PredictXError::PollAlreadyResolved);
    }

    // 4. Compute the voting deadline (checked arithmetic — no silent overflow).
    let now: u64 = env.ledger().timestamp();
    let voting_end_time: u64 = now
        .checked_add(VOTING_WINDOW_SECS)
        .expect("timestamp overflow");

    // 5. Build a zeroed VoteTally with every field from the actual struct.
    //    evidence_hash is not a field of VoteTally; it is stored separately
    //    (see step 6b).
    let tally = VoteTally {
use crate::{storage, DataKey, MAX_VOTERS};
use predictx_shared::{
    Dispute, PollStatus, PredictXError, VoteChoice, VoteTally, AUTO_RESOLVE_THRESHOLD_BPS,
    BPS_DENOMINATOR, DISPUTE_FEE, MULTI_SIG_REQUIRED, VOTING_WINDOW_SECS,
};
use soroban_sdk::{token, Address, Env, String, Symbol};
    PollStatus, PredictXError, VoteChoice, VoteTally, AUTO_RESOLVE_THRESHOLD_BPS, BPS_DENOMINATOR,
    MULTI_SIG_REQUIRED, VOTING_WINDOW_SECS,
    VOTER_REWARD_BPS, VOTING_WINDOW_SECS,
};
use soroban_sdk::{Address, Env, IntoVal, Symbol};

fn has_user_staked(env: &Env, poll_id: u64, voter: &Address) -> Result<bool, PredictXError> {
    let prediction_market: Address = env
        .storage()
        .instance()
        .get(&DataKey::PredictionMarket)
        .ok_or(PredictXError::NotInitialized)?;

    Ok(env.invoke_contract(
        &prediction_market,
        &Symbol::new(env, "has_user_staked"),
        (poll_id, voter.clone()).into_val(env),
    ))
}
use soroban_sdk::{token, Address, Env, Symbol};
    BPS_DENOMINATOR, DISPUTE_WINDOW_SECS, MULTI_SIG_REQUIRED, VOTING_WINDOW_SECS,
    BPS_DENOMINATOR, MULTI_SIG_REQUIRED, VOTING_WINDOW_SECS,
    TEMPORARY_STORAGE_EXTEND_TO_SECS, TEMPORARY_STORAGE_TTL_SECS, VOTING_WINDOW_SECS,
use crate::{storage, MAX_VOTERS};
use predictx_shared::{
    DataKey, PollStatus, PredictXError, VoteChoice, VoteTally, AUTO_RESOLVE_THRESHOLD_BPS,
    PollStatus, PredictXError, UserStats, VoteChoice, VoteTally, AUTO_RESOLVE_THRESHOLD_BPS,
    BPS_DENOMINATOR, VOTING_WINDOW_SECS,
};
use soroban_sdk::{Address, Env, String, Symbol};

/// Record a voter's choice on a poll.
///
/// # Voter cap policy
///
/// The roster of distinct voters per poll is bounded by [`MAX_VOTERS`]. The
/// cap is **window-scoped**, not a permanent freeze: it only limits how many
/// distinct addresses may be recorded during the poll's voting window, and it
/// never blocks the poll from reaching a settlement path.
///
/// When the cap is reached:
///
/// - New distinct addresses are rejected with [`PredictXError::MaxVotersReached`]
///   (see [`cast_vote`]); the roster is *not* silently truncated or rotated,
///   so already-recorded votes stay intact and auditable.
/// - The poll remains resolvable. [`auto_resolve`] only depends on the tally
///   and the voting window, so a capped poll still settles once the window
///   closes and the leading outcome clears
///   [`AUTO_RESOLVE_THRESHOLD_BPS`]. A capped poll is therefore never left
///   permanently unsettleable by community vote.
/// - Recovery for a poll that cannot reach consensus is handled by the
///   admin/community resolution path (tracked separately), which does not
///   require reopening the roster.
///
/// # Abuse model for exhausting the roster
///
/// Because [`cast_vote`] does not yet gate on stake or eligibility (see the
/// "excluding stakers" note below), an adversary can fill all [`MAX_VOTERS`]
/// slots with sybil addresses and lock out honest voters for the remainder of
/// the window. The cap bounds the blast radius of that griefing: it limits the
/// roster to a fixed size, keeps every recorded vote auditable, and — crucially
/// — does not prevent the poll from settling. Mitigating the sybil fill itself
/// (stake-weighting, eligibility proofs, or a per-window reset) is out of scope
/// for this change and tracked separately.
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
/// Extends the TTL of a temporary storage entry so it survives past the
/// voting window. Re‑arms the TTL to the configured extend‑to value.
fn bump_ttl(env: &Env, key: &DataKey) {
    env.storage().temporary().extend_ttl(key, TEMPORARY_STORAGE_TTL_SECS as u32, TEMPORARY_STORAGE_EXTEND_TO_SECS as u32);
}

/// Record a voter's choice on a poll.
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
        .has(&DataKey::OraclePollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    if crate::read_poll_status(env, poll_id) != PollStatus::Voting {
        return Err(PredictXError::VotingNotOpen);
    }

    // The tally is the source of truth for the window opened by
    // `initiate_voting`. Fall back to the status timestamp for legacy/test
    // voting records that do not yet have a tally.
    let voting_end_time = storage::read_tally(env, poll_id)
        .map(|tally| tally.voting_end_time)
        .unwrap_or_else(|| {
            crate::read_poll_status_updated_at(env, poll_id)
                .checked_add(VOTING_WINDOW_SECS)
                .unwrap_or(0)
        });
    if env.ledger().timestamp() >= voting_end_time {
        return Err(PredictXError::VotingWindowExpired);
    }

    // Each address may vote at most once per poll.
    let mut voters = storage::read_voters(env, poll_id);
    if storage::has_voted(env, poll_id, &voter) || voters.contains(voter.clone()) {
        return Err(PredictXError::AlreadyVoted);
    }

    if has_user_staked(env, poll_id, &voter)? {
        return Err(PredictXError::VoterIsStaker);
    // Cap is window-scoped: once the roster is full, new distinct addresses
    // are rejected for this window only. The poll still settles via
    // `auto_resolve` (or the admin/community path), so this is not a
    // permanent freeze. See the cap policy in `cast_vote`'s doc comment.
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
    };

    // 6a. Persist the tally in temporary storage — it is only needed for the
    //     duration of the two-hour voting window.
    env.storage()
        .temporary()
        .set(&DataKey::VoteTally(poll_id), &tally);

    // 6b. Store the evidence hash alongside the tally so voters and resolvers
    //     can retrieve it.  VoteTally itself has no evidence_hash field.
    env.storage()
        .temporary()
        .set(&DataKey::VotingEvidence(poll_id), &evidence_hash);

    // 7. Advance poll status to Voting using the same StoredPollStatus pattern
    //    that set_poll_status (lib.rs:50) uses.
    let stored_status = StoredPollStatus {
        status: PollStatus::Voting,
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
    bump_ttl(env, &DataKey::VoteTally(poll_id));
    bump_ttl(env, &DataKey::HasVoted(poll_id, voter.clone()));
    // Persist the choice itself (not just the count) so reward eligibility can
    // be checked against the resolved outcome later.
    storage::write_vote_choice(env, poll_id, &voter, choice);

    // Bump the voter's lifetime `votes_cast` counter. The `UserStats` record is
    // owned by this contract for the community-voting fields; the staking-side
    // fields are written by PredictionMarket into its own record, and the
    // frontend reads both side by side.
    let mut stats = storage::read_user_stats(env, &voter);
    stats.votes_cast = stats.votes_cast.saturating_add(1);
    storage::write_user_stats(env, &voter, &stats);
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
        .has(&DataKey::OraclePollStatus(poll_id))
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
        .has(&DataKey::OraclePollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    // Rewards only open once the poll has a resolved, decisive outcome.
    let outcome: VoteChoice = env
        .storage()
        .persistent()
        .get(&DataKey::OraclePollOutcome(poll_id))
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

    // Accumulate the voter's lifetime reward earnings across every poll they
    // have claimed on. `share` is zero for an unfunded poll, which is a no-op.
    let mut stats = storage::read_user_stats(env, &voter);
    stats.voting_rewards_earned = stats.voting_rewards_earned.saturating_add(share);
    storage::write_user_stats(env, &voter, &stats);

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
/// resolution threshold after the voting window closes. A leading `Unclear`
/// returns `ConsensusNotReached` and leaves the poll in `Voting` for admin
/// handling; only Yes/No outcomes are stored as terminal results.
pub fn auto_resolve(env: &Env, poll_id: u64) -> Result<VoteChoice, PredictXError> {
    if !env
        .storage()
        .persistent()
        .has(&DataKey::OraclePollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    if crate::read_poll_status(env, poll_id) != PollStatus::Voting {
        return Err(PredictXError::VotingNotOpen);
    }

    let mut tally = storage::read_tally(env, poll_id).ok_or(PredictXError::PollNotFound)?;
    bump_ttl(env, &DataKey::VoteTally(poll_id));
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

    if outcome == VoteChoice::Unclear {
        return Err(PredictXError::ConsensusNotReached);
    }

    if tally.total_voters == 0 {
        return Err(PredictXError::ConsensusNotReached);
    }
    let (leading_is_yes, share_bps) = consensus_bps(&tally);

    if share_bps < AUTO_RESOLVE_THRESHOLD_BPS {
        return Err(PredictXError::ConsensusNotReached);
    }

    // ── Effects ───────────────────────────────────────────────────────────────

    // Reserve the voters' share of the pool once, at resolution time. Reaching
    // this point guarantees the poll settles exactly once: every later attempt
    // is rejected by the `PollStatus` guard above, so the reserve is never
    // recomputed.
    tally.reward_pool = voter_reward_reserve(total_pool);
    storage::write_tally(env, &tally);
    let outcome = if leading_is_yes {
        VoteChoice::Yes
    } else {
        VoteChoice::No
    };

    let now = env.ledger().timestamp();
    let stored_status = crate::StoredPollStatus {
        status: PollStatus::Resolved,
        updated_at: now,
    };
    env.storage()
        .persistent()
        .set(&DataKey::PollStatus(poll_id), &stored_status);

    Ok(())
        .set(&DataKey::OraclePollStatus(poll_id), &stored_status);
    env.storage()
        .persistent()
        .set(&DataKey::OraclePollOutcome(poll_id), &outcome);

    env.events().publish(
        (Symbol::new(env, "AutoResolved"), poll_id, outcome),
        share_bps,
    );

    Ok(outcome)
}

/// Share of the total pool reserved for eligible voters at resolution time.
/// Open a dispute against `poll_id`.
///
/// Flow (Checks → Effects):
/// 1. Authenticates the caller as the initiator.
/// 2. Verifies the poll is known to the oracle, else `PollNotFound`.
/// 3. Rejects a second dispute while an unresolved one is already open, else
///    `DisputeAlreadyOpen`.
/// 4. Persists a fresh `Dispute` with zero approvals and returns it.
///
/// Policy (out of scope to build): once a dispute is resolved (`resolved ==
/// true`) a new dispute may be opened again, since the guard only blocks
/// *unresolved* disputes. Re-disputing after resolution is intentionally not
/// implemented here.
pub fn initiate_dispute(
    env: &Env,
    initiator: Address,
    poll_id: u64,
    evidence_hash: String,
    dispute_fee: i128,
) -> Result<Dispute, PredictXError> {
    initiator.require_auth();
/// - `Unclear` votes are excluded from the denominator: they signal "cannot
///  judge", not a preference.
/// - A Yes/No tie resolves to Yes (`true`) at 5000 bps, so the result is
///  deterministic.
/// - A tally with no decisive votes (e.g. all `Unclear`) returns `(false, 0)`
///  instead of dividing by zero.
///
/// Pure and side-effect free so the routing thresholds can be unit-tested
/// against it directly.
pub(crate) fn consensus_bps(tally: &VoteTally) -> (bool, u32) {
    let decisive = u64::from(tally.yes_votes) + u64::from(tally.no_votes);
    if decisive == 0 {
        return (false, 0);
    }

    // ── Checks ────────────────────────────────────────────────────────────────

    // Only accept disputes on polls the oracle already knows about.
    if !env
        .storage()
        .persistent()
        .has(&DataKey::PollStatus(poll_id))
    {
        return Err(PredictXError::PollNotFound);
    }

    // One poll, one open dispute: a griefer must not be able to stack disputes
    // on the same poll and make the admin approval state meaningless.
    if let Some(existing) = storage::read_dispute(env, poll_id) {
        if !existing.resolved {
            return Err(PredictXError::DisputeAlreadyOpen);
        }
    }

    // ── Effects ───────────────────────────────────────────────────────────────

    let dispute = Dispute {
        poll_id,
        initiator,
        evidence_hash,
        dispute_fee,
        admin_approvals: 0,
        required_approvals: MULTI_SIG_REQUIRED,
        resolved: false,
        initiated_at: env.ledger().timestamp(),
    };

    storage::write_dispute(env, &dispute);
    Ok(dispute)
}

/// Read the dispute recorded for `poll_id`.
///
/// Returns `PollNotFound` when no dispute has ever been opened for the poll.
pub fn get_dispute(env: &Env, poll_id: u64) -> Result<Dispute, PredictXError> {
    storage::read_dispute(env, poll_id).ok_or(PredictXError::PollNotFound)
}

/// Share of the decisive (Yes/No) votes held by the leading outcome.
///
/// Computes `total_pool * VOTER_REWARD_BPS / 10_000`, rounded down. Pure so
/// it can be unit-tested directly: non-positive pools reserve nothing, and
/// the default `VOTER_REWARD_BPS` of 100 yields exactly 1% of the pool.
pub(crate) fn voter_reward_reserve(total_pool: i128) -> i128 {
    if total_pool <= 0 {
        return 0;
    }
    total_pool * i128::from(VOTER_REWARD_BPS) / i128::from(BPS_DENOMINATOR)
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
/// Open a dispute against a `Resolved` poll within the dispute window.
///
/// Flow (Checks → Effects):
/// 1. Authenticates the caller as the dispute initiator.
/// 2. Verifies the poll is known, else `PollNotFound`.
/// 3. Requires the poll to be `Resolved`, else `PollNotActive` — an unresolved
///    poll has no outcome to dispute.
/// 4. Rejects a poll that already has an open dispute, else `DisputeAlreadyOpen`.
/// 5. Rejects a dispute raised more than [`DISPUTE_WINDOW_SECS`] after the poll
///    resolved, else `DisputeWindowClosed`.
/// 6. Stores the [`Dispute`] record, moves the poll to `PollStatus::Disputed`,
///    and emits `DisputeInitiated`.
///
/// The dispute fee is deliberately not handled here; it is tracked separately.
pub fn initiate_dispute(
    env: &Env,
    initiator: Address,
    poll_id: u64,
    evidence_hash: String,
) -> Result<(), PredictXError> {
    initiator.require_auth();
        // Register poll 1 as a known poll. `initiate_voting` (#80) will later
        // be the real production path for this transition.
        client.set_poll_status(&admin, &1_u64, &PollStatus::Voting);
        client.set_poll_status(&1_u64, &PollStatus::Voting, &None);

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
    use predictx_shared::{
        PollStatus, PredictXError, VoteChoice, MULTI_SIG_REQUIRED, VOTING_WINDOW_SECS,
    };
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env, String,
    };

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

    // Emitted only once the transfer has actually succeeded, so indexers never
    // see a reward that was not paid.
    env.events().publish(
        (Symbol::new(env, "VoterRewardClaimed"), poll_id, voter),
        amount,
    );

    Ok(amount)
}

/// Read-only preview of the reward `voter` could claim for `poll_id`.
///
/// Mirrors [`claim_voter_reward`] exactly and returns `0` — never an error —
/// for every ineligible case: an unknown or unsettled poll, a caller who never
/// voted, an already-claimed reward, or an empty pool. This lets the SDK treat
/// it symmetrically with the staker-side claimable view.
pub fn get_voter_reward(env: &Env, poll_id: u64, voter: &Address) -> i128 {
/// Record an admin approval toward resolving a `Disputed` poll to `outcome`.
///
/// Flow (Checks → Effects):
/// 1. Verifies the caller is a registered admin, else `Unauthorized`.
/// 2. Verifies the poll is known and currently `Disputed`, else `PollNotFound`
///    or `PollNotActive`.
/// 3. Increments the approval count for `outcome`.
/// 4. When the count reaches [`MULTI_SIG_REQUIRED`] the poll is resolved to
///    `outcome`; below the threshold it stays `Disputed`.
///
/// The approval ledger is a minimal per-outcome counter; preventing an admin
/// from approving twice is left to the dedicated approval-ledger issue.
pub fn approve_dispute(
    env: &Env,
    admin: Address,
    poll_id: u64,
    outcome: VoteChoice,
) -> Result<PollStatus, PredictXError> {
    storage::require_admin(env, &admin)?;
    admin.require_auth();

    require_disputed(env, poll_id)?;

    let approvals = storage::read_dispute_approvals(env, poll_id, outcome) + 1;
    storage::write_dispute_approvals(env, poll_id, outcome, approvals);

    if approvals >= MULTI_SIG_REQUIRED {
        finalize_dispute(env, poll_id, outcome)?;
        return Ok(PollStatus::Resolved);
    }

    Ok(PollStatus::Disputed)
}

/// Resolve a `Disputed` poll to `outcome` once it holds enough approvals.
///
/// Returns `InsufficientAdminApprovals` while the agreeing approvals for
/// `outcome` are below [`MULTI_SIG_REQUIRED`], `PollNotFound` for an unknown
/// poll, and `PollNotActive` when the poll is not currently `Disputed`.
pub fn resolve_dispute(
    env: &Env,
    admin: Address,
    poll_id: u64,
    outcome: VoteChoice,
) -> Result<VoteChoice, PredictXError> {
    storage::require_admin(env, &admin)?;
    admin.require_auth();

    require_disputed(env, poll_id)?;

    if storage::read_dispute_approvals(env, poll_id, outcome) < MULTI_SIG_REQUIRED {
        return Err(PredictXError::InsufficientAdminApprovals);
    }

    finalize_dispute(env, poll_id, outcome)
}

/// Ensure `poll_id` is a known poll currently under dispute.
fn require_disputed(env: &Env, poll_id: u64) -> Result<(), PredictXError> {
    if !env
        .storage()
        .persistent()
        .has(&DataKey::PollStatus(poll_id))
    {
        return 0;
    }

    if crate::read_poll_status(env, poll_id) != PollStatus::Resolved {
        return 0;
    }

    let tally = match storage::read_tally(env, poll_id) {
        Some(tally) => tally,
        None => return 0,
    };

    let voters = storage::read_voters(env, poll_id);
    if !voters.contains(voter.clone()) {
        return 0;
    }

    if storage::has_claimed_reward(env, poll_id, voter) {
        return 0;
    }

    voter_reward_share(&tally, voters.len())
}
    #[test]
    fn cast_vote_rejects_active_poll() {
        let (env, admin, client) = setup();
        client.set_poll_status(&admin, &1_u64, &PollStatus::Active);

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
        return Err(PredictXError::PollNotFound);
    }
    if crate::read_poll_status(env, poll_id) != PollStatus::Disputed {
        return Err(PredictXError::PollNotActive);
    }
    Ok(())
}

/// Mark a disputed poll `Resolved` to `outcome` and emit `DisputeResolved`.
fn finalize_dispute(
    env: &Env,
    poll_id: u64,
    outcome: VoteChoice,
) -> Result<VoteChoice, PredictXError> {
    let stored_status = crate::StoredPollStatus {
        status: PollStatus::Resolved,
        updated_at: env.ledger().timestamp(),
    if crate::read_poll_status(env, poll_id) != PollStatus::Resolved {
        return Err(PredictXError::PollNotActive);
    }

    if let Some(existing) = storage::read_dispute(env, poll_id) {
        if !existing.resolved {
            return Err(PredictXError::DisputeAlreadyOpen);
        }
    }

    let resolution_time = crate::read_poll_status_updated_at(env, poll_id);
    let deadline = resolution_time
        .checked_add(DISPUTE_WINDOW_SECS)
        .unwrap_or(u64::MAX);
    let now = env.ledger().timestamp();
    if now > deadline {
        return Err(PredictXError::DisputeWindowClosed);
    }

    let dispute = Dispute {
        poll_id,
        initiator: initiator.clone(),
        evidence_hash: evidence_hash.clone(),
        dispute_fee: 0,
        admin_approvals: 0,
        required_approvals: MULTI_SIG_REQUIRED,
        resolved: false,
        initiated_at: now,
    };
    storage::write_dispute(env, &dispute);

    let stored_status = crate::StoredPollStatus {
        status: PollStatus::Disputed,
        updated_at: now,
    };
    env.storage()
        .persistent()
        .set(&DataKey::PollStatus(poll_id), &stored_status);
    env.storage()
        .persistent()
        .set(&DataKey::PollOutcome(poll_id), &outcome);

    env.events().publish(
        (Symbol::new(env, "DisputeResolved"), poll_id, outcome),
        MULTI_SIG_REQUIRED,
    );

    Ok(outcome)

    env.events().publish(
        (Symbol::new(env, "DisputeInitiated"), poll_id, initiator),
        evidence_hash,
    );

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

/// Resolve an open dispute on a poll under admin / multi-sig control.
///
/// Flow (Checks → Effects):
/// 1. Authenticates the caller as an admin.
/// 2. Verifies the caller is a registered admin, else `Unauthorized`.
/// 3. Verifies that an open dispute exists for `poll_id`, else `PollNotFound`.
/// 4. Ensures the dispute has not already been resolved, else `PollAlreadyResolved`.
/// 5. Validates that admin approvals have reached the required multi-sig threshold
///    (`dispute.required_approvals`, defaulting to `MULTI_SIG_REQUIRED`), else `InsufficientAdminApprovals`.
/// 6. Writes the final outcome to persistent storage (`DataKey::PollOutcome(poll_id)`).
/// 7. Marks `dispute.resolved = true` and updates dispute in persistent storage.
/// 8. Sets poll status to `PollStatus::Resolved` with the current timestamp.
/// 9. Emits a `DisputeResolved` event with `poll_id` and `final_outcome`.
pub fn resolve_dispute(
    env: &Env,
    admin: Address,
    poll_id: u64,
    final_outcome: VoteChoice,
/// Record an admin's approval of an outcome for a contested poll.
///
/// Flow (Checks → Effects):
/// 1. Verifies that `admin` is a registered admin, else `Unauthorized`.
/// 2. Authenticates `admin`.
/// 3. If `admin` has not yet approved `poll_id`:
///    - Increments the approval counter for `poll_id`.
/// 4. Stores `admin`'s chosen outcome under `DataKey::Approval(poll_id, admin)`.
pub fn approve(
    env: &Env,
    admin: Address,
    poll_id: u64,
    outcome: VoteChoice,
) -> Result<(), PredictXError> {
    storage::require_admin(env, &admin)?;
    admin.require_auth();

    // ── Checks ────────────────────────────────────────────────────────────────

    let mut dispute = storage::read_dispute(env, poll_id).ok_or(PredictXError::PollNotFound)?;

    if dispute.resolved || crate::read_poll_status(env, poll_id) == PollStatus::Resolved {
        return Err(PredictXError::PollAlreadyResolved);
    }

    // TODO: Gate resolution on multi-sig approval threshold using MULTI_SIG_REQUIRED
    // once the multi-sig approval ledger (#93) and threshold enforcement (#94) are merged.
    let required_approvals = if dispute.required_approvals == 0 {
        MULTI_SIG_REQUIRED
    } else {
        dispute.required_approvals
    use predictx_shared::{PollStatus, PredictXError, VoteChoice, VOTING_WINDOW_SECS};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env, String,
    };

    if dispute.admin_approvals < required_approvals {
        return Err(PredictXError::InsufficientAdminApprovals);
    }

    // ── Effects ───────────────────────────────────────────────────────────────

    dispute.resolved = true;
    storage::write_dispute(env, &dispute);

    let now = env.ledger().timestamp();
    let stored_status = crate::StoredPollStatus {
        status: PollStatus::Resolved,
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
    env.storage()
        .persistent()
        .set(&DataKey::PollOutcome(poll_id), &final_outcome);

    env.events().publish(
        (Symbol::new(env, "DisputeResolved"), poll_id, final_outcome),
        final_outcome,
    );

    Ok(())

    env.events().publish(
        (Symbol::new(env, "DisputeInitiated"), poll_id),
        initiator,
    );

    Ok(dispute)
    if !storage::has_approved(env, poll_id, &admin) {
        let count = storage::read_approval_count(env, poll_id);
        storage::write_approval_count(env, poll_id, count.saturating_add(1));
    }

    storage::write_approval(env, poll_id, &admin, outcome);

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use predictx_shared::{
        Dispute, PollStatus, PredictXError, VoteChoice, MULTI_SIG_REQUIRED, VOTING_WINDOW_SECS,
        Dispute, PollStatus, PredictXError, VoteChoice, DISPUTE_FEE, MULTI_SIG_REQUIRED,
        VOTING_WINDOW_SECS,
        PollStatus, PredictXError, VoteChoice, VoteTally, VOTER_REWARD_BPS, VOTING_WINDOW_SECS,
    };
    use soroban_sdk::{
        contract, contractimpl, contracttype,
        testutils::{Address as _, Ledger},
        Address, Env, String,
        token, Address, Env, Vec,
    };

    use crate::{storage, VotingOracle, VotingOracleClient};
    #[contract]
    struct TestPredictionMarket;

    #[contracttype]
    enum TestDataKey {
        Staked(Address),
    }

    #[contractimpl]
    impl TestPredictionMarket {
        pub fn set_staked(env: Env, user: Address, staked: bool) {
            env.storage()
                .instance()
                .set(&TestDataKey::Staked(user), &staked);
        }

        pub fn has_user_staked(env: Env, _poll_id: u64, user: Address) -> bool {
            env.storage()
                .instance()
                .get(&TestDataKey::Staked(user))
                .unwrap_or(false)
        }
    }

    use crate::{VotingOracle, VotingOracleClient};

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
        client.initialize(&admin);
        let prediction_market_id = env.register(TestPredictionMarket, ());
        client.set_prediction_market(&prediction_market_id);
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

    /// Read a poll's tally from the contract's own storage (tests run outside
    /// the contract, so direct storage access must be wrapped).
    fn stored_tally(env: &Env, client: &VotingOracleClient, poll_id: u64) -> Option<VoteTally> {
        env.as_contract(&client.address, || storage::read_tally(env, poll_id))
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
    fn cast_vote_rejects_staker() {
        let (env, _admin, client) = setup();
        let prediction_market_id = env.register(TestPredictionMarket, ());
        client.set_prediction_market(&prediction_market_id);
        let prediction_market =
            TestPredictionMarketClient::new(&env, &prediction_market_id);
        let staker = voter(&env);
        prediction_market.set_staked(&staker, &true);

        let err = client
            .try_cast_vote(&staker, &1_u64, &VoteChoice::Yes)
            .expect_err("stakers must not vote on their own poll");

        assert_eq!(err, Ok(PredictXError::VoterIsStaker));
    }

    #[test]
    fn cast_vote_allows_configured_prediction_market_non_staker() {
        let (env, _admin, client) = setup();
        let non_staker = voter(&env);

        let tally = client.cast_vote(&non_staker, &1_u64, &VoteChoice::Yes);

        assert_eq!(tally.total_voters, 1);
        assert_eq!(tally.yes_votes, 1);
    }

    #[test]
    fn capped_poll_still_reaches_settlement_path() {
        let (env, _admin, client) = setup();

        // Fill the roster to the cap with a decisive Yes majority.
        for _ in 0..MAX_VOTERS {
            client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        }

        // The cap is hit: a further distinct voter is rejected...
        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes)
            .expect_err("voter roster cap must be enforced");
        assert_eq!(err, Ok(PredictXError::MaxVotersReached));

        // ...but the capped poll is not frozen: it still settles once the
        // voting window closes and consensus clears the threshold.
        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);
        let outcome = client.auto_resolve(&1_u64);

        assert_eq!(outcome, VoteChoice::Yes);
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Resolved);
        assert_eq!(client.get_poll_outcome(&1_u64), VoteChoice::Yes);
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
        let (env, admin, client) = setup();
        client.set_poll_status(&admin, &1_u64, &PollStatus::Active);
    fn cast_vote_rejects_locked_poll() {
        let (env, _admin, client) = setup();
        client.set_poll_status(&2_u64, &PollStatus::Locked, &None);

        let err = client
            .try_cast_vote(&voter(&env), &2_u64, &VoteChoice::Yes)
            .expect_err("locked poll must reject voting");

        assert_eq!(err, Ok(PredictXError::VotingNotOpen));
    }

    #[test]
    fn cast_vote_rejects_resolved_poll() {
        let (env, _admin, _token, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Resolved);
        let (env, admin, client) = setup();
        client.set_poll_status(&admin, &1_u64, &PollStatus::Resolved);
        let (env, _admin, client) = setup();
        client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);
        client.auto_resolve(&1_u64);

        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes)
            .expect_err("resolved poll must reject voting");

        assert_eq!(err, Ok(PredictXError::VotingNotOpen));
    }

    #[test]
    fn cast_vote_succeeds_one_second_before_deadline() {
        let (env, _admin, client) = setup();
        env.ledger()
            .set_timestamp(1_000_000 + VOTING_WINDOW_SECS - 1);

        let tally = client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);

        assert_eq!(tally.yes_votes, 1);
        assert_eq!(tally.total_voters, 1);
    }

    #[test]
    fn cast_vote_rejects_at_deadline_without_changing_tally() {
        let (env, _admin, client) = setup();
        let initial = client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        env.ledger()
            .set_timestamp(1_000_000 + VOTING_WINDOW_SECS);

        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::No)
            .expect_err("votes at the deadline must be rejected");

        assert_eq!(err, Ok(PredictXError::VotingWindowExpired));
        let stored = crate::storage::read_tally(&env, 1).unwrap();
        assert_eq!(stored.yes_votes, initial.yes_votes);
        assert_eq!(stored.no_votes, initial.no_votes);
        assert_eq!(stored.unclear_votes, initial.unclear_votes);
        assert_eq!(stored.total_voters, initial.total_voters);
    }

    #[test]
    fn cast_vote_rejects_after_deadline() {
        let (env, _admin, client) = setup();
        env.ledger()
            .set_timestamp(1_000_000 + VOTING_WINDOW_SECS + 1);

        let err = client
            .try_cast_vote(&voter(&env), &1_u64, &VoteChoice::No)
            .expect_err("votes after the deadline must be rejected");

        assert_eq!(err, Ok(PredictXError::VotingWindowExpired));
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
        cast_votes(&env, &client, 54, 10);
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

    // ── resolve_dispute ───────────────────────────────────────────────────────

    fn setup_disputed_poll(
        env: &Env,
        client: &VotingOracleClient,
        admin_approvals: u32,
        required_approvals: u32,
        initial_outcome: VoteChoice,
    ) {
        // Put poll in Disputed state
        client.set_poll_status(&1_u64, &PollStatus::Disputed);

        env.as_contract(&client.address, || {
            // Record initial poll outcome
            env.storage()
                .persistent()
                .set(&crate::DataKey::PollOutcome(1_u64), &initial_outcome);

            // Persist dispute record
            let dispute = Dispute {
                poll_id: 1,
                initiator: Address::generate(env),
                evidence_hash: String::from_str(env, "QmEvidenceHash123"),
                dispute_fee: 100_000_000,
                admin_approvals,
                required_approvals,
                resolved: false,
                initiated_at: env.ledger().timestamp(),
            };
            super::storage::write_dispute(env, &dispute);
        });
    }

    #[test]
    fn resolve_dispute_below_threshold_returns_insufficient_admin_approvals() {
        let (env, admin, client) = setup();
        setup_disputed_poll(&env, &client, 2, MULTI_SIG_REQUIRED, VoteChoice::Yes);

        let err = client
            .try_resolve_dispute(&admin, &1_u64, &VoteChoice::No)
            .expect_err("resolution below threshold must fail");

        assert_eq!(err, Ok(PredictXError::InsufficientAdminApprovals));
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Disputed);
        assert!(!client.get_dispute(&1_u64).resolved);
        assert_eq!(client.get_poll_outcome(&1_u64), VoteChoice::Yes);
    }

    #[test]
    fn resolve_dispute_allows_outcome_differing_from_original() {
        let (env, admin, client) = setup();
        // Original outcome is Yes, dispute has enough approvals (3 >= 3)
        setup_disputed_poll(&env, &client, 3, MULTI_SIG_REQUIRED, VoteChoice::Yes);

        // Ruling decides the outcome is No (differs from original Yes)
        client.resolve_dispute(&admin, &1_u64, &VoteChoice::No);

        assert_eq!(client.get_poll_outcome(&1_u64), VoteChoice::No);
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Resolved);
        assert!(client.get_dispute(&1_u64).resolved);
    }

    #[test]
    fn resolve_dispute_cannot_be_resolved_again() {
        let (env, admin, client) = setup();
        setup_disputed_poll(&env, &client, 3, MULTI_SIG_REQUIRED, VoteChoice::Yes);

        client.resolve_dispute(&admin, &1_u64, &VoteChoice::No);

        // Attempting to resolve the already-resolved dispute must fail
        let err = client
            .try_resolve_dispute(&admin, &1_u64, &VoteChoice::Unclear)
            .expect_err("a resolved dispute cannot be resolved again");

        assert_eq!(err, Ok(PredictXError::PollAlreadyResolved));
    }

    #[test]
    fn resolve_dispute_emits_dispute_resolved_event() {
        use soroban_sdk::{testutils::Events, TryIntoVal};

        let (env, admin, client) = setup();
        setup_disputed_poll(&env, &client, 3, MULTI_SIG_REQUIRED, VoteChoice::Yes);

        client.resolve_dispute(&admin, &1_u64, &VoteChoice::No);

        let events = env.events().all();
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
                if name == soroban_sdk::Symbol::new(&env, "DisputeResolved") {
                    let event_poll_id: u64 = topics.get(1).unwrap().try_into_val(&env).unwrap();
                    let event_outcome: VoteChoice =
                        topics.get(2).unwrap().try_into_val(&env).unwrap();
                    let data_outcome: VoteChoice = data.try_into_val(&env).unwrap();
                    assert_eq!(event_poll_id, 1);
                    assert_eq!(event_outcome, VoteChoice::No);
                    assert_eq!(data_outcome, VoteChoice::No);
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
        assert!(found, "DisputeResolved event must be emitted");
    }

    #[test]
    fn resolve_dispute_rejects_non_admin() {
        let (env, _admin, client) = setup();
        let stranger = Address::generate(&env);
        setup_disputed_poll(&env, &client, 3, MULTI_SIG_REQUIRED, VoteChoice::Yes);

        let err = client
            .try_resolve_dispute(&stranger, &1_u64, &VoteChoice::No)
            .expect_err("non-admin caller must be rejected");

        assert_eq!(err, Ok(PredictXError::Unauthorized));
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Disputed);
        assert!(!client.get_dispute(&1_u64).resolved);
    }

    #[test]
    fn resolve_dispute_rejects_unknown_poll() {
        let (_env, admin, client) = setup();

        let err = client
            .try_resolve_dispute(&admin, &999_u64, &VoteChoice::No)
            .expect_err("unknown poll dispute must be rejected");

        assert_eq!(err, Ok(PredictXError::PollNotFound));
        assert!(found, "DisputeInitiated event must be emitted");
    }

    #[test]
    fn initiate_dispute_rejects_unfunded_initiator() {
        let (env, _admin, _token, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Resolved);

        let unfunded_initiator = Address::generate(&env);
        let evidence = String::from_str(&env, "QmEvidence123");
    fn same_voter_can_vote_on_two_different_polls() {
        let (env, admin, client) = setup();
        let v = voter(&env);

        client.cast_vote(&v, &1_u64, &VoteChoice::Yes);
        client.set_poll_status(&admin, &2_u64, &PollStatus::Voting);
        client.set_poll_status(&2_u64, &PollStatus::Voting, &None);

        let err = client
            .try_initiate_dispute(&unfunded_initiator, &1_u64, &evidence)
            .expect_err("disputing with no fee transferred must fail");

        assert_eq!(err, Ok(PredictXError::DisputeFeeRequired));
    }

    // ── Disputes ──────────────────────────────────────────────────────────────

    fn evidence(env: &Env) -> String {
        String::from_str(env, "ipfs://evidence")
    }

    #[test]
    fn initiate_dispute_records_and_get_dispute_returns_it() {
        let (env, _admin, client) = setup();
        let initiator = voter(&env);

        let written = client.initiate_dispute(&initiator, &1_u64, &evidence(&env), &500_i128);
        let read = client.get_dispute(&1_u64);

        assert_eq!(read, written);
        assert_eq!(read.poll_id, 1);
        assert_eq!(read.initiator, initiator);
        assert_eq!(read.evidence_hash, evidence(&env));
        assert_eq!(read.dispute_fee, 500);
        assert_eq!(read.admin_approvals, 0);
        assert_eq!(read.required_approvals, MULTI_SIG_REQUIRED);
        assert!(!read.resolved);
        assert_eq!(read.initiated_at, 1_000_000);
    }

    #[test]
    fn second_dispute_on_open_dispute_is_rejected() {
        let (env, _admin, client) = setup();

        client.initiate_dispute(&voter(&env), &1_u64, &evidence(&env), &500_i128);

        let err = client
            .try_initiate_dispute(&voter(&env), &1_u64, &evidence(&env), &500_i128)
            .expect_err("a second dispute on an unresolved poll must be rejected");

        assert_eq!(err, Ok(PredictXError::DisputeAlreadyOpen));
    }

    #[test]
    fn get_dispute_on_undisputed_poll_returns_poll_not_found() {
        let (env, _admin, client) = setup();

        let err = client
            .try_get_dispute(&1_u64)
            .expect_err("an undisputed poll must not return a dispute");

        assert_eq!(err, Ok(PredictXError::PollNotFound));
        assert_eq!(err, Ok(PredictXError::ConsensusNotReached));
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Voting);
        let missing_outcome = client
            .try_get_poll_outcome(&1_u64)
            .expect_err("a rejected poll must not have a stored outcome");
        assert_eq!(missing_outcome, Ok(PredictXError::PollNotFound));
    }

    #[test]
    fn auto_resolve_rejects_unclear_leading_tally() {
        let (env, _admin, client) = setup();
        crate::storage::write_tally(
            &env,
            &predictx_shared::VoteTally {
                poll_id: 1,
                yes_votes: 5,
                no_votes: 5,
                unclear_votes: 90,
                total_voters: 100,
                voting_end_time: 1_000_000 + VOTING_WINDOW_SECS,
                reward_pool: 0,
            },
        );
        env.ledger().set_timestamp(1_000_000 + VOTING_WINDOW_SECS);

        let err = client
            .try_auto_resolve(&1_u64)
            .expect_err("an Unclear majority must not resolve the poll");

        assert_eq!(err, Ok(PredictXError::ConsensusNotReached));
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Voting);
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
    // ── Admin approvals ───────────────────────────────────────────────────────

    #[test]
    fn approve_increments_counter_and_stores_outcome() {
        let (_env, admin, client) = setup();

        assert_eq!(client.get_approval_count(&1_u64), 0);
        assert_eq!(client.get_approval(&1_u64, &admin), None);

        client.approve(&admin, &1_u64, &VoteChoice::Yes);

        assert_eq!(client.get_approval_count(&1_u64), 1);
        assert_eq!(client.get_approval(&1_u64, &admin), Some(VoteChoice::Yes));
    }

    #[test]
    fn approve_twice_from_same_admin_increments_counter_only_once() {
        let (_env, admin, client) = setup();

        client.approve(&admin, &1_u64, &VoteChoice::Yes);
        assert_eq!(client.get_approval_count(&1_u64), 1);

        // Second approval by same admin must not increment the counter
        client.approve(&admin, &1_u64, &VoteChoice::No);
        assert_eq!(client.get_approval_count(&1_u64), 1);
        assert_eq!(client.get_approval(&1_u64, &admin), Some(VoteChoice::No));
    }

    #[test]
    fn approvals_from_different_admins_each_count() {
        let (env, admin1, client) = setup();
        let admin2 = Address::generate(&env);
        let admin3 = Address::generate(&env);

        client.add_admin(&admin1, &admin2);
        client.add_admin(&admin1, &admin3);

        client.approve(&admin1, &1_u64, &VoteChoice::Yes);
        assert_eq!(client.get_approval_count(&1_u64), 1);

        client.approve(&admin2, &1_u64, &VoteChoice::Yes);
        assert_eq!(client.get_approval_count(&1_u64), 2);

        client.approve(&admin3, &1_u64, &VoteChoice::No);
        assert_eq!(client.get_approval_count(&1_u64), 3);

        assert_eq!(client.get_approval(&1_u64, &admin1), Some(VoteChoice::Yes));
        assert_eq!(client.get_approval(&1_u64, &admin2), Some(VoteChoice::Yes));
        assert_eq!(client.get_approval(&1_u64, &admin3), Some(VoteChoice::No));
    }

    #[test]
    fn non_admin_approving_returns_unauthorized() {
        let (env, _admin, client) = setup();
        let non_admin = Address::generate(&env);

        let err = client
            .try_approve(&non_admin, &1_u64, &VoteChoice::Yes)
            .expect_err("non-admin approval must fail");

        assert_eq!(err, Ok(PredictXError::Unauthorized));
        assert_eq!(client.get_approval_count(&1_u64), 0);
        assert_eq!(client.get_approval(&1_u64, &non_admin), None);
    }

    #[test]
    fn approval_counts_are_isolated_per_poll() {
        let (_env, admin, client) = setup();

        assert_eq!(client.get_approval_count(&1_u64), 0);
        assert_eq!(client.get_approval_count(&2_u64), 0);

        client.approve(&admin, &1_u64, &VoteChoice::Yes);

        assert_eq!(client.get_approval_count(&1_u64), 1);
        assert_eq!(client.get_approval_count(&2_u64), 0);

        client.approve(&admin, &2_u64, &VoteChoice::No);

        assert_eq!(client.get_approval_count(&1_u64), 1);
        assert_eq!(client.get_approval_count(&2_u64), 1);
        assert_eq!(client.get_approval(&1_u64, &admin), Some(VoteChoice::Yes));
        assert_eq!(client.get_approval(&2_u64, &admin), Some(VoteChoice::No));
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

    // ── get_voter_reward / VoterRewardClaimed ─────────────────────────────────

    #[test]
    fn get_voter_reward_matches_the_claim_payout() {
        let (_env, client, _token_address, voters) = setup_reward_poll(4, 401);
        let first = voters.get(0).unwrap();

        // floor(401 / 4): the remainder stays in the contract.
        let preview = client.get_voter_reward(&1_u64, &first);
        assert_eq!(preview, 100);

        let paid = client.claim_voter_reward(&first, &1_u64);
        assert_eq!(paid, preview);
    }

    #[test]
    fn get_voter_reward_returns_zero_for_ineligible_voters() {
        let (env, client, _token_address, voters) = setup_reward_poll(2, 200);
        let voter_zero = voters.get(0).unwrap();

        // Never voted on poll 1.
        assert_eq!(client.get_voter_reward(&1_u64, &voter(&env)), 0);

        // Unknown poll.
        assert_eq!(client.get_voter_reward(&999_u64, &voter_zero), 0);

        // A poll that is still open for voting has not settled yet.
        client.set_poll_status(&2_u64, &PollStatus::Voting);
        client.cast_vote(&voter_zero, &2_u64, &VoteChoice::Yes);
        assert_eq!(client.get_voter_reward(&2_u64, &voter_zero), 0);
    }

    #[test]
    fn get_voter_reward_returns_zero_after_claim() {
        let (_env, client, _token_address, voters) = setup_reward_poll(2, 200);
        let first = voters.get(0).unwrap();

        assert_eq!(client.get_voter_reward(&1_u64, &first), 100);

        client.claim_voter_reward(&first, &1_u64);

        assert_eq!(client.get_voter_reward(&1_u64, &first), 0);
    }

    #[test]
    fn claim_voter_reward_emits_event_after_successful_transfer() {
        use soroban_sdk::{testutils::Events, TryIntoVal};

        let (env, client, _token_address, voters) = setup_reward_poll(2, 200);
        let first = voters.get(0).unwrap();

        let paid = client.claim_voter_reward(&first, &1_u64);

        let events = env.events().all();
        let (_, topics, data) = events.get(events.len() - 1).unwrap();
        let name: soroban_sdk::Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
        let topic_poll_id: u64 = topics.get(1).unwrap().try_into_val(&env).unwrap();
        let topic_voter: Address = topics.get(2).unwrap().try_into_val(&env).unwrap();
        let amount: i128 = data.try_into_val(&env).unwrap();

        assert_eq!(name, soroban_sdk::Symbol::new(&env, "VoterRewardClaimed"));
        assert_eq!(topic_poll_id, 1);
        assert_eq!(topic_voter, first);
        assert_eq!(amount, paid);
    // ── multi-sig dispute resolution ─────────────────────────────────────────

    /// Register three extra admins so a poll can gather distinct approvals.
    fn add_three_admins(
        env: &Env,
        client: &VotingOracleClient,
        admin: &Address,
    ) -> (Address, Address, Address) {
        let first = Address::generate(env);
        let second = Address::generate(env);
        let third = Address::generate(env);
        client.add_admin(admin, &first);
        client.add_admin(admin, &second);
        client.add_admin(admin, &third);
        (first, second, third)
    }

    #[test]
    fn two_approvals_leave_disputed_poll_unresolved() {
        let (env, admin, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Disputed);
        let (first, second, _third) = add_three_admins(&env, &client, &admin);

        assert_eq!(
            client.approve_dispute(&first, &1_u64, &VoteChoice::Yes),
            PollStatus::Disputed
        );
        assert_eq!(
            client.approve_dispute(&second, &1_u64, &VoteChoice::Yes),
            PollStatus::Disputed
        );

        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Disputed);
        assert_eq!(
            client.get_dispute_approvals(&1_u64, &VoteChoice::Yes),
            2
        );
    }

    #[test]
    fn third_agreeing_approval_resolves_the_dispute() {
        let (env, admin, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Disputed);
        let (first, second, third) = add_three_admins(&env, &client, &admin);

        client.approve_dispute(&first, &1_u64, &VoteChoice::No);
        client.approve_dispute(&second, &1_u64, &VoteChoice::No);
        assert_eq!(
            client.approve_dispute(&third, &1_u64, &VoteChoice::No),
            PollStatus::Resolved
        );

        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Resolved);
        assert_eq!(client.get_poll_outcome(&1_u64), VoteChoice::No);
    }

    #[test]
    fn approvals_split_across_outcomes_do_not_resolve() {
        let (env, admin, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Disputed);
        let (first, second, third) = add_three_admins(&env, &client, &admin);

        client.approve_dispute(&first, &1_u64, &VoteChoice::Yes);
        client.approve_dispute(&second, &1_u64, &VoteChoice::No);
        client.approve_dispute(&third, &1_u64, &VoteChoice::Unclear);

        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Disputed);
    }

    #[test]
    fn resolution_below_threshold_returns_insufficient_admin_approvals() {
        let (env, admin, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Disputed);
        let (first, second, _third) = add_three_admins(&env, &client, &admin);

        client.approve_dispute(&first, &1_u64, &VoteChoice::Yes);
        client.approve_dispute(&second, &1_u64, &VoteChoice::Yes);

        let err = client
            .try_resolve_dispute(&first, &1_u64, &VoteChoice::Yes)
            .expect_err("resolution below the multi-sig threshold must be rejected");

        assert_eq!(err, Ok(PredictXError::InsufficientAdminApprovals));
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Disputed);
    // ── dispute window ───────────────────────────────────────────────────────

    #[test]
    fn dispute_within_window_is_accepted() {
        let (env, _admin, client) = setup();
        // Poll 1 resolves at the current ledger time (1_000_000).
        client.set_poll_status(&1_u64, &PollStatus::Resolved);
        let initiator = Address::generate(&env);

        env.ledger().set_timestamp(1_000_000 + 23 * 60 * 60);
        client.initiate_dispute(
            &initiator,
            &1_u64,
            &String::from_str(&env, "ipfs://evidence"),
        );

        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Disputed);

        let dispute = client.get_dispute(&1_u64);
        assert_eq!(dispute.poll_id, 1);
        assert_eq!(dispute.initiator, initiator);
        assert!(!dispute.resolved);
    }

    #[test]
    fn dispute_after_window_is_rejected() {
        let (env, _admin, client) = setup();
        client.set_poll_status(&1_u64, &PollStatus::Resolved);
        let initiator = Address::generate(&env);

        env.ledger().set_timestamp(1_000_000 + 25 * 60 * 60);
        let err = client
            .try_initiate_dispute(
                &initiator,
                &1_u64,
                &String::from_str(&env, "too late"),
            )
            .expect_err("a dispute after the 24-hour window must be rejected");

        assert_eq!(err, Ok(PredictXError::DisputeWindowClosed));
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Resolved);
    }

    #[test]
    fn dispute_against_unresolved_poll_is_rejected() {
        let (env, _admin, client) = setup();
        let initiator = Address::generate(&env);

        // `setup` leaves poll 1 in `Voting` — there is no outcome to dispute.
        let err = client
            .try_initiate_dispute(
                &initiator,
                &1_u64,
                &String::from_str(&env, "premature"),
            )
            .expect_err("an unresolved poll cannot be disputed");

        assert_eq!(err, Ok(PredictXError::PollNotActive));
        assert_eq!(client.get_poll_status(&1_u64), PollStatus::Voting);
    }

    // ── UserStats (#106) ──────────────────────────────────────────────────────

    #[test]
    fn votes_cast_increments_once_per_vote_across_polls() {
        let (env, _admin, client) = setup();
        let v = voter(&env);

        client.cast_vote(&v, &1_u64, &VoteChoice::Yes);
        client.set_poll_status(&2_u64, &PollStatus::Voting);
        client.cast_vote(&v, &2_u64, &VoteChoice::No);

        let stats = client.get_user_stats(&v);
        assert_eq!(stats.votes_cast, 2);
        assert_eq!(stats.voting_rewards_earned, 0);
    }

    #[test]
    fn voting_rewards_earned_accumulates_across_polls() {
        let (env, admin, client) = setup();
        let winner = voter(&env);
        client.cast_vote(&winner, &1_u64, &VoteChoice::Yes);
        for _ in 0..29 {
            client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        }
        fund_and_resolve_yes(&env, &client, &admin, 300);
        assert_eq!(client.claim_reward(&winner, &1_u64), 10);

        // Second poll: same voter wins again and earns another share.
        client.set_poll_status(&2_u64, &PollStatus::Voting);
        client.cast_vote(&winner, &2_u64, &VoteChoice::Yes);
        for _ in 0..29 {
            client.cast_vote(&voter(&env), &2_u64, &VoteChoice::Yes);
        }
        client.set_reward_pool(&admin, &2_u64, &600);
        env.ledger().set_timestamp(2_000_000 + VOTING_WINDOW_SECS);
        assert_eq!(client.auto_resolve(&2_u64), VoteChoice::Yes);
        assert_eq!(client.claim_reward(&winner, &2_u64), 20);

        let stats = client.get_user_stats(&winner);
        assert_eq!(stats.votes_cast, 2);
        assert_eq!(stats.voting_rewards_earned, 30);
    }

    #[test]
    fn losing_voter_stats_unchanged_on_rejected_claim() {
        let (env, admin, client) = setup();
        let loser = voter(&env);
        client.cast_vote(&loser, &1_u64, &VoteChoice::No);
        for _ in 0..30 {
            client.cast_vote(&voter(&env), &1_u64, &VoteChoice::Yes);
        }
        fund_and_resolve_yes(&env, &client, &admin, 300);

        let err = client
            .try_claim_reward(&loser, &1_u64)
            .expect_err("losing voter must not be paid");
        assert_eq!(err, Ok(PredictXError::VoterNotEligible));

        let stats = client.get_user_stats(&loser);
        assert_eq!(stats.votes_cast, 1);
        assert_eq!(stats.voting_rewards_earned, 0);
    }
}

}