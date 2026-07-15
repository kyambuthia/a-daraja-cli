import type { DarajaClient } from "../client.js";
import { currentTimestamp, generateStkPassword, normalizePhone } from "../security.js";
import type {
  PromptOptions,
  StkPushResponse,
  StkTransactionType,
} from "../types.js";
import { stkTransactionTypeValue } from "../types.js";

export async function stkPush(
  client: DarajaClient,
  options: PromptOptions
): Promise<StkPushResponse> {
  const phone = normalizePhone(options.phone);
  const shortcode = client.credentials.shortcode;
  const timestamp = currentTimestamp();
  const password = generateStkPassword(
    shortcode,
    client.credentials.passkey,
    timestamp
  );
  const transactionType = stkTransactionTypeValue(options.type ?? "paybill");

  return client.post<StkPushResponse>("/mpesa/stkpush/v1/processrequest", {
    BusinessShortCode: shortcode,
    Password: password,
    Timestamp: timestamp,
    TransactionType: transactionType,
    Amount: String(Math.trunc(options.amount)),
    PartyA: phone,
    PartyB: shortcode,
    PhoneNumber: phone,
    CallBackURL: options.callbackUrl,
    AccountReference: options.reference ?? "Payment",
    TransactionDesc: options.description ?? "Payment",
  }, { idempotencyKey: options.idempotencyKey });
}

export async function stkPushQuery(
  client: DarajaClient,
  checkoutRequestId: string
): Promise<StkPushResponse> {
  const shortcode = client.credentials.shortcode;
  const timestamp = currentTimestamp();
  const password = generateStkPassword(
    shortcode,
    client.credentials.passkey,
    timestamp
  );

  return client.post<StkPushResponse>("/mpesa/stkpush/v1/query", {
    BusinessShortCode: shortcode,
    Password: password,
    Timestamp: timestamp,
    CheckoutRequestID: checkoutRequestId,
  });
}

export type { PromptOptions, StkTransactionType };
