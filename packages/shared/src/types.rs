use soroban_sdk::{contracttype, Address, String};

// ── Enums ─────────────────────────────────────────────────────────────────────

/// Lifecycle state of a poll.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PollStatus {
    /// Accepting stakes.
    Active = 0,
    /// Past lock time — no more stakes.
    Locked = 1,
    /// Match ended — community voting in progress.
    Voting = 2,
    /// Vote consensus between 60–85% — needs admin review.
    AdminReview = 3,
    /// Under dispute review.
    Disputed = 4,
    /// Outcome determined — claims open.
    Resolved = 5,
    /// Emergency cancelled — refunds available.
    Cancelled = 6,
}

/// Category of prediction a poll covers.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PollCategory {
    /// e.g. "Will Palmer score?"
    PlayerEvent = 0,
    /// e.g. "Will Chelsea win?"
    TeamEvent = 1,
    /// e.g. "Over 2.5 goals?"
    ScorePrediction = 2,
    /// Custom / other predictions.
    Other = 3,
}

/// Which side of a poll a user staked on.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StakeSide {
    Yes = 0,
    No = 1,
}

/// A community voter's choice.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoteChoice {
    Yes = 0,
    No = 1,
    /// For genuinely ambiguous outcomes.
    Unclear = 2,
}

// ── Structs ───────────────────────────────────────────────────────────────────

/// A football match that polls are grouped under.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Match {
    pub match_id: u64,
    /// Home team name (max 256 bytes).
    pub home_team: String,
    /// Away team name (max 256 bytes).
    pub away_team: String,
    /// League or competition name.
    pub league: String,
    /// Stadium / venue name.
    pub venue: String,
    /// Unix timestamp for kick-off.
    pub kickoff_time: u64,
    /// Admin who created the match.
    pub created_by: Address,
    /// Set to `true` after the match ends; prerequisite for poll voting.
    pub is_finished: bool,
}

/// A prediction market poll belonging to a match.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Poll {
    pub poll_id: u64,
    pub match_id: u64,
    pub creator: Address,
    /// Prediction question (max 256 chars).
    pub question: String,
    pub category: PollCategory,
    /// Unix timestamp after which staking is disallowed.
    pub lock_time: u64,
    /// Total token amount staked on Yes (i128 — Soroban token standard).
    pub yes_pool: i128,
    /// Total token amount staked on No.
    pub no_pool: i128,
    /// Number of individual stakers on Yes.
    pub yes_count: u32,
    /// Number of individual stakers on No.
    pub no_count: u32,
    pub status: PollStatus,
    /// `None` until resolved; `Some(true)` = Yes won, `Some(false)` = No won.
    pub outcome: Option<bool>,
    /// Unix timestamp when the poll was resolved.
    pub resolution_time: u64,
    /// Unix timestamp when the poll was created.
    pub created_at: u64,
}

/// A single user's stake on a poll.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stake {
    pub user: Address,
    pub poll_id: u64,
    /// Token amount staked.
    pub amount: i128,
    pub side: StakeSide,
    /// Whether the user has claimed their reward.
    pub claimed: bool,
    pub staked_at: u64,
}

/// Aggregated community vote data for a poll.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoteTally {
    pub poll_id: u64,
    pub yes_votes: u32,
    pub no_votes: u32,
    pub unclear_votes: u32,
    pub total_voters: u32,
    pub voting_end_time: u64,
    /// 0.5–1% of total pool reserved as voter incentives.
    pub reward_pool: i128,
}

/// A dispute raised against a resolved poll.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dispute {
    pub poll_id: u64,
    pub initiator: Address,
    /// IPFS hash or external evidence reference.
    pub evidence_hash: String,
    pub dispute_fee: i128,
    pub admin_approvals: u32,
    /// Multi-sig threshold (default: 3).
    pub required_approvals: u32,
    pub resolved: bool,
    pub initiated_at: u64,
}

/// Platform-wide aggregate statistics.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformStats {
    pub total_value_locked: i128,
    pub total_polls_created: u64,
    pub total_stakes_placed: u64,
    pub total_payouts: i128,
    pub total_users: u64,
}

/// Per-user activity statistics.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserStats {
    pub total_staked: i128,
    pub total_won: i128,
    pub total_lost: i128,
    pub polls_participated: u32,
    pub polls_won: u32,
    pub polls_lost: u32,
    pub votes_cast: u32,
    pub voting_rewards_earned: i128,
}