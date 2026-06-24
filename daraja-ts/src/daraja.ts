import { stkPush, stkPushQuery } from "./api/stk-push.js";
import type { PromptOptions } from "./api/stk-push.js";
import { DarajaClient } from "./client.js";
import {
  environmentFromSdkKey,
  loadCredentials,
  loadSdkKey,
} from "./config.js";
import { missingApiKey } from "./error.js";
import type { StkPushResponse } from "./types.js";

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

  /** Access the underlying HTTP client for advanced API calls. */
  getClient(): DarajaClient {
    return this.client;
  }
}