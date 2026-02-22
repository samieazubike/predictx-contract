/// Platform fee in basis points (BPS). `500` = 5%.
pub const PLATFORM_FEE_BPS: u32 = 500;

/// Maximum voter reward in basis points. `100` = 1%.
pub const VOTER_REWARD_BPS: u32 = 100;

/// Duration of the community voting window in seconds. `7_200` = 2 hours.
pub const VOTING_WINDOW_SECS: u64 = 7_200;

/// Duration of the dispute window in seconds. `86_400` = 24 hours.
pub const DISPUTE_WINDOW_SECS: u64 = 86_400;

/// Vote share threshold for automatic resolution in BPS. `8_500` = 85%.
pub const AUTO_RESOLVE_THRESHOLD_BPS: u32 = 8_500;

/// Vote share threshold for admin review in BPS. `6_000` = 60%.
pub const ADMIN_REVIEW_THRESHOLD_BPS: u32 = 6_000;

/// Number of admin signatures required for multi-sig actions.
pub const MULTI_SIG_REQUIRED: u32 = 3;

/// Maximum length (in characters) for a poll question.
pub const MAX_QUESTION_LENGTH: u32 = 256;

/// Maximum number of polls that can be attached to a single match.
pub const MAX_POLLS_PER_MATCH: u32 = 50;

/// Basis points denominator. Used as: `amount * fee_bps / BPS_DENOMINATOR`.
pub const BPS_DENOMINATOR: u32 = 10_000;

/// Timeout in seconds after which emergency withdrawal may be permitted. `604_800` = 7 days.
pub const EMERGENCY_TIMEOUT_SECS: u64 = 604_800;
