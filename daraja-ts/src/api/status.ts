import type { DarajaClient } from "../client.js";
import type { ConversationResponse, QueryOptions } from "../types.js";
import { initiatorName, initiatorSecurityCredential } from "./common.js";
import { validateOptionalText, validateRequiredText, validateUrl } from "../validation.js";

const TRANSACTION_STATUS_PATH = "/mpesa/transactionstatus/v1/query";

export async function queryStatus(
  client: DarajaClient,
  options: QueryOptions
): Promise<ConversationResponse> {
  validateRequiredText("Transaction ID", options.transactionId, 128);
  validateRequiredText("Remarks", options.remarks ?? "Status check", 100);
  validateOptionalText("Occasion", options.occasion ?? "", 100);
  validateUrl("Result URL", options.resultUrl, client.environment);
  validateUrl("Timeout URL", options.timeoutUrl, client.environment);
  const securityCredential = await initiatorSecurityCredential(client);

  return client.postWithoutIdempotency<ConversationResponse>(TRANSACTION_STATUS_PATH, {
    Initiator: initiatorName(client),
    SecurityCredential: securityCredential,
    CommandID: "TransactionStatusQuery",
    TransactionID: options.transactionId,
    PartyA: client.credentials.shortcode,
    IdentifierType: "1",
    ResultURL: options.resultUrl,
    QueueTimeOutURL: options.timeoutUrl,
    Remarks: options.remarks ?? "Status check",
    Occasion: options.occasion ?? "",
  });
}
