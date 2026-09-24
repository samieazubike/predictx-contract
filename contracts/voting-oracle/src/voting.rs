use predictx_shared::{
    constants::{ADMIN_REVIEW_THRESHOLD_BPS, AUTO_RESOLVE_THRESHOLD_BPS},
    PollStatus, VoteTally,
};
use soroban_sdk::{Env, Symbol};
use crate::{DataKey, StoredPollStatus};

pub fn determine_poll_status(tally: &VoteTally) -> PollStatus {
    let decisive_votes = tally.yes_votes + tally.no_votes;

    if decisive_votes == 0 {
        return PollStatus::Disputed;
    }

    let max_votes = if tally.yes_votes > tally.no_votes {
        tally.yes_votes
    } else {
        tally.no_votes
    };

    let consensus_bps = (max_votes as u64 * 10_000) / (decisive_votes as u64);

    if consensus_bps < ADMIN_REVIEW_THRESHOLD_BPS as u64 {
        PollStatus::Disputed
    } else if consensus_bps < AUTO_RESOLVE_THRESHOLD_BPS as u64 {
        PollStatus::AdminReview
    } else {
        PollStatus::Resolved
    }
}

pub fn process_tally(env: &Env, tally: &VoteTally) -> PollStatus {
    let status = determine_poll_status(tally);

    let stored = StoredPollStatus {
        status,
        updated_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&DataKey::PollStatus(tally.poll_id), &stored);

    env.events().publish(
        (Symbol::new(env, "PollStatusChanged"), tally.poll_id),
        status,
    );

    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_55_percent_consensus_disputed() {
        let tally = VoteTally {
            poll_id: 1,
            yes_votes: 55,
            no_votes: 45,
            unclear_votes: 0,
            total_voters: 100,
            voting_end_time: 0,
            reward_pool: 0,
        };
        assert_eq!(determine_poll_status(&tally), PollStatus::Disputed);
    }

    #[test]
    fn test_zero_decisive_votes_disputed() {
        let tally = VoteTally {
            poll_id: 2,
            yes_votes: 0,
            no_votes: 0,
            unclear_votes: 10,
            total_voters: 10,
            voting_end_time: 0,
            reward_pool: 0,
        };
        assert_eq!(determine_poll_status(&tally), PollStatus::Disputed);
    }

    #[test]
    fn test_above_review_threshold() {
        let tally = VoteTally {
            poll_id: 3,
            yes_votes: 65,
            no_votes: 35,
            unclear_votes: 0,
            total_voters: 100,
            voting_end_time: 0,
            reward_pool: 0,
        };
        assert_eq!(determine_poll_status(&tally), PollStatus::AdminReview);
        
        let tally_resolved = VoteTally {
            poll_id: 4,
            yes_votes: 90,
            no_votes: 10,
            unclear_votes: 0,
            total_voters: 100,
            voting_end_time: 0,
            reward_pool: 0,
        };
        assert_eq!(determine_poll_status(&tally_resolved), PollStatus::Resolved);
    }
}
