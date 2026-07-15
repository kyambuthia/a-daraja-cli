import { DarajaError } from "./error.js";
import type { C2bConfirmation, StkCallbackBody } from "./types.js";
import { validatePhone, validateRequiredText, validateShortcode } from "./validation.js";

type StkCallback = StkCallbackBody["Body"]["stkCallback"];

export interface ExpectedStkPayment {
  checkoutRequestId: string;
  merchantRequestId?: string;
  amount: number;
}

export interface MatchedStkCallback {
  checkoutRequestId: string;
  merchantRequestId: string;
  amount: number;
  receiptNumber: string;
  phoneNumber?: string;
}

function asRecord(value: unknown, message: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new DarajaError(message);
  }
  return value as Record<string, unknown>;
}

function requiredString(record: Record<string, unknown>, key: string, context: string): string {
  const value = record[key];
  if (typeof value !== "string" || !value.trim()) {
    throw new DarajaError(`Invalid ${context}: missing or invalid ${key}.`);
  }
  return value;
}

function parsePositiveDecimal(value: string, context: string): void {
  if (!/^\d+(?:\.\d+)?$/.test(value) || !/[1-9]/.test(value)) {
    throw new DarajaError(`Invalid ${context}: amount must be positive.`);
  }
}

export function parseStkCallback(body: unknown): StkCallback {
  const root = asRecord(body, "Invalid STK callback: expected an object.");
  const envelope = asRecord(root.Body, "Invalid STK callback: missing Body.");
  const callback = asRecord(
    envelope.stkCallback,
    "Invalid STK callback: missing stkCallback."
  );
  requiredString(callback, "MerchantRequestID", "STK callback");
  requiredString(callback, "CheckoutRequestID", "STK callback");
  requiredString(callback, "ResultDesc", "STK callback");
  if (!Number.isInteger(callback.ResultCode)) {
    throw new DarajaError("Invalid STK callback: ResultCode must be an integer.");
  }
  if (callback.CallbackMetadata !== undefined) {
    const metadata = asRecord(
      callback.CallbackMetadata,
      "Invalid STK callback: CallbackMetadata must be an object."
    );
    if (!Array.isArray(metadata.Item)) {
      throw new DarajaError("Invalid STK callback: CallbackMetadata.Item must be an array.");
    }
    for (const item of metadata.Item) {
      const record = asRecord(item, "Invalid STK callback metadata item.");
      requiredString(record, "Name", "STK callback metadata");
      if (!(typeof record.Value === "string" || typeof record.Value === "number")) {
        throw new DarajaError("Invalid STK callback metadata: Value must be a string or number.");
      }
    }
  }
  return callback as unknown as StkCallback;
}

/** ResultCode alone is not proof of authenticity; reconcile before fulfilment. */
export function stkCallbackPaid(callback: StkCallback): boolean {
  return callback.ResultCode === 0;
}

export function stkCallbackMetadata(callback: StkCallback): Record<string, string | number> {
  const items = callback.CallbackMetadata?.Item ?? [];
  const metadata: Record<string, string | number> = {};
  for (const item of items) {
    if (Object.hasOwn(metadata, item.Name)) {
      throw new DarajaError(`Invalid STK callback metadata: duplicate ${item.Name}.`);
    }
    metadata[item.Name] = item.Value;
  }
  return metadata;
}

/**
 * Check an untrusted callback against the payment that was initiated.
 * This prevents mismatched fulfilment but does not authenticate the sender.
 */
