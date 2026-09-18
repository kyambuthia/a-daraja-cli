import { TokenManager } from "./auth.js";
import { certificatePem } from "./certs.js";
import {
  DarajaError,
  localStateUncertain,
  requestOutcomeUnknown,
} from "./error.js";
import {
  type IdempotencyBackend,
  IdempotencyStore,
  requestFingerprint,
  scopedIdempotencyKey,
  validateIdempotencyKey,
} from "./idempotency.js";
import { generateSecurityCredential, generateStkPassword } from "./security.js";
import {
  apiErrorFromBody,
  parseIdempotentJsonBody,
  parsePlainJsonBody,
} from "./response.js";
import type { Credentials, Environment } from "./types.js";
import { environmentBaseUrl } from "./types.js";

export interface DarajaClientOptions {
  /** Sandbox-only endpoint override for local simulators and tests. */
  testBaseUrlOverride?: string;
  /** Explicitly unsafe test escape hatch. Never use for money movement. */
  dangerouslyDisableIdempotencyForTesting?: boolean;
  idempotencyStore?: IdempotencyBackend;
  requestTimeoutMs?: number;
}

export interface RequestOptions {
  idempotencyKey: string;
}

export class DarajaClient {
  private readonly tokenManager: TokenManager;
  private readonly idempotencyStore?: IdempotencyBackend;
  readonly #credentials: Credentials;

  constructor(
    credentials: Credentials,
    readonly environment: Environment,
    private readonly options: DarajaClientOptions = {}
  ) {
    if (options.testBaseUrlOverride && environment !== "sandbox") {
      throw new DarajaError(
        "Endpoint overrides are sandbox-only; production credentials cannot use a custom base URL."
      );
    }
    this.#credentials = { ...credentials };
    this.tokenManager = new TokenManager(
      credentials.consumerKey,
      credentials.consumerSecret,
      environment,
      options.testBaseUrlOverride,
      options.requestTimeoutMs ?? 30_000
    );

    if (!options.dangerouslyDisableIdempotencyForTesting) {
      this.idempotencyStore = options.idempotencyStore ?? new IdempotencyStore();
    }
  }

  baseUrl(): string {
    return this.options.testBaseUrlOverride ?? environmentBaseUrl(this.environment);
  }

