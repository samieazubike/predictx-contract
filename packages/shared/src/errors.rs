use soroban_sdk::contracterror;

/// All errors that can be returned by PredictX contracts.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PredictXError {
    /// Contract has not been initialised yet.
    NotInitialized = 1,
    /// Contract has already been initialised.
    AlreadyInitialized = 2,
    /// Caller is not the admin.
    Unauthorized = 3,
    /// Poll does not exist.
    PollNotFound = 4,
    /// Poll is not in an active/open state.
    PollNotActive = 5,
    /// Poll is locked — no more stakes accepted.
    PollLocked = 6,
    /// Poll is not locked yet.
    PollNotLocked = 7,
    /// Poll outcome has already been resolved.
    PollAlreadyResolved = 8,
    /// Caller does not have sufficient token balance.
    InsufficientBalance = 9,
    /// Stake amount must be greater than zero.
    StakeAmountZero = 10,
    /// Caller has already placed a stake on this poll.
    AlreadyStaked = 11,
    /// Caller has not staked on this poll.
    NotStaker = 12,
    /// Reward has already been claimed.
    AlreadyClaimed = 13,
    /// Caller did not stake on the winning side.
    NotOnWinningSide = 14,
    /// Voting window is not open.
    VotingNotOpen = 15,
    /// Caller has already cast a vote.
    AlreadyVoted = 16,
    /// Stakers cannot vote on their own poll.
    VoterIsStaker = 17,
    /// Voting window has expired.
    VotingWindowExpired = 18,
    /// A dispute is already open for this poll.
    DisputeAlreadyOpen = 19,
    /// Dispute fee was not provided.
    DisputeFeeRequired = 20,
    /// Poll category value is invalid.
    InvalidPollCategory = 21,
    /// Lock/kickoff time must be in the future.
    InvalidLockTime = 22,
    /// Match does not exist.
    MatchNotFound = 23,
    /// Match has already started — updates not allowed.
    MatchAlreadyStarted = 24,
    /// Poll question exceeds maximum length.
    PollQuestionTooLong = 25,
    /// Match already has the maximum number of polls.
    MaxPollsPerMatchReached = 26,
    /// Outcome value is not valid for this poll.
    InvalidOutcome = 27,
    /// Community vote did not reach consensus threshold.
    ConsensusNotReached = 28,
    /// Admin address is already registered.
    AdminAlreadyRegistered = 29,
    /// Not enough admin approvals for this action.
    InsufficientAdminApprovals = 30,
    /// Emergency withdrawal is not permitted at this time.
    EmergencyWithdrawNotAllowed = 31,
    /// Token transfer failed.
    TransferFailed = 32,
}