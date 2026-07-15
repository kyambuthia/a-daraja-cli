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
    private readonly baseUrlOverride?: string,
    private readonly requestTimeoutMs: number = 30_000
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

    let response: Response | undefined;
    for (let attempt = 0; attempt < 3; attempt += 1) {
      try {
        response = await fetch(url, {
          headers: { Authorization: `Basic ${basic}` },
          signal: AbortSignal.timeout(this.requestTimeoutMs),
        });
      } catch (error) {
        if (attempt < 2) {
          await new Promise((resolve) => setTimeout(resolve, 100 * 2 ** attempt));
          continue;
        }
        throw new DarajaError(`Authentication failed before a token was obtained: ${error}`);
      }
      if ((response.status === 429 || response.status >= 500) && attempt < 2) {
        await new Promise((resolve) => setTimeout(resolve, 100 * 2 ** attempt));
        continue;
      }
      break;
    }

    if (!response) {
      throw new DarajaError("Authentication failed before a token was obtained.");
    }

    if (!response.ok) {
      throw new DarajaError(
        `Authentication failed: HTTP ${response.status}: credentials were rejected; response body omitted`
      );
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
      expiresAt: Date.now() + Math.max(0, expiresIn - 30) * 1000,
    };

    return this.cached.accessToken;
  }

  invalidate(): void {
    this.cached = null;
  }
}
