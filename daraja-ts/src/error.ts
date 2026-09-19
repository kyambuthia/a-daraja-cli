export type DarajaErrorKind =
  | "api"
  | "configuration"
  | "idempotency"
  | "network"
  | "request-outcome-unknown"
  | "local-state-uncertain";

export class DarajaError extends Error {
  readonly status?: number;
  readonly code?: string;
  readonly kind: DarajaErrorKind;

  constructor(
    message: string,
    options: { status?: number; code?: string; kind?: DarajaErrorKind } = {}
  ) {
    super(message);
    this.name = "DarajaError";
    this.status = options.status;
    this.code = options.code;
    this.kind = options.kind ?? "configuration";
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
    { status, code, kind: "api" }
  );
}

export function missingApiKey(): DarajaError {
  return new DarajaError("DARAJA_KEY environment variable is not set.");
}

export function requestOutcomeUnknown(message: string): DarajaError {
  return new DarajaError(
    `Request outcome is unknown: ${message} Reconcile before retrying.`,
    { kind: "request-outcome-unknown" }
  );
}

export function localStateUncertain(message: string): DarajaError {
  return new DarajaError(
    `Provider outcome may be known but local safety state could not be persisted: ${message}`,
    { kind: "local-state-uncertain" }
  );
}
