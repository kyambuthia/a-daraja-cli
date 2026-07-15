import type { DarajaClient } from "../client.js";
import type { ConversationResponse, RefundOptions } from "../types.js";
import { initiatorName, initiatorSecurityCredential } from "./common.js";

const REVERSAL_PATH = "/mpesa/reversal/v1/request";

export async function reverseTransaction(
  client: DarajaClient,
  options: RefundOptions
): Promise<ConversationResponse> {
  const securityCredential = await initiatorSecurityCredential(client);

  return client.post<ConversationResponse>(REVERSAL_PATH, {
    Initiator: initiatorName(client),
    SecurityCredential: securityCredential,
    CommandID: "TransactionReversal",
    TransactionID: options.transactionId,
    Amount: String(Math.trunc(options.amount)),
    ReceiverParty: options.receiverParty,
    RecieverIdentifierType: options.receiverIdentifierType ?? "11",
    ResultURL: options.resultUrl,
    QueueTimeOutURL: options.timeoutUrl,
    Remarks: options.remarks ?? "Refund",
    Occasion: options.occasion ?? "",
  }, { idempotencyKey: options.idempotencyKey });
}
