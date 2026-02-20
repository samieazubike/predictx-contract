use soroban_sdk::{contracttype, Address, String};

// ── Enums ────────────────────────────────────────────────────────────────────

/// Lifecycle state of a prediction poll.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PollStatus {
    /// Accepting new stakes from users.
    Active,
    /// Past lock_time — no more stakes; awaiting match result.
    Locked,
    /// Match finished; community voting window is open.
    Voting,
    /// Vote consensus between 60 – 85 %; requires admin sign-off.
    AdminReview,
    /// A formal dispute has been raised and is under review.
    Disputed,
    /// Outcome confirmed; winners may claim payouts.
    Resolved,
    /// Poll was emergency-cancelled; all stakes are refundable.
    Cancelled,
}

/// High-level category describing what a poll predicts.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PollCategory {
    /// Individual player event (e.g. "Will Palmer score?").
    PlayerEvent,
    /// Team-level event (e.g. "Will Chelsea win?").
    TeamEvent,
    /// Score-based prediction (e.g. "Over 2.5 goals?").
    ScorePrediction,
    /// Any prediction that does not fit the above categories.
    Other,
}

/// Which outcome a user is staking on.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StakeSide {
    Yes,
    No,
}

/// A community voter's assessment of the poll outcome.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteChoice {
    /// Voter believes the "yes" outcome occurred.
    Yes,
    /// Voter believes the "no" outcome occurred.
    No,
    /// Voter cannot determine the outcome (ambiguous evidence).
    Unclear,
}

// ── Structs ──────────────────────────────────────────────────────────────────

/// A real-world football (or other sports) fixture that polls are attached to.
///
/// Stored in **persistent** storage keyed by `DataKey::Match(match_id)` so it
/// survives contract upgrades.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Match {
    /// Unique sequential identifier allocated by the contract.
    pub match_id: u64,
    /// Name of the home team (max 256 bytes).
    pub home_team: String,
    /// Name of the away team (max 256 bytes).
    pub away_team: String,
    /// League / competition name.
    pub league: String,
    /// Stadium or venue name.
    pub venue: String,
    /// Unix timestamp of scheduled kick-off.
    pub kickoff_time: u64,
    /// Admin address that registered this match.
    pub created_by: Address,
    /// Set to `true` once the match result is confirmed on-chain.
    pub is_finished: bool,
}

/// A binary prediction market attached to a match.
///
/// Stored in **persistent** storage keyed by `DataKey::Poll(poll_id)`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Poll {
    /// Unique sequential identifier.
    pub poll_id: u64,
    /// Parent match this poll belongs to.
    pub match_id: u64,
    /// Address that created the poll.
    pub creator: Address,
    /// Human-readable question (max `MAX_QUESTION_LENGTH` bytes).
    pub question: String,
    /// Broad category of the prediction.
    pub category: PollCategory,
    /// Unix timestamp after which no new stakes are accepted.
    pub lock_time: u64,
    /// Total tokens staked on the YES side (i128 per Soroban token standard).
    pub yes_pool: i128,
    /// Total tokens staked on the NO side.
    pub no_pool: i128,
    /// Count of unique stakers on the YES side.
    pub yes_count: u32,
    /// Count of unique stakers on the NO side.
    pub no_count: u32,
    /// Current lifecycle state.
    pub status: PollStatus,
    /// Resolved outcome: `Some(true)` = Yes, `Some(false)` = No, `None` = pending.
    pub outcome: Option<bool>,
    /// Unix timestamp when the outcome was finalised (0 if unresolved).
    pub resolution_time: u64,
    /// Unix timestamp when the poll was created.
    pub created_at: u64,
}

/// A single user's stake on one side of a poll.
///
/// Stored in **persistent** storage keyed by `DataKey::Stake(poll_id, user)`
/// so it is available throughout the claim period.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Stake {
    /// Staker's wallet address.
    pub user: Address,
    /// Poll this stake belongs to.
    pub poll_id: u64,
    /// Token amount staked (i128).
    pub amount: i128,
    /// Which outcome the user is backing.
    pub side: StakeSide,
    /// `true` once the user has successfully claimed their payout.
    pub claimed: bool,
    /// Unix timestamp of stake placement.
    pub staked_at: u64,
}

/// Accumulated community vote totals for a poll during the voting window.
///
/// Stored in **temporary** storage keyed by `DataKey::VoteTally(poll_id)`
/// because it is only required until the voting window closes and the outcome
/// is written to the persistent `Poll` record.
#[contracttype]
#[derive(Clone, Debug)]
pub struct VoteTally {
    /// Poll being voted on.
    pub poll_id: u64,
    /// Votes cast for YES.
    pub yes_votes: u32,
    /// Votes cast for NO.
    pub no_votes: u32,
    /// Votes cast as UNCLEAR.
    pub unclear_votes: u32,
    /// Total unique voters.
    pub total_voters: u32,
    /// Unix timestamp when the voting window closes.
    pub voting_end_time: u64,
    /// Token pool reserved for voter incentives (0.5 – 1 % of total pool).
    pub reward_pool: i128,
}

/// A formal dispute raised against a resolved or resolving poll.
///
/// Stored in **persistent** storage keyed by `DataKey::Dispute(poll_id)`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Dispute {
    /// Poll under dispute.
    pub poll_id: u64,
    /// Address that raised the dispute.
    pub initiator: Address,
    /// IPFS hash or other external reference to supporting evidence.
    pub evidence_hash: String,
    /// Fee paid by the initiator to open the dispute (returned if upheld).
    pub dispute_fee: i128,
    /// Number of admin approvals collected so far.
    pub admin_approvals: u32,
    /// Approvals required to resolve (`MULTI_SIG_REQUIRED`).
    pub required_approvals: u32,
    /// `true` once admins have reached a decision.
    pub resolved: bool,
    /// Unix timestamp when the dispute was opened.
    pub initiated_at: u64,
}

/// Aggregate statistics for the entire platform.
///
/// Stored in **instance** storage keyed by `DataKey::PlatformStats` because
/// it is small, frequently updated, and should live as long as the contract.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PlatformStats {
    /// Sum of all tokens currently held across active and locked polls.
    pub total_value_locked: i128,
    /// Cumulative number of polls ever created.
    pub total_polls_created: u64,
    /// Cumulative number of individual stakes placed.
    pub total_stakes_placed: u64,
    /// Cumulative tokens paid out to winners.
    pub total_payouts: i128,
    /// Count of unique addresses that have ever interacted with the platform.
    pub total_users: u64,
}

/// Per-user lifetime statistics.
///
/// Stored in **persistent** storage keyed by `DataKey::UserStats(user)`.
#[contracttype]
#[derive(Clone, Debug)]
pub struct UserStats {
    /// Total tokens ever staked across all polls.
    pub total_staked: i128,
    /// Total tokens received as winnings.
    pub total_won: i128,
    /// Total tokens lost to the losing pool.
    pub total_lost: i128,
    /// Number of polls the user has staked on.
    pub polls_participated: u32,
    /// Number of those polls where the user was on the winning side.
    pub polls_won: u32,
    /// Number of those polls where the user was on the losing side.
    pub polls_lost: u32,
    /// Total community votes cast.
    pub votes_cast: u32,
    /// Cumulative voting incentive tokens earned.
    pub voting_rewards_earned: i128,
}