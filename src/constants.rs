/// Platform fee taken from the losing pool on resolution (basis points).
/// 500 bps = 5 %.
pub const PLATFORM_FEE_BPS: u32 = 500;

/// Maximum share of the total pool distributed to voters as rewards (bps).
/// 100 bps = 1 %.
pub const VOTER_REWARD_BPS: u32 = 100;

/// Duration (seconds) that the community voting window stays open after a
/// match finishes. 7 200 s = 2 hours.
pub const VOTING_WINDOW_SECS: u64 = 7_200;

/// Duration (seconds) during which a dispute can be raised after a poll is
/// resolved. 86 400 s = 24 hours.
pub const DISPUTE_WINDOW_SECS: u64 = 86_400;

/// Community vote share (bps) required for automatic resolution without
/// admin intervention. 8 500 bps = 85 %.
pub const AUTO_RESOLVE_THRESHOLD_BPS: u32 = 8_500;

/// Lower bound (bps) below which the vote is sent to admin review rather
/// than auto-resolved. 6 000 bps = 60 %.
pub const ADMIN_REVIEW_THRESHOLD_BPS: u32 = 6_000;

/// Number of admin signatures required to resolve a disputed poll.
pub const MULTI_SIG_REQUIRED: u32 = 3;

/// Maximum byte length of a poll question string.
pub const MAX_QUESTION_LENGTH: u32 = 256;

/// Maximum number of polls that can be attached to a single match.
pub const MAX_POLLS_PER_MATCH: u32 = 50;

/// Divisor used when converting basis-point values to fractions.
/// e.g. fee = amount * PLATFORM_FEE_BPS / BPS_DENOMINATOR
pub const BPS_DENOMINATOR: u32 = 10_000;

/// Seconds after which an unresolved poll can be emergency-cancelled and
/// stakes refunded. 604 800 s = 7 days.
pub const EMERGENCY_TIMEOUT_SECS: u64 = 604_800;