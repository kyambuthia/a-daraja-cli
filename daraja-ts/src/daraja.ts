import { queryBalance } from "./api/balance.js";
import { b2cPayment } from "./api/b2c.js";
import { registerUrls, simulateC2b } from "./api/c2b.js";
import { reverseTransaction } from "./api/reversal.js";
import { stkPush, stkPushQuery } from "./api/stk-push.js";
import type { PromptOptions } from "./api/stk-push.js";
import { queryStatus } from "./api/status.js";
import { DarajaClient } from "./client.js";
import {
  environmentFromSdkKey,
  loadCredentials,
  loadSdkKey,
} from "./config.js";
import { missingApiKey } from "./error.js";
import type {
  BalanceOptions,
  C2bRegisterResponse,
  ConversationResponse,
  QueryOptions,
  RefundOptions,
  RegisterOptions,
  SendOptions,
  SimulateOptions,
  StkPushResponse,
} from "./types.js";

export class Daraja {
  private constructor(private readonly client: DarajaClient) {}

  static async fromEnv(): Promise<Daraja> {
    const key = process.env.DARAJA_KEY ?? (await loadSdkKey().catch(() => ""));
    if (!key) throw missingApiKey();
    return Daraja.fromKey(key);
  }

  static async fromKey(sdkKey: string): Promise<Daraja> {
    const credentials = await loadCredentials(sdkKey);
    const environment = environmentFromSdkKey(sdkKey);
    const client = new DarajaClient(credentials, environment);
    return new Daraja(client);
  }

  /** Send M-Pesa PIN prompt to customer's phone (STK Push). */
  async prompt(options: PromptOptions): Promise<StkPushResponse> {
    return stkPush(this.client, options);
  }

  /** Check if customer completed a payment prompt. */
  async promptStatus(checkoutRequestId: string): Promise<StkPushResponse> {
    return stkPushQuery(this.client, checkoutRequestId);
  }

  /** Send money to a phone number (B2C). */
  async send(options: SendOptions): Promise<ConversationResponse> {
    return b2cPayment(this.client, options);
  }

  /** Query the status of an M-Pesa transaction by receipt ID. */
  async query(options: QueryOptions): Promise<ConversationResponse> {
    return queryStatus(this.client, options);
  }

  /** Request an account balance check (result arrives at resultUrl). */
  async balance(options: BalanceOptions): Promise<ConversationResponse> {
    return queryBalance(this.client, options);
  }

  /** Reverse / refund a completed transaction. */
  async refund(options: RefundOptions): Promise<ConversationResponse> {
    return reverseTransaction(this.client, options);
  }

  /** Register C2B validation and confirmation URLs with Daraja. */
  async register(options: RegisterOptions): Promise<C2bRegisterResponse> {
    return registerUrls(this.client, options);
  }

  /** Simulate an incoming PayBill payment (sandbox only). */
  async simulate(options: SimulateOptions): Promise<C2bRegisterResponse> {
    return simulateC2b(this.client, options);
  }

  /** Access the underlying HTTP client for advanced API calls. */
  getClient(): DarajaClient {
    return this.client;
  }
}