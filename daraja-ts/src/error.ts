export class DarajaError extends Error {
  readonly status?: number;
  readonly code?: string;

  constructor(message: string, options: { status?: number; code?: string } = {}) {
    super(message);
    this.name = "DarajaError";
    this.status = options.status;
    this.code = options.code;
  }
}

export function apiError(status: number, body: string): DarajaError {
  let parsed: Record<string, unknown> = {};
  try {
    const value = JSON.parse(body) as unknown;
    if (value && typeof value === "object" && !Array.isArray(value)) {
      parsed = value as Record<string, unknown>;
    }
  } catch {
    // Raw response bodies are deliberately omitted from errors.
  }
  const firstString = (keys: string[], fallback: string, limit: number): string => {
    const value = keys.map((key) => parsed[key]).find((item) => typeof item === "string");
    return (typeof value === "string" ? value : fallback).slice(0, limit);
  };
  const code = firstString(["errorCode", "ResponseCode", "requestId"], "unknown", 64);
  const message = firstString(
    ["errorMessage", "ResponseDescription", "message"],
    "request rejected; response body omitted",
    256
  );
  return new DarajaError(
    `Daraja API request failed (HTTP ${status}, code ${code}): ${message}`,
    { status, code }
  );
}

export function missingApiKey(): DarajaError {
  return new DarajaError("DARAJA_KEY environment variable is not set.");
}