export function matchStkCallback(
  callback: StkCallback,
  expected: ExpectedStkPayment
): MatchedStkCallback {
  if (!stkCallbackPaid(callback)) {
    throw new DarajaError(`STK payment was not successful: ${callback.ResultDesc}`);
  }
  if (callback.CheckoutRequestID !== expected.checkoutRequestId) {
    throw new DarajaError("STK callback does not match the expected checkout request ID.");
  }
  if (
    expected.merchantRequestId !== undefined &&
    callback.MerchantRequestID !== expected.merchantRequestId
  ) {
    throw new DarajaError("STK callback does not match the expected merchant request ID.");
  }
  const metadata = stkCallbackMetadata(callback);
  if (typeof metadata.Amount !== "number" || metadata.Amount !== expected.amount) {
    throw new DarajaError("STK callback amount does not match the expected payment amount.");
  }
  if (typeof metadata.MpesaReceiptNumber !== "string" || !metadata.MpesaReceiptNumber.trim()) {
    throw new DarajaError("Successful STK callback is missing MpesaReceiptNumber.");
  }
  const phoneNumber = metadata.PhoneNumber;
  if (phoneNumber !== undefined && typeof phoneNumber !== "string" && typeof phoneNumber !== "number") {
    throw new DarajaError("STK callback PhoneNumber has an invalid type.");
  }
  return {
    checkoutRequestId: callback.CheckoutRequestID,
    merchantRequestId: callback.MerchantRequestID,
    amount: metadata.Amount,
    receiptNumber: metadata.MpesaReceiptNumber,
    phoneNumber: phoneNumber === undefined ? undefined : validatePhone(String(phoneNumber)),
  };
}

/** Stable key applications can use to reject callback replay. */
export function stkCallbackDeduplicationKey(callback: StkCallback): string {
  const receipt = stkCallbackMetadata(callback).MpesaReceiptNumber;
  return typeof receipt === "string" && receipt.trim()
    ? `stk:receipt:${receipt}`
    : `stk:checkout:${callback.CheckoutRequestID}:result:${callback.ResultCode}`;
}

/**
 * Cryptographic callback verification is unavailable from the documented
 * contract. This function always fails closed, regardless of authenticator
 * header presence.
 */
export function verifyCallback(
  body: string | Buffer,
  authenticatorHeader: string | null | undefined
): void {
  const bytes = typeof body === "string" ? Buffer.from(body) : body;
  if (bytes.length === 0 || !authenticatorHeader?.trim()) {
    throw new DarajaError("Invalid callback authenticator.");
  }
  throw new DarajaError(
    "Callback cryptographic verification is unavailable; refusing to trust this callback."
  );
}

/** Parse and validate a C2B confirmation or validation callback body. */
export function parseC2bConfirmation(body: unknown): C2bConfirmation {
  const parsed = asRecord(body, "Invalid C2B callback: expected an object.");
  const confirmation: C2bConfirmation = {
    TransactionType: requiredString(parsed, "TransactionType", "C2B callback"),
    TransID: requiredString(parsed, "TransID", "C2B callback"),
    TransTime: requiredString(parsed, "TransTime", "C2B callback"),
    TransAmount: requiredString(parsed, "TransAmount", "C2B callback"),
    BusinessShortCode: requiredString(parsed, "BusinessShortCode", "C2B callback"),
    BillRefNumber: requiredString(parsed, "BillRefNumber", "C2B callback"),
    MSISDN: requiredString(parsed, "MSISDN", "C2B callback"),
    FirstName: typeof parsed.FirstName === "string" ? parsed.FirstName : undefined,
  };
  validateRequiredText("Transaction ID", confirmation.TransID, 128);
  if (!/^\d{14}$/.test(confirmation.TransTime)) {
    throw new DarajaError("Invalid C2B callback: TransTime must contain 14 digits.");
  }
  parsePositiveDecimal(confirmation.TransAmount, "C2B callback");
  validateShortcode(confirmation.BusinessShortCode);
  confirmation.MSISDN = validatePhone(confirmation.MSISDN);
  validateRequiredText("Bill reference", confirmation.BillRefNumber, 100);
  return confirmation;
}

export function c2bCallbackDeduplicationKey(callback: C2bConfirmation): string {
  return `c2b:transaction:${callback.TransID}`;
}
