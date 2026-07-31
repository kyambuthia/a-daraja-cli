import { TokenManager } from "./auth.js";
import { apiError, DarajaError } from "./error.js";
import {
  idempotencyKeyForBody,
  IdempotencyStore,
  requestFingerprint,
  scopedIdempotencyKey,
  validateIdempotencyKey,
} from "./idempotency.js";
import type { Credentials, Environment } from "./types.js";
import { environmentBaseUrl } from "./types.js";

export interface DarajaClientOptions {
  baseUrlOverride?: string;
  idempotency?: boolean;
  idempotencyStore?: IdempotencyStore;
  requestTimeoutMs?: number;
}

export interface RequestOptions {
  idempotencyKey?: string;
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
      options.baseUrlOverride,
      options.requestTimeoutMs ?? 30_000
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

  async post<T>(
    path: string,
    body: unknown,
    requestOptions: RequestOptions = {}
  ): Promise<T> {
    const url = `${this.baseUrl()}${path}`;
    const bodyBytes = Buffer.from(JSON.stringify(body));
    const fingerprint = requestFingerprint(bodyBytes);
    const callerKey =
      requestOptions.idempotencyKey ?? idempotencyKeyForBody(bodyBytes);
    validateIdempotencyKey(callerKey);
    const scopedKey = scopedIdempotencyKey(
      this.environment,
      this.credentials.shortcode,
      path,
      callerKey
    );

    if (this.idempotencyStore) {
      const result = await this.idempotencyStore.acquire(scopedKey, fingerprint);
      if (result.kind === "success") {
        return JSON.parse(result.response.toString("utf8")) as T;
      }
      if (result.kind === "pending") {
        throw new DarajaError(
          "Idempotency conflict: a payment with this key is already in flight."
        );
      }
      if (result.kind === "fingerprint-mismatch") {
        throw new DarajaError(
          "Idempotency key was already used with different request parameters."
        );
      }
      if (result.kind === "failed") {
        throw new DarajaError(
          "A previous request with this idempotency key failed; review it before using a new key."
        );
      }
      if (result.kind === "unknown") {
        throw new DarajaError(
          "Payment outcome is unknown; reconcile the transaction before retrying."
        );
      }
    }

    let response: Response;
    try {
      response = await this.sendPost(url, bodyBytes, callerKey, false);
    } catch (error) {
      if (this.idempotencyStore) {
        await this.idempotencyStore.saveUnknown(scopedKey);
      }
      throw error;
    }

    let text: string;
    try {
      text = await response.text();
    } catch (error) {
      if (this.idempotencyStore) {
        await this.idempotencyStore.saveUnknown(scopedKey);
      }
      throw new DarajaError(`Network error: failed to read response: ${error}`);
    }

    if (!response.ok) {
      if (this.idempotencyStore) {
        await this.idempotencyStore.saveFailed(scopedKey);
      }
      throw apiError(response.status, text);
    }

    let parsed: T;
    try {
      parsed = JSON.parse(text) as T;
    } catch (error) {
      if (this.idempotencyStore) {
        await this.idempotencyStore.saveUnknown(scopedKey);
      }
      throw new DarajaError(`Network error: invalid JSON response: ${error}`);
    }

    if (this.idempotencyStore) {
      await this.idempotencyStore.saveSuccess(
        scopedKey,
        Buffer.from(text, "utf8")
      );
    }

    return parsed;
  }

  /** Send a POST query without local idempotency caching or a header. */
  async postWithoutIdempotency<T>(path: string, body: unknown): Promise<T> {
    const url = `${this.baseUrl()}${path}`;
    const bodyBytes = Buffer.from(JSON.stringify(body));
    const response = await this.sendPost(url, bodyBytes, undefined, false);
    const text = await response.text();
    if (!response.ok) {
      throw apiError(response.status, text);
    }
    try {
      return JSON.parse(text) as T;
    } catch (error) {
      throw new DarajaError(`Network error: invalid JSON response: ${error}`);
    }
  }

  private async sendPost(
    url: string,
    body: Buffer,
    idempotencyKey: string | undefined,
    isRetry: boolean
  ): Promise<Response> {
    const token = await this.tokenManager.getToken();
    const response = await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
        ...(idempotencyKey ? { "Idempotency-Key": idempotencyKey } : {}),
      },
      body: new Uint8Array(body),
      signal: AbortSignal.timeout(this.options.requestTimeoutMs ?? 30_000),
    });

    if (response.status === 401 && !isRetry) {
      this.tokenManager.invalidate();
      return this.sendPost(url, body, idempotencyKey, true);
    }

    return response;
  }
}
