/** Platform fee in basis points (5%). Mirrors `PLATFORM_FEE_BPS` in shared crate. */
export const PLATFORM_FEE_BPS = 500;

/** Voter reward reservation in basis points (1%). Mirrors `VOTER_REWARD_BPS`. */
export const VOTER_REWARD_BPS = 100;

/** Basis-point denominator (10 000 = 100%). */
export const BPS_DENOMINATOR = 10_000;

/** Voting window after poll locks (seconds) — 2 hours. */
export const VOTING_WINDOW_SECS = 7_200;

/** Dispute window (seconds) — 24 hours. */
export const DISPUTE_WINDOW_SECS = 86_400;

/** Consensus threshold for auto-resolve (85%). */
export const AUTO_RESOLVE_THRESHOLD_BPS = 8_500;

/** Minimum consensus for admin review (60%). */
export const ADMIN_REVIEW_THRESHOLD_BPS = 6_000;

/** Multi-sig threshold for admin actions. */
export const MULTI_SIG_REQUIRED = 3;

/** Maximum poll question length (characters). */
export const MAX_QUESTION_LENGTH = 256;

/** Maximum polls allowed per match. */
export const MAX_POLLS_PER_MATCH = 50;

/** Emergency refund timeout — 7 days (seconds). */
export const EMERGENCY_TIMEOUT_SECS = 604_800;

/** Minimum stake amount in base units (10 tokens @ 7 decimals). */
export const MIN_STAKE_AMOUNT = 10_000_000n;
