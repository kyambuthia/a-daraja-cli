import type { DarajaClient } from "../client.js";
import type { B2cCommand, ConversationResponse, SendOptions } from "../types.js";
import { b2cCommandValue } from "../types.js";
import { initiatorName, initiatorSecurityCredential } from "./common.js";
import { validateAmount, validateOptionalText, validatePhone, validateRequiredText, validateUrl } from "../validation.js";

const B2C_PAYMENT_PATH = "/mpesa/b2c/v1/paymentrequest";

export async function b2cPayment(
  client: DarajaClient,
  options: SendOptions
): Promise<ConversationResponse> {
  validateAmount(options.amount);
  const phone = validatePhone(options.phone);
  validateRequiredText("Remarks", options.remarks ?? "Payment", 100);
  validateOptionalText("Occasion", options.occasion ?? "", 100);
  validateUrl("Result URL", options.resultUrl, client.environment);
  validateUrl("Timeout URL", options.timeoutUrl, client.environment);
  const command: B2cCommand = options.type ?? "business";
  const securityCredential = await initiatorSecurityCredential(client);

  return client.post<ConversationResponse>(B2C_PAYMENT_PATH, {
    InitiatorName: initiatorName(client),
    SecurityCredential: securityCredential,
    CommandID: b2cCommandValue(command),
    Amount: String(options.amount),
    PartyA: client.shortcode(),
    PartyB: phone,
    Remarks: options.remarks ?? "Payment",
    QueueTimeOutURL: options.timeoutUrl,
    ResultURL: options.resultUrl,
    Occasion: options.occasion ?? "",
  }, { idempotencyKey: options.idempotencyKey });
}
