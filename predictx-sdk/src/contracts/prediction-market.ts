/**
 * PredictX SDK — PredictionMarket contract client.
 *
 * Wraps all `prediction-market` contract invocations in typed,
 * ergonomic methods.  Read operations use `simulateTransaction`;
 * write operations build, sign, and submit a full transaction.
 */

import {
  Contract,
  SorobanRpc,
  TransactionBuilder,
  Networks,
  BASE_FEE,
  xdr,
  scValToNative,
  nativeToScVal,
  Address,
  type Keypair,
} from "@stellar/stellar-sdk";

import type {
  Match,
  Poll,
  Stake,
  PoolInfo,
  PlatformStats,
  PollCategory,
  StakeSide,
  StellarWallet,
  TransactionResult,
  CreateMatchParams,
  CreatePollParams,
  NetworkName,
} from "../types/index";
import { parseSorobanError } from "../errors";
import { calculatePotentialWinnings } from "../utils/calculations";

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

const NETWORK_PASSPHRASE: Record<NetworkName, string> = {
  mainnet: Networks.PUBLIC,
  testnet: Networks.TESTNET,
  futurenet: Networks.FUTURENET,
  standalone: Networks.STANDALONE,
};

function toDate(unixSecs: unknown): Date {
  return new Date(Number(unixSecs) * 1000);
}

function rawToMatch(raw: Record<string, unknown>): Match {
  return {
    matchId: Number(raw["match_id"]),
    homeTeam: String(raw["home_team"]),
    awayTeam: String(raw["away_team"]),
    league: String(raw["league"]),
    venue: String(raw["venue"]),
    kickoffTime: toDate(raw["kickoff_time"]),
    createdBy: String(raw["created_by"]),
    isFinished: Boolean(raw["is_finished"]),
  };
}

function rawToPoll(raw: Record<string, unknown>): Poll {
  return {
    pollId: Number(raw["poll_id"]),
    matchId: Number(raw["match_id"]),
    creator: String(raw["creator"]),
    question: String(raw["question"]),
    category: Number(raw["category"]) as PollCategory,
    lockTime: toDate(raw["lock_time"]),
    yesPool: BigInt(String(raw["yes_pool"])),
    noPool: BigInt(String(raw["no_pool"])),
    yesCount: Number(raw["yes_count"]),
    noCount: Number(raw["no_count"]),
    status: Number(raw["status"]),
    outcome:
      raw["outcome"] != null ? Boolean(raw["outcome"]) : undefined,
    resolutionTime:
      raw["resolution_time"] != null
        ? toDate(raw["resolution_time"])
        : undefined,
    createdAt: toDate(raw["created_at"]),
  };
}

function rawToStake(raw: Record<string, unknown>): Stake {
  return {
    user: String(raw["user"]),
    pollId: Number(raw["poll_id"]),
    amount: BigInt(String(raw["amount"])),
    side: Number(raw["side"]) as StakeSide,
    claimed: Boolean(raw["claimed"]),
    stakedAt: toDate(raw["staked_at"]),
  };
}

function rawToPoolInfo(raw: Record<string, unknown>): PoolInfo {
  return {
    yesPool: BigInt(String(raw["yes_pool"])),
    noPool: BigInt(String(raw["no_pool"])),
    yesCount: Number(raw["yes_count"]),
    noCount: Number(raw["no_count"]),
  };
}

function rawToPlatformStats(raw: Record<string, unknown>): PlatformStats {
  return {
    totalValueLocked: BigInt(String(raw["total_value_locked"])),
    totalPollsCreated: Number(raw["total_polls_created"]),
    totalStakesPlaced: Number(raw["total_stakes_placed"]),
    totalPayouts: BigInt(String(raw["total_payouts"])),
    totalUsers: Number(raw["total_users"]),
  };
}

// ---------------------------------------------------------------------------
// PredictionMarketClient
// ---------------------------------------------------------------------------

/** Options passed to state-mutating calls. */
interface InvokeOptions {
  wallet: StellarWallet;
  /** Max fee in stroops (default 100 stroops = BASE_FEE). */
  fee?: string;
}

