import type { DarajaClient } from "../client.js";
import type { ConversationResponse, QueryOptions } from "../types.js";
import { initiatorName, initiatorSecurityCredential } from "./common.js";

const TRANSACTION_STATUS_PATH = "/mpesa/transactionstatus/v1/query";

export async function queryStatus(
  client: DarajaClient,
  options: QueryOptions
): Promise<ConversationResponse> {
  const securityCredential = await initiatorSecurityCredential(client);

  return client.post<ConversationResponse>(TRANSACTION_STATUS_PATH, {
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