import { DarajaError } from "./error.js";
import type { StkCallbackBody } from "./types.js";

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