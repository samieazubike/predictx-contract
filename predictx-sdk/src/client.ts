/**
 * PredictX SDK — Top-level `PredictXClient`.
 *
 * Aggregates all contract clients into a single entrypoint and provides
 * the higher-level methods used by the frontend (poll cards, staking modal,
 * dashboard, voting centre, real-time events).
 */

import { SorobanRpc } from "@stellar/stellar-sdk";

import type {
  Match,
  Poll,
  Stake,
  StakeSide,
  VoteChoice,
  VoteTally,
  PlatformStats,
  UserStats,
  PredictionHistoryEntry,
  StellarWallet,
  TransactionResult,
  CreateMatchParams,
  CreatePollParams,
  PredictXConfig,
  EventCallback,
  Unsubscribe,
  NetworkName,
} from "./types/index";
import { PollStatus } from "./types/index";

import { PredictionMarketClient } from "./contracts/prediction-market";
import { VotingOracleClient } from "./contracts/voting-oracle";
import { TreasuryClient } from "./contracts/treasury";
import { PollFactoryClient } from "./contracts/poll-factory";
import { parseEvent } from "./events/parser";
import { calculatePotentialWinnings } from "./utils/calculations";

// Default RPC endpoints per network
const DEFAULT_RPC_URLS: Record<NetworkName, string> = {
  mainnet: "https://horizon.stellar.org",
  testnet: "https://soroban-testnet.stellar.org",
  futurenet: "https://rpc-futurenet.stellar.org",
  standalone: "http://localhost:8000/soroban/rpc",
};

/**
 * The unified PredictX SDK client.
 *
 * @example
 * ```ts
 * import { PredictXClient } from "@predictx/sdk";
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
 * // Connect a wallet (e.g. Freighter)
 * await client.connect(myWallet);
 *
 * // Read data
 * const polls = await client.getPollsByMatch(1);
 *
 * // Submit a stake
 * const result = await client.stake(pollId, 100_000_000n, StakeSide.Yes);
 * ```
 */
export class PredictXClient {
  private readonly predictionMarket: PredictionMarketClient;
  private readonly votingOracle: VotingOracleClient;
  private readonly treasury: TreasuryClient;
  private readonly pollFactory: PollFactoryClient;
  private readonly rpcUrl: string;
  private readonly network: NetworkName;
  private wallet: StellarWallet | null = null;

  constructor(config: PredictXConfig) {
    const rpcUrl =
      config.rpcUrl ?? DEFAULT_RPC_URLS[config.network];

    this.rpcUrl = rpcUrl;
    this.network = config.network;

    const baseConfig = { network: config.network, rpcUrl };

    this.predictionMarket = new PredictionMarketClient({
      ...baseConfig,
      contractId: config.contractIds.predictionMarket,
    });
    this.votingOracle = new VotingOracleClient({
      ...baseConfig,
      contractId: config.contractIds.votingOracle,
    });
    this.treasury = new TreasuryClient({
      ...baseConfig,
      contractId: config.contractIds.treasury,
    });
    this.pollFactory = new PollFactoryClient({
      ...baseConfig,
      contractId: config.contractIds.pollFactory,
    });
  }

  // -------------------------------------------------------------------------
  // Connection
  // -------------------------------------------------------------------------

  /**
   * Connects a Stellar wallet (e.g. Freighter) to the client.
   * Must be called before any state-mutating operation.
   *
   * @param wallet - A StellarWallet implementation.
   */
  async connect(wallet: StellarWallet): Promise<void> {
    this.wallet = wallet;
  }

  /** Returns the currently connected wallet's public key, or null. */
  get connectedAddress(): string | null {
    return this.wallet?.publicKey ?? null;
  }

  private requireWallet(): StellarWallet {
    if (!this.wallet) {
      throw new Error(
        "No wallet connected. Call connect(wallet) before submitting transactions.",
      );
    }
    return this.wallet;
  }

  // -------------------------------------------------------------------------
  // Matches
  // -------------------------------------------------------------------------

  /**
   * Fetches a single match.
   *
   * @param matchId - The match ID.
   */
  async getMatch(matchId: number): Promise<Match> {
    return this.predictionMarket.getMatch(matchId);
  }

