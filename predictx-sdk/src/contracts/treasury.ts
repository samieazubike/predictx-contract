/**
 * PredictX SDK — Treasury contract client.
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

interface InvokeOptions {
  wallet: StellarWallet;
  fee?: string;
}

/**
 * Client for the `treasury` contract.
 */
export class TreasuryClient {
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
   * Returns the balance recorded in treasury for a given address.
   *
   * @param who - Stellar address to query.
   */
  async getBalance(who: string): Promise<bigint> {
    return this.simulate<bigint>(
      "balance",
      nativeToScVal(who, { type: "address" }),
    );
  }

  /**
   * Records a deposit into the treasury (called by protocol contracts).
   *
   * @param from    - Depositor address.
   * @param amount  - Amount in base units.
   * @param options - Wallet + fee options.
   */
  async deposit(
    from: string,
    amount: bigint,
    options: InvokeOptions,
  ): Promise<TransactionResult<bigint>> {
    return this.invoke<bigint>(
      "deposit",
      options,
      nativeToScVal(from, { type: "address" }),
      nativeToScVal(amount, { type: "i128" }),
    );
  }
}
