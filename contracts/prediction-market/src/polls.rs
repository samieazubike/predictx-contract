use soroban_sdk::{Address, Env, String, Symbol, Vec};
use predictx_shared::{Match, Poll, PollCategory, PollStatus, PredictXError};
use predictx_shared::{MAX_QUESTION_LENGTH, MAX_POLLS_PER_MATCH};
use crate::DataKey;

// ── Internal helpers ──────────────────────────────────────────────────────────

fn get_match(env: &Env, match_id: u64) -> Result<Match, PredictXError> {
    env.storage()
        .persistent()
        .get(&DataKey::Match(match_id))
        .ok_or(PredictXError::MatchNotFound)
}

fn get_next_poll_id(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::NextPollId)
        .unwrap_or(1u64)
}

fn set_next_poll_id(env: &Env, id: u64) {
    env.storage().instance().set(&DataKey::NextPollId, &id);
}

fn get_poll_count(env: &Env) -> u64 {
    get_next_poll_id(env).saturating_sub(1)
}

fn get_poll(env: &Env, poll_id: u64) -> Result<Poll, PredictXError> {
    env.storage()
        .persistent()
        .get(&DataKey::Poll(poll_id))
        .ok_or(PredictXError::PollNotFound)
}

fn store_poll(env: &Env, poll: &Poll) {
    env.storage().persistent().set(&DataKey::Poll(poll.poll_id), poll);
}

fn get_match_polls(env: &Env, match_id: u64) -> Result<Vec<u64>, PredictXError> {
    if !env.storage().persistent().has(&DataKey::Match(match_id)) {
        return Err(PredictXError::MatchNotFound);
    }
    Ok(env
        .storage()
        .persistent()
        .get(&DataKey::MatchPolls(match_id))
        .unwrap_or(Vec::new(env)))
}

fn add_poll_to_match(env: &Env, match_id: u64, poll_id: u64) -> Result<(), PredictXError> {
    let mut polls: Vec<u64> = get_match_polls(env, match_id)?;
    polls.push_back(poll_id);
    env.storage().persistent().set(&DataKey::MatchPolls(match_id), &polls);
    Ok(())
}

fn get_polls_by_status(env: &Env, status: PollStatus) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::PollsByStatus(status))
        .unwrap_or(Vec::new(env))
}

fn add_poll_to_status_index(env: &Env, poll_id: u64, status: PollStatus) {
    let mut polls: Vec<u64> = get_polls_by_status(env, status);
    polls.push_back(poll_id);
    env.storage().persistent().set(&DataKey::PollsByStatus(status), &polls);
}

fn remove_poll_from_status_index(env: &Env, poll_id: u64, status: PollStatus) {
    let polls: Vec<u64> = get_polls_by_status(env, status);
    let mut new_polls = Vec::new(env);
    for id in polls.iter() {
        if id != poll_id {
            new_polls.push_back(id);
        }
    }
    env.storage().persistent().set(&DataKey::PollsByStatus(status), &new_polls);
}

fn update_poll_status(env: &Env, poll: &mut Poll, new_status: PollStatus) {
    remove_poll_from_status_index(env, poll.poll_id, poll.status);
    poll.status = new_status;
    add_poll_to_status_index(env, poll.poll_id, new_status);
    store_poll(env, poll);
}

fn validate_question_length(_env: &Env, question: &String) -> Result<(), PredictXError> {
    let len = question.len() as u32;
    if len > MAX_QUESTION_LENGTH {
        return Err(PredictXError::PollQuestionTooLong);
    }
    Ok(())
}

fn validate_lock_time(env: &Env, lock_time: u64, kickoff_time: u64) -> Result<(), PredictXError> {
    let now = env.ledger().timestamp();
    
    // Lock time must be in the future
    if lock_time <= now {
        return Err(PredictXError::InvalidLockTime);
    }
    
    // Lock time must be <= match kickoff time
    if lock_time > kickoff_time {
        return Err(PredictXError::InvalidLockTime);
    }
    
    Ok(())
}

fn validate_poll_count_for_match(env: &Env, match_id: u64) -> Result<(), PredictXError> {
    let polls: Vec<u64> = get_match_polls(env, match_id)?;
    if polls.len() as u32 >= MAX_POLLS_PER_MATCH {
        return Err(PredictXError::MaxPollsPerMatchReached);
    }
    Ok(())
}

// ── Poll Creation ─────────────────────────────────────────────────────────────

