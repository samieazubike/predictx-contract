use crate::DataKey;
use predictx_shared::{PredictXError, VoteChoice, VoteTally};
use soroban_sdk::{Address, Env, Vec};

// ── Admin registry storage ────────────────────────────────────────────────────

/// Read the registered admins, defaulting to an empty list.
pub fn read_admins(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::AdminList)
        .unwrap_or(Vec::new(env))
}

/// Persist the registered admins.
pub fn write_admins(env: &Env, admins: &Vec<Address>) {
    env.storage().instance().set(&DataKey::AdminList, admins);
}

/// Whether `addr` is a registered admin.
pub fn is_admin(env: &Env, addr: &Address) -> bool {
    read_admins(env).contains(addr.clone())
}

/// Ensure `caller` is a registered admin, else `Unauthorized`.
pub fn require_admin(env: &Env, caller: &Address) -> Result<(), PredictXError> {
    if is_admin(env, caller) {
        Ok(())
    } else {
        Err(PredictXError::Unauthorized)
    }
}

// ── Vote tally storage ────────────────────────────────────────────────────────

/// Read the vote tally for a poll, if one has been stored yet.
///
/// The tally lives in *persistent* storage. In temporary storage it could
/// expire mid-window (silently resetting the counts so a late voter decides
/// the outcome alone) or before `auto_resolve` runs (stranding the poll).
pub fn read_tally(env: &Env, poll_id: u64) -> Option<VoteTally> {
    env.storage().persistent().get(&DataKey::VoteTally(poll_id))
}

/// Store the vote tally for a poll.
pub fn write_tally(env: &Env, tally: &VoteTally) {
    env.storage()
        .persistent()
        .set(&DataKey::VoteTally(tally.poll_id), tally);
}

// ── Voter roster storage ─────────────────────────────────────────────────────

/// Read the persistent voter roster for a poll, defaulting to an empty list.
pub fn read_voters(env: &Env, poll_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&DataKey::Voters(poll_id))
        .unwrap_or(Vec::new(env))
}

/// Persist the voter roster for a poll.
pub fn write_voters(env: &Env, poll_id: u64, voters: &Vec<Address>) {
    env.storage()
        .persistent()
        .set(&DataKey::Voters(poll_id), voters);
}

// ── Vote-dedup storage ────────────────────────────────────────────────────────

/// Whether `voter` has already cast a vote on `poll_id`.
pub fn has_voted(env: &Env, poll_id: u64, voter: &Address) -> bool {
    env.storage()
        .temporary()
        .get(&DataKey::HasVoted(poll_id, voter.clone()))
        .unwrap_or(false)
}

/// Record that `voter` cast a vote on `poll_id`.
///
/// The marker lives in *temporary* storage so it expires with the tally when
/// the voting window closes.
pub fn write_voted(env: &Env, poll_id: u64, voter: &Address) {
    env.storage()
        .temporary()
        .set(&DataKey::HasVoted(poll_id, voter.clone()), &true);
}

// ── Voter reward storage ──────────────────────────────────────────────────────

/// The choice `voter` recorded on `poll_id`, if they voted.
pub fn read_vote_choice(env: &Env, poll_id: u64, voter: &Address) -> Option<VoteChoice> {
    env.storage()
        .persistent()
        .get(&DataKey::VoterChoice(poll_id, voter.clone()))
}

/// Persist the choice `voter` recorded on `poll_id`.
///
/// Stored in *persistent* storage (unlike the temporary tally and dedup marker)
/// because the choice must outlive the voting window so eligible voters can
/// still be identified when rewards are claimed.
pub fn write_vote_choice(env: &Env, poll_id: u64, voter: &Address, choice: VoteChoice) {
    env.storage()
        .persistent()
        .set(&DataKey::VoterChoice(poll_id, voter.clone()), &choice);
}

/// The voter reward reserve for `poll_id` (0 when unset).
pub fn read_reward_pool(env: &Env, poll_id: u64) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::RewardPool(poll_id))
        .unwrap_or(0)
}

/// Persist the voter reward reserve for `poll_id`.
pub fn write_reward_pool(env: &Env, poll_id: u64, amount: i128) {
    env.storage()
        .persistent()
        .set(&DataKey::RewardPool(poll_id), &amount);
}

/// Whether `voter` has already claimed their reward on `poll_id`.
pub fn has_claimed_reward(env: &Env, poll_id: u64, voter: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::RewardClaimed(poll_id, voter.clone()))
        .unwrap_or(false)
}

/// Record that `voter` claimed their reward on `poll_id`.
pub fn write_reward_claimed(env: &Env, poll_id: u64, voter: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::RewardClaimed(poll_id, voter.clone()), &true);
}