  /**
   * Returns all matches up to the current match count.
   * Useful for building a match list / lobby.
   */
  async getUpcomingMatches(): Promise<Match[]> {
    const count = await this.predictionMarket.getMatchCount();
    const ids = Array.from({ length: count }, (_, i) => i + 1);
    const results = await Promise.allSettled(
      ids.map((id) => this.predictionMarket.getMatch(id)),
    );
    return results
      .filter((r): r is PromiseFulfilledResult<Match> => r.status === "fulfilled")
      .map((r) => r.value)
      .filter((m) => !m.isFinished);
  }

  /**
   * Creates a new match (admin only).
   */
  async createMatch(
    params: CreateMatchParams,
  ): Promise<TransactionResult<number>> {
    return this.predictionMarket.createMatch(params, {
      wallet: this.requireWallet(),
    });
  }

  // -------------------------------------------------------------------------
  // Polls
  // -------------------------------------------------------------------------

  /**
   * Fetches a single poll.
   *
   * @param pollId - The poll ID.
   */
  async getPoll(pollId: number): Promise<Poll> {
    return this.predictionMarket.getPoll(pollId);
  }

  /**
   * Returns all polls linked to a given match.
   *
   * @param matchId - The match ID.
   */
  async getPollsByMatch(matchId: number): Promise<Poll[]> {
    const pollIds = await this.predictionMarket.getMatchPolls(matchId);
    const results = await Promise.allSettled(
      pollIds.map((id) => this.predictionMarket.getPoll(id)),
    );
    return results
      .filter((r): r is PromiseFulfilledResult<Poll> => r.status === "fulfilled")
      .map((r) => r.value);
  }

  /**
   * Returns the most active polls sorted by total pool size.
   *
   * @param limit - Maximum number of polls to return (default 10).
   */
  async getTrendingPolls(limit = 10): Promise<Poll[]> {
    const stats = await this.predictionMarket.getPlatformStats();
    const totalPolls = stats.totalPollsCreated;
    const ids = Array.from({ length: Number(totalPolls) }, (_, i) => i + 1);

    const results = await Promise.allSettled(
      ids.map((id) => this.predictionMarket.getPoll(id)),
    );
    return results
      .filter((r): r is PromiseFulfilledResult<Poll> => r.status === "fulfilled")
      .map((r) => r.value)
      .filter((p) => p.status === PollStatus.Active)
      .sort((a, b) =>
        Number(b.yesPool + b.noPool - a.yesPool - a.noPool),
      )
      .slice(0, limit);
  }

  /**
   * Creates a new prediction poll.
   *
   * @param params  - Poll parameters.
   */
  async createPoll(
    params: CreatePollParams,
  ): Promise<TransactionResult<number>> {
    return this.predictionMarket.createPoll(params, {
      wallet: this.requireWallet(),
    });
  }

  // -------------------------------------------------------------------------
  // Staking
  // -------------------------------------------------------------------------

  /**
   * Places a stake on a poll.
   *
   * @param pollId - Target poll.
   * @param amount - Amount in raw base units (bigint).
   * @param side   - StakeSide.Yes or StakeSide.No.
   */
  async stake(
    pollId: number,
    amount: bigint,
    side: StakeSide,
  ): Promise<TransactionResult<Stake>> {
    return this.predictionMarket.stake(pollId, amount, side, {
      wallet: this.requireWallet(),
    });
  }

  /**
   * Returns the stake a user has placed on a poll, or null if not staked.
   *
   * @param pollId - The poll ID.
   * @param user   - Stellar address (defaults to connected wallet).
   */
  async getStake(pollId: number, user?: string): Promise<Stake | null> {
    const address = user ?? this.connectedAddress;
    if (!address) return null;
    const hasStaked = await this.predictionMarket.hasUserStaked(
      pollId,
      address,
    );
    if (!hasStaked) return null;
    return this.predictionMarket.getStakeInfo(pollId, address);
  }

  /**
   * Returns the poll IDs on which a user has staked.
   *
   * @param user - Stellar address (defaults to connected wallet).
   */
  async getUserStakes(user?: string): Promise<number[]> {
    const address = user ?? this.connectedAddress;
    if (!address) return [];
    return this.predictionMarket.getUserStakes(address);
  }

