#![no_std]

pub mod constants;
pub mod errors;
pub mod storage;
pub mod types;

// Re-export everything so downstream contracts just do:
// use predictx_contract::{Poll, DataKey, PredictXError, ...};
pub use constants::*;
pub use errors::PredictXError;
pub use storage::DataKey;
pub use types::{
    Dispute, Match, PlatformStats, Poll, PollCategory, PollStatus, Stake, StakeSide,
    UserStats, VoteChoice, VoteTally,
};

use soroban_sdk::{contract, contractimpl, Env};

/// Placeholder entry-point contract.
/// Feature contracts (#3 – #18) will replace or extend this.
#[contract]
pub struct PredictXContract;

#[contractimpl]
impl PredictXContract {
    /// Returns `true` once the shared module compiles and is reachable.
    /// Replace with real initialization logic in a later milestone.
    pub fn ping(_env: Env) -> bool {
        true
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std; // no_std crate — link std explicitly for the test harness

    use super::*;
    use soroban_sdk::{
        testutils::Address as _,
        Address, Env, String,
    };

    // ── Helper ────────────────────────────────────────────────────────────────

    fn dummy_address(env: &Env) -> Address {
        Address::generate(env)
    }

    fn short_string(env: &Env, s: &str) -> String {
        String::from_str(env, s)
    }

    // ── Constants ─────────────────────────────────────────────────────────────

    #[test]
    fn test_constants_values() {
        assert_eq!(PLATFORM_FEE_BPS, 500);
        assert_eq!(VOTER_REWARD_BPS, 100);
        assert_eq!(VOTING_WINDOW_SECS, 7_200);
        assert_eq!(DISPUTE_WINDOW_SECS, 86_400);
        assert_eq!(AUTO_RESOLVE_THRESHOLD_BPS, 8_500);
        assert_eq!(ADMIN_REVIEW_THRESHOLD_BPS, 6_000);
        assert_eq!(MULTI_SIG_REQUIRED, 3);
        assert_eq!(MAX_QUESTION_LENGTH, 256);
        assert_eq!(MAX_POLLS_PER_MATCH, 50);
        assert_eq!(BPS_DENOMINATOR, 10_000);
        assert_eq!(EMERGENCY_TIMEOUT_SECS, 604_800);
    }

    #[test]
    fn test_fee_arithmetic() {
        // 5 % of 1_000_000 tokens
        let amount: u64 = 1_000_000;
        let fee = amount * (PLATFORM_FEE_BPS as u64) / (BPS_DENOMINATOR as u64);
        assert_eq!(fee, 50_000);
    }

    // ── Errors ────────────────────────────────────────────────────────────────

    #[test]
    fn test_error_codes_are_unique() {
        // Verify that the numeric values we rely on are stable
        assert_eq!(PredictXError::NotInitialized as u32, 1);
        assert_eq!(PredictXError::AlreadyInitialized as u32, 2);
        assert_eq!(PredictXError::Unauthorized as u32, 3);
        assert_eq!(PredictXError::TransferFailed as u32, 32);
    }

    #[test]
    fn test_error_equality() {
        assert_eq!(PredictXError::PollNotFound, PredictXError::PollNotFound);
        assert_ne!(PredictXError::PollNotFound, PredictXError::MatchNotFound);
    }

    // ── Enums (contracttype round-trip via Soroban env) ───────────────────────

    #[test]
    fn test_poll_status_variants() {
        // Ensure all variants are reachable and distinct
        let variants = [
            PollStatus::Active,
            PollStatus::Locked,
            PollStatus::Voting,
            PollStatus::AdminReview,
            PollStatus::Disputed,
            PollStatus::Resolved,
            PollStatus::Cancelled,
        ];
        // All seven must be non-equal to each other
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn test_stake_side_variants() {
        assert_eq!(StakeSide::Yes, StakeSide::Yes);
        assert_ne!(StakeSide::Yes, StakeSide::No);
    }

    #[test]
    fn test_vote_choice_variants() {
        assert_ne!(VoteChoice::Yes, VoteChoice::No);
        assert_ne!(VoteChoice::No, VoteChoice::Unclear);
    }

    #[test]
    fn test_poll_category_variants() {
        assert_eq!(PollCategory::PlayerEvent, PollCategory::PlayerEvent);
        assert_ne!(PollCategory::TeamEvent, PollCategory::ScorePrediction);
    }

    // ── Structs ───────────────────────────────────────────────────────────────

    #[test]
    fn test_match_struct_construction() {
        let env = Env::default();
        let m = Match {
            match_id: 1,
            home_team: short_string(&env, "Arsenal"),
            away_team: short_string(&env, "Chelsea"),
            league: short_string(&env, "Premier League"),
            venue: short_string(&env, "Emirates Stadium"),
            kickoff_time: 1_700_000_000,
            created_by: dummy_address(&env),
            is_finished: false,
        };
        assert_eq!(m.match_id, 1);
        assert!(!m.is_finished);
    }

    #[test]
    fn test_poll_struct_construction() {
        let env = Env::default();
        let p = Poll {
            poll_id: 42,
            match_id: 1,
            creator: dummy_address(&env),
            question: short_string(&env, "Will Salah score?"),
            category: PollCategory::PlayerEvent,
            lock_time: 1_700_000_000,
            yes_pool: 5_000_000,
            no_pool: 3_000_000,
            yes_count: 10,
            no_count: 6,
            status: PollStatus::Active,
            outcome: None,
            resolution_time: 0,
            created_at: 1_699_000_000,
        };
        assert_eq!(p.poll_id, 42);
        assert_eq!(p.yes_pool + p.no_pool, 8_000_000);
        assert!(p.outcome.is_none());
    }

    #[test]
    fn test_stake_struct_construction() {
        let env = Env::default();
        let s = Stake {
            user: dummy_address(&env),
            poll_id: 42,
            amount: 1_000_000,
            side: StakeSide::Yes,
            claimed: false,
            staked_at: 1_699_500_000,
        };
        assert_eq!(s.amount, 1_000_000);
        assert!(!s.claimed);
    }

    #[test]
    fn test_vote_tally_construction() {
        let env = Env::default();
        let _ = env; // env required for contracttype in future
        let vt = VoteTally {
            poll_id: 1,
            yes_votes: 70,
            no_votes: 20,
            unclear_votes: 10,
            total_voters: 100,
            voting_end_time: 1_700_100_000,
            reward_pool: 80_000,
        };
        assert_eq!(vt.yes_votes + vt.no_votes + vt.unclear_votes, vt.total_voters);
    }

    #[test]
    fn test_dispute_construction() {
        let env = Env::default();
        let d = Dispute {
            poll_id: 5,
            initiator: dummy_address(&env),
            evidence_hash: short_string(&env, "QmXoypizjW3WknFiJnKLwHCnL72vedxjQkDDP1mXWo6uco"),
            dispute_fee: 10_000_000,
            admin_approvals: 0,
            required_approvals: MULTI_SIG_REQUIRED,
            resolved: false,
            initiated_at: 1_700_200_000,
        };
        assert_eq!(d.required_approvals, 3);
        assert!(!d.resolved);
    }

    #[test]
    fn test_platform_stats_default_zero() {
        let ps = PlatformStats {
            total_value_locked: 0,
            total_polls_created: 0,
            total_stakes_placed: 0,
            total_payouts: 0,
            total_users: 0,
        };
        assert_eq!(ps.total_value_locked, 0);
    }

    #[test]
    fn test_user_stats_default_zero() {
        let us = UserStats {
            total_staked: 0,
            total_won: 0,
            total_lost: 0,
            polls_participated: 0,
            polls_won: 0,
            polls_lost: 0,
            votes_cast: 0,
            voting_rewards_earned: 0,
        };
        assert_eq!(us.polls_won + us.polls_lost, us.polls_participated);
    }

    // ── Storage key uniqueness ────────────────────────────────────────────────

    #[test]
    fn test_storage_key_variants_are_distinct() {
        let env = Env::default();
        let addr = dummy_address(&env);

        // Composite keys with different IDs must differ
        let k1 = DataKey::Poll(1);
        let k2 = DataKey::Poll(2);
        assert_ne!(std::format!("{:?}", k1), std::format!("{:?}", k2));

        // Composite keys with different addresses must differ
        let addr2 = dummy_address(&env);
        let k3 = DataKey::Stake(1, addr.clone());
        let k4 = DataKey::Stake(1, addr2);
        assert_ne!(std::format!("{:?}", k3), std::format!("{:?}", k4));
    }

    // ── Contract smoke-test ───────────────────────────────────────────────────

    #[test]
    fn test_ping() {
        let env = Env::default();
        let cid = env.register(PredictXContract, ());
        let client = PredictXContractClient::new(&env, &cid);
        assert!(client.ping());
    }
}