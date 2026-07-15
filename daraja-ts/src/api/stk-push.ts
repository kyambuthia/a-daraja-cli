import type { DarajaClient } from "../client.js";
import { currentTimestamp, generateStkPassword } from "../security.js";
import type {
  PromptOptions,
  StkPushResponse,
  StkTransactionType,
} from "../types.js";
import { stkTransactionTypeValue } from "../types.js";
import { validateAmount, validatePhone, validateRequiredText, validateUrl } from "../validation.js";

export async function stkPush(
  client: DarajaClient,
  options: PromptOptions
): Promise<StkPushResponse> {
  validateAmount(options.amount);
  const phone = validatePhone(options.phone);
  validateRequiredText("Description", options.description ?? "Payment", 100);
  validateRequiredText("Account reference", options.reference ?? "Payment", 100);
  validateUrl("Callback URL", options.callbackUrl, client.environment);
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
    Amount: String(options.amount),
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
  validateRequiredText("Checkout request ID", checkoutRequestId, 128);
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
