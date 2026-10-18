use soroban_sdk::{contracttype, Address};

/// Storage keys for all PredictX contracts.
///
/// Storage tier guidance:
/// - **Instance**  : admin config, counters, flags — lives as long as the contract.
/// - **Persistent**: polls, matches, stakes, user data — must survive TTL extensions.
/// - **Temporary** : vote tallies — only needed during the voting window.
#[contracttype]
pub enum DataKey {
    // ── Instance storage ──────────────────────────────────────────────────────
    /// Admin `Address`. (Instance)
    Admin,
    /// Soroban token contract `Address` used for staking. (Instance)
    TokenAddress,
    /// Platform fee in basis points. (Instance)
    PlatformFeeBps,
    /// Duration of the voting window in seconds. (Instance)
    VotingWindowSecs,
    /// Duration of the dispute window in seconds. (Instance)
    DisputeWindowSecs,
    /// Consensus threshold in basis points. (Instance)
    ConsensusThresholdBps,
    /// Auto-incrementing poll ID counter. (Instance)
    NextPollId,
    /// Auto-incrementing match ID counter. (Instance)
    NextMatchId,
    /// Initialisation flag. (Instance)
    Initialized,
    /// Registered admins list `Vec<Address>`. (Instance)
    AdminList,
    /// Platform-wide aggregate stats `PlatformStats`. (Instance)
    PlatformStats,
    /// Treasury token balance `i128`. (Instance)
    TreasuryBalance,

    // ── Persistent storage ────────────────────────────────────────────────────
    /// `match_id` → `Match`. (Persistent)
    Match(u64),
    /// `poll_id` → `Poll`. (Persistent)
    Poll(u64),
    /// `(poll_id, user)` → `Stake`. (Persistent)
    Stake(u64, Address),
    /// `user` → `Vec<u64>` poll IDs the user has staked on. (Persistent)
    UserStakes(Address),
    /// `match_id` → `Vec<u64>` poll IDs attached to the match. (Persistent)
    MatchPolls(u64),
    /// `(poll_id, user)` → `bool` — has this user staked? (Persistent)
    HasStaked(u64, Address),
    /// `poll_id` → `Dispute`. (Persistent)
    Dispute(u64),
    /// `(poll_id, admin)` → `bool` — has this admin approved? (Persistent)
    AdminApproval(u64, Address),
    /// `user` → `UserStats`. (Persistent)
    UserStats(Address),
    /// `(poll_id, voter)` → `i128` unclaimed voter reward. (Persistent)
    VoterReward(u64, Address),

    // ── Temporary storage ─────────────────────────────────────────────────────
    /// `poll_id` → `VoteTally`. (Temporary — only needed during voting window)
    VoteTally(u64),
    /// `(poll_id, voter)` → `bool` — has this voter cast a vote? (Temporary)
    HasVoted(u64, Address),
}