/**
 * Client for the `prediction-market` contract.
 *
 * @example
 * ```ts
 * const client = new PredictionMarketClient({
 *   contractId: "CXXX...",
 *   network: "testnet",
 *   rpcUrl: "https://soroban-testnet.stellar.org",
 * });
 * const poll = await client.getPoll(1);
 * ```
 */
export class PredictionMarketClient {
  private readonly contract: Contract;
  private readonly server: SorobanRpc.Server;
  private readonly networkPassphrase: string;

  constructor(config: {
    contractId: string;
    network: NetworkName;
    rpcUrl: string;
  }) {
    this.contract = new Contract(config.contractId);
    this.server = new SorobanRpc.Server(config.rpcUrl, { allowHttp: true });
    this.networkPassphrase = NETWORK_PASSPHRASE[config.network];
  }

  // -------------------------------------------------------------------------
  // Private: simulation & submission helpers
  // -------------------------------------------------------------------------

  private async simulate<T>(
    method: string,
    ...args: xdr.ScVal[]
  ): Promise<T> {
    try {
      const account = await this.server.getAccount(
        "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN", // dummy for simulation
      );
      const tx = new TransactionBuilder(account, {
        fee: BASE_FEE,
        networkPassphrase: this.networkPassphrase,
      })
        .addOperation(this.contract.call(method, ...args))
        .setTimeout(30)
        .build();

      const simResult = await this.server.simulateTransaction(tx);
      if (SorobanRpc.Api.isSimulationError(simResult)) {
        throw parseSorobanError(new Error(simResult.error));
      }
      if (!SorobanRpc.Api.isSimulationSuccess(simResult)) {
        throw new Error("Simulation failed");
      }
      const scVal = simResult.result?.retval;
      return scValToNative(scVal!) as T;
    } catch (err) {
      throw parseSorobanError(err);
    }
  }

  private async invoke<T>(
    method: string,
    options: InvokeOptions,
    ...args: xdr.ScVal[]
  ): Promise<TransactionResult<T>> {
    try {
      const account = await this.server.getAccount(options.wallet.publicKey);
      const tx = new TransactionBuilder(account, {
        fee: options.fee ?? BASE_FEE,
        networkPassphrase: this.networkPassphrase,
      })
        .addOperation(this.contract.call(method, ...args))
        .setTimeout(30)
        .build();

      const simResult = await this.server.simulateTransaction(tx);
      if (SorobanRpc.Api.isSimulationError(simResult)) {
        throw parseSorobanError(new Error(simResult.error));
      }

      const prepared = SorobanRpc.assembleTransaction(tx, simResult).build();
      const signedXdr = await options.wallet.signTransaction(
        prepared.toXDR(),
        this.networkPassphrase,
      );

      const submitted = await this.server.sendTransaction(
        TransactionBuilder.fromXDR(signedXdr, this.networkPassphrase),
      );

      if (submitted.status === "ERROR") {
        throw new Error(`Transaction failed: ${submitted.errorResult}`);
      }

      // Poll for confirmation
      let getResponse = await this.server.getTransaction(submitted.hash);
      while (getResponse.status === SorobanRpc.Api.GetTransactionStatus.NOT_FOUND) {
        await new Promise((r) => setTimeout(r, 1000));
        getResponse = await this.server.getTransaction(submitted.hash);
      }

      if (getResponse.status !== SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
        throw new Error(`Transaction failed with status: ${getResponse.status}`);
      }

      const returnValue = getResponse.returnValue
        ? (scValToNative(getResponse.returnValue) as T)
        : (undefined as T);

      return {
        value: returnValue,
        hash: submitted.hash,
        ledger: getResponse.ledger,
      };
    } catch (err) {
      throw parseSorobanError(err);
    }
  }

  // -------------------------------------------------------------------------
  // Matches
  // -------------------------------------------------------------------------

  /**
   * Fetches a single match by ID.
   *
   * @param matchId - The match ID.
   */
  async getMatch(matchId: number): Promise<Match> {
    const raw = await this.simulate<Record<string, unknown>>(
      "get_match",
      nativeToScVal(matchId, { type: "u64" }),
    );
    return rawToMatch(raw);
  }

  /**
   * Returns the total number of matches created.
   */
  async getMatchCount(): Promise<number> {
    return this.simulate<number>("get_match_count");
  }

