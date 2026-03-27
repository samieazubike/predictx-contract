/**
 * PredictX SDK — VotingOracle contract client.
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
} from "@stellar/stellar-sdk";

import type {
  VoteTally,
  VoteChoice,
  StellarWallet,
  TransactionResult,
  NetworkName,
} from "../types/index";
import { parseSorobanError } from "../errors";

const NETWORK_PASSPHRASE: Record<NetworkName, string> = {
  mainnet: Networks.PUBLIC,
  testnet: Networks.TESTNET,
  futurenet: Networks.FUTURENET,
  standalone: Networks.STANDALONE,
};

function toDate(unixSecs: unknown): Date {
  return new Date(Number(unixSecs) * 1000);
}

function rawToVoteTally(raw: Record<string, unknown>): VoteTally {
  return {
    pollId: Number(raw["poll_id"]),
    yesVotes: Number(raw["yes_votes"]),
    noVotes: Number(raw["no_votes"]),
    unclearVotes: Number(raw["unclear_votes"]),
    totalVoters: Number(raw["total_voters"]),
    votingEndTime: toDate(raw["voting_end_time"]),
    rewardPool: BigInt(String(raw["reward_pool"])),
  };
}

interface InvokeOptions {
  wallet: StellarWallet;
  fee?: string;
}

/**
 * Client for the `voting-oracle` contract.
 */
export class VotingOracleClient {
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

  private async simulate<T>(method: string, ...args: xdr.ScVal[]): Promise<T> {
    try {
      const account = await this.server.getAccount(
        "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN",
      );
      const tx = new TransactionBuilder(account, {
        fee: BASE_FEE,
        networkPassphrase: this.networkPassphrase,
      })
        .addOperation(this.contract.call(method, ...args))
        .setTimeout(30)
        .build();

      const sim = await this.server.simulateTransaction(tx);
      if (SorobanRpc.Api.isSimulationError(sim)) {
        throw parseSorobanError(new Error(sim.error));
      }
      return scValToNative(sim.result!.retval) as T;
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

      const sim = await this.server.simulateTransaction(tx);
      if (SorobanRpc.Api.isSimulationError(sim)) {
        throw parseSorobanError(new Error(sim.error));
      }

      const prepared = SorobanRpc.assembleTransaction(tx, sim).build();
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

      let getResponse = await this.server.getTransaction(submitted.hash);
      while (
        getResponse.status === SorobanRpc.Api.GetTransactionStatus.NOT_FOUND
      ) {
        await new Promise((r) => setTimeout(r, 1000));
        getResponse = await this.server.getTransaction(submitted.hash);
      }

      if (
        getResponse.status !== SorobanRpc.Api.GetTransactionStatus.SUCCESS
      ) {
        throw new Error(`Transaction failed: ${getResponse.status}`);
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

  /**
   * Casts a vote on a locked poll.
   *
   * @param pollId - The poll to vote on.
   * @param choice - VoteChoice.Yes, .No, or .Unclear.
   * @param options - Wallet + fee options.
   */
  async castVote(
    pollId: number,
    choice: VoteChoice,
    options: InvokeOptions,
  ): Promise<TransactionResult<VoteTally>> {
    const result = await this.invoke<Record<string, unknown>>(
      "cast_vote",
      options,
      nativeToScVal(options.wallet.publicKey, { type: "address" }),
      nativeToScVal(pollId, { type: "u64" }),
      nativeToScVal(choice, { type: "u32" }),
    );
    return { ...result, value: rawToVoteTally(result.value) };
  }

  /**
   * Returns the current vote tally for a poll.
   */
  async getVoteTally(pollId: number): Promise<VoteTally> {
    const raw = await this.simulate<Record<string, unknown>>(
      "get_vote_tally",
      nativeToScVal(pollId, { type: "u64" }),
    );
    return rawToVoteTally(raw);
  }

  /**
   * Returns the oracle status of a poll.
   */
  async getPollStatus(pollId: number): Promise<number> {
    return this.simulate<number>(
      "get_poll_status",
      nativeToScVal(pollId, { type: "u64" }),
    );
  }

  /**
   * Returns the timestamp of the last poll-status update.
   */
  async getPollStatusUpdatedAt(pollId: number): Promise<Date> {
    const ts = await this.simulate<number>(
      "get_poll_status_updated_at",
      nativeToScVal(pollId, { type: "u64" }),
    );
    return new Date(ts * 1000);
  }

  /**
   * Claims the voting reward for a correctly-voted resolved poll.
   */
  async claimVotingReward(
    pollId: number,
    options: InvokeOptions,
  ): Promise<TransactionResult<bigint>> {
    return this.invoke<bigint>(
      "claim_voting_reward",
      options,
      nativeToScVal(options.wallet.publicKey, { type: "address" }),
      nativeToScVal(pollId, { type: "u64" }),
    );
  }
}