  /** Test-only escape hatch with an intentionally alarming name. */
  dangerouslyWithoutIdempotencyForTesting(): DarajaClient {
    return new DarajaClient(this.#credentials, this.environment, {
      ...this.options,
      dangerouslyDisableIdempotencyForTesting: true,
    });
  }

  shortcode(): string {
    return this.#credentials.shortcode;
  }

  stkPassword(timestamp: string): string {
    return generateStkPassword(
      this.#credentials.shortcode,
      this.#credentials.passkey,
      timestamp
    );
  }

  initiatorName(): string {
    const value = this.#credentials.initiatorName;
    if (!value) throw new DarajaError("initiator_name is not configured.");
    return value;
  }

  async initiatorSecurityCredential(): Promise<string> {
    const password = this.#credentials.initiatorPassword;
    if (!password) throw new DarajaError("initiator_password is not configured.");
    const pem = await certificatePem(this.environment);
    return generateSecurityCredential(password, pem);
  }

  async post<T>(
    path: string,
    body: unknown,
    requestOptions: RequestOptions
  ): Promise<T> {
    const url = `${this.baseUrl()}${path}`;
    const bodyBytes = Buffer.from(JSON.stringify(body));
    const fingerprint = requestFingerprint(bodyBytes);
    const callerKey = requestOptions.idempotencyKey;
    validateIdempotencyKey(callerKey);
    const scopedKey = scopedIdempotencyKey(
      this.environment,
      this.#credentials.shortcode,
      path,
      callerKey
    );

    if (this.idempotencyStore) {
      let result: Awaited<ReturnType<IdempotencyBackend["acquire"]>>;
      try {
        result = await this.idempotencyStore.acquire(scopedKey, fingerprint);
      } catch (error) {
        if (error instanceof DarajaError) throw error;
        throw new DarajaError(
          `Failed to reserve idempotency key; no request was sent: ${error}`,
          { kind: "idempotency" }
        );
      }
      if (result.kind === "success") {
        return JSON.parse(result.response.toString("utf8")) as T;
      }
      if (result.kind === "pending") {
        throw new DarajaError(
          "Idempotency conflict: a payment with this key is already in flight.",
          { kind: "idempotency" }
        );
      }
      if (result.kind === "fingerprint-mismatch") {
        throw new DarajaError(
          "Idempotency key was already used with different request parameters.",
          { kind: "idempotency" }
        );
      }
      if (result.kind === "failed") {
        throw new DarajaError(
          "A previous request with this idempotency key failed; review it before using a new key.",
          { kind: "idempotency" }
        );
      }
      if (result.kind === "unknown") {
        throw new DarajaError(
          "Payment outcome is unknown; reconcile the transaction before retrying.",
          { kind: "idempotency" }
        );
      }
    }

    let response: Response;
    try {
      response = await this.sendPost(url, bodyBytes, callerKey, false);
    } catch (error) {
      if (this.idempotencyStore) {
        try {
          if (
            error instanceof DarajaError &&
            error.kind === "request-outcome-unknown"
          ) {
            await this.idempotencyStore.saveUnknown(scopedKey);
          } else {
            await this.idempotencyStore.saveFailed(scopedKey);
          }
        } catch (persistenceError) {
          throw localStateUncertain(`record request failure: ${persistenceError}`);
        }
      }
      throw error;
    }

    let text: string;
    try {
      text = await response.text();
    } catch (error) {
      if (this.idempotencyStore) {
        try {
          await this.idempotencyStore.saveUnknown(scopedKey);
        } catch (persistenceError) {
          throw localStateUncertain(`record unreadable response: ${persistenceError}`);
        }
      }
      throw requestOutcomeUnknown(`failed to read provider response: ${error}`);
    }

    if (!response.ok) {
      if (this.idempotencyStore) {
        try {
          await this.idempotencyStore.saveFailed(scopedKey);
        } catch (persistenceError) {
          throw localStateUncertain(`record definitive provider failure: ${persistenceError}`);
        }
      }
      throw apiErrorFromBody(response.status, text);
    }

    let parsed: T;
    try {
      parsed = parseIdempotentJsonBody<T>(text);
    } catch (error) {
      if (this.idempotencyStore) {
        try {
          await this.idempotencyStore.saveUnknown(scopedKey);
        } catch (persistenceError) {
          throw localStateUncertain(`record invalid provider response: ${persistenceError}`);
        }
      }
      throw error;
    }

    if (this.idempotencyStore) {
      try {
        await this.idempotencyStore.saveSuccess(
          scopedKey,
          Buffer.from(text, "utf8")
        );
      } catch (persistenceError) {
        throw localStateUncertain(`record successful provider response: ${persistenceError}`);
      }
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
      throw apiErrorFromBody(response.status, text);
    }
    return parsePlainJsonBody<T>(text);
  }

  private async sendPost(
    url: string,
    body: Buffer,
    idempotencyKey: string | undefined,
    isRetry: boolean
  ): Promise<Response> {
    const token = await this.tokenManager.getToken();
    let response: Response;
    try {
      response = await fetch(url, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${token}`,
          "Content-Type": "application/json",
          ...(idempotencyKey ? { "Idempotency-Key": idempotencyKey } : {}),
        },
        body: new Uint8Array(body),
        signal: AbortSignal.timeout(this.options.requestTimeoutMs ?? 30_000),
      });
    } catch (error) {
      throw requestOutcomeUnknown(`request transport failed: ${error}`);
    }

    if (response.status === 401 && !isRetry) {
      this.tokenManager.invalidate();
      return this.sendPost(url, body, idempotencyKey, true);
    }

    return response;
  }
}
