import { apiError, DarajaError, requestOutcomeUnknown } from "./error.js";

/**
 * Internal Daraja HTTP response helpers.
 *
 * Centralizes shared JSON body parsing and bounded API error construction so
 * raw provider bodies stay redacted. Not part of the public SDK surface.
 */

/** Bounded API error for a rejected provider response; raw body is omitted. */
export function apiErrorFromBody(status: number, body: string): DarajaError {
  return apiError(status, body);
}

function tryParseJson<T>(text: string): { ok: true; value: T } | { ok: false; cause: unknown } {
  try {
    return { ok: true, value: JSON.parse(text) as T };
  } catch (cause) {
    return { ok: false, cause };
  }
}

/** Parse an idempotent payment response; invalid JSON leaves outcome unknown. */
export function parseIdempotentJsonBody<T>(text: string): T {
  const result = tryParseJson<T>(text);
  if (result.ok) return result.value;
  throw requestOutcomeUnknown(`provider returned invalid JSON: ${result.cause}`);
}

/** Parse a non-idempotent query response; invalid JSON is a network error. */
export function parsePlainJsonBody<T>(text: string): T {
  const result = tryParseJson<T>(text);
  if (result.ok) return result.value;
  throw new DarajaError(`Network error: invalid JSON response: ${result.cause}`, {
    kind: "network",
  });
}