pub fn create_poll(
    env: &Env,
    creator: Address,
    match_id: u64,
    question: String,
    category: PollCategory,
    lock_time: u64,
) -> Result<u64, PredictXError> {
    // Require auth from creator
    creator.require_auth();
    
    // Validate match exists and hasn't started
    let m = get_match(env, match_id)?;
    let now = env.ledger().timestamp();
    if now >= m.kickoff_time {
        return Err(PredictXError::MatchAlreadyStarted);
    }
    
    // Validate question length
    validate_question_length(env, &question)?;
    
    // Validate lock time
    validate_lock_time(env, lock_time, m.kickoff_time)?;
    
    // Validate max polls per match
    validate_poll_count_for_match(env, match_id)?;
    
    // Auto-increment poll ID
    let poll_id = get_next_poll_id(env);
    set_next_poll_id(env, poll_id + 1);
    
    // Initialize poll
    let poll = Poll {
        poll_id,
        match_id,
        creator: creator.clone(),
        question: question.clone(),
        category,
        lock_time,
        yes_pool: 0,
        no_pool: 0,
        yes_count: 0,
        no_count: 0,
        status: PollStatus::Active,
        outcome: None,
        resolution_time: 0,
        created_at: now,
    };
    
    // Store poll
    store_poll(env, &poll);
    
    // Add to match polls
    add_poll_to_match(env, match_id, poll_id)?;
    
    // Add to status index
    add_poll_to_status_index(env, poll_id, PollStatus::Active);
    
    // Update platform stats
    let stats = crate::get_platform_stats(env);
    let new_stats = predictx_shared::PlatformStats {
        total_polls_created: stats.total_polls_created + 1,
        ..stats
    };
    crate::set_platform_stats(env, &new_stats);
    
    // Emit event
    env.events().publish(
        (Symbol::new(env, "PollCreated"), poll_id),
        (creator, match_id, question, category, lock_time),
    );
    
    Ok(poll_id)
}

// ── Auto-lock ───────────────────────────────────────────────────────────────────

pub fn check_and_lock_poll(env: &Env, poll_id: u64) -> Result<PollStatus, PredictXError> {
    let mut poll = get_poll(env, poll_id)?;
    let now = env.ledger().timestamp();
    
    // Only transition if currently Active and time has passed lock_time
    if poll.status == PollStatus::Active && now >= poll.lock_time {
        update_poll_status(env, &mut poll, PollStatus::Locked);
        
        // Emit event
        env.events().publish(
            (Symbol::new(env, "PollLocked"), poll_id),
            (poll.lock_time, now),
        );
    }
    
    Ok(poll.status)
}

// ── Status Transition Functions (called by oracle/admin) ─────────────────────

pub fn set_poll_voting(
    env: &Env,
    oracle: Address,
    poll_id: u64,
) -> Result<(), PredictXError> {
    oracle.require_auth();
    
    // Verify this is the authorized oracle
    let stored_oracle = crate::get_oracle(env)?;
    if oracle != stored_oracle {
        return Err(PredictXError::Unauthorized);
    }
    
    let mut poll = get_poll(env, poll_id)?;
    
    // Can only transition from Locked to Voting
    if poll.status != PollStatus::Locked {
        return Err(PredictXError::PollNotLocked);
    }
    
    let old_status = poll.status;
    update_poll_status(env, &mut poll, PollStatus::Voting);
    
    // Emit event
    env.events().publish(
        (Symbol::new(env, "PollStatusChanged"), poll_id),
        (old_status as u32, PollStatus::Voting as u32),
    );
    
    Ok(())
}

pub fn set_poll_resolved(
    env: &Env,
    oracle: Address,
    poll_id: u64,
    outcome: bool,
) -> Result<(), PredictXError> {
    oracle.require_auth();
    
    // Verify this is the authorized oracle
    let stored_oracle = crate::get_oracle(env)?;
    if oracle != stored_oracle {
        return Err(PredictXError::Unauthorized);
    }
    
    let mut poll = get_poll(env, poll_id)?;
    
    // Can transition from Voting or AdminReview or Disputed to Resolved
    let valid_prior_statuses = [PollStatus::Voting, PollStatus::AdminReview, PollStatus::Disputed];
    if !valid_prior_statuses.contains(&poll.status) {
        return Err(PredictXError::PollAlreadyResolved);
    }
    
    let old_status = poll.status;
    let now = env.ledger().timestamp();
    
    poll.outcome = Some(outcome);
    poll.resolution_time = now;
    update_poll_status(env, &mut poll, PollStatus::Resolved);
    
    // Emit event
    env.events().publish(
        (Symbol::new(env, "PollStatusChanged"), poll_id),
        (old_status as u32, PollStatus::Resolved as u32, outcome),
    );
    
    Ok(())
}

