import { DarajaError } from "./error.js";
import type { C2bConfirmation, StkCallbackBody } from "./types.js";

export function parseStkCallback(body: unknown): StkCallbackBody["Body"]["stkCallback"] {
  const parsed = body as StkCallbackBody;
  const callback = parsed?.Body?.stkCallback;
  if (!callback?.CheckoutRequestID) {
    throw new DarajaError("Invalid STK callback: missing CheckoutRequestID.");
  }
  return callback;
}

export function stkCallbackPaid(callback: StkCallbackBody["Body"]["stkCallback"]): boolean {
  return callback.ResultCode === 0;
}

export function stkCallbackMetadata(
  callback: StkCallbackBody["Body"]["stkCallback"]
): Record<string, string | number> {
  const items = callback.CallbackMetadata?.Item ?? [];
  return Object.fromEntries(items.map((item) => [item.Name, item.Value]));
}

/**
 * Verify a C2B callback.
 *
 * The current public Daraja documentation does not expose enough detail to
 * implement the signature algorithm and key management safely. Fail closed;
 * never treat header presence as authentication.
 */
export function verifyCallback(
  body: string | Buffer,
  signatureHeader: string | null | undefined
): void {
  const bytes = typeof body === "string" ? Buffer.from(body) : body;
  if (bytes.length === 0) {
    throw new DarajaError(
      "Invalid callback signature: request did not come from Safaricom."
    );
  }
  if (!signatureHeader?.trim()) {
    throw new DarajaError(
      "Invalid callback signature: request did not come from Safaricom."
    );
  }
  throw new DarajaError(
    "Callback signature verification is not configured; refusing to trust this callback."
  );
}

/** Parse a C2B confirmation or validation callback body. */
export function parseC2bConfirmation(body: unknown): C2bConfirmation {
  const parsed = body as C2bConfirmation;
  if (!parsed?.TransID || !parsed?.TransAmount) {
    throw new DarajaError("Invalid C2B callback: missing TransID or TransAmount.");
  }
  return parsed;
}
