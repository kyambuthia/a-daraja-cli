import { DarajaError } from "./error.js";
import type { Environment } from "./types.js";
import { environmentBaseUrl } from "./types.js";

interface Token {
  accessToken: string;
  expiresAt: number;
}

export class TokenManager {
  private cached: Token | null = null;

  constructor(
    private readonly consumerKey: string,
    private readonly consumerSecret: string,
    private readonly environment: Environment,
    private readonly baseUrlOverride?: string
  ) {}

  async getToken(): Promise<string> {
    if (this.cached && Date.now() < this.cached.expiresAt) {
      return this.cached.accessToken;
    }

    const baseUrl = this.baseUrlOverride ?? environmentBaseUrl(this.environment);
    const url = `${baseUrl}/oauth/v1/generate?grant_type=client_credentials`;
    const basic = Buffer.from(
      `${this.consumerKey}:${this.consumerSecret}`
    ).toString("base64");

    const response = await fetch(url, {
      headers: { Authorization: `Basic ${basic}` },
    });

    if (!response.ok) {
      const body = await response.text();
      throw new DarajaError(`Authentication failed: HTTP ${response.status}: ${body}`);
    }

    const data = (await response.json()) as {
      access_token: string;
      expires_in: string;
    };

    const expiresIn = Number.parseInt(data.expires_in, 10);
    if (Number.isNaN(expiresIn)) {
      throw new DarajaError(`Authentication failed: invalid expires_in '${data.expires_in}'`);
    }

    this.cached = {
      accessToken: data.access_token,
      expiresAt: Date.now() + expiresIn * 1000,
    };

    return this.cached.accessToken;
  }

  invalidate(): void {
    this.cached = null;
  }
}