pub fn set_poll_admin_review(
    env: &Env,
    oracle: Address,
    poll_id: u64,
) -> Result<(), PredictXError> {
    oracle.require_auth();
    
    // Verify this is the authorized oracle
    let stored_oracle = crate::get_oracle(env)?;
    if oracle != stored_oracle {
        return Err(PredictXError::Unauthorized);
    }
    
    let mut poll = get_poll(env, poll_id)?;
    
    // Can only transition from Voting to AdminReview
    if poll.status != PollStatus::Voting {
        return Err(PredictXError::VotingNotOpen);
    }
    
    let old_status = poll.status;
    update_poll_status(env, &mut poll, PollStatus::AdminReview);
    
    // Emit event
    env.events().publish(
        (Symbol::new(env, "PollStatusChanged"), poll_id),
        (old_status as u32, PollStatus::AdminReview as u32),
    );
    
    Ok(())
}

pub fn set_poll_disputed(
    env: &Env,
    oracle: Address,
    poll_id: u64,
) -> Result<(), PredictXError> {
    oracle.require_auth();
    
    // Verify this is the authorized oracle
    let stored_oracle = crate::get_oracle(env)?;
    if oracle != stored_oracle {
        return Err(PredictXError::Unauthorized);
    }
    
    let mut poll = get_poll(env, poll_id)?;
    
    // Can transition from Voting or AdminReview to Disputed
    let valid_prior_statuses = [PollStatus::Voting, PollStatus::AdminReview];
    if !valid_prior_statuses.contains(&poll.status) {
        return Err(PredictXError::InvalidOutcome);
    }
    
    let old_status = poll.status;
    update_poll_status(env, &mut poll, PollStatus::Disputed);
    
    // Emit event
    env.events().publish(
        (Symbol::new(env, "PollStatusChanged"), poll_id),
        (old_status as u32, PollStatus::Disputed as u32),
    );
    
    Ok(())
}

pub fn set_poll_cancelled(
    env: &Env,
    admin: Address,
    poll_id: u64,
) -> Result<(), PredictXError> {
    admin.require_auth();
    
    // Verify this is the admin
    let stored_admin = crate::get_admin(env)?;
    if admin != stored_admin {
        return Err(PredictXError::Unauthorized);
    }
    
    let mut poll = get_poll(env, poll_id)?;
    
    // Cannot cancel already resolved or cancelled polls
    if poll.status == PollStatus::Resolved || poll.status == PollStatus::Cancelled {
        return Err(PredictXError::PollAlreadyResolved);
    }
    
    let old_status = poll.status;
    update_poll_status(env, &mut poll, PollStatus::Cancelled);
    
    // Emit events
    env.events().publish(
        (Symbol::new(env, "PollStatusChanged"), poll_id),
        (old_status as u32, PollStatus::Cancelled as u32),
    );
    
    env.events().publish(
        (Symbol::new(env, "PollCancelled"), poll_id),
        (),
    );
    
    Ok(())
}

// ── View Functions ────────────────────────────────────────────────────────────

pub fn get_poll_view(env: &Env, poll_id: u64) -> Result<Poll, PredictXError> {
    get_poll(env, poll_id)
}

pub fn get_polls_by_match_view(env: &Env, match_id: u64) -> Result<Vec<Poll>, PredictXError> {
    let poll_ids = get_match_polls(env, match_id)?;
    let mut polls = Vec::new(env);
    
    for poll_id in poll_ids.iter() {
        if let Ok(poll) = get_poll(env, poll_id) {
            polls.push_back(poll);
        }
    }
    
    Ok(polls)
}

pub fn get_polls_by_status_view(env: &Env, status: PollStatus) -> Vec<u64> {
    get_polls_by_status(env, status)
}

pub fn get_poll_count_view(env: &Env) -> u64 {
    get_poll_count(env)
}

pub fn get_poll_status(env: &Env, poll_id: u64) -> Result<PollStatus, PredictXError> {
    let poll = get_poll(env, poll_id)?;
    Ok(poll.status)
}

// ── Convenience function ──────────────────────────────────────────────────────

pub fn get_poll_status_with_auto_lock(env: &Env, poll_id: u64) -> Result<PollStatus, PredictXError> {
    // Try to auto-lock if needed, then return status
    check_and_lock_poll(env, poll_id)
}
