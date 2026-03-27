/**
 * PredictX SDK — Error mapping layer.
 *
 * Maps on-chain `PredictXError` u32 codes to typed TypeScript errors so
 * callers never need to decode raw contract error codes.
 */

/** Numeric error codes emitted by the PredictX smart contracts. */
export enum PredictXErrorCode {
  NotInitialized = 1,
  AlreadyInitialized = 2,
  Unauthorized = 3,
  PollNotFound = 4,
  PollNotActive = 5,
  PollLocked = 6,
  PollNotLocked = 7,
  PollAlreadyResolved = 8,
  InsufficientBalance = 9,
  StakeAmountZero = 10,
  AlreadyStaked = 11,
  NotStaker = 12,
  AlreadyClaimed = 13,
  NotOnWinningSide = 14,
  VotingNotOpen = 15,
  AlreadyVoted = 16,
  VoterIsStaker = 17,
  VotingWindowExpired = 18,
  DisputeAlreadyOpen = 19,
  DisputeFeeRequired = 20,
  InvalidPollCategory = 21,
  InvalidLockTime = 22,
  MatchNotFound = 23,
  MatchAlreadyStarted = 24,
  PollQuestionTooLong = 25,
  MaxPollsPerMatchReached = 26,
  InvalidOutcome = 27,
  ConsensusNotReached = 28,
  AdminAlreadyRegistered = 29,
  InsufficientAdminApprovals = 30,
  EmergencyWithdrawNotAllowed = 31,
  TransferFailed = 32,
  ContractPaused = 33,
  StakeBelowMinimum = 34,
}

const ERROR_MESSAGES: Record<PredictXErrorCode, string> = {
  [PredictXErrorCode.NotInitialized]: "Contract is not initialized",
  [PredictXErrorCode.AlreadyInitialized]: "Contract is already initialized",
  [PredictXErrorCode.Unauthorized]: "Unauthorized: admin privileges required",
  [PredictXErrorCode.PollNotFound]: "Poll not found",
  [PredictXErrorCode.PollNotActive]: "Poll is no longer active",
  [PredictXErrorCode.PollLocked]: "Poll is locked — no more stakes accepted",
  [PredictXErrorCode.PollNotLocked]: "Poll has not been locked yet",
  [PredictXErrorCode.PollAlreadyResolved]: "Poll has already been resolved",
  [PredictXErrorCode.InsufficientBalance]: "Insufficient token balance",
  [PredictXErrorCode.StakeAmountZero]: "Stake amount must be greater than zero",
  [PredictXErrorCode.AlreadyStaked]: "You have already staked on this poll",
  [PredictXErrorCode.NotStaker]: "You have not staked on this poll",
  [PredictXErrorCode.AlreadyClaimed]: "Winnings have already been claimed",
  [PredictXErrorCode.NotOnWinningSide]: "Your stake was on the losing side",
  [PredictXErrorCode.VotingNotOpen]: "Voting is not currently open for this poll",
  [PredictXErrorCode.AlreadyVoted]: "You have already voted on this poll",
  [PredictXErrorCode.VoterIsStaker]: "Stakers cannot vote on their own poll",
  [PredictXErrorCode.VotingWindowExpired]: "The voting window has expired",
  [PredictXErrorCode.DisputeAlreadyOpen]: "A dispute is already open for this poll",
  [PredictXErrorCode.DisputeFeeRequired]: "A dispute fee is required",
  [PredictXErrorCode.InvalidPollCategory]: "Invalid poll category",
  [PredictXErrorCode.InvalidLockTime]: "Lock time must be in the future",
  [PredictXErrorCode.MatchNotFound]: "Match not found",
  [PredictXErrorCode.MatchAlreadyStarted]: "Match has already started",
  [PredictXErrorCode.PollQuestionTooLong]: `Poll question exceeds maximum length`,
  [PredictXErrorCode.MaxPollsPerMatchReached]: "Maximum polls per match reached",
  [PredictXErrorCode.InvalidOutcome]: "Invalid poll outcome",
  [PredictXErrorCode.ConsensusNotReached]: "Voting consensus was not reached",
  [PredictXErrorCode.AdminAlreadyRegistered]: "Admin is already registered",
  [PredictXErrorCode.InsufficientAdminApprovals]: "Insufficient admin approvals",
  [PredictXErrorCode.EmergencyWithdrawNotAllowed]: "Emergency withdrawal is not allowed yet",
  [PredictXErrorCode.TransferFailed]: "Token transfer failed",
  [PredictXErrorCode.ContractPaused]: "Contract is currently paused",
  [PredictXErrorCode.StakeBelowMinimum]: "Stake amount is below the minimum required",
};

/** Typed error thrown by PredictX SDK operations. */
export class PredictXError extends Error {
  constructor(
    public readonly code: PredictXErrorCode,
    public readonly context?: string,
  ) {
    const base = ERROR_MESSAGES[code] ?? `Contract error code ${code}`;
    super(context ? `${base}: ${context}` : base);
    this.name = "PredictXError";
  }
}

/**
 * Attempts to parse a raw Soroban contract error and returns a typed
 * `PredictXError`.  Falls back to a generic `Error` for unknown codes.
 *
 * @param raw - The raw error thrown by the Stellar SDK.
 */
export function parseSorobanError(raw: unknown): Error {
  if (raw instanceof PredictXError) return raw;

  const message = raw instanceof Error ? raw.message : String(raw);

  // Soroban error codes surface as "Error(Contract, #N)" in the message
  const match = message.match(/Error\(Contract,\s*#(\d+)\)/);
  if (match) {
    const code = parseInt(match[1], 10) as PredictXErrorCode;
    if (code in PredictXErrorCode) {
      return new PredictXError(code);
    }
  }

  return raw instanceof Error ? raw : new Error(message);
}
