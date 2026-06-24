import { TokenManager } from "./auth.js";
import { DarajaError } from "./error.js";
import {
  checkIdempotencyCache,
  idempotencyKeyForBody,
  IdempotencyStore,
} from "./idempotency.js";
import type { Credentials, Environment } from "./types.js";
import { environmentBaseUrl } from "./types.js";

export interface DarajaClientOptions {
  baseUrlOverride?: string;
  idempotency?: boolean;
  idempotencyStore?: IdempotencyStore;
}

export class DarajaClient {
  private readonly tokenManager: TokenManager;
  private readonly idempotencyStore?: IdempotencyStore;

  constructor(
    readonly credentials: Credentials,
    readonly environment: Environment,
    private readonly options: DarajaClientOptions = {}
  ) {
    this.tokenManager = new TokenManager(
      credentials.consumerKey,
      credentials.consumerSecret,
      environment,
      options.baseUrlOverride
    );

    if (options.idempotency !== false) {
      this.idempotencyStore = options.idempotencyStore ?? new IdempotencyStore();
    }
  }

  baseUrl(): string {
    return this.options.baseUrlOverride ?? environmentBaseUrl(this.environment);
  }

  withoutIdempotency(): DarajaClient {
    return new DarajaClient(this.credentials, this.environment, {
      ...this.options,
      idempotency: false,
    });
  }

  async post<T>(path: string, body: unknown): Promise<T> {
    const url = `${this.baseUrl()}${path}`;
    const bodyBytes = Buffer.from(JSON.stringify(body));

    if (this.idempotencyStore) {
      const cached = await checkIdempotencyCache(
        this.idempotencyStore,
        bodyBytes
      );
      if (cached) {
        return JSON.parse(cached.toString("utf8")) as T;
      }
    }

    const idempotencyKey = idempotencyKeyForBody(bodyBytes);
    const response = await this.sendPost(url, bodyBytes, idempotencyKey, false);
    const text = await response.text();

    if (!response.ok) {
      if (this.idempotencyStore) {
        await this.idempotencyStore.saveFailed(idempotencyKey);
      }
      throw new DarajaError(`Network error: HTTP ${response.status}: ${text}`);
    }

    if (this.idempotencyStore) {
      await this.idempotencyStore.saveSuccess(
        idempotencyKey,
        Buffer.from(text, "utf8")
      );
    }

    return JSON.parse(text) as T;
  }

  private async sendPost(
    url: string,
    body: Buffer,
    idempotencyKey: string,
    isRetry: boolean
  ): Promise<Response> {
    const token = await this.tokenManager.getToken();
    const response = await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
        "Idempotency-Key": idempotencyKey,
      },
      body: new Uint8Array(body),
    });

    if (response.status === 401 && !isRetry) {
      this.tokenManager.invalidate();
      return this.sendPost(url, body, idempotencyKey, true);
    }

    return response;
  }
}