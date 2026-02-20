use soroban_sdk::{contracttype, Address};

/// All storage keys used by the PredictX platform.
///
/// Storage tier guidance (Soroban):
/// - **Instance**:   small config/counters that live as long as the contract.
/// - **Persistent**: core data that must survive contract upgrades and
///                   be available indefinitely (polls, stakes, users).
/// - **Temporary**:  data only needed within a bounded window (vote tallies).
#[contracttype]
#[derive(Clone, Debug)]
pub enum DataKey {
    // ── Instance storage ────────────────────────────────────────────────────

    /// Primary admin address. [Instance]
    Admin,

    /// Address of the Soroban token contract used for staking (e.g. USDC). [Instance]
    TokenAddress,

    /// Platform fee in basis points (500 = 5 %). [Instance]
    PlatformFeeBps,

    /// Duration (seconds) that community voting stays open. [Instance]
    VotingWindowSecs,

    /// Duration (seconds) during which disputes can be raised. [Instance]
    DisputeWindowSecs,

    /// Vote share (bps) required for automatic resolution (8500 = 85 %). [Instance]
    ConsensusThresholdBps,

    /// Monotonically increasing counter used to allocate new poll IDs. [Instance]
    NextPollId,

    /// Monotonically increasing counter used to allocate new match IDs. [Instance]
    NextMatchId,

    /// Aggregate platform-wide statistics. [Instance]
    PlatformStats,

    /// Whether `initialize()` has been called. [Instance]
    Initialized,

    /// Current treasury token balance (platform fee accumulator). [Instance]
    TreasuryBalance,

    // ── Persistent storage ───────────────────────────────────────────────────

    /// `match_id` → `Match`. [Persistent]
    Match(u64),

    /// `poll_id` → `Poll`. [Persistent]
    Poll(u64),

    /// `(poll_id, user)` → `Stake`. [Persistent]
    Stake(u64, Address),

    /// `user` → `Vec<u64>` — list of poll_ids the user has staked on. [Persistent]
    UserStakes(Address),

    /// `match_id` → `Vec<u64>` — list of poll_ids attached to a match. [Persistent]
    MatchPolls(u64),

    /// `(poll_id, user)` → `bool` — guards against double-staking. [Persistent]
    HasStaked(u64, Address),

    /// `poll_id` → `Dispute`. [Persistent]
    Dispute(u64),

    /// List of all registered admin addresses. [Persistent]
    AdminList,

    /// `(poll_id, admin)` → `bool` — records an admin's resolution vote. [Persistent]
    AdminApproval(u64, Address),

    /// `user` → `UserStats`. [Persistent]
    UserStats(Address),

    /// `(poll_id, voter)` → `i128` — voter incentive amount, set on resolution. [Persistent]
    VoterReward(u64, Address),

    // ── Temporary storage ────────────────────────────────────────────────────

    /// `poll_id` → `VoteTally`. [Temporary — only valid during voting window]
    VoteTally(u64),

    /// `(poll_id, voter)` → `bool` — guards against double-voting. [Temporary]
    HasVoted(u64, Address),
}