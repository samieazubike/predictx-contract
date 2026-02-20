use soroban_sdk::contracterror;

/// All error codes for the PredictX platform.
/// Codes are sequential with no gaps so on-chain clients can
/// exhaustively match without silent fallthrough.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredictXError {
    /// Contract has not been initialized yet.
    NotInitialized = 1,
    /// Contract has already been initialized.
    AlreadyInitialized = 2,
    /// Caller is not authorised to perform this action.
    Unauthorized = 3,
    /// No poll exists for the given poll_id.
    PollNotFound = 4,
    /// Poll is not in Active status.
    PollNotActive = 5,
    /// Poll has passed its lock_time; no more stakes accepted.
    PollLocked = 6,
    /// Poll is not yet locked; action requires Locked status.
    PollNotLocked = 7,
    /// Poll outcome has already been determined.
    PollAlreadyResolved = 8,
    /// Caller's token balance is insufficient for the stake amount.
    InsufficientBalance = 9,
    /// Stake amount must be greater than zero.
    StakeAmountZero = 10,
    /// User has already placed a stake on this poll.
    AlreadyStaked = 11,
    /// Caller has no stake on this poll.
    NotStaker = 12,
    /// Caller has already claimed their winnings for this poll.
    AlreadyClaimed = 13,
    /// Caller's stake was on the losing side; no payout available.
    NotOnWinningSide = 14,
    /// Voting window is not currently open for this poll.
    VotingNotOpen = 15,
    /// Caller has already cast a vote on this poll.
    AlreadyVoted = 16,
    /// Stakers may not vote on their own poll (conflict of interest).
    VoterIsStaker = 17,
    /// The voting window has expired.
    VotingWindowExpired = 18,
    /// A dispute is already open for this poll.
    DisputeAlreadyOpen = 19,
    /// Dispute requires a non-zero fee to be paid.
    DisputeFeeRequired = 20,
    /// Provided poll category value is not valid.
    InvalidPollCategory = 21,
    /// lock_time must be in the future and before kickoff.
    InvalidLockTime = 22,
    /// No match exists for the given match_id.
    MatchNotFound = 23,
    /// Match has already started; cannot modify.
    MatchAlreadyStarted = 24,
    /// Poll question exceeds MAX_QUESTION_LENGTH bytes.
    PollQuestionTooLong = 25,
    /// Match already has MAX_POLLS_PER_MATCH polls attached.
    MaxPollsPerMatchReached = 26,
    /// Provided outcome value is not a recognised variant.
    InvalidOutcome = 27,
    /// Community vote did not reach AUTO_RESOLVE_THRESHOLD_BPS consensus.
    ConsensusNotReached = 28,
    /// Address is already registered as an admin.
    AdminAlreadyRegistered = 29,
    /// Multi-sig resolution requires at least MULTI_SIG_REQUIRED approvals.
    InsufficientAdminApprovals = 30,
    /// Emergency withdrawal conditions have not been met.
    EmergencyWithdrawNotAllowed = 31,
    /// Token transfer via the Soroban token interface failed.
    TransferFailed = 32,
}