  /**
   * Client-side preview of potential winnings for a proposed stake.
   *
   * @param pollId - The poll to stake on.
   * @param side   - StakeSide.Yes or StakeSide.No.
   * @param amount - Hypothetical stake amount (base units).
   */
  async calculatePotentialWinnings(
    pollId: number,
    side: StakeSide,
    amount: bigint,
  ): Promise<bigint> {
    return this.predictionMarket.calculatePotentialWinnings(
      pollId,
      side,
      amount,
    );
  }

  // -------------------------------------------------------------------------
  // Claims
  // -------------------------------------------------------------------------

  /**
   * Claims winnings for a resolved poll.
   *
   * @param pollId - The resolved poll.
   */
  async claimWinnings(pollId: number): Promise<TransactionResult<bigint>> {
    return this.predictionMarket.claimWinnings(pollId, {
      wallet: this.requireWallet(),
    });
  }

  /**
   * Calculates claimable winnings (post-resolution view).
   *
   * @param pollId - The resolved poll.
   * @param user   - Stellar address (defaults to connected wallet).
   */
  async calculateWinnings(
    pollId: number,
    user?: string,
  ): Promise<bigint> {
    const address = user ?? this.connectedAddress;
    if (!address) return 0n;

    const stake = await this.getStake(pollId, address);
    if (!stake || stake.claimed) return 0n;

    return this.predictionMarket.calculatePotentialWinnings(
      pollId,
      stake.side,
      stake.amount,
    );
  }

  // -------------------------------------------------------------------------
  // Voting
  // -------------------------------------------------------------------------

  /**
   * Casts a vote on a locked poll.
   *
   * @param pollId - The poll to vote on.
   * @param choice - VoteChoice.Yes, .No, or .Unclear.
   */
  async castVote(
    pollId: number,
    choice: VoteChoice,
  ): Promise<TransactionResult<VoteTally>> {
    return this.votingOracle.castVote(pollId, choice, {
      wallet: this.requireWallet(),
    });
  }

  /**
   * Returns the current vote tally for a poll.
   *
   * @param pollId - The poll ID.
   */
  async getVoteTally(pollId: number): Promise<VoteTally> {
    return this.votingOracle.getVoteTally(pollId);
  }

  /**
   * Returns polls that are in the Voting phase (eligible for the connected
   * wallet to vote on, provided they are not a staker).
   *
   * @param user - Stellar address (defaults to connected wallet).
   */
  async getVotingOpportunities(user?: string): Promise<Poll[]> {
    const address = user ?? this.connectedAddress;
    const stats = await this.predictionMarket.getPlatformStats();
    const totalPolls = Number(stats.totalPollsCreated);
    const ids = Array.from({ length: totalPolls }, (_, i) => i + 1);

    const results = await Promise.allSettled(
      ids.map((id) => this.predictionMarket.getPoll(id)),
    );
    const votingPolls = results
      .filter((r): r is PromiseFulfilledResult<Poll> => r.status === "fulfilled")
      .map((r) => r.value)
      .filter((p) => p.status === PollStatus.Voting);

    if (!address) return votingPolls;

    // Filter out polls where the user is already a staker
    const stakeChecks = await Promise.allSettled(
      votingPolls.map((p) =>
        this.predictionMarket.hasUserStaked(p.pollId, address),
      ),
    );
    return votingPolls.filter(
      (_, i) =>
        stakeChecks[i].status === "fulfilled" &&
        !(stakeChecks[i] as PromiseFulfilledResult<boolean>).value,
    );
  }

  /**
   * Claims the voting reward for a correctly-voted resolved poll.
   *
   * @param pollId - The resolved poll.
   */
  async claimVotingReward(
    pollId: number,
  ): Promise<TransactionResult<bigint>> {
    return this.votingOracle.claimVotingReward(pollId, {
      wallet: this.requireWallet(),
    });
  }

  // -------------------------------------------------------------------------
  // Stats
  // -------------------------------------------------------------------------

  /**
   * Returns aggregate platform statistics.
   */
  async getPlatformStats(): Promise<PlatformStats> {
    return this.predictionMarket.getPlatformStats();
  }

