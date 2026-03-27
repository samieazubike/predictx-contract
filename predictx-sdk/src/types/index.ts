/**
 * PredictX SDK — Shared Type Definitions
 *
 * All types mirror the on-chain contract structs exactly:
 *   - `i128` fields → `bigint`
 *   - `u32` / `u64` numeric fields → `number`
 *   - `Address` → `string` (Stellar account/contract ID)
 *   - `u64` Unix timestamps → `Date`
 *   - `Option<T>` → `T | undefined`
 */

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/** The lifecycle status of a prediction poll. */
export enum PollStatus {
  /** Accepting stakes. */
  Active = 0,
  /** Past lock time — no more stakes. */
  Locked = 1,
  /** Community voting in progress. */
  Voting = 2,
  /** Vote consensus 60–85%, needs admin review. */
  AdminReview = 3,
  /** Under dispute review. */
  Disputed = 4,
  /** Outcome determined — claims open. */
  Resolved = 5,
  /** Emergency cancelled — refunds available. */
  Cancelled = 6,
}

/** Poll question category. */
export enum PollCategory {
  PlayerEvent = 0,
  TeamEvent = 1,
  ScorePrediction = 2,
  Other = 3,
}

/** Which side of a poll a user staked on. */
export enum StakeSide {
  Yes = 0,
  No = 1,
}

/** Oracle / community vote choice. */
export enum VoteChoice {
  Yes = 0,
  No = 1,
  /** For genuinely ambiguous outcomes. */
  Unclear = 2,
}

// ---------------------------------------------------------------------------
// Core structs
// ---------------------------------------------------------------------------

/** A football match that polls can be created against. */
export interface Match {
  matchId: number;
  homeTeam: string;
  awayTeam: string;
  league: string;
  venue: string;
  kickoffTime: Date;
  createdBy: string;
  isFinished: boolean;
}

/** A prediction poll. */
export interface Poll {
  pollId: number;
  matchId: number;
  creator: string;
  question: string;
  category: PollCategory;
  lockTime: Date;
  /** Total tokens staked on the YES side (7-decimal precision). */
  yesPool: bigint;
  /** Total tokens staked on the NO side (7-decimal precision). */
  noPool: bigint;
  /** Number of individual YES stakers. */
  yesCount: number;
  /** Number of individual NO stakers. */
  noCount: number;
  status: PollStatus;
  /** Outcome once resolved; undefined while poll is pending. */
  outcome?: boolean;
  resolutionTime?: Date;
  createdAt: Date;
}

/** A user's stake on a poll. */
export interface Stake {
  user: string;
  pollId: number;
  amount: bigint;
  side: StakeSide;
  claimed: boolean;
  stakedAt: Date;
  /** Calculated client-side — not stored on-chain. */
  potentialWinnings?: bigint;
  /** Return-on-investment percentage (client-side). */
  roi?: number;
}

/** Snapshot of pool balances for a poll. */
export interface PoolInfo {
  yesPool: bigint;
  noPool: bigint;
  yesCount: number;
  noCount: number;
}

/** Community vote tally for a poll. */
export interface VoteTally {
  pollId: number;
  yesVotes: number;
  noVotes: number;
  unclearVotes: number;
  totalVoters: number;
  votingEndTime: Date;
  /** 0.5–1% of pool reserved for voter incentives. */
  rewardPool: bigint;
}

/** A dispute raised against a poll outcome. */
export interface Dispute {
  pollId: number;
  initiator: string;
  evidenceHash: string;
  disputeFee: bigint;
  adminApprovals: number;
  requiredApprovals: number;
  resolved: boolean;
  initiatedAt: Date;
}

/** Aggregate platform statistics. */
export interface PlatformStats {
  totalValueLocked: bigint;
  totalPollsCreated: number;
  totalStakesPlaced: number;
  totalPayouts: bigint;
  totalUsers: number;
}

/** Per-user statistics. */
export interface UserStats {
  totalStaked: bigint;
  totalWon: bigint;
  totalLost: bigint;
  pollsParticipated: number;
  pollsWon: number;
  pollsLost: number;
  votesCast: number;
  votingRewardsEarned: bigint;
}

/** A single entry in a user's prediction history. */
export interface PredictionHistoryEntry {
  pollId: number;
  question: string;
  side: StakeSide;
  amount: bigint;
  outcome?: boolean;
  winnings?: bigint;
  status: PollStatus;
  stakedAt: Date;
}

// ---------------------------------------------------------------------------
// SDK configuration & connection
// ---------------------------------------------------------------------------

/** Supported Stellar networks. */
export type NetworkName = "mainnet" | "testnet" | "futurenet" | "standalone";

/** SDK initialisation config. */
export interface PredictXConfig {
  network: NetworkName;
  /** RPC URL — defaults to Horizon/Soroban public endpoints. */
  rpcUrl?: string;
  /** Contract IDs — required for all operations. */
  contractIds: {
    predictionMarket: string;
    votingOracle: string;
    treasury: string;
    pollFactory: string;
  };
  /** XLM token contract address used by the protocol. */
  tokenAddress?: string;
}

/** A minimal wallet interface that the SDK accepts. */
export interface StellarWallet {
  /** Public key of the connected account. */
  publicKey: string;
  /** Sign a transaction XDR and return the signed XDR. */
  signTransaction(xdr: string, network?: string): Promise<string>;
}

// ---------------------------------------------------------------------------
// SDK operation results
// ---------------------------------------------------------------------------

/** Wraps the outcome of any state-mutating on-chain call. */
export interface TransactionResult<T> {
  /** Contract return value (decoded). */
  value: T;
  /** Transaction hash. */
  hash: string;
  /** Ledger sequence number. */
  ledger: number;
}

/** Parameters for creating a new match. */
export interface CreateMatchParams {
  homeTeam: string;
  awayTeam: string;
  league: string;
  venue: string;
  kickoffTime: Date;
}

/** Parameters for creating a new poll. */
export interface CreatePollParams {
  matchId: number;
  question: string;
  category: PollCategory;
  lockTime: Date;
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/** Callback type for real-time event subscriptions. */
export type EventCallback = (event: PredictXEvent) => void;

/** Unsubscribe function returned by `subscribeToEvents`. */
export type Unsubscribe = () => void;

/** Discriminated union of all PredictX contract events. */
export type PredictXEvent =
  | PollCreatedEvent
  | PollCancelledEvent
  | StakePlacedEvent
  | EmergencyWithdrawalEvent
  | MatchCreatedEvent
  | ContractPausedEvent
  | ContractUnpausedEvent;

export interface PollCreatedEvent {
  type: "poll:created";
  pollId: number;
  matchId: number;
  creator: string;
  question: string;
  category: PollCategory;
  lockTime: Date;
}

export interface PollCancelledEvent {
  type: "poll:cancelled";
  pollId: number;
  admin: string;
}

export interface StakePlacedEvent {
  type: "stake:placed";
  pollId: number;
  staker: string;
  amount: bigint;
  side: StakeSide;
}

export interface EmergencyWithdrawalEvent {
  type: "stake:emergency_withdrawal";
  pollId: number;
  user: string;
  amount: bigint;
}

export interface MatchCreatedEvent {
  type: "match:created";
  matchId: number;
  homeTeam: string;
  awayTeam: string;
}

export interface ContractPausedEvent {
  type: "contract:paused";
  admin: string;
}

export interface ContractUnpausedEvent {
  type: "contract:unpaused";
  admin: string;
}
