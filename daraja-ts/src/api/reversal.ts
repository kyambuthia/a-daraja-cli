import type { DarajaClient } from "../client.js";
import type { ConversationResponse, RefundOptions } from "../types.js";
import { initiatorName, initiatorSecurityCredential } from "./common.js";
import { validateAmount, validateOptionalText, validateRequiredText, validateShortcode, validateUrl } from "../validation.js";

const REVERSAL_PATH = "/mpesa/reversal/v1/request";

export async function reverseTransaction(
  client: DarajaClient,
  options: RefundOptions
): Promise<ConversationResponse> {
  validateAmount(options.amount);
  validateRequiredText("Transaction ID", options.transactionId, 128);
  validateShortcode(options.receiverParty);
  validateRequiredText("Receiver identifier type", options.receiverIdentifierType ?? "11", 8);
  validateRequiredText("Remarks", options.remarks ?? "Refund", 100);
  validateOptionalText("Occasion", options.occasion ?? "", 100);
  validateUrl("Result URL", options.resultUrl, client.environment);
  validateUrl("Timeout URL", options.timeoutUrl, client.environment);
  const securityCredential = await initiatorSecurityCredential(client);

  return client.post<ConversationResponse>(REVERSAL_PATH, {
    Initiator: initiatorName(client),
    SecurityCredential: securityCredential,
    CommandID: "TransactionReversal",
    TransactionID: options.transactionId,
    Amount: String(options.amount),
    ReceiverParty: options.receiverParty,
    RecieverIdentifierType: options.receiverIdentifierType ?? "11",
    ResultURL: options.resultUrl,
    QueueTimeOutURL: options.timeoutUrl,
    Remarks: options.remarks ?? "Refund",
    Occasion: options.occasion ?? "",
  }, { idempotencyKey: options.idempotencyKey });
}
