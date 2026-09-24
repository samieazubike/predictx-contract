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
    }
}