  /**
   * Creates a new match (admin only).
   */
  async createMatch(
    params: CreateMatchParams,
    options: InvokeOptions,
  ): Promise<TransactionResult<number>> {
    const result = await this.invoke<bigint>(
      "create_match",
      options,
      nativeToScVal(options.wallet.publicKey, { type: "address" }),
      nativeToScVal(params.homeTeam, { type: "string" }),
      nativeToScVal(params.awayTeam, { type: "string" }),
      nativeToScVal(params.league, { type: "string" }),
      nativeToScVal(params.venue, { type: "string" }),
      nativeToScVal(Math.floor(params.kickoffTime.getTime() / 1000), {
        type: "u64",
      }),
    );
    return { ...result, value: Number(result.value) };
  }

  /**
   * Marks a match as finished, enabling the voting phase.
   */
  async finishMatch(
    matchId: number,
    options: InvokeOptions,
  ): Promise<TransactionResult<void>> {
    return this.invoke<void>(
      "finish_match",
      options,
      nativeToScVal(options.wallet.publicKey, { type: "address" }),
      nativeToScVal(matchId, { type: "u64" }),
    );
  }

  // -------------------------------------------------------------------------
  // Polls
  // -------------------------------------------------------------------------

  /**
   * Fetches a single poll by ID.
   *
   * @param pollId - The poll ID.
   */
  async getPoll(pollId: number): Promise<Poll> {
    const raw = await this.simulate<Record<string, unknown>>(
      "get_poll",
      nativeToScVal(pollId, { type: "u64" }),
    );
    return rawToPoll(raw);
  }

  /**
   * Returns the IDs of all polls linked to a match.
   *
   * @param matchId - The match ID.
   */
  async getMatchPolls(matchId: number): Promise<number[]> {
    const raw = await this.simulate<bigint[]>(
      "get_match_polls",
      nativeToScVal(matchId, { type: "u64" }),
    );
    return raw.map(Number);
  }

  /**
   * Creates a new prediction poll (anyone can create).
   *
   * @param params  - Poll parameters.
   * @param options - Wallet + fee options.
   */
  async createPoll(
    params: CreatePollParams,
    options: InvokeOptions,
  ): Promise<TransactionResult<number>> {
    const result = await this.invoke<bigint>(
      "create_poll",
      options,
      nativeToScVal(options.wallet.publicKey, { type: "address" }),
      nativeToScVal(params.matchId, { type: "u64" }),
      nativeToScVal(params.question, { type: "string" }),
      nativeToScVal(params.category, { type: "u32" }),
      nativeToScVal(Math.floor(params.lockTime.getTime() / 1000), {
        type: "u64",
      }),
    );
    return { ...result, value: Number(result.value) };
  }

  /**
   * Cancels a poll (admin only). Enables emergency withdrawals.
   */
  async cancelPoll(
    pollId: number,
    options: InvokeOptions,
  ): Promise<TransactionResult<void>> {
    return this.invoke<void>(
      "cancel_poll",
      options,
      nativeToScVal(options.wallet.publicKey, { type: "address" }),
      nativeToScVal(pollId, { type: "u64" }),
    );
  }

  // -------------------------------------------------------------------------
  // Staking
  // -------------------------------------------------------------------------

  /**
   * Places a stake on a poll.
   *
   * @param pollId  - Target poll.
   * @param amount  - Amount in raw base units (bigint, 7 decimals).
   * @param side    - StakeSide.Yes (0) or StakeSide.No (1).
   * @param options - Wallet + fee options.
   */
  async stake(
    pollId: number,
    amount: bigint,
    side: StakeSide,
    options: InvokeOptions,
  ): Promise<TransactionResult<Stake>> {
    const result = await this.invoke<Record<string, unknown>>(
      "stake",
      options,
      nativeToScVal(options.wallet.publicKey, { type: "address" }),
      nativeToScVal(pollId, { type: "u64" }),
      nativeToScVal(amount, { type: "i128" }),
      nativeToScVal(side, { type: "u32" }),
    );
    return { ...result, value: rawToStake(result.value) };
  }

