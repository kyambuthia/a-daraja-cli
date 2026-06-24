import type { DarajaClient } from "../client.js";
import { DarajaError } from "../error.js";
import { securityCredentialForEnvironment } from "../security.js";

export function initiatorName(client: DarajaClient): string {
  const name = client.credentials.initiatorName;
  if (!name) {
    throw new DarajaError("initiator_name is not configured.");
  }
  return name;
}

export async function initiatorSecurityCredential(
  client: DarajaClient
): Promise<string> {
  const password = client.credentials.initiatorPassword;
  if (!password) {
    throw new DarajaError("initiator_password is not configured.");
  }
  return securityCredentialForEnvironment(password, client.environment);
}