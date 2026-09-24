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
        updated_at: now,
    };
    env.storage()
        .persistent()
        .set(&DataKey::PollStatus(poll_id), &stored_status);

    Ok(())
}
