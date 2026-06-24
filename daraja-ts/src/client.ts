import { TokenManager } from "./auth.js";
import { DarajaError } from "./error.js";
import type { Credentials, Environment } from "./types.js";
import { environmentBaseUrl } from "./types.js";

export class DarajaClient {
  private readonly tokenManager: TokenManager;

  constructor(
    readonly credentials: Credentials,
    readonly environment: Environment,
    private readonly baseUrlOverride?: string
  ) {
    this.tokenManager = new TokenManager(
      credentials.consumerKey,
      credentials.consumerSecret,
      environment,
      baseUrlOverride
    );
  }

  baseUrl(): string {
    return this.baseUrlOverride ?? environmentBaseUrl(this.environment);
  }

  async post<T>(path: string, body: unknown): Promise<T> {
    const url = `${this.baseUrl()}${path}`;
    const response = await this.sendPost(url, body, false);
    const text = await response.text();

    if (!response.ok) {
      throw new DarajaError(`Network error: HTTP ${response.status}: ${text}`);
    }

    return JSON.parse(text) as T;
  }

  private async sendPost(
    url: string,
    body: unknown,
    isRetry: boolean
  ): Promise<Response> {
    const token = await this.tokenManager.getToken();
    const response = await fetch(url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });

    if (response.status === 401 && !isRetry) {
      this.tokenManager.invalidate();
      return this.sendPost(url, body, true);
    }

    return response;
  }
}