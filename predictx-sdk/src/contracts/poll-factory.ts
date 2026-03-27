/**
 * PredictX SDK — PollFactory contract client.
 *
 * Creates standalone polls (not linked to a match).
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
  Poll,
  PollCategory,
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

function rawToPoll(raw: Record<string, unknown>): Poll {
  return {
    pollId: Number(raw["poll_id"]),
    matchId: Number(raw["match_id"] ?? 0),
    creator: String(raw["creator"]),
    question: String(raw["question"]),
    category: Number(raw["category"]) as PollCategory,
    lockTime: toDate(raw["lock_time"]),
    yesPool: BigInt(String(raw["yes_pool"] ?? 0)),
    noPool: BigInt(String(raw["no_pool"] ?? 0)),
    yesCount: Number(raw["yes_count"] ?? 0),
    noCount: Number(raw["no_count"] ?? 0),
    status: Number(raw["status"] ?? 0),
    outcome: raw["outcome"] != null ? Boolean(raw["outcome"]) : undefined,
    resolutionTime: raw["resolution_time"] != null
      ? toDate(raw["resolution_time"])
      : undefined,
    createdAt: toDate(raw["created_at"] ?? 0),
  };
}

interface InvokeOptions {
  wallet: StellarWallet;
  fee?: string;
}

/**
 * Client for the `poll-factory` contract.
 */
export class PollFactoryClient {
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
   * Creates a standalone poll (not linked to any match).
   *
   * @param question       - Poll question (max 256 chars).
   * @param lockTime       - When staking closes.
   * @param options        - Wallet + fee options.
   */
  async createPoll(
    question: string,
    lockTime: Date,
    options: InvokeOptions,
  ): Promise<TransactionResult<number>> {
    const result = await this.invoke<bigint>(
      "create_poll",
      options,
      nativeToScVal(options.wallet.publicKey, { type: "address" }),
      nativeToScVal(question, { type: "string" }),
      nativeToScVal(Math.floor(lockTime.getTime() / 1000), { type: "u64" }),
    );
    return { ...result, value: Number(result.value) };
  }

  /**
   * Fetches a standalone poll by ID.
   */
  async getPoll(pollId: number): Promise<Poll> {
    const raw = await this.simulate<Record<string, unknown>>(
      "get_poll",
      nativeToScVal(pollId, { type: "u64" }),
    );
    return rawToPoll(raw);
  }
}