  /**
   * Computes per-user statistics by scanning the user's stake history.
   *
   * @param user - Stellar address (defaults to connected wallet).
   */
  async getUserStats(user?: string): Promise<UserStats> {
    const address = user ?? this.connectedAddress;
    if (!address) {
      return {
        totalStaked: 0n,
        totalWon: 0n,
        totalLost: 0n,
        pollsParticipated: 0,
        pollsWon: 0,
        pollsLost: 0,
        votesCast: 0,
        votingRewardsEarned: 0n,
      };
    }

    const pollIds = await this.predictionMarket.getUserStakes(address);
    const entries = await this.getUserHistory(address);

    let totalStaked = 0n;
    let totalWon = 0n;
    let totalLost = 0n;
    let pollsWon = 0;
    let pollsLost = 0;

    for (const entry of entries) {
      totalStaked += entry.amount;
      if (entry.status === PollStatus.Resolved) {
        if (entry.winnings && entry.winnings > 0n) {
          totalWon += entry.winnings;
          pollsWon++;
        } else if (entry.outcome !== undefined) {
          totalLost += entry.amount;
          pollsLost++;
        }
      }
    }

    return {
      totalStaked,
      totalWon,
      totalLost,
      pollsParticipated: pollIds.length,
      pollsWon,
      pollsLost,
      votesCast: 0, // Voting ledger not yet exposed in Phase 1
      votingRewardsEarned: 0n,
    };
  }

  /**
   * Returns the user's full prediction history.
   *
   * @param user - Stellar address (defaults to connected wallet).
   */
  async getUserHistory(user?: string): Promise<PredictionHistoryEntry[]> {
    const address = user ?? this.connectedAddress;
    if (!address) return [];

    const pollIds = await this.predictionMarket.getUserStakes(address);
    const results = await Promise.allSettled(
      pollIds.map(async (pollId) => {
        const [poll, stake] = await Promise.all([
          this.predictionMarket.getPoll(pollId),
          this.predictionMarket.getStakeInfo(pollId, address),
        ]);

        let winnings: bigint | undefined;
        if (
          poll.status === PollStatus.Resolved &&
          poll.outcome !== undefined
        ) {
          const onWinningSide =
            (poll.outcome && stake.side === 0) ||
            (!poll.outcome && stake.side === 1);
          if (onWinningSide && !stake.claimed) {
            winnings = await this.predictionMarket.calculatePotentialWinnings(
              pollId,
              stake.side,
              stake.amount,
            );
          }
        }

        return {
          pollId: poll.pollId,
          question: poll.question,
          side: stake.side,
          amount: stake.amount,
          outcome: poll.outcome,
          winnings,
          status: poll.status,
          stakedAt: stake.stakedAt,
        } satisfies PredictionHistoryEntry;
      }),
    );

    return results
      .filter(
        (r): r is PromiseFulfilledResult<PredictionHistoryEntry> =>
          r.status === "fulfilled",
      )
      .map((r) => r.value);
  }

  // -------------------------------------------------------------------------
  // Real-time events
  // -------------------------------------------------------------------------

  /**
   * Subscribes to PredictX contract events via Soroban RPC streaming.
   * Returns an unsubscribe function — call it to stop receiving events.
   *
   * @param callback - Called for each parsed PredictXEvent.
   *
   * @example
   * ```ts
   * const unsubscribe = client.subscribeToEvents((event) => {
   *   if (event.type === "stake:placed") {
   *     console.log("New stake:", event.staker, event.amount);
   *   }
   * });
   * // Later:
   * unsubscribe();
   * ```
   */
  subscribeToEvents(callback: EventCallback): Unsubscribe {
    const server = new SorobanRpc.Server(this.rpcUrl, { allowHttp: true });
    let active = true;
    let latestLedger = 0;

    const poll = async () => {
      if (!active) return;
      try {
        // Fetch events from the latest known ledger onwards
        const response = await (server as any).getEvents({
          startLedger: latestLedger > 0 ? latestLedger : undefined,
          filters: [{ type: "contract" }],
          limit: 100,
        });
        for (const raw of response?.events ?? []) {
          const event = parseEvent(raw);
          if (event) callback(event);
        }
        if (response?.latestLedger) {
          latestLedger = response.latestLedger + 1;
        }
      } catch {
        // Non-fatal: network hiccup, retry next poll
      }
      if (active) setTimeout(poll, 5_000);
    };

    poll();
    return () => {
      active = false;
    };
  }
}