  /**
   * Returns stake information for a user on a poll.
   *
   * @param pollId - The poll ID.
   * @param user   - Stellar address of the staker.
   */
  async getStakeInfo(pollId: number, user: string): Promise<Stake> {
    const raw = await this.simulate<Record<string, unknown>>(
      "get_stake_info",
      nativeToScVal(pollId, { type: "u64" }),
      nativeToScVal(user, { type: "address" }),
    );
    return rawToStake(raw);
  }

  /**
   * Returns all poll IDs the user has staked on.
   */
  async getUserStakes(user: string): Promise<number[]> {
    const raw = await this.simulate<bigint[]>(
      "get_user_stakes",
      nativeToScVal(user, { type: "address" }),
    );
    return raw.map(Number);
  }

  /**
   * Returns whether a user has staked on a given poll.
   */
  async hasUserStaked(pollId: number, user: string): Promise<boolean> {
    return this.simulate<boolean>(
      "has_user_staked",
      nativeToScVal(pollId, { type: "u64" }),
      nativeToScVal(user, { type: "address" }),
    );
  }

  /**
   * Returns the current pool info for a poll.
   */
  async getPoolInfo(pollId: number): Promise<PoolInfo> {
    const raw = await this.simulate<Record<string, unknown>>(
      "get_pool_info",
      nativeToScVal(pollId, { type: "u64" }),
    );
    return rawToPoolInfo(raw);
  }

  /**
   * Calculates potential winnings for a hypothetical stake (view only).
   *
   * @param pollId - The poll ID.
   * @param side   - Which side to stake on.
   * @param amount - Hypothetical stake amount (raw base units).
   */
  async calculatePotentialWinnings(
    pollId: number,
    side: StakeSide,
    amount: bigint,
  ): Promise<bigint> {
    const pool = await this.getPoolInfo(pollId);
    const { winnings } = calculatePotentialWinnings(
      amount,
      side === 0 ? "yes" : "no",
      pool.yesPool,
      pool.noPool,
    );
    return winnings;
  }

  // -------------------------------------------------------------------------
  // Claims
  // -------------------------------------------------------------------------

  /**
   * Claims winnings for a resolved poll (winning stakers only).
   */
  async claimWinnings(
    pollId: number,
    options: InvokeOptions,
  ): Promise<TransactionResult<bigint>> {
    const result = await this.invoke<bigint>(
      "claim_winnings",
      options,
      nativeToScVal(options.wallet.publicKey, { type: "address" }),
      nativeToScVal(pollId, { type: "u64" }),
    );
    return result;
  }

  /**
   * Emergency withdrawal for cancelled / disputed polls (after 7-day timeout).
   */
  async emergencyWithdraw(
    pollId: number,
    options: InvokeOptions,
  ): Promise<TransactionResult<bigint>> {
    const result = await this.invoke<bigint>(
      "emergency_withdraw",
      options,
      nativeToScVal(options.wallet.publicKey, { type: "address" }),
      nativeToScVal(pollId, { type: "u64" }),
    );
    return result;
  }

  // -------------------------------------------------------------------------
  // Platform stats
  // -------------------------------------------------------------------------

  /**
   * Returns aggregate platform statistics.
   */
  async getPlatformStats(): Promise<PlatformStats> {
    const raw = await this.simulate<Record<string, unknown>>(
      "get_platform_stats",
    );
    return rawToPlatformStats(raw);
  }

  /**
   * Returns the token address used by the protocol.
   */
  async getTokenAddress(): Promise<string> {
    return this.simulate<string>("get_token_address");
  }

  /**
   * Returns the treasury contract address.
   */
  async getTreasuryAddress(): Promise<string> {
    return this.simulate<string>("get_treasury_address");
  }

  /**
   * Returns the platform fee in basis points.
   */
  async getPlatformFeeBps(): Promise<number> {
    return this.simulate<number>("get_platform_fee_bps");
  }

  /**
   * Returns the current contract balance.
   */
  async getContractBalance(): Promise<bigint> {
    return this.simulate<bigint>("get_contract_balance");
  }

  /**
   * Returns whether the contract is paused.
   */
  async isPaused(): Promise<boolean> {
    return this.simulate<boolean>("is_paused");
  }
}
