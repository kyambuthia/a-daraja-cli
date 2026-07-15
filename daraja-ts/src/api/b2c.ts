import type { DarajaClient } from "../client.js";
import { normalizePhone } from "../security.js";
import type { B2cCommand, ConversationResponse, SendOptions } from "../types.js";
import { b2cCommandValue } from "../types.js";
import { initiatorName, initiatorSecurityCredential } from "./common.js";

const B2C_PAYMENT_PATH = "/mpesa/b2c/v1/paymentrequest";

export async function b2cPayment(
  client: DarajaClient,
  options: SendOptions
): Promise<ConversationResponse> {
  const phone = normalizePhone(options.phone);
  const command: B2cCommand = options.type ?? "business";
  const securityCredential = await initiatorSecurityCredential(client);

  return client.post<ConversationResponse>(B2C_PAYMENT_PATH, {
    InitiatorName: initiatorName(client),
    SecurityCredential: securityCredential,
    CommandID: b2cCommandValue(command),
    Amount: String(Math.trunc(options.amount)),
    PartyA: client.credentials.shortcode,
    PartyB: phone,
    Remarks: options.remarks ?? "Payment",
    QueueTimeOutURL: options.timeoutUrl,
    ResultURL: options.resultUrl,
    Occasion: options.occasion ?? "",
  }, { idempotencyKey: options.idempotencyKey });
}
