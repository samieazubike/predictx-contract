/**
 * @predictx/sdk — TypeScript SDK for the PredictX Soroban prediction market.
 *
 * ## Quick Start
 *
 * ```ts
 * import {
 *   PredictXClient,
 *   PollStatus,
 *   StakeSide,
 *   VoteChoice,
 *   formatTokenAmount,
 *   calculatePotentialWinnings,
 * } from "@predictx/sdk";
 *
 * const client = new PredictXClient({
 *   network: "testnet",
 *   contractIds: {
 *     predictionMarket: "CXXX...",
 *     votingOracle:     "CYYY...",
 *     treasury:         "CZZZ...",
 *     pollFactory:      "CWWW...",
 *   },
 * });
 *
 * // Connect Freighter (or any StellarWallet implementation)
 * await client.connect(freighterWallet);
 *
 * // Browse polls
 * const polls = await client.getTrendingPolls(10);
 *
 * // Stake
 * const result = await client.stake(pollId, 100_000_000n, StakeSide.Yes);
 * ```
 */

// Main client
export { PredictXClient } from "./client";

// Contract-level clients (for advanced use)
export { PredictionMarketClient } from "./contracts/prediction-market";
export { VotingOracleClient } from "./contracts/voting-oracle";
export { TreasuryClient } from "./contracts/treasury";
export { PollFactoryClient } from "./contracts/poll-factory";

// Types
export type {
  Match,
  Poll,
  Stake,
  PoolInfo,
  VoteTally,
  Dispute,
  PlatformStats,
  UserStats,
  PredictionHistoryEntry,
  PredictXConfig,
  StellarWallet,
  TransactionResult,
  CreateMatchParams,
  CreatePollParams,
  EventCallback,
  Unsubscribe,
  PredictXEvent,
  PollCreatedEvent,
  PollCancelledEvent,
  StakePlacedEvent,
  EmergencyWithdrawalEvent,
  MatchCreatedEvent,
  ContractPausedEvent,
  ContractUnpausedEvent,
  NetworkName,
} from "./types/index";

export { PollStatus, PollCategory, StakeSide, VoteChoice } from "./types/index";

// Errors
export { PredictXError, PredictXErrorCode, parseSorobanError } from "./errors";

// Event utilities
export { parseEvent } from "./events/parser";

// Calculation utilities
export {
  calculatePotentialWinnings,
  calculateClaimableWinnings,
  calculatePlatformFee,
  calculateVoterRewardPool,
  formatTokenAmount,
  parseTokenAmount,
} from "./utils/calculations";

// Formatting utilities
export {
  truncateAddress,
  formatDate,
  formatCountdown,
  formatPercent,
} from "./utils/formatting";

// Constants
export {
  PLATFORM_FEE_BPS,
  VOTER_REWARD_BPS,
  BPS_DENOMINATOR,
  VOTING_WINDOW_SECS,
  DISPUTE_WINDOW_SECS,
  AUTO_RESOLVE_THRESHOLD_BPS,
  ADMIN_REVIEW_THRESHOLD_BPS,
  MAX_QUESTION_LENGTH,
  MAX_POLLS_PER_MATCH,
  EMERGENCY_TIMEOUT_SECS,
  MIN_STAKE_AMOUNT,
} from "./utils/constants";
