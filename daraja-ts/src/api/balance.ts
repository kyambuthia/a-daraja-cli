import type { DarajaClient } from "../client.js";
import type { BalanceOptions, ConversationResponse } from "../types.js";
import { initiatorName, initiatorSecurityCredential } from "./common.js";

const ACCOUNT_BALANCE_PATH = "/mpesa/accountbalance/v1/query";

export async function queryBalance(
  client: DarajaClient,
  options: BalanceOptions
): Promise<ConversationResponse> {
  const securityCredential = await initiatorSecurityCredential(client);

  return client.post<ConversationResponse>(ACCOUNT_BALANCE_PATH, {
    Initiator: initiatorName(client),
    SecurityCredential: securityCredential,
    CommandID: "AccountBalance",
    PartyA: client.credentials.shortcode,
    IdentifierType: "4",
    Remarks: "Balance check",
    QueueTimeOutURL: options.timeoutUrl,
    ResultURL: options.resultUrl,
  }, { idempotencyKey: options.idempotencyKey });
}
