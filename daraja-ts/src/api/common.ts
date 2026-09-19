import type { DarajaClient } from "../client.js";

export function initiatorName(client: DarajaClient): string {
  return client.initiatorName();
}

export async function initiatorSecurityCredential(
  client: DarajaClient
): Promise<string> {
  return client.initiatorSecurityCredential();